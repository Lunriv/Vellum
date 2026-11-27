use async_trait::async_trait;
use serde_json::json;
use tokio::time::{sleep, Duration};

use crate::kernel::{
    ActionProposal, CheckDifficulty, DynamicWorldPatch, ProposedCheck, ProposedEffect,
    SessionState, WorldChange,
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
        history: &[TurnHistoryItem],
    ) -> Result<ActionProposal, String>;

    async fn stream_narration(
        &self,
        input: &str,
        state: &SessionState,
        ctx: &NarrationContext<'_>,
    ) -> Result<Vec<String>, String>;

    async fn answer_ooc(
        &self,
        question: &str,
        state: &SessionState,
        history: &[TurnHistoryItem],
    ) -> Result<String, String>;
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

fn mock_npc_voice(scene_id: &str, input: &str, outcome_summary: Option<&str>) -> Option<String> {
    if let Some(summary) = outcome_summary {
        if let Some(confession) = summary
            .lines()
            .find(|line| line.contains("低声吐露：") || line.contains("吐露："))
        {
            let speaker = match scene_id {
                "manor_foyer" => "老管家 Arthur",
                "study_room" => "女主人 Evelyn",
                "maids_parlor" => "女仆 Mary",
                _ => "在场者",
            };
            let quoted = confession
                .split_once("：「")
                .map(|(_, rest)| format!("「{rest}"))
                .unwrap_or_else(|| confession.to_string());
            return Some(format!("【{speaker}】他压低嗓音，指节微微发白：{quoted}"));
        }
    }

    match scene_id {
        "manor_foyer" => {
            if input.contains("已经把一切告诉我")
                || input.contains("都招了")
                || input.contains("诈唬")
            {
                Some("【老管家 Arthur】「哦？不知您是从哪位下人口中听来的闲话。」他缓缓整了整袖口，指腹却在铜钥匙串上停顿了一瞬，「雨夜路滑，阁下可别听信流言伤了体面。」".into())
            } else {
                Some("【老管家 Arthur】「外头暴雨封山，女主人此刻精神欠佳。」他提起黄铜提灯，微光扫过你的衣角，右手下意识摩挲着拇指，「若是为三年前那桩早已结案的旧闻而来，恕老朽多嘴——酒窖那边湿滑阴冷，还是莫要靠近为好。」".into())
            }
        }
        "study_room" => Some(
            "【女主人 Evelyn】「请不要翻动先父的书桌，阁下。」她站在雨雾朦胧的高窗前，左手不动声色地按住腕间的黑线，「三年前那位访客早已乘早班马车离去，那些被剪过的旧报纸不过是小报胡说八道。」".into(),
        ),
        "maids_parlor" => Some(
            "【女仆 Mary】「嘘——轻点声！」她双手紧攥着银托盘边缘，惊慌地往走廊门缝瞥了一眼，「管家先生不准任何人提昨夜三点的事……可那三声钟响，真的是从酒窖底下传上来的！」".into(),
        ),
        _ => None,
    }
}

fn mock_instinct_whisper(state: &SessionState) -> String {
    let scene_id = state.current_scene.scene_id.as_str();
    let top_skill = state
        .investigator
        .skills
        .iter()
        .max_by_key(|(_, rating)| *rating)
        .map(|(skill, _)| skill.as_str())
        .unwrap_or("spot_hidden");

    match (scene_id, top_skill) {
        ("manor_foyer", "medicine") => {
            "【直觉·医学】Arthur 袖口边缘残留的并非雨泥，而是一层刚凝固不久的暗褐蜡油与微弱铁锈味。".into()
        }
        ("manor_foyer", _) => {
            "【直觉·心理学】他越是礼貌地提醒你别去酒窖，握紧黄铜提灯的指关节就越泛白——他在掩护什么。".into()
        }
        ("study_room", "medicine") => {
            "【直觉·医学】书桌锁孔周围残留着极淡的血腥与松节油气味，抽屉里一定藏着近期沾染过的织物。".into()
        }
        ("study_room", _) => {
            "【直觉·侦查】Evelyn 每次听见你靠近书桌都会下意识按住左腕，而书架后的旧报纸边缘明显有新剪的折痕。".into()
        }
        ("maids_parlor", _) => {
            "【直觉·心理学】Mary 的恐惧是真实的，但她摩挲银托盘背面的动作像是在无声提醒你看那里。".into()
        }
        ("rain_courtyard", _) => {
            "【直觉·侦查】温室玻璃后那串被雨水冲淡的泥痕笔直延伸向酒窖后窗，绝非普通园丁留下的脚印。".into()
        }
        ("cellar_entrance", _) => {
            "【直觉·侦查】潮湿石阶深处飘来新燃过的蜂蜡与草药气息，门后不止藏着旧酒桶。".into()
        }
        _ => "【直觉·侦查】潮湿的空气里藏着不协调的细节，值得顺着眼前的破绽继续追查。".into(),
    }
}

