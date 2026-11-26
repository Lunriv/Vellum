use super::LlmProvider;
use crate::kernel::{ActionProposal, ClockVisibility, SessionState};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct LiveProvider {
    client: Client,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

/// Builds a sanitized player-visible context for LLM calls.
/// Strictly strips `raw_truth` and `KeeperHidden` clocks in Rust before serialization.
pub(crate) fn player_visible_context(state: &SessionState) -> Value {
    let visible_clocks: Vec<Value> = state
        .clocks
        .values()
        .filter(|c| c.visibility == ClockVisibility::PlayerVisible)
        .map(|c| {
            json!({
                "clockId": c.clock_id,
                "name": c.name,
                "current": c.current,
                "max": c.max,
                "lastReason": c.last_reason,
            })
        })
        .collect();

    json!({
        "sessionId": state.session_id,
        "moduleId": state.module_id,
        "investigator": state.investigator,
        "currentScene": state.current_scene,
        "visibleClocks": visible_clocks,
        "clues": state.clues.values().collect::<Vec<_>>(),
        "handouts": state.handouts.values().collect::<Vec<_>>(),
        "pendingInterrupt": state.pending_interrupt,
    })
}

impl LiveProvider {
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("VELLUM_LLM_ENDPOINT")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_API_BASE"))
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".into());
        let api_key = std::env::var("VELLUM_LLM_API_KEY")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_API_KEY"))
            .map_err(|_| "VELLUM_LLM_API_KEY is required in live mode".to_string())?;
        let model = std::env::var("VELLUM_LLM_MODEL")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_MODEL"))
            .unwrap_or_else(|_| "gpt-4o-mini".into());
        Ok(Self {
            client: Client::new(),
            endpoint,
            api_key,
            model,
        })
    }
    async fn request(
        &self,
        system: &str,
        user: &str,
        stream: bool,
    ) -> Result<reqwest::Response, String> {
        self.client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&json!({
                "model": self.model,
                "stream": stream,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user }
                ]
            }))
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())
    }
}

fn extract_sse_delta_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let payload = trimmed.strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let value = serde_json::from_str::<Value>(payload).ok()?;
    value["choices"][0]["delta"]["content"]
        .as_str()
        .map(str::to_owned)
}

#[async_trait]
impl LlmProvider for LiveProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<ActionProposal, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state)).unwrap_or_default();
        let response = self
            .request(
                "Return only a JSON ActionProposal matching the Vellum schema. The kernel validates all effects.",
                &format!("Current state: {context_json}\nPlayer action: {input}"),
                false,
            )
            .await?;
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        let content = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "LLM response did not contain message content".to_string())?;
        serde_json::from_str(content)
            .map_err(|error| format!("invalid ActionProposal JSON: {error}"))
    }

    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<Vec<String>, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state)).unwrap_or_default();
        let response = self
            .request(
                "Narrate the accepted game result. Return concise plain text only.",
                &format!("State: {context_json}\nAction: {input}"),
                true,
            )
            .await?;

        let mut chunks = Vec::new();
        let mut line_buf: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            let bytes = item.map_err(|error| error.to_string())?;
            line_buf.extend_from_slice(&bytes);
            while let Some(pos) = line_buf.iter().position(|&b| b == b'\n') {
                let raw_line: Vec<u8> = line_buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&raw_line);
                if let Some(chunk) = extract_sse_delta_line(&line) {
                    chunks.push(chunk);
                }
            }
        }
        if !line_buf.is_empty() {
            let line = String::from_utf8_lossy(&line_buf);
            if let Some(chunk) = extract_sse_delta_line(&line) {
                chunks.push(chunk);
            }
        }
        Ok(chunks)
    }

    async fn answer_ooc(&self, question: &str, state: &SessionState) -> Result<String, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state)).unwrap_or_default();
        let response = self
            .request(
                "Answer the player's out-of-character rules question without revealing keeper-only facts.",
                &format!("State: {context_json}\nQuestion: {question}"),
                false,
            )
            .await?;
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        Ok(value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleRegistry;
    use crate::runtime::orchestrator::build_initial_session_state;

    #[test]
    fn live_provider_context_strips_raw_truth_and_hidden_clocks() {
        let modules = ModuleRegistry::load_default().expect("load default modules");
        let module = modules.get("crimson_rain_manor").expect("module exists");
        let mut state =
            build_initial_session_state("s-live-1", module, "investigator_reporter").unwrap();
        state.raw_truth = json!({ "secretCulprit": "EVELYN_RITUAL_TOP_SECRET" });

        let ctx = player_visible_context(&state);
        let serialized = ctx.to_string();
        assert!(!serialized.contains("EVELYN_RITUAL_TOP_SECRET"));
        assert!(!serialized.contains("raw_truth"));
        assert!(!serialized.contains("ritual_countdown"));
        assert!(serialized.contains("manor_anomaly"));
    }
}
