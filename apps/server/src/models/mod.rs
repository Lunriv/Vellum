use async_trait::async_trait;
use serde_json::json;
use tokio::time::{sleep, Duration};

use crate::kernel::{
    skill_display_label, ActionProposal, CheckDifficulty, ClockVisibility, DynamicWorldPatch,
    ProposedCheck, ProposedEffect, SessionState, WorldChange,
};
use crate::modules::{
    evaluate_module_predicate,
    generator::{build_mock_generated_world, extract_module_topics, WorldGenRequest},
    ModuleIr,
};

pub mod live;

/// One completed previous turn used as multi-turn conversational memory for the Keeper.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnHistoryItem {
    pub player_input: Option<String>,
    pub narration: String,
}

/// Extra authoritative context passed to `stream_narration` so the Keeper narrates
/// committed world changes, dice roll outcomes, and multi-turn continuity faithfully.
#[derive(Debug, Clone, Default)]
pub struct NarrationContext<'a> {
    pub history: &'a [TurnHistoryItem],
    pub outcome_summary: Option<&'a str>,
    pub is_opening: bool,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        history: &[TurnHistoryItem],
    ) -> Result<ActionProposal, String>;

    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        ctx: &NarrationContext<'_>,
    ) -> Result<Vec<String>, String>;

    async fn answer_ooc(
        &self,
        question: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        history: &[TurnHistoryItem],
    ) -> Result<String, String>;

    async fn generate_world(&self, req: &WorldGenRequest) -> Result<ModuleIr, String>;
}

#[derive(Clone)]
pub struct MockProvider {
    pub delay: Duration,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(0),
        }
    }
}

impl MockProvider {
    pub fn with_delay(delay: Duration) -> Self {
        Self { delay }
    }
}

fn find_matching_scene_exit(
    input: &str,
    state: &SessionState,
    module: Option<&ModuleIr>,
) -> Option<String> {
    let move_markers = [
        "前往", "去", "进入", "回到", "返回", "下楼", "绕到", "推门", "撤回", "潜入",
    ];
    if !move_markers.iter().any(|m| input.contains(m)) {
        return None;
    }
    let m = module?;
    if let Some(cur) = m.find_scene(&state.current_scene.scene_id) {
        for exit in &cur.exits {
            let accessible = exit
                .condition
                .as_ref()
                .map(|c| evaluate_module_predicate(c, state, None))
                .unwrap_or(true);
            if !accessible {
                continue;
            }
            let target_title = m
                .find_scene(&exit.target_scene_id)
                .map(|s| s.title.as_str())
                .unwrap_or("");
            if input.contains(&exit.label)
                || (!target_title.is_empty() && input.contains(target_title))
                || exit
                    .label
                    .trim_start_matches("前往")
                    .trim_start_matches("返回")
                    .trim_start_matches("回到")
                    .chars()
                    .count()
                    >= 2
                    && input.contains(
                        exit.label
                            .trim_start_matches("前往")
                            .trim_start_matches("返回")
                            .trim_start_matches("回到"),
                    )
            {
                return Some(exit.target_scene_id.clone());
            }
        }
    }
    // Fallback: match any scene title mentioned in input
    for scene in &m.scenes {
        if scene.scene_id != state.current_scene.scene_id && input.contains(&scene.title) {
            return Some(scene.scene_id.clone());
        }
    }
    None
}

