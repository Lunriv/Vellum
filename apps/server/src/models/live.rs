use std::time::Duration;

use super::{LlmProvider, NarrationContext, TurnHistoryItem};
use crate::kernel::{ActionProposal, ClockVisibility, SessionState};
use crate::modules::{
    evaluate_module_predicate,
    generator::{
        extract_module_topics, parse_and_sanitize_module_value, validate_generated_world_richness,
        WorldGenRequest,
    },
    ModuleIr,
};
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

/// Strips the trailing `【建议行动】...` menu from historical assistant turns so multi-turn memory
/// stays lean and focused on what actually happened and was said in-character.
pub(crate) fn compact_history_narration(raw: &str) -> String {
    let trimmed = if let Some((before, _)) = raw.split_once("【建议行动】") {
        before.trim()
    } else {
        raw.trim()
    };
    let mut out = String::new();
    for ch in trimmed.chars().take(360) {
        out.push(ch);
    }
    if trimmed.chars().count() > 360 {
        out.push('…');
    }
    out
}

/// Builds a sanitized player-visible context for LLM calls.
/// Strictly strips `raw_truth`, unrevealed `protected_truths`, and `KeeperHidden` clocks in Rust.
pub(crate) fn player_visible_context(state: &SessionState, module: Option<&ModuleIr>) -> Value {
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

    let scene_hints = module
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
                    // For present NPCs, attach ONLY public surface roleplay cues (personality,
                    // coverStory, behavioralTells, workingKnowledge). NEVER include protected_truths
                    // or raw_truth!
                    let public_persona = if e.kind == "npc" {
                        module.and_then(|m| m.find_npc(&e.entity_id)).map(|npc| {
                            json!({
                                "personality": npc.personality,
                                "coverStory": npc.cover_story,
                                "behavioralTells": npc.behavioral_tells,
                                "workingKnowledge": npc.working_knowledge,
                            })
                        })
                    } else {
                        None
                    };
                    json!({
                        "entityId": e.entity_id,
                        "name": e.name,
                        "kind": e.kind,
                        "summary": e.summary,
                        "discoverableClueIds": e.clue_ids,
                        "publicPersona": public_persona,
                    })
                })
                .collect();
            json!({
                "exits": exits,
                "presentEntities": entities,
            })
        });

    let available_topics = module.map(extract_module_topics).unwrap_or_default();
    let world_tags: Vec<&String> = state.events.iter().collect();

    json!({
        "sessionId": state.session_id,
        "moduleId": state.module_id,
        "moduleTitle": module.map(|m| m.title.as_str()).unwrap_or(&state.module_id),
        "investigator": state.investigator,
        "currentScene": state.current_scene,
        "sceneNavigation": scene_hints,
        "availableTopics": available_topics,
        "activeWorldTags": world_tags,
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

pub(crate) fn build_chat_messages(
    system: &str,
    history: &[TurnHistoryItem],
    current_user: &str,
) -> Vec<Value> {
    let mut messages = vec![json!({ "role": "system", "content": system })];
    for item in history {
        if let Some(player) = item
            .player_input
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            messages.push(json!({ "role": "user", "content": format!("玩家行动：{player}") }));
        }
        let compact = compact_history_narration(&item.narration);
        if !compact.is_empty() {
            messages.push(json!({ "role": "assistant", "content": compact }));
        }
    }
    messages.push(json!({ "role": "user", "content": current_user }));
    messages
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
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            endpoint,
            api_key,
            model,
        })
    }

    fn build_request_body(
        &self,
        messages: Vec<Value>,
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
            "messages": messages,
        });
        if let Some(obj) = body.as_object_mut() {
            if json_mode {
                obj.insert("response_format".into(), json!({ "type": "json_object" }));
            }
            if is_dashscope_qwen(&self.endpoint, &self.model) {
                obj.insert("enable_thinking".into(), json!(false));
            }
        }
        body
    }

    async fn request(
        &self,
        messages: Vec<Value>,
        stream: bool,
        json_mode: bool,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<reqwest::Response, String> {
        let body = self.build_request_body(messages, stream, json_mode, max_tokens, temperature);
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

const PROPOSAL_SYSTEM_PROMPT: &str = r#"你是 Vellum 通用桌面跑团引擎的行动意图提案器（ActionProposal Planner）。
你只负责结合最近对话记忆与当前世界可见状态，把玩家本次行动解析为严格合法的 JSON；所有掷骰、技能值校验和世界状态修改均由确定性内核执行。
严禁输出任何 JSON 之外的文字、解释或 Markdown 代码块。

必须返回严格合法的 JSON：
{
  "intent": "简短总结玩家意图",
  "targetEntityIds": ["相关实体ID"],
  "actionType": "observe|move|investigate|check|confront|bluff|off_rail|conclude",
  "proposedChecks": [
    {
      "checkId": "check_1",
      "skill": "spot_hidden|psychology|persuade|locksmith|medicine|science|intimidate|fighting|dodge|stealth",
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
  "topicsToConfront": ["仅限 availableTopics 中的主题 ID"],
  "proposedWorldPatches": [
    {
      "patchId": "patch_1",
      "source": "player_improvisation",
      "scope": "当前 sceneId",
      "lifetime": "session",
      "causalEventId": "player_off_rail_action",
      "changes": [{ "kind": "temporary_tag", "targetId": "简短英文蛇形事件标签", "value": true }],
      "resourceCost": null,
      "requiredPermission": null
    }
  ]
}

规则约束：
1. 移动场景：使用 sceneNavigation.exits 中 accessible=true 的 targetSceneId，放入 proposedEffects（effectType="move_scene", targetId=targetSceneId）。
2. 有风险行动或玩家带技能标签（如 [心理学]/[侦查]/[锁匠]/[医学]/[说服]/[恐吓]）：放入至多 1 个 proposedChecks，proposedEffects 留空（掷骰后再由内核裁定）。
3. 直接查探明面物件或温和交谈：可从 sceneNavigation.presentEntities[*].discoverableClueIds 选择对应 clueId，放入 proposedEffects（effectType="unlock_clue"）。
4. 诈唬试探：玩家用未经证实的说辞试探在场 NPC 时，用 effectType="player_bluff", targetId=NPC 的 entityId。
5. 自由即兴破局（Off-Rail）：若玩家提出打破常规的创意行动（如砸门、拉闸断电、引开守卫、堵门、制造混乱），允许使用 actionType="off_rail"，放入 1 个 scoped `temporary_tag` 的 proposedWorldPatches，并附带 `record_event`（targetId=同一个标签）及 1 个可见危机时钟推进（targetId 取 visibleClocks[0].clockId，value=1，payload={"reason":"简短后果"}）。
6. 仅当玩家明确要求结案或带着证据撤离结束本局时，使用 effectType="end_session"。
7. 严禁编造不在 sceneNavigation、clues、visibleClocks 或 availableTopics 中的既有 ID。"#;

const NARRATION_SYSTEM_PROMPT: &str = r#"你是 Vellum 通用跑团引擎的桌面主持人（Keeper / GM），适用于任何题材与世界观（悬疑、科幻、克苏鲁、密室、谍战等）。
核心原则：**我们在玩一场紧凑刺激的桌面游戏，不是在读慢吞吞的长篇小说！** 禁止堆砌冗长景物描写和抒情废话，每一句都必须推进局势、制造压力或回应玩家刚刚的博弈举动。

严格按以下短促、高张力的多声道格式分段输出（禁止使用 **、# 等 Markdown 符号）：

【守秘人】
只写 1–2 句干脆利落的现场变动与博弈后果（必须忠实演出本回合内核裁定的移动、检定成败、线索解锁、世界改动或时钟推进；绝不替玩家做决定）。

【<当前在场 NPC 名字>】
若 sceneNavigation.presentEntities 中有 NPC，让他立刻用 1–2 句有潜台词的短台词反击、推诿、试探或露怯（结合其 publicPersona，严禁剧透未解锁秘密）。若无 NPC 则省略此段。

【直觉·<技能名>】
仅当有真正有用的战术破绽时写 1 句短提示；不需要凑数时可省略。

【抉择】
1 句尖锐的局势逼问，点出眼前的风险与机会。

【建议行动】
给 2–3 条极短的战术备选（每条不超过 18 字，可带 [技能] 标记），供玩家快速出手，同时不限制玩家自由发挥。

字数铁律：全文控制在 140–230 字以内，快节奏、强交互、零废话。若本回合结果包含「结案收尾」，则改写为利落终章并省略【抉择】与【建议行动】。"#;

const OOC_SYSTEM_PROMPT: &str = r#"你是 Vellum 跑团引擎的场外规则与状态助手（OOC）。
要求：
1. 用简洁、直白的中文回答玩家的规则疑问或当前可见角色状态（生命、魔力、理智、幸运、位置、线索数、世界标签）。
2. 严禁推进剧情、严禁剧透未发现的线索或幕后真相，只能引用给定玩家可见状态。
3. 严禁输出 Markdown 符号（如 **、*、#、```），只用干净纯中文。"#;

const WORLD_GEN_SYSTEM_PROMPT: &str = r#"你是 Vellum 通用跑团引擎的世界编译器（World Compiler）。
任务：把用户输入的一句话创意、世界观设定或剧本大纲，编译成一份可直接运行、多房间网状连通、多方博弈的完整 TRPG 模组 JSON（ModuleIr）。
严禁输出任何 JSON 之外的文字、注释或 Markdown 代码块；必须输出紧凑单行 JSON（禁止多余缩进换行）。

必须严格遵循以下结构与数组规模（示例展示了 4 场景网状连通结构；若用户要求 5 或 6 个场景，请按同样格式继续扩展 s5/s6）：
{"moduleId":"简短英文蛇形ID","title":"中文世界名","openingPrologue":"2句极具压迫感的开场危机","initialSceneId":"s1","investigators":[{"investigatorId":"inv_1","name":"角色1姓名","occupation":"职业","tagline":"一句话人设","biography":"1句背景","motivation":"核心动机","manorConnection":"入场渊源","hpMax":10,"mpMax":11,"sanStart":55,"luckStart":50,"startingItems":[{"itemId":"item_1","name":"贴合题材的随身道具","description":"简短用途"}],"skills":{"spot_hidden":70,"psychology":65,"persuade":60,"locksmith":35,"dodge":45}},{"investigatorId":"inv_2","name":"角色2姓名","occupation":"不同职业","tagline":"人设","biography":"背景","motivation":"动机","manorConnection":"渊源","hpMax":12,"mpMax":10,"sanStart":50,"luckStart":55,"startingItems":[{"itemId":"item_2","name":"道具","description":"用途"}],"skills":{"fighting":65,"intimidate":65,"locksmith":60,"spot_hidden":55,"dodge":55}},{"investigatorId":"inv_3","name":"角色3姓名","occupation":"不同职业","tagline":"人设","biography":"背景","motivation":"动机","manorConnection":"渊源","hpMax":9,"mpMax":13,"sanStart":60,"luckStart":55,"startingItems":[{"itemId":"item_3","name":"道具","description":"用途"}],"skills":{"medicine":70,"science":65,"psychology":65,"persuade":55,"stealth":50}}],"scenes":[{"sceneId":"s1","title":"入口枢纽区","atmosphere":"rain_manor","description":"1-2句现场画面与紧张感","tags":["hub"],"exits":[{"targetSceneId":"s2","label":"前往区域2","bidirectional":true},{"targetSceneId":"s3","label":"前往区域3","bidirectional":true}],"entities":[{"entityId":"npc_1","name":"人物1","role":"gatekeeper","kind":"npc","summary":"1句外观与破绽","clueIds":["c2"]},{"entityId":"obj_1","name":"开场物件","role":"object","kind":"object","summary":"1句特征","clueIds":["c1"]}]},{"sceneId":"s2","title":"区域2名称","atmosphere":"candlelight","description":"1-2句画面","tags":["sector"],"exits":[{"targetSceneId":"s1","label":"返回区域1","bidirectional":true},{"targetSceneId":"s4","label":"进入禁区(需先掌握线索c3)","bidirectional":true,"condition":{"clue_discovered":"c3"}}],"entities":[{"entityId":"npc_2","name":"人物2","role":"witness","kind":"npc","summary":"1句外观与破绽","clueIds":["c3"]},{"entityId":"obj_2","name":"关键物件2","role":"object","kind":"object","summary":"1句特征","clueIds":["c4"]}]},{"sceneId":"s3","title":"区域3名称","atmosphere":"fog","description":"1-2句画面","tags":["sector"],"exits":[{"targetSceneId":"s1","label":"返回区域1","bidirectional":true},{"targetSceneId":"s4","label":"从隐蔽通道潜入禁区","bidirectional":true}],"entities":[{"entityId":"npc_3","name":"人物3","role":"suspect","kind":"npc","summary":"1句外观与破绽","clueIds":["c5"]},{"entityId":"obj_3","name":"物件3","role":"object","kind":"object","summary":"1句特征","clueIds":[]}]},{"sceneId":"s4","title":"核心禁区名","atmosphere":"san_strain","description":"1-2句高压现场","tags":["climax"],"exits":[{"targetSceneId":"s2","label":"撤回区域2","bidirectional":true},{"targetSceneId":"s3","label":"撤回区域3","bidirectional":true}],"entities":[{"entityId":"obj_4","name":"核心铁证物件","role":"object","kind":"object","summary":"1句特征","clueIds":["c6"]}]}],"clues":[{"clueId":"c1","title":"开场线索名","summary":"揭示的初始事实","sourceLabel":"区域1","acquisitionPaths":[{"pathId":"p1","description":"开场自带","condition":"always"}],"sceneIds":["s1"]},{"clueId":"c2","title":"线索2","summary":"关键破绽","sourceLabel":"人物1","acquisitionPaths":[{"pathId":"p2","description":"心理学看穿","condition":{"skill_success":"psychology"}}],"sceneIds":["s1"]},{"clueId":"c3","title":"通行关键线索3","summary":"解锁禁区的方法或密码","sourceLabel":"人物2","acquisitionPaths":[{"pathId":"p3","description":"说服或审问","condition":{"skill_success":"persuade"}}],"sceneIds":["s2"]},{"clueId":"c4","title":"物证线索4","summary":"现场痕迹","sourceLabel":"区域2","acquisitionPaths":[{"pathId":"p4","description":"侦查发现","condition":{"skill_success":"spot_hidden"}}],"sceneIds":["s2"]},{"clueId":"c5","title":"幕后线索5","summary":"揭露隐藏交易","sourceLabel":"人物3","acquisitionPaths":[{"pathId":"p5","description":"撬锁或技术破解","condition":{"skill_success":"locksmith"}}],"sceneIds":["s3"]},{"clueId":"c6","title":"终局铁证6","summary":"直指真凶与阴谋的决定性证据","sourceLabel":"核心禁区","acquisitionPaths":[{"pathId":"p6","description":"勘查核心禁区","condition":{"skill_success":"spot_hidden"}}],"sceneIds":["s4"]}],"handouts":[{"handoutId":"h1","kind":"report","title":"开场实体证物名","clueId":"c1","sourceFacts":["硬事实1","硬事实2"],"presentationText":"极具代入感的原件文字（字条/报告/日志/电报等，3-4行）"}],"npcs":[{"entityId":"npc_1","name":"人物1","personality":"鲜明性格与口吻","workingKnowledge":["公开信息"],"coverStory":"敷衍玩家的表面说辞","behavioralTells":["心虚微动作"],"protectedTruths":[{"secretId":"sec_1","rawTruth":"完整隐情","tell":"破绽","stages":[{"tier":"partial","predicate":{"evidence_presented":"c2"},"releasableFactIds":["f1"],"truth":"被迫松口的一句原话"}]}]},{"entityId":"npc_2","name":"人物2","personality":"鲜明性格","workingKnowledge":["公开信息"],"coverStory":"表面说辞","behavioralTells":["微动作"],"protectedTruths":[{"secretId":"sec_2","rawTruth":"隐情","tell":"破绽","stages":[{"tier":"partial","predicate":{"evidence_presented":"c4"},"releasableFactIds":["f1"],"truth":"被迫承认的一句原话"}]}]},{"entityId":"npc_3","name":"人物3","personality":"鲜明性格","workingKnowledge":["公开信息"],"coverStory":"表面说辞","behavioralTells":["微动作"],"protectedTruths":[{"secretId":"sec_3","rawTruth":"幕后黑手真相","tell":"破绽","stages":[{"tier":"confession","predicate":{"evidence_presented":"c6"},"releasableFactIds":["f2"],"truth":"真相败露后的反击台词"}]}]}],"clocks":[{"clockId":"visible_threat","name":"贴合题材的可见危机时钟","visibility":"player_visible","max":6,"stages":[{"at":0,"label":"征兆初现"},{"at":3,"label":"局势恶化"},{"at":6,"label":"全面失控"}]},{"clockId":"hidden_timer","name":"贴合题材的隐藏倒计时","visibility":"keeper_hidden","max":6,"stages":[{"at":0,"label":"暗中推进"},{"at":3,"label":"逼近临界"},{"at":6,"label":"阴谋达成"}]}],"endings":[{"endingId":"end_truth","title":"真相大白结局名","summary":"1-2句结局","condition":{"clue_discovered":"c6"}},{"endingId":"end_crisis","title":"局势失控结局名","summary":"1-2句悲剧结局","condition":{"clock_at_least":{"clockId":"visible_threat","value":6}}}],"reviewFacts":[{"factId":"f1","title":"幕后真相条目1","detail":"幕后完整还原","sourceClueIds":["c1","c2"]},{"factId":"f2","title":"幕后真相条目2","detail":"核心反转揭秘","sourceClueIds":["c5","c6"]}]}

铁律：
1. scenes 数组元素个数必须 >= 用户指定的场景数（至少 4 个不同场景，用贴合题材的英文蛇形 sceneId 替换 s1/s2/s3/s4，绝不允许只输出 1 个场景）。
2. npcs 数组必须包含 3 个不同 NPC，并分别放在不同场景的 entities（kind="npc"）中。
3. 仅第 1 条线索用 "always"，其余线索必须使用 {"skill_success":"..."}（仅限 spot_hidden, psychology, persuade, locksmith, medicine, science, intimidate, fighting, dodge, stealth）。
4. 全部剧情、角色、道具、时钟与证物紧扣用户输入的世界观，强冲突、高反转、有真实博弈感，绝不套用旧庄园模板！"#;

#[async_trait]
impl LlmProvider for LiveProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        history: &[TurnHistoryItem],
    ) -> Result<ActionProposal, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state, module)).unwrap_or_default();
        let current_user =
            format!("当前玩家可见状态 JSON：\n{context_json}\n\n玩家本次行动：{input}");
        let messages = build_chat_messages(PROPOSAL_SYSTEM_PROMPT, history, &current_user);
        let response = self.request(messages, false, true, 560, 0.2).await?;
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
        module: Option<&ModuleIr>,
        ctx: &NarrationContext<'_>,
    ) -> Result<Vec<String>, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state, module)).unwrap_or_default();
        let outcome_block = ctx
            .outcome_summary
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("\n\n本回合已确定的内核裁定结果（必须忠实演出）：\n{s}"))
            .unwrap_or_default();
        let lead = if ctx.is_opening {
            format!(
                "当前已提交的开场现场状态：\n{context_json}{outcome_block}\n\n请以短促利落的跑团开局切入现场：迅速交代危机张力，让在场人物直接开口抛矛盾，最后给出【抉择】和 2–3 条简短【建议行动】。"
            )
        } else {
            format!(
                "当前已提交的现场状态：\n{context_json}{outcome_block}\n\n玩家本次行动：{input}"
            )
        };
        let messages = build_chat_messages(NARRATION_SYSTEM_PROMPT, ctx.history, &lead);
        let response = self.request(messages, true, false, 620, 0.75).await?;

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

    async fn answer_ooc(
        &self,
        question: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        history: &[TurnHistoryItem],
    ) -> Result<String, String> {
        let context_json =
            serde_json::to_string(&player_visible_context(state, module)).unwrap_or_default();
        let current_user =
            format!("当前玩家可见状态：\n{context_json}\n\n玩家场外提问：{question}");
        let messages = build_chat_messages(OOC_SYSTEM_PROMPT, history, &current_user);
        let response = self.request(messages, false, false, 360, 0.3).await?;
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

    async fn generate_world(&self, req: &WorldGenRequest) -> Result<ModuleIr, String> {
        let scene_target = req.scene_count.unwrap_or(4).clamp(3, 6);
        let title_hint = req
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|t| format!("\n期望标题：{t}"))
            .unwrap_or_default();
        let tone_hint = req
            .tone
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|t| format!("\n风格基调：{t}"))
            .unwrap_or_default();

        let user_prompt = format!(
            "请根据以下世界创意或模组文本，编译成完整可玩的 Vellum ModuleIr JSON（设计 {scene_target} 个连通场景）：\n\n{}{}{}",
            req.prompt.trim(),
            title_hint,
            tone_hint
        );

        let mut last_err = String::new();
        for attempt in 0..2 {
            let prompt_for_try = if attempt == 0 {
                user_prompt.clone()
            } else {
                format!("{user_prompt}\n\n上一次生成校验未通过（{last_err}），请输出紧凑单行 JSON、修正所有引用关系并确保完整闭合。")
            };
            let messages = build_chat_messages(WORLD_GEN_SYSTEM_PROMPT, &[], &prompt_for_try);
            let started = std::time::Instant::now();
            let response = self.request(messages, false, true, 8192, 0.72).await?;
            let value = response
                .json::<Value>()
                .await
                .map_err(|e| format!("解析世界生成返回失败：{e}"))?;
            let finish_reason = value["choices"][0]["finish_reason"]
                .as_str()
                .unwrap_or("unknown");
            let Some(content) = value["choices"][0]["message"]["content"].as_str() else {
                last_err = "模型返回缺少有效内容".into();
                tracing::warn!(attempt, finish_reason, "generate_world missing content");
                continue;
            };
            tracing::info!(
                attempt,
                elapsed_ms = started.elapsed().as_millis() as u64,
                chars = content.chars().count(),
                finish_reason,
                "generate_world llm response received"
            );
            let slice = extract_json_object(content);
            match serde_json::from_str::<Value>(slice) {
                Ok(raw_json) => match parse_and_sanitize_module_value(raw_json, None) {
                    Ok(valid) => {
                        tracing::info!(
                            attempt,
                            scenes = valid.scenes.len(),
                            npcs = valid.npcs.len(),
                            clues = valid.clues.len(),
                            investigators = valid.investigators.len(),
                            "generate_world sanitized candidate"
                        );
                        match validate_generated_world_richness(&valid, scene_target as usize) {
                            Ok(()) => return Ok(valid),
                            Err(e) => {
                                last_err = e;
                                tracing::warn!(attempt, error = %last_err, "generate_world richness check failed");
                            }
                        }
                    }
                    Err(e) => {
                        last_err = e;
                        tracing::warn!(attempt, error = %last_err, "generate_world validation failed");
                    }
                },
                Err(e) => {
                    last_err = format!("JSON 语法不合法：{e}");
                    tracing::warn!(attempt, finish_reason, error = %last_err, "generate_world parse failed");
                }
            }
        }
        Err(format!("世界编译未能通过静态校验：{last_err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleRegistry;
    use crate::runtime::orchestrator::build_initial_session_state;

    #[test]
    fn live_provider_context_strips_raw_truth_protected_truths_and_hidden_clocks() {
        let modules = ModuleRegistry::load_default().expect("load default modules");
        let module = modules.get("crimson_rain_manor").expect("module exists");
        let mut state =
            build_initial_session_state("s-live-1", &module, "investigator_reporter").unwrap();
        state.raw_truth = json!({ "secretCulprit": "EVELYN_RITUAL_TOP_SECRET" });

        let ctx = player_visible_context(&state, Some(&module));
        let serialized = ctx.to_string();
        assert!(!serialized.contains("EVELYN_RITUAL_TOP_SECRET"));
        assert!(!serialized.contains("raw_truth"));
        assert!(!serialized.contains("ritual_countdown"));
        assert!(!serialized.contains("以家族债务胁迫"));
        assert!(!serialized.contains("arthur_assistance"));
        assert!(serialized.contains("coverStory"));
        assert!(serialized.contains("behavioralTells"));
        assert!(serialized.contains("manor_anomaly"));
        assert!(serialized.contains("study_room"));
    }

    #[test]
    fn build_chat_messages_preserves_history_and_strips_old_choice_menus() {
        let history = vec![
            TurnHistoryItem {
                player_input: None,
                narration: "【守秘人】开场切入。\n\n【建议行动】\n1. 旧选项".into(),
            },
            TurnHistoryItem {
                player_input: Some("询问在场者".into()),
                narration: "【值班主管】「这里没事。」\n\n【建议行动】\n1. 再问".into(),
            },
        ];
        let msgs = build_chat_messages("SYS", &history, "CURRENT");
        assert_eq!(msgs.len(), 5);
        assert_eq!(msgs[1]["role"], json!("assistant"));
        assert!(!msgs[1]["content"].as_str().unwrap().contains("旧选项"));
        assert_eq!(msgs[2]["role"], json!("user"));
        assert!(msgs[2]["content"].as_str().unwrap().contains("询问在场者"));
        assert_eq!(msgs[4]["content"], json!("CURRENT"));
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
        };
        let messages = build_chat_messages("sys", &[], "usr");
        let body = provider.build_request_body(messages, false, true, 512, 0.2);
        assert_eq!(body["enable_thinking"], json!(false));
        assert_eq!(body["response_format"]["type"], json!("json_object"));
        assert_eq!(body["max_tokens"], json!(512));
    }
}
