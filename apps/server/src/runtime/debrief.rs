use serde::Serialize;

use crate::kernel::{ClockVisibility, GameEventKind, GameEventRecord, SessionState};
use crate::modules::{evaluate_module_predicate, ModuleIr};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChronologyEntryDto {
    pub title: String,
    pub detail: String,
    pub source_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStageDebriefDto {
    pub tier: String,
    pub unlocked: bool,
    pub truth: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcMotiveDebriefDto {
    pub entity_id: String,
    pub name: String,
    pub motive: String,
    pub secret_stages: Vec<SecretStageDebriefDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissedClueDto {
    pub clue_id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalClockDto {
    pub clock_id: String,
    pub name: String,
    pub visibility: ClockVisibility,
    pub current: u32,
    pub max: u32,
    pub stage_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerImpactDto {
    pub choice: String,
    pub consequence: String,
    pub source_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeEndingDto {
    pub title: String,
    pub condition_summary: String,
    pub achieved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseDebriefDto {
    pub session_id: String,
    pub module_title: String,
    pub spoiler_warning: bool,
    pub ending_title: String,
    pub ending_summary: String,
    pub true_chronology: Vec<ChronologyEntryDto>,
    pub npc_motives: Vec<NpcMotiveDebriefDto>,
    pub missed_clues: Vec<MissedClueDto>,
    pub final_clocks: Vec<FinalClockDto>,
    pub player_impact: Vec<PlayerImpactDto>,
    pub alternative_endings: Vec<AlternativeEndingDto>,
}

/// Derives Case Debrief strictly from committed `GameEventRecord`s, final `SessionState`, and `ModuleIr`.
/// Never fabricates unoccurred events or choices.
pub fn build_case_debrief(
    session_id: &str,
    state: &SessionState,
    events: &[GameEventRecord],
    module: &ModuleIr,
) -> CaseDebriefDto {
    let mut achieved_ending = None;
    let mut alternative_endings = Vec::new();

    for ending in &module.endings {
        let achieved =
            achieved_ending.is_none() && evaluate_module_predicate(&ending.condition, state, None);
        if achieved {
            achieved_ending = Some(ending);
        }
        alternative_endings.push(AlternativeEndingDto {
            title: ending.title.clone(),
            condition_summary: ending.summary.clone(),
            achieved,
        });
    }

    let (ending_title, ending_summary) = if let Some(ending) = achieved_ending {
        (ending.title.clone(), ending.summary.clone())
    } else {
        (
            "未竟的调查余波".into(),
            format!(
                "调查员提前结束了在「{}」的查探，部分核心真相仍留在暗处。",
                module.title
            ),
        )
    };

    // Derive chronology strictly from real committed events
    let mut true_chronology = Vec::new();
    let mut player_impact = Vec::new();

    for ev in events {
        match &ev.event {
            GameEventKind::ClueUnlocked { clue } => {
                true_chronology.push(ChronologyEntryDto {
                    title: format!("发现线索：{}", clue.title),
                    detail: clue.summary.clone(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::RollResolved { attempt } => {
                player_impact.push(PlayerImpactDto {
                    choice: format!(
                        "{} 检定（第 {} 次尝试）",
                        attempt.skill_label.as_deref().unwrap_or(&attempt.skill),
                        attempt.attempt_index
                    ),
                    consequence: format!(
                        "掷出 {}（目标 {}），结果：{}",
                        attempt.roll_value,
                        attempt.target_number,
                        if attempt.passed {
                            "成功"
                        } else {
                            "未通过"
                        }
                    ),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::LuckSpent { amount } => {
                player_impact.push(PlayerImpactDto {
                    choice: format!("决定消耗 {} 点幸运改判", amount),
                    consequence: "以宝贵的幸运换取关键线索或局势转机。".into(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::PushRollAccepted { description } => {
                player_impact.push(PlayerImpactDto {
                    choice: format!("选择孤注一掷：{}", description),
                    consequence: "承担升级风险强行推进调查。".into(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::WorldPatchApplied { patch } => {
                true_chronology.push(ChronologyEntryDto {
                    title: format!("即兴行动改变现场（{}）", patch.scope),
                    detail: format!("由 {} 触发的受控世界变更已提交。", patch.source),
                    source_event_ids: vec![ev.event_id.clone()],
                });
                player_impact.push(PlayerImpactDto {
                    choice: "偏离预设路线的即兴策略".into(),
                    consequence: "成功开辟了替代调查途径并改写了当前局势。".into(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::ClockAdvanced {
                clock_id,
                amount,
                reason,
                ..
            } => {
                let clock_name = state
                    .clocks
                    .get(clock_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or(clock_id.as_str());
                true_chronology.push(ChronologyEntryDto {
                    title: format!("进度钟推进：{} (+{})", clock_name, amount),
                    detail: reason.clone(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            GameEventKind::SessionEnded => {
                true_chronology.push(ChronologyEntryDto {
                    title: "调查正式结案".into(),
                    detail: ending_summary.clone(),
                    source_event_ids: vec![ev.event_id.clone()],
                });
            }
            _ => {}
        }
    }

    if true_chronology.is_empty() {
        if let Some(first_ev) = events.first() {
            true_chronology.push(ChronologyEntryDto {
                title: format!("踏入「{}」", module.title),
                detail: "展开现场调查并完成记录归档。".into(),
                source_event_ids: vec![first_ev.event_id.clone()],
            });
        }
    }

    let npc_motives = module
        .npcs
        .iter()
        .map(|npc| {
            let mut motive_parts = Vec::new();
            let mut secret_stages = Vec::new();
            for secret in &npc.protected_truths {
                motive_parts.push(secret.raw_truth.clone());
                for stage in &secret.stages {
                    let unlocked = stage
                        .releasable_fact_ids
                        .iter()
                        .any(|fid| state.npc_known_facts.contains(&format!("player:{fid}")))
                        || evaluate_module_predicate(&stage.predicate, state, None);
                    secret_stages.push(SecretStageDebriefDto {
                        tier: stage.tier.clone(),
                        unlocked,
                        truth: stage.truth.clone(),
                    });
                }
            }
            NpcMotiveDebriefDto {
                entity_id: npc.entity_id.clone(),
                name: npc.name.clone(),
                motive: if motive_parts.is_empty() {
                    npc.personality.clone()
                } else {
                    motive_parts.join(" ")
                },
                secret_stages,
            }
        })
        .collect();

    let missed_clues = module
        .clues
        .iter()
        .filter(|clue| !state.clues.contains_key(&clue.clue_id))
        .map(|clue| MissedClueDto {
            clue_id: clue.clue_id.clone(),
            title: clue.title.clone(),
            summary: clue.summary.clone(),
        })
        .collect();

    let final_clocks = state
        .clocks
        .values()
        .map(|clock| FinalClockDto {
            clock_id: clock.clock_id.clone(),
            name: clock.name.clone(),
            visibility: clock.visibility,
            current: clock.current,
            max: clock.max,
            stage_label: crate::kernel::clocks::current_stage_label(clock),
        })
        .collect();

    CaseDebriefDto {
        session_id: session_id.to_string(),
        module_title: module.title.clone(),
        spoiler_warning: true,
        ending_title,
        ending_summary,
        true_chronology,
        npc_motives,
        missed_clues,
        final_clocks,
        player_impact,
        alternative_endings,
    }
}