fn generic_npc_voice(
    state: &SessionState,
    module: Option<&ModuleIr>,
    input: &str,
    outcome_summary: Option<&str>,
) -> Option<String> {
    let m = module?;
    let npc = m.npcs.iter().find(|n| {
        state
            .current_scene
            .present_entity_ids
            .contains(&n.entity_id)
    })?;

    if let Some(summary) = outcome_summary {
        if let Some(confession) = summary
            .lines()
            .find(|line| line.contains("低声吐露：") || line.contains("吐露："))
        {
            let quoted = confession
                .split_once("：「")
                .map(|(_, rest)| format!("「{rest}"))
                .unwrap_or_else(|| confession.to_string());
            return Some(format!(
                "【{}】对方呼吸一滞，压低了声音：{quoted}",
                npc.name
            ));
        }
    }

    let tell = npc
        .behavioral_tells
        .first()
        .map(String::as_str)
        .unwrap_or("眼神微微闪烁了一下");
    if input.contains("已经把一切告诉我") || input.contains("都招了") || input.contains("诈唬")
    {
        Some(format!(
            "【{}】「你从哪听来的？」{}，「别拿没影的事诈我。」",
            npc.name, tell
        ))
    } else {
        Some(format!(
            "【{}】「{}」话音落下时，{}。",
            npc.name, npc.cover_story, tell
        ))
    }
}

fn generic_instinct_whisper(state: &SessionState, module: Option<&ModuleIr>) -> String {
    let top_skill = state
        .investigator
        .skills
        .iter()
        .max_by_key(|(_, rating)| *rating)
        .map(|(skill, _)| skill.as_str())
        .unwrap_or("spot_hidden");
    let skill_cn = skill_display_label(top_skill);
    let hint = module
        .and_then(|m| m.find_scene(&state.current_scene.scene_id))
        .and_then(|scene| {
            scene
                .entities
                .iter()
                .find(|e| !e.summary.is_empty())
                .map(|e| format!("留意「{}」——{}", e.name, e.summary))
        })
        .unwrap_or_else(|| "现场细节与表面说辞对不上号，值得继续深挖。".into());
    format!("【直觉·{skill_cn}】{hint}")
}

