use std::sync::Arc;
use std::time::Duration;

use super::LlmProvider;
use crate::kernel::{ActionProposal, ClockVisibility, SessionState};
use crate::modules::{evaluate_module_predicate, ModuleRegistry};
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
    modules: Option<Arc<ModuleRegistry>>,
}

/// Builds a sanitized player-visible context for LLM calls.
/// Strictly strips `raw_truth` and `KeeperHidden` clocks in Rust before serialization.
pub(crate) fn player_visible_context(
    state: &SessionState,
    modules: Option<&ModuleRegistry>,
) -> Value {
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

    let scene_hints = modules
        .and_then(|reg| reg.get(&state.module_id))
        .and_then(|m| m.find_scene(&state.current_scene.scene_id))
        .map(|scene| {
            let exits: Vec<Value> = scene
                .exits
                .iter()
                .map(|exit| {
                    let accessible = exit
                        .condition
                        .as_ref()
                        .map(|cond| evaluate_module_predicate(cond, state, None))
                        .unwrap_or(true);
                    json!({
                        "targetSceneId": exit.target_scene_id,
                        "label": exit.label,
                        "accessible": accessible,
                    })
                })
                .collect();
            let entities: Vec<Value> = scene
                .entities
                .iter()
                .map(|e| {
                    json!({
                        "entityId": e.entity_id,
                        "name": e.name,
                        "kind": e.kind,
                        "summary": e.summary,
                        "discoverableClueIds": e.clue_ids,
                    })
                })
                .collect();
            json!({
                "exits": exits,
                "presentEntities": entities,
            })
        });

    json!({
        "sessionId": state.session_id,
        "moduleId": state.module_id,
        "investigator": state.investigator,
        "currentScene": state.current_scene,
        "sceneNavigation": scene_hints,
        "visibleClocks": visible_clocks,
        "clues": state.clues.values().collect::<Vec<_>>(),
        "handouts": state.handouts.values().collect::<Vec<_>>(),
        "pendingInterrupt": state.pending_interrupt,
    })
}

fn is_dashscope_qwen(endpoint: &str, model: &str) -> bool {
    endpoint.contains("dashscope") || model.to_lowercase().starts_with("qwen")
}

/// Extracts a raw JSON object string from LLM output, tolerating markdown code fences.
pub(crate) fn extract_json_object(raw: &str) -> &str {
    let trimmed = raw.trim();
    let without_fence = if let Some(rest) = trimmed.strip_prefix("```") {
        let after_lang = rest.split_once('\n').map(|(_, body)| body).unwrap_or(rest);
        after_lang.trim().trim_end_matches("```").trim()
    } else {
        trimmed
    };
    if let (Some(start), Some(end)) = (without_fence.find('{'), without_fence.rfind('}')) {
        if start <= end {
            return &without_fence[start..=end];
        }
    }
    without_fence
}

impl LiveProvider {
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("VELLUM_LLM_ENDPOINT")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_API_BASE"))
            .unwrap_or_else(|_| {
                "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".into()
            });
        let api_key = std::env::var("VELLUM_LLM_API_KEY")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_API_KEY"))
            .map_err(|_| "VELLUM_LLM_API_KEY is required in live mode".to_string())?;
        let model = std::env::var("VELLUM_LLM_MODEL")
            .or_else(|_| std::env::var("VELLUM_PROPOSAL_MODEL"))
            .unwrap_or_else(|_| "qwen-plus".into());
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            endpoint,
            api_key,
            model,
            modules: None,
        })
    }

    pub fn with_modules(mut self, modules: Arc<ModuleRegistry>) -> Self {
        self.modules = Some(modules);
        self
    }

    fn build_request_body(
        &self,
        system: &str,
        user: &str,
        stream: bool,
        json_mode: bool,
        max_tokens: u32,
        temperature: f32,
    ) -> Value {
        let mut body = json!({
            "model": self.model,
            "stream": stream,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });
        if let Some(obj) = body.as_object_mut() {
            if json_mode {
                obj.insert("response_format".into(), json!({ "type": "json_object" }));
            }
            // Disable extra reasoning tokens on DashScope/Qwen models to keep latency & cost low
            if is_dashscope_qwen(&self.endpoint, &self.model) {
                obj.insert("enable_thinking".into(), json!(false));
            }
        }
        body
    }

    async fn request(
        &self,
        system: &str,
        user: &str,
        stream: bool,
        json_mode: bool,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<reqwest::Response, String> {
        let body =
            self.build_request_body(system, user, stream, json_mode, max_tokens, temperature);
        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| format!("模型请求失败：{error}"))?;
        let status = resp.status();
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!("模型接口返回错误 ({status})：{err_text}"));
        }
        Ok(resp)
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