fn mock_choices_for_scene(scene_id: &str) -> (&'static str, [&'static str; 3]) {
    match scene_id {
        "manor_foyer" => (
            "老管家正静候你的答复，提灯微光照亮通往书房与偏厅的走廊——你打算怎么试探？",
            [
                "[心理学] 盯住 Arthur 的袖口与微表情，追问三年前访客的下落",
                "[侦查] 借口欣赏油画，凑近检查家族肖像背后的暗格",
                "礼貌颔首，转身前往书房拜会女主人 Evelyn",
            ],
        ),
        "study_room" => (
            "Evelyn 正冷淡地注视着你的每一步，书桌锁抽屉与书架旧报纸近在咫尺——你的下一步是？",
            [
                "[锁匠] 趁她移开视线，尝试撬开书桌锁住的抽屉",
                "[侦查] 翻看书架后被裁剪过的庄园旧剪报",
                "暂退一步，前往女仆偏厅向佣人打听昨夜动静",
            ],
        ),
        "maids_parlor" => (
            "Mary 紧张地攥着银托盘，似乎只要再多一分安抚就会说出实情——你怎么应对？",
            [
                "温和安抚女仆 Mary，低声询问昨夜三点的钟声",
                "请她翻转银托盘，检查背面刻着的庄园旧徽记",
                "离开偏厅，冒雨前往暴雨庭院查探后窗",
            ],
        ),
        "rain_courtyard" => (
            "暴雨倾盆而下，荒废温室与铁链锁住的酒窖后窗就在眼前——你决定怎么破局？",
            [
                "[侦查] 蹲身细查温室外通往后窗的泥痕与脚印",
                "找把铁铲强行撬开锈蚀的酒窖后窗（可能闹出动静）",
                "绕行前往酒窖入口，直接查看铁门",
            ],
        ),
        "cellar_entrance" => (
            "铁门缝隙里渗出蜡油与铁锈的异味，真相似乎只隔着一道门——你要怎么做？",
            [
                "[锁匠] 尝试撬开酒窖铁门，取出门后的酒窖账本",
                "调查酒窖铁门与门缝周边留下的仪式痕迹",
                "带着现有线索返回门厅，与老管家当面对质",
            ],
        ),
        _ => (
            "雨声渐渐沉重，下一步你打算如何推进调查？",
            [
                "观察周围环境的异常细节",
                "返回门厅重新盘点线索",
                "继续追问在场人物",
            ],
        ),
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn propose_action(
        &self,
        input: &str,
        state: &SessionState,
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
        // Skip check when player explicitly invoked Off-Rail iron shovel / back window improvisation.
        let is_off_rail_shovel = input.contains("铁铲") || input.contains("后窗");
        if !is_off_rail_shovel
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
                || input.contains("交谈")
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
                    && (input.contains("询问") || input.contains("安抚") || input.contains("交谈")))
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
        ctx: &NarrationContext<'_>,
    ) -> Result<Vec<String>, String> {
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let scene = &state.current_scene;
        let ended = input.contains("揭露真相")
            || input.contains("结案")
            || input.contains("带着证据离开")
            || ctx
                .outcome_summary
                .map(|s| s.contains("结案收尾"))
                .unwrap_or(false);

        if ended {
            let finale = format!(
                "【守秘人】当你在{}把收集到的证据摊开在摇曳的烛火之下，绯雨庄园维持多年的沉默终于裂开缝隙。暴雨之外的天际隐约泛起微光，这场雨夜调查已尘埃落定。",
                scene.title
            );
            return Ok(vec![finale]);
        }

        let kp_lead = if ctx.is_opening {
            format!(
                "【守秘人】1927 年深秋的暴雨将山路冲成黑色的河。你以{}「{}」的身份推开绯雨庄园沉重的橡木门，怀中那封没有署名的委托函已被雨汽浸得微凉。{}里烛影摇曳，老管家 Arthur 提着黄铜提灯从壁炉阴影中缓步迎出。",
                state.investigator.occupation, state.investigator.name, scene.title
            )
        } else if let Some(summary) = ctx.outcome_summary.filter(|s| s.contains("检定")) {
            let clean = summary.replace('\n', " ");
            format!(
                "【守秘人】{}的雨声仿佛在骰子落定的一刻骤然收紧。{}",
                scene.title, clean
            )
        } else if input.contains("已经把一切告诉我")
            || input.contains("都招了")
            || input.contains("诈唬")
        {
            format!(
                "【守秘人】你的话音落在{}潮湿的空气里，壁炉火星噼啪一爆。对方面色骤然一僵，手指下意识攥紧衣襟，显然一时难以辨清你的虚实。",
                scene.title
            )
        } else if input.contains("铁铲") || input.contains("后窗") {
            "【守秘人】你借着暴雨声掩护，用庭院找到的工具撬开了锈蚀的后窗扣环。铁锈崩裂的脆响混入雷声，一股夹杂着蜂蜡与湿土的气息从下方扑面而来，通往酒窖的暗道已在脚下敞开。".into()
        } else if input.contains("前往")
            || input.contains("去")
            || input.contains("回到")
            || input.contains("返回")
        {
            format!(
                "【守秘人】你踏过泛着水光的木地板步入{}。{}",
                scene.title, scene.description
            )
        } else {
            format!(
                "【守秘人】在{}昏黄晃动的灯影里，你顺着「{}」的念头逼近查探。潮湿木纹与烛影交错间，隐约露出了值得追问的破绽。",
                scene.title, input
            )
        };

        let mut blocks = vec![kp_lead];
        if let Some(npc_line) = mock_npc_voice(&scene.scene_id, input, ctx.outcome_summary) {
            blocks.push(npc_line);
        }
        blocks.push(mock_instinct_whisper(state));

        let (hook, choices) = mock_choices_for_scene(&scene.scene_id);
        blocks.push(format!("【抉择】{hook}"));
        blocks.push(format!(
            "【建议行动】\n1. {}\n2. {}\n3. {}",
            choices[0], choices[1], choices[2]
        ));

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
}