fn generic_choices(state: &SessionState, module: Option<&ModuleIr>) -> (String, Vec<String>) {
    let mut choices = Vec::new();
    if let Some(m) = module {
        if let Some(scene) = m.find_scene(&state.current_scene.scene_id) {
            if let Some(npc) = scene.entities.iter().find(|e| e.kind == "npc") {
                choices.push(format!("[心理学] 试探{}的破绽", npc.name));
            }
            if let Some(obj) = scene.entities.iter().find(|e| e.kind == "object") {
                choices.push(format!("[侦查] 细查{}", obj.name));
            }
            for exit in &scene.exits {
                let accessible = exit
                    .condition
                    .as_ref()
                    .map(|c| evaluate_module_predicate(c, state, None))
                    .unwrap_or(true);
                if accessible && choices.len() < 3 {
                    choices.push(exit.label.clone());
                }
            }
        }
    }
    while choices.len() < 2 {
        choices.push(if choices.is_empty() {
            "细查现场异常痕迹".into()
        } else {
            "盘问在场人物的真实意图".into()
        });
    }
    let hook = format!(
        "「{}」的局势正在收紧，你下一步怎么出手？",
        state.current_scene.title
    );
    (hook, choices)
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        _history: &[TurnHistoryItem],
    ) -> Result<ActionProposal, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let lower = input.to_lowercase();
        let present: Vec<String> = state
            .current_scene
            .present_entity_ids
            .iter()
            .cloned()
            .collect();

        let mut action_type = "observe".to_string();
        let mut proposed_checks = Vec::new();
        let mut proposed_effects = Vec::new();
        let mut evidence_to_present = Vec::new();
        let mut topics_to_confront = Vec::new();
        let mut proposed_world_patches = Vec::new();

        // 1. Generic movement across current Module's Scene Graph
        if let Some(scene_id) = find_matching_scene_exit(input, state, module) {
            action_type = "move".into();
            proposed_effects.push(ProposedEffect {
                effect_type: "move_scene".into(),
                target_id: Some(scene_id),
                value: None,
                payload: None,
            });
        }

        // 2. Bluff detection (player claims unverified fact to present NPC)
        if (input.contains("已经把一切告诉我")
            || input.contains("都招了")
            || input.contains("全告诉我了")
            || input.contains("诈唬"))
            && !present.is_empty()
        {
            action_type = "bluff".into();
            proposed_effects.push(ProposedEffect {
                effect_type: "player_bluff".into(),
                target_id: present.first().cloned(),
                value: None,
                payload: Some(json!({ "claim": input })),
            });
        }

        // 3. Generic evidence presentation & topic confrontation
        if input.contains("出示")
            || input.contains("递给")
            || input.contains("展示")
            || input.contains("对质")
            || input.contains("质问")
        {
            action_type = "confront".into();
            for clue in state.clues.values() {
                if input.contains(&clue.title)
                    || clue.title.chars().count() >= 2
                        && (input.contains(&clue.title.chars().take(2).collect::<String>())
                            || input.contains(
                                &clue
                                    .title
                                    .chars()
                                    .rev()
                                    .take(2)
                                    .collect::<String>()
                                    .chars()
                                    .rev()
                                    .collect::<String>(),
                            ))
                {
                    evidence_to_present.push(clue.clue_id.clone());
                }
            }
            if let Some(m) = module {
                for topic in extract_module_topics(m) {
                    if input.to_lowercase().contains(&topic.to_lowercase()) {
                        topics_to_confront.push(topic);
                    }
                }
            }
        }

        // 4. Skill checks (Path C -> Pending Interrupt)
        let is_off_rail = input.contains("铁铲")
            || input.contains("后窗")
            || input.contains("破坏")
            || input.contains("纵火")
            || input.contains("破窗")
            || input.contains("硬闯");
        if !is_off_rail
            && (input.contains("撬")
                || input.contains("开锁")
                || input.contains("锁匠")
                || input.contains("强行打开"))
        {
            action_type = "check".into();
            let rating = state
                .investigator
                .skills
                .get("locksmith")
                .copied()
                .unwrap_or(25) as u32;
            proposed_checks.push(ProposedCheck {
                check_id: "locksmith_check".into(),
                skill: "locksmith".into(),
                difficulty: CheckDifficulty::Regular,
                target_number: rating,
                bonus_penalty_dice: 0,
                allow_push: true,
                action_kind: "investigate".into(),
            });
        } else if input.contains("微表情")
            || input.contains("心理学")
            || input.contains("察言观色")
            || input.contains("审视")
        {
            action_type = "check".into();
            let rating = state
                .investigator
                .skills
                .get("psychology")
                .copied()
                .unwrap_or(45) as u32;
            proposed_checks.push(ProposedCheck {
                check_id: "psychology_check".into(),
                skill: "psychology".into(),
                difficulty: CheckDifficulty::Hard,
                target_number: rating,
                bonus_penalty_dice: 0,
                allow_push: true,
                action_kind: "investigate".into(),
            });
        } else if input.contains("细查") || input.contains("侦查") || input.contains("搜查") {
            action_type = "check".into();
            let rating = state
                .investigator
                .skills
                .get("spot_hidden")
                .copied()
                .unwrap_or(50) as u32;
            proposed_checks.push(ProposedCheck {
                check_id: "spot_hidden_check".into(),
                skill: "spot_hidden".into(),
                difficulty: CheckDifficulty::Regular,
                target_number: rating,
                bonus_penalty_dice: 0,
                allow_push: true,
                action_kind: "investigate".into(),
            });
        }

        // 5. Generic Off-Rail improvisation patch (scoped tag + visible & hidden clocks)
        if is_off_rail {
            action_type = "off_rail".into();
            let tag_id = if input.contains("后窗") || input.contains("铁铲") {
                "courtyard_window_opened".to_string()
            } else {
                format!("improvised_action_{}", state.events.len() + 1)
            };
            proposed_world_patches.push(DynamicWorldPatch {
                patch_id: format!("patch-{}", state.events.len() + 1),
                source: "player_improvisation".into(),
                scope: state.current_scene.scene_id.clone(),
                lifetime: "session".into(),
                causal_event_id: Some("player_off_rail_action".into()),
                changes: vec![WorldChange {
                    kind: "temporary_tag".into(),
                    target_id: tag_id.clone(),
                    value: json!(true),
                }],
                resource_cost: None,
                required_permission: None,
            });
            proposed_effects.push(ProposedEffect {
                effect_type: "record_event".into(),
                target_id: Some(tag_id),
                value: None,
                payload: None,
            });
            if let Some(vis_clk) = state
                .clocks
                .values()
                .find(|c| c.visibility == ClockVisibility::PlayerVisible)
            {
                proposed_effects.push(ProposedEffect {
                    effect_type: "advance_clock".into(),
                    target_id: Some(vis_clk.clock_id.clone()),
                    value: Some(1),
                    payload: Some(json!({ "reason": "你的破局举动惊动了现场局势" })),
                });
            }
            if let Some(hid_clk) = state
                .clocks
                .values()
                .find(|c| c.visibility == ClockVisibility::KeeperHidden)
            {
                proposed_effects.push(ProposedEffect {
                    effect_type: "advance_clock".into(),
                    target_id: Some(hid_clk.clock_id.clone()),
                    value: Some(1),
                    payload: Some(json!({ "reason": "外界骚动令幕后推进加快了一步" })),
                });
            }
        }

        // 6. Generic direct clue discoveries based on present scene entities
        if action_type == "observe"
            && (input.contains("调查")
                || input.contains("查看")
                || input.contains("检查")
                || input.contains("翻看")
                || input.contains("询问")
                || input.contains("交谈")
                || input.contains("安抚"))
        {
            action_type = "investigate".into();
            if let Some(scene) = module.and_then(|m| m.find_scene(&state.current_scene.scene_id)) {
                let mut unlocked_any = false;
                for ent in &scene.entities {
                    let name_hit = input.contains(&ent.name)
                        || ent.name.chars().count() >= 2
                            && input.contains(&ent.name.chars().take(2).collect::<String>());
                    if name_hit {
                        for cid in &ent.clue_ids {
                            if !state.clues.contains_key(cid) {
                                proposed_effects.push(ProposedEffect {
                                    effect_type: "unlock_clue".into(),
                                    target_id: Some(cid.clone()),
                                    value: None,
                                    payload: None,
                                });
                                unlocked_any = true;
                            }
                        }
                    }
                }
                if !unlocked_any {
                    if let Some(cid) = scene
                        .entities
                        .iter()
                        .flat_map(|e| e.clue_ids.iter())
                        .find(|cid| !state.clues.contains_key(*cid))
                    {
                        proposed_effects.push(ProposedEffect {
                            effect_type: "unlock_clue".into(),
                            target_id: Some(cid.clone()),
                            value: None,
                            payload: None,
                        });
                    }
                }
            }
        }

        // 7. Explicit ending resolution
        if input.contains("揭露真相")
            || input.contains("结案")
            || input.contains("带着证据离开")
            || lower.contains("conclude")
        {
            action_type = "conclude".into();
            proposed_effects.push(ProposedEffect {
                effect_type: "end_session".into(),
                target_id: None,
                value: None,
                payload: Some(json!({ "reason": input })),
            });
        }

        Ok(ActionProposal {
            intent: input.to_string(),
            target_entity_ids: present,
            action_type,
            proposed_checks,
            proposed_effects,
            evidence_to_present,
            topics_to_confront,
            proposed_world_patches,
        })
    }

    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
        module: Option<&ModuleIr>,
        ctx: &NarrationContext<'_>,
    ) -> Result<Vec<String>, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let scene = &state.current_scene;
        let world_title = module.map(|m| m.title.as_str()).unwrap_or("当前世界");
        let ended = input.contains("揭露真相")
            || input.contains("结案")
            || input.contains("带着证据离开")
            || ctx
                .outcome_summary
                .map(|s| s.contains("结案收尾"))
                .unwrap_or(false);

        if ended {
            return Ok(vec![format!(
                "【守秘人】当你在「{}」把收集到的铁证摊开，围绕「{}」的层层掩饰终于崩解，本局调查尘埃落定。",
                scene.title, world_title
            )]);
        }

        let kp_lead = if ctx.is_opening {
            let prologue = module
                .map(|m| m.opening_prologue.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or(scene.description.as_str());
            format!(
                "【守秘人】{} 你以{}「{}」的身份踏入「{}」。{}",
                prologue,
                state.investigator.occupation,
                state.investigator.name,
                scene.title,
                scene.description
            )
        } else if let Some(summary) = ctx.outcome_summary.filter(|s| s.contains("检定")) {
            let clean = summary.replace('\n', " ");
            format!(
                "【守秘人】「{}」的局势随骰子落定骤然变化。{}",
                scene.title, clean
            )
        } else if input.contains("前往")
            || input.contains("去")
            || input.contains("回到")
            || input.contains("返回")
        {
            format!("【守秘人】你来到「{}」。{}", scene.title, scene.description)
        } else {
            format!(
                "【守秘人】你在「{}」顺着「{}」切入试探，现场的气氛立刻紧绷起来。",
                scene.title, input
            )
        };

        let mut blocks = vec![kp_lead];
        if let Some(npc_line) = generic_npc_voice(state, module, input, ctx.outcome_summary) {
            blocks.push(npc_line);
        }
        blocks.push(generic_instinct_whisper(state, module));

        let (hook, choices) = generic_choices(state, module);
        blocks.push(format!("【抉择】{hook}"));
        let choice_lines: Vec<String> = choices
            .into_iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect();
        blocks.push(format!("【建议行动】\n{}", choice_lines.join("\n")));

        let mut chunks = Vec::new();
        for (idx, block) in blocks.into_iter().enumerate() {
            if idx > 0 {
                chunks.push("\n\n".into());
            }
            chunks.push(block);
        }
        Ok(chunks)
    }

    async fn answer_ooc(
        &self,
        question: &str,
        state: &SessionState,
        _module: Option<&ModuleIr>,
        _history: &[TurnHistoryItem],
    ) -> Result<String, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let inv = &state.investigator;
        if question.contains("孤注一掷") {
            Ok(format!(
                "【场外规则说明】首次技能检定失败后（大失败、理智与战斗检定除外），若场景允许，你可以提出新的尝试方式进行一次孤注一掷。守秘人会先告知二次失败的升级代价，你确认后才会掷第二次骰子；单次检定最多孤注一掷一次，且二次结果不可再花幸运补救。（当前角色 {} 剩余幸运：{}）",
                inv.name, inv.luck
            ))
        } else if question.contains("幸运")
            || question.contains("理智")
            || question.contains("状态")
        {
            Ok(format!(
                "【当前可见角色状态】{}（{}）目前生命 {}/{}、魔力 {}/{}、理智 {}/{}、幸运 {}/{}；当前位于「{}」，已记录线索 {} 条。此查询不推进游戏时间。",
                inv.name,
                inv.occupation,
                inv.hp,
                inv.hp_max,
                inv.mp,
                inv.mp_max,
                inv.san,
                inv.san_max,
                inv.luck,
                inv.luck_max,
                state.current_scene.title,
                state.clues.len()
            ))
        } else if question.contains("困难") || question.contains("极难") {
            Ok("【检定等级规则】百面骰（d100）掷出 1 为大成功；常规成功需 ≤ 技能值，困难成功需 ≤ 技能值/2（向下取整），极难成功需 ≤ 技能值/5（向下取整）。技能值小于 50 时掷出 96–100 为大失败，技能值 ≥ 50 时仅 100 为大失败。困难/极难检定未通过时（非大失败），可用足够幸运补足差值。".into())
        } else {
            Ok(format!(
                "【场外规则助手】关于「{}」：本局采用 CoC 7e 调查子集规则，所有检定与数值均由确定性内核裁定；当前你在「{}」，生命 {}/{}、理智 {}/{}、幸运 {}。随时可在检定等待期提问，不影响当前回合。",
                question, state.current_scene.title, inv.hp, inv.hp_max, inv.san, inv.san_max, inv.luck
            ))
        }
    }

    async fn generate_world(&self, req: &WorldGenRequest) -> Result<ModuleIr, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        Ok(build_mock_generated_world(req))
    }
}