const PROPOSAL_SYSTEM_PROMPT: &str = r#"你是 CoC 7e 调查游戏《绯雨庄园》的行动意图提案器（ActionProposal Planner）。
你只负责把玩家输入的行动解析为严格合法的 JSON 对象，所有掷骰、技能值校验和世界状态修改均由确定性内核执行。
严禁输出任何 JSON 之外的文字、解释或 Markdown 代码块。

必须严格返回如下结构的 JSON：
{
  "intent": "简短总结玩家意图",
  "targetEntityIds": ["相关实体ID"],
  "actionType": "observe|move|investigate|check|confront|bluff|off_rail|conclude",
  "proposedChecks": [
    {
      "checkId": "check_1",
      "skill": "spot_hidden|psychology|persuade|locksmith|medicine|science|intimidate|fighting|dodge",
      "difficulty": "regular|hard|extreme",
      "targetNumber": 50,
      "bonusPenaltyDice": 0,
      "allowPush": true,
      "actionKind": "investigate"
    }
  ],
  "proposedEffects": [
    { "effectType": "move_scene|unlock_clue|player_bluff|record_event|advance_clock|end_session", "targetId": "对应ID或null", "value": null, "payload": null }
  ],
  "evidenceToPresent": ["仅限玩家 clues 中已拥有的 clueId"],
  "topicsToConfront": ["bell_tower|missing_guest|ritual_truth|lady_evelyn"],
  "proposedWorldPatches": []
}

规则约束：
1. 若玩家移动去其他场景，使用 sceneNavigation.exits 中 accessible=true 的 targetSceneId，放入 proposedEffects（effectType="move_scene", targetId=targetSceneId）。
2. 若行动有不确定风险（撬锁、识破谎言、搜寻暗格、说服施压等），放入至多 1 个 proposedChecks，此时 proposedEffects 留空（由内核在掷骰后裁定结果）。
3. 若玩家直接查看场景内明面线索或温和交谈，可从 sceneNavigation.presentEntities[*].discoverableClueIds 中选择对应 clueId，放入 proposedEffects（effectType="unlock_clue"）。
4. 若玩家试图诈唬在场 NPC（声称对方同伴已招供等未经证实的说法），使用 effectType="player_bluff", targetId=NPC的entityId。
5. 仅当玩家明确要求揭露真相结案或带着证据离开结束本局时，才使用 effectType="end_session"。
6. 严禁编造不在 sceneNavigation 或 clues 中的 ID。"#;

const NARRATION_SYSTEM_PROMPT: &str = r#"你是 1920 年代克苏鲁神话调查短篇《绯雨庄园》的中文守秘人（Keeper）。
写作要求：
1. 仅根据已提交的当前场景、调查员身份与已解锁线索，用克制、沉浸、有文学质感的小说笔触描写 2–3 段中文叙事（约 140–240 字）。
2. 严禁替玩家行动、替玩家做决定或自行宣布骰子结果；不要剧透尚未解锁的隐藏真相或未出现的线索。
3. 严禁输出标题、项目符号、系统提示、规则术语或 Markdown 格式，只输出自然流畅的正文段落。"#;

