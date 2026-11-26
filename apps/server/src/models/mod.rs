use async_trait::async_trait;
use serde_json::json;
use tokio::time::{sleep, Duration};

use crate::kernel::{
    ActionProposal, CheckDifficulty, DynamicWorldPatch, ProposedCheck, ProposedEffect,
    SessionState, WorldChange,
};

pub mod live;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<ActionProposal, String>;
    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
    ) -> Result<Vec<String>, String>;
    async fn answer_ooc(&self, question: &str, state: &SessionState) -> Result<String, String>;
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

#[async_trait]
impl LlmProvider for MockProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
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

        // 1. Movement across Scene Graph
        let target_scene = if input.contains("书房")
            && (input.contains("前往")
                || input.contains("去")
                || input.contains("进入")
                || input.contains("回到"))
        {
            Some("study_room")
        } else if input.contains("酒窖")
            && (input.contains("前往")
                || input.contains("去")
                || input.contains("下楼")
                || input.contains("绕到"))
        {
            Some("cellar_entrance")
        } else if (input.contains("偏厅") || input.contains("女仆"))
            && (input.contains("前往") || input.contains("去") || input.contains("找"))
        {
            Some("maids_parlor")
        } else if input.contains("庭院")
            && (input.contains("前往")
                || input.contains("去")
                || input.contains("推门")
                || input.contains("后门"))
        {
            Some("rain_courtyard")
        } else if input.contains("门厅")
            && (input.contains("前往") || input.contains("回") || input.contains("去"))
        {
            Some("manor_foyer")
        } else {
            None
        };

        if let Some(scene_id) = target_scene {
            action_type = "move".into();
            proposed_effects.push(ProposedEffect {
                effect_type: "move_scene".into(),
                target_id: Some(scene_id.into()),
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

        // 3. Evidence presentation & topic confrontation (Protected Truth triggers)
        if input.contains("出示")
            || input.contains("递给")
            || input.contains("展示")
            || input.contains("对质")
            || input.contains("质问")
        {
            action_type = "confront".into();
            if input.contains("账本") && state.clues.contains_key("cellar_ledger_clue") {
                evidence_to_present.push("cellar_ledger_clue".into());
            }
            if input.contains("袖口") && state.clues.contains_key("bloodstained_cuff_clue") {
                evidence_to_present.push("bloodstained_cuff_clue".into());
            }
            if input.contains("剪报") && state.clues.contains_key("manor_clipping_clue") {
                evidence_to_present.push("manor_clipping_clue".into());
            }
            if input.contains("仪式结论") && state.clues.contains_key("conclusion_ritual") {
                evidence_to_present.push("conclusion_ritual".into());
            }
            if input.contains("钟声") {
                topics_to_confront.push("bell_tower".into());
            }
            if input.contains("失踪") || input.contains("访客") || input.contains("三年前") {
                topics_to_confront.push("missing_guest".into());
            }
            if input.contains("仪式") || input.contains("真相") {
                topics_to_confront.push("ritual_truth".into());
            }
            if input.contains("Evelyn") || input.contains("女主人") {
                topics_to_confront.push("lady_evelyn".into());
            }
        }

        // 4. Skill checks (Path C -> Pending Interrupt)
        if input.contains("撬") || input.contains("开锁") || input.contains("强行打开") {
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
            || input.contains("审视管家")
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
        } else if input.contains("细查")
            || input.contains("侦查")
            || input.contains("搜查书架")
            || input.contains("检查泥痕")
        {
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

        // 5. Off-Rail improvisation patch (e.g. shovel / back window / lantern)
        if input.contains("铁铲") || input.contains("后窗") || input.contains("火") {
            action_type = "off_rail".into();
            proposed_world_patches.push(DynamicWorldPatch {
                patch_id: format!("patch-{}", state.events.len() + 1),
                source: "player_improvisation".into(),
                scope: state.current_scene.scene_id.clone(),
                lifetime: "session".into(),
                causal_event_id: Some("player_off_rail_action".into()),
                changes: vec![WorldChange {
                    kind: "temporary_tag".into(),
                    target_id: "courtyard_window_opened".into(),
                    value: json!(true),
                }],
                resource_cost: None,
                required_permission: None,
            });
            proposed_effects.push(ProposedEffect {
                effect_type: "record_event".into(),
                target_id: Some("courtyard_window_opened".into()),
                value: None,
                payload: None,
            });
            proposed_effects.push(ProposedEffect {
                effect_type: "advance_clock".into(),
                target_id: Some("manor_anomaly".into()),
                value: Some(1),
                payload: Some(
                    json!({ "reason": "你在暴雨中强行开辟了后窗通道，惊动了庄园深处的动静" }),
                ),
            });
            proposed_effects.push(ProposedEffect {
                effect_type: "advance_clock".into(),
                target_id: Some("ritual_countdown".into()),
                value: Some(1),
                payload: Some(json!({ "reason": "酒窖内的准备因外界骚动而加快" })),
            });
        }

        // 6. Direct clue discoveries based on scene & intent
        if action_type == "observe"
            && (input.contains("调查")
                || input.contains("查看")
                || input.contains("检查")
                || input.contains("翻看")
                || input.contains("询问")
                || input.contains("托盘")
                || input.contains("剪报")
                || input.contains("账本")
                || input.contains("袖口")
                || input.contains("肖像"))
        {
            action_type = "investigate".into();
            if input.contains("托盘")
                || state.current_scene.scene_id == "maids_parlor" && input.contains("检查")
            {
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("silver_mark_clue".into()),
                    value: None,
                    payload: None,
                });
            }
            if input.contains("剪报") || input.contains("肖像") || input.contains("报纸") {
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("manor_clipping_clue".into()),
                    value: None,
                    payload: None,
                });
            }
            if input.contains("袖口") || input.contains("抽屉") {
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("bloodstained_cuff_clue".into()),
                    value: None,
                    payload: None,
                });
            }
            if input.contains("账本")
                || (state.current_scene.scene_id == "cellar_entrance" && input.contains("调查"))
            {
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("cellar_ledger_clue".into()),
                    value: None,
                    payload: None,
                });
            }
            if input.contains("钟声")
                || (state.current_scene.scene_id == "maids_parlor"
                    && (input.contains("询问") || input.contains("安抚")))
            {
                topics_to_confront.push("bell_tower".into());
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("mary_testimony_clue".into()),
                    value: None,
                    payload: None,
                });
            }
            if input.contains("泥痕") || input.contains("温室") {
                proposed_effects.push(ProposedEffect {
                    effect_type: "unlock_clue".into(),
                    target_id: Some("courtyard_tracks_clue".into()),
                    value: None,
                    payload: None,
                });
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
    ) -> Result<Vec<String>, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let scene = &state.current_scene;
        let p1 = format!("{}里雨声未歇，灯影在潮湿的木纹上微微晃动。", scene.title);
        let p2 = if input.contains("已经把一切告诉我")
            || input.contains("都招了")
            || input.contains("诈唬")
        {
            "听见你的话，对方面色骤然一僵，手指下意识攥紧了衣襟——他显然无法立刻分辨你的虚实，只能强撑着礼数试探你的底牌。".into()
        } else if input.contains("铁铲") || input.contains("后窗") {
            "你借着暴雨声掩护，用庭院找到的工具撬开了锈蚀的后窗扣环。一股夹杂着蜡油与湿土的气息扑面而来，通往酒窖的暗道已在脚下敞开。".into()
        } else if input.contains("揭露真相")
            || input.contains("结案")
            || input.contains("带着证据离开")
        {
            "当证据摊开在摇曳的烛火之下，绯雨庄园维持多年的沉默终于裂开缝隙。暴雨之外的天际隐约泛起微光，这场雨夜调查已尘埃落定。".into()
        } else if input.contains("前往")
            || input.contains("去")
            || input.contains("回到")
            || input.contains("返回")
        {
            format!("你穿过微凉的走廊步入{}。{}", scene.title, scene.description)
        } else {
            format!("你顺着「{}」的思路仔细查探。壁炉灰烬与窗棂水痕之间，几处不合常理的细节逐渐浮现，等着你继续追问或交叉核实。", input)
        };
        Ok(vec![p1, "\n\n".into(), p2])
    }

    async fn answer_ooc(&self, question: &str, state: &SessionState) -> Result<String, String> {
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
}
