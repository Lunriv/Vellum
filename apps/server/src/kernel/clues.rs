use super::SessionState;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum Predicate {
    All {
        all: Vec<Predicate>,
    },
    Any {
        any: Vec<Predicate>,
    },
    Not {
        not: Box<Predicate>,
    },
    HasItem {
        has_item: String,
    },
    SkillSuccess {
        skill_success: String,
    },
    NpcRelationAtLeast {
        npc_relation_at_least: RelationPredicate,
    },
    FlagEquals {
        flag_equals: FlagPredicate,
    },
    ClockAtLeast {
        clock_at_least: ClockPredicate,
    },
    ClueDiscovered {
        clue_discovered: String,
    },
    SceneVisited {
        scene_visited: String,
    },
    EventOccurred {
        event_occurred: String,
    },
    EvidencePresented {
        evidence_presented: BehaviorPredicate,
    },
    TopicConfronted {
        topic_confronted: BehaviorPredicate,
    },
    NpcHeardEvent {
        npc_heard_event: String,
    },
    NpcKnowsFact {
        npc_knows_fact: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationPredicate {
    pub entity_id: String,
    pub value: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlagPredicate {
    pub key: String,
    pub value: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClockPredicate {
    pub clock_id: String,
    pub value: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BehaviorPredicate {
    pub clue_id: Option<String>,
    pub target_entity_id: Option<String>,
    pub topic_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ActionContext {
    pub present_entity_ids: std::collections::BTreeSet<String>,
    pub unlocked_clues: std::collections::BTreeSet<String>,
    pub evidence_presented: std::collections::BTreeSet<String>,
    pub topics_confronted: std::collections::BTreeSet<String>,
    pub event_ids: std::collections::BTreeSet<String>,
}

pub fn evaluate(
    predicate: &Predicate,
    state: &SessionState,
    action: Option<&ActionContext>,
) -> bool {
    match predicate {
        Predicate::All { all } => all.iter().all(|p| evaluate(p, state, action)),
        Predicate::Any { any } => any.iter().any(|p| evaluate(p, state, action)),
        Predicate::Not { not } => !evaluate(not, state, action),
        Predicate::HasItem { has_item } => {
            state.items.get(has_item).copied().unwrap_or(0) > 0
                || state
                    .investigator
                    .inventory
                    .iter()
                    .any(|i| i.item_id == *has_item && i.quantity > 0)
        }
        Predicate::SkillSuccess { skill_success } => state.skill_successes.contains(skill_success),
        Predicate::NpcRelationAtLeast {
            npc_relation_at_least,
        } => {
            state
                .flags
                .get(&format!("relation:{}", npc_relation_at_least.entity_id))
                .and_then(Value::as_i64)
                .unwrap_or(0)
                >= npc_relation_at_least.value as i64
        }
        Predicate::FlagEquals { flag_equals } => {
            state.flags.get(&flag_equals.key) == Some(&flag_equals.value)
        }
        Predicate::ClockAtLeast { clock_at_least } => state
            .clocks
            .get(&clock_at_least.clock_id)
            .map(|c| c.current >= clock_at_least.value)
            .unwrap_or(false),
        Predicate::ClueDiscovered { clue_discovered } => state.clues.contains_key(clue_discovered),
        Predicate::SceneVisited { scene_visited } => {
            state.current_scene.visited.contains(scene_visited)
        }
        Predicate::EventOccurred { event_occurred } => state.events.contains(event_occurred),
        Predicate::NpcHeardEvent { npc_heard_event } => {
            state.npc_heard_events.contains(npc_heard_event)
        }
        Predicate::NpcKnowsFact { npc_knows_fact } => {
            state.npc_known_facts.contains(npc_knows_fact)
        }
        Predicate::EvidencePresented { evidence_presented } => action
            .map(|a| {
                evidence_presented
                    .target_entity_id
                    .as_ref()
                    .map(|id| a.present_entity_ids.contains(id))
                    .unwrap_or(true)
                    && evidence_presented
                        .clue_id
                        .as_ref()
                        .map(|id| {
                            a.unlocked_clues.contains(id) && a.evidence_presented.contains(id)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false),
        Predicate::TopicConfronted { topic_confronted } => action
            .map(|a| {
                topic_confronted
                    .target_entity_id
                    .as_ref()
                    .map(|id| a.present_entity_ids.contains(id))
                    .unwrap_or(true)
                    && topic_confronted
                        .topic_id
                        .as_ref()
                        .map(|id| a.topics_confronted.contains(id))
                        .unwrap_or(false)
            })
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn behavior_requires_action() {
        let state: SessionState = serde_json::from_value(serde_json::json!({"sessionId":"s","moduleId":"m","moduleVersion":"1","investigator":{"investigatorId":"i","name":"n","occupation":"o","hp":1,"hpMax":1,"mp":1,"mpMax":1,"san":1,"sanMax":1,"luck":1,"luckMax":1},"currentScene":{"sceneId":"x","title":"x","description":"","atmosphere":"fog","visited":[],"presentEntityIds":[]},"rawTruth":null})).unwrap();
        let p = Predicate::EvidencePresented {
            evidence_presented: BehaviorPredicate {
                clue_id: Some("c".into()),
                target_entity_id: Some("n".into()),
                topic_id: None,
            },
        };
        assert!(!evaluate(&p, &state, None));
    }
}