const OOC_SYSTEM_PROMPT: &str = r#"你是 Vellum CoC 7e 调查游戏的场外规则与状态助手（OOC）。
要求：
1. 用简洁、易懂的中文回答玩家的规则疑问或当前可见角色状态（生命、魔力、理智、幸运、当前位置与线索数）。
2. 严禁推进剧情、严禁剧透未发现的线索或幕后真相，只能引用给定状态中的玩家可见信息。"#;

#[async_trait]
impl LlmProvider for LiveProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<ActionProposal, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state, self.modules.as_deref()))
                .unwrap_or_default();
        let response = self
            .request(
                PROPOSAL_SYSTEM_PROMPT,
                &format!("当前玩家可见状态 JSON：\n{context_json}\n\n玩家本次行动：{input}"),
                false,
                true,
                512,
                0.2,
            )
            .await?;
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| format!("解析模型返回失败：{error}"))?;
        let content = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "模型返回缺少有效内容".to_string())?;
        let json_slice = extract_json_object(content);
        let mut proposal: ActionProposal = serde_json::from_str(json_slice)
            .map_err(|error| format!("模型返回的行动提案 JSON 不合法：{error}"))?;
        if proposal.intent.trim().is_empty() {
            proposal.intent = input.to_string();
        }
        Ok(proposal)
    }

    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<Vec<String>, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state, self.modules.as_deref()))
                .unwrap_or_default();
        let response = self
            .request(
                NARRATION_SYSTEM_PROMPT,
                &format!("当前已提交的现场状态：\n{context_json}\n\n玩家刚执行的行动：{input}"),
                true,
                false,
                600,
                0.75,
            )
            .await?;

        let mut chunks = Vec::new();
        let mut line_buf: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            let bytes = item.map_err(|error| format!("读取流式叙事失败：{error}"))?;
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
            serde_json::to_string(&player_visible_context(state, self.modules.as_deref()))
                .unwrap_or_default();
        let response = self
            .request(
                OOC_SYSTEM_PROMPT,
                &format!("当前玩家可见状态：\n{context_json}\n\n玩家场外提问：{question}"),
                false,
                false,
                350,
                0.3,
            )
            .await?;
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| format!("解析场外问答失败：{error}"))?;
        Ok(value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
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

        let ctx = player_visible_context(&state, Some(&modules));
        let serialized = ctx.to_string();
        assert!(!serialized.contains("EVELYN_RITUAL_TOP_SECRET"));
        assert!(!serialized.contains("raw_truth"));
        assert!(!serialized.contains("ritual_countdown"));
        assert!(serialized.contains("manor_anomaly"));
        assert!(serialized.contains("study_room"));
    }

    #[test]
    fn extract_json_object_handles_fences_and_minimal_proposal() {
        let raw = "```json\n{\"intent\":\"撬开抽屉\",\"actionType\":\"check\",\"proposedChecks\":[{\"skill\":\"locksmith\"}]}\n```";
        let extracted = extract_json_object(raw);
        let parsed: ActionProposal =
            serde_json::from_str(extracted).expect("should parse minimal fenced proposal");
        assert_eq!(parsed.action_type, "check");
        assert_eq!(parsed.proposed_checks.len(), 1);
        assert_eq!(parsed.proposed_checks[0].skill, "locksmith");
        assert!(parsed.proposed_effects.is_empty());
    }

    #[test]
    fn dashscope_request_body_disables_thinking_and_sets_json_mode() {
        let provider = LiveProvider {
            client: Client::new(),
            endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".into(),
            api_key: "test-key".into(),
            model: "qwen-plus".into(),
            modules: None,
        };
        let body = provider.build_request_body("sys", "usr", false, true, 512, 0.2);
        assert_eq!(body["enable_thinking"], json!(false));
        assert_eq!(body["response_format"]["type"], json!("json_object"));
        assert_eq!(body["max_tokens"], json!(512));
    }
}
