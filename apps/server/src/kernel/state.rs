use super::{GameEventKind, SessionState, TurnStatus};
use serde_json::Value;

fn bounded_resource(current: &mut i32, delta: i32, maximum: i32) {
    *current = current.saturating_add(delta).clamp(0, maximum.max(0));
}

fn value_i32(value: &Value) -> Option<i32> {
    value.as_i64().and_then(|number| i32::try_from(number).ok())
}

fn apply_world_change(state: &mut SessionState, kind: &str, target_id: &str, value: &Value) {
    match kind {
        "set_flag" => {
            state.flags.insert(target_id.to_owned(), value.clone());
        }
        "remove_flag" => {
            state.flags.remove(target_id);
        }
        "set_item" => {
            if let Some(quantity) = value_i32(value) {
                state.items.insert(target_id.to_owned(), quantity.max(0));
            }
        }
        "add_item" => {
            if let Some(quantity) = value_i32(value) {
                let item = state.items.entry(target_id.to_owned()).or_default();
                *item = item.saturating_add(quantity).max(0);
            }
        }
        "remove_item" => {
            state.items.remove(target_id);
        }
        "add_entity" | "add_scene_entity" => {
            state
                .current_scene
                .present_entity_ids
                .insert(target_id.to_owned());
        }
        "remove_entity" | "remove_scene_entity" => {
            state.current_scene.present_entity_ids.remove(target_id);
        }
        "set_condition" => {
            state.investigator.conditions.insert(target_id.to_owned());
        }
        "remove_condition" => {
            state.investigator.conditions.remove(target_id);
        }
        _ => {}
    }
}

pub fn apply_event(state: &mut SessionState, event: &GameEventKind) {
    match event {
        GameEventKind::InvestigatorSelected { investigator } => {
            state.investigator = investigator.clone()
        }
        GameEventKind::TurnStarted { .. } => state.turn_status = Some(TurnStatus::Routing),
        GameEventKind::PlayerActionAccepted { .. } => {}
        GameEventKind::RollResolved { attempt } => {
            if attempt.passed {
                state.skill_successes.insert(attempt.skill.clone());
            }
            state.turn_status = Some(if attempt.passed {
                TurnStatus::Committing
            } else {
                TurnStatus::AwaitingRecovery
            });
        }
        GameEventKind::LuckSpent { amount } => {
            state.investigator.luck = state.investigator.luck.saturating_sub(*amount as i32);
            state.turn_status = Some(TurnStatus::Committing);
        }
        GameEventKind::ResourceChanged { resource, delta } => match resource.as_str() {
            "hp" => bounded_resource(
                &mut state.investigator.hp,
                *delta,
                state.investigator.hp_max,
            ),
            "mp" => bounded_resource(
                &mut state.investigator.mp,
                *delta,
                state.investigator.mp_max,
            ),
            "san" => bounded_resource(
                &mut state.investigator.san,
                *delta,
                state.investigator.san_max,
            ),
            "luck" => bounded_resource(
                &mut state.investigator.luck,
                *delta,
                state.investigator.luck_max,
            ),
            _ => {
                let value = state.items.entry(resource.clone()).or_default();
                *value = value.saturating_add(*delta).max(0);
            }
        },
        GameEventKind::ItemAcquired { item } => {
            let quantity = state.items.entry(item.item_id.clone()).or_default();
            *quantity = quantity.saturating_add(item.quantity).max(0);
            if let Some(existing) = state
                .investigator
                .inventory
                .iter_mut()
                .find(|existing| existing.item_id == item.item_id)
            {
                existing.quantity = existing.quantity.saturating_add(item.quantity).max(0);
            } else {
                state.investigator.inventory.push(item.clone());
            }
        }
        GameEventKind::ItemLost { item_id, quantity } => {
            if let Some(value) = state.items.get_mut(item_id) {
                *value = value.saturating_sub(*quantity).max(0);
            }
            if let Some(existing) = state
                .investigator
                .inventory
                .iter_mut()
                .find(|existing| existing.item_id == *item_id)
            {
                existing.quantity = existing.quantity.saturating_sub(*quantity).max(0);
            }
        }
        GameEventKind::ClueUnlocked { clue } => {
            state.clues.insert(clue.clue_id.clone(), clue.clone());
        }
        GameEventKind::HandoutReleased { handout } => {
            state
                .handouts
                .insert(handout.handout_id.clone(), handout.clone());
        }
        GameEventKind::ClockAdvanced {
            clock_id,
            amount,
            reason,
            caused_by,
        } => {
            if let Some(clock) = state.clocks.get_mut(clock_id) {
                clock.current = clock.current.saturating_add(*amount).min(clock.max);
                clock.last_reason = Some(reason.clone());
                clock.last_caused_by = Some(caused_by.clone());
            }
        }
        GameEventKind::SceneChanged { scene } => {
            let mut visited = state.current_scene.visited.clone();
            visited.extend(scene.visited.iter().cloned());
            visited.insert(scene.scene_id.clone());
            state.current_scene = scene.clone();
            state.current_scene.visited = visited;
        }
        GameEventKind::WorldPatchApplied { patch } => {
            for change in &patch.changes {
                apply_world_change(state, &change.kind, &change.target_id, &change.value);
            }
        }
        GameEventKind::InterruptCreated { interrupt } => {
            state.pending_interrupt = Some(interrupt.clone());
            state.turn_status = Some(interrupt.status);
        }
        GameEventKind::InterruptResolved { interrupt_id } => {
            if state
                .pending_interrupt
                .as_ref()
                .is_some_and(|pending| pending.interrupt_id == *interrupt_id)
            {
                state.pending_interrupt = None;
            }
        }
        GameEventKind::TurnCommitted => state.turn_status = Some(TurnStatus::Narrating),
        GameEventKind::NarrationAccepted { .. } => state.turn_status = Some(TurnStatus::Completed),
        GameEventKind::NpcKnowledgeAcquired { entity_id, fact_id } => {
            state
                .npc_known_facts
                .insert(format!("{entity_id}:{fact_id}"));
        }
        GameEventKind::NpcBeliefChanged { entity_id, claim } => {
            state.npc_beliefs.insert(entity_id.clone(), claim.clone());
        }
        GameEventKind::NpcRelationChanged {
            entity_id,
            relation,
            value,
        } => {
            state
                .npc_relations
                .insert(format!("{entity_id}:{relation}"), *value);
        }
        GameEventKind::EvidencePresented { clue_id } => {
            state.evidence_presented.insert(clue_id.clone());
        }
        GameEventKind::TopicConfronted { topic_id } => {
            state.topics_confronted.insert(topic_id.clone());
        }
        GameEventKind::EventOccurred { event_id } => {
            state.events.insert(event_id.clone());
        }
        GameEventKind::FlagSet { key, value } => {
            state.flags.insert(key.clone(), value.clone());
        }
        GameEventKind::ConditionAdded { condition } => {
            state.investigator.conditions.insert(condition.clone());
        }
        GameEventKind::SessionEnded => state.turn_status = Some(TurnStatus::Completed),
        GameEventKind::SessionCreated { .. }
        | GameEventKind::ActionProposed { .. }
        | GameEventKind::RollRequested { .. }
        | GameEventKind::PushRollAccepted { .. } => {}
    }
}

pub fn replay_events(
    mut initial: SessionState,
    events: impl IntoIterator<Item = GameEventKind>,
) -> SessionState {
    for event in events {
        apply_event(&mut initial, &event);
    }
    initial
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        DynamicWorldPatch, InventoryItem, InvestigatorState, SceneState, WorldChange,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn state() -> SessionState {
        SessionState {
            session_id: "session".into(),
            module_id: "module".into(),
            module_version: "1".into(),
            investigator: InvestigatorState {
                investigator_id: "investigator".into(),
                name: "Test".into(),
                occupation: "Test".into(),
                hp: 8,
                hp_max: 10,
                mp: 4,
                mp_max: 6,
                san: 50,
                san_max: 60,
                luck: 30,
                luck_max: 40,
                characteristics: BTreeMap::new(),
                skills: BTreeMap::new(),
                inventory: vec![],
                conditions: BTreeSet::new(),
            },
            current_scene: SceneState {
                scene_id: "foyer".into(),
                title: "Foyer".into(),
                description: String::new(),
                atmosphere: "rain_manor".into(),
                visited: ["foyer".into()].into_iter().collect(),
                present_entity_ids: BTreeSet::new(),
            },
            clocks: BTreeMap::new(),
            clues: BTreeMap::new(),
            handouts: BTreeMap::new(),
            flags: BTreeMap::new(),
            items: BTreeMap::new(),
            skill_successes: BTreeSet::new(),
            events: BTreeSet::new(),
            evidence_presented: BTreeSet::new(),
            topics_confronted: BTreeSet::new(),
            npc_heard_events: BTreeSet::new(),
            npc_known_facts: BTreeSet::new(),
            npc_beliefs: BTreeMap::new(),
            npc_relations: BTreeMap::new(),
            raw_truth: Value::Null,
            pending_interrupt: None,
            turn_status: None,
        }
    }

    #[test]
    fn scene_change_accumulates_visited_scenes() {
        let mut state = state();
        let scene = SceneState {
            scene_id: "study".into(),
            title: "Study".into(),
            description: String::new(),
            atmosphere: "candlelight".into(),
            visited: ["study".into()].into_iter().collect(),
            present_entity_ids: BTreeSet::new(),
        };
        apply_event(&mut state, &GameEventKind::SceneChanged { scene });
        assert_eq!(
            state.current_scene.visited,
            ["foyer", "study"].into_iter().map(String::from).collect()
        );
    }

    #[test]
    fn resource_events_update_investigator_resources_and_clamp() {
        let mut state = state();
        apply_event(
            &mut state,
            &GameEventKind::ResourceChanged {
                resource: "hp".into(),
                delta: -20,
            },
        );
        apply_event(
            &mut state,
            &GameEventKind::ResourceChanged {
                resource: "mp".into(),
                delta: 20,
            },
        );
        apply_event(
            &mut state,
            &GameEventKind::ResourceChanged {
                resource: "san".into(),
                delta: -3,
            },
        );
        apply_event(
            &mut state,
            &GameEventKind::ResourceChanged {
                resource: "luck".into(),
                delta: 20,
            },
        );
        assert_eq!((state.investigator.hp, state.investigator.mp), (0, 6));
        assert_eq!((state.investigator.san, state.investigator.luck), (47, 40));
    }

    #[test]
    fn belief_relation_and_patch_events_are_replayable() {
        let mut state = state();
        apply_event(
            &mut state,
            &GameEventKind::NpcBeliefChanged {
                entity_id: "maid".into(),
                claim: "suspects the investigator".into(),
            },
        );
        apply_event(
            &mut state,
            &GameEventKind::NpcRelationChanged {
                entity_id: "maid".into(),
                relation: "trust".into(),
                value: 2,
            },
        );
        apply_event(
            &mut state,
            &GameEventKind::WorldPatchApplied {
                patch: DynamicWorldPatch {
                    patch_id: "patch".into(),
                    source: "player".into(),
                    scope: "scene".into(),
                    lifetime: "session".into(),
                    causal_event_id: None,
                    changes: vec![
                        WorldChange {
                            kind: "set_flag".into(),
                            target_id: "window_open".into(),
                            value: Value::Bool(true),
                        },
                        WorldChange {
                            kind: "add_scene_entity".into(),
                            target_id: "shovel".into(),
                            value: Value::Null,
                        },
                    ],
                    resource_cost: None,
                    required_permission: None,
                },
            },
        );
        assert_eq!(
            state.npc_beliefs.get("maid"),
            Some(&"suspects the investigator".to_owned())
        );
        assert_eq!(state.npc_relations.get("maid:trust"), Some(&2));
        assert_eq!(state.flags.get("window_open"), Some(&Value::Bool(true)));
        assert!(state.current_scene.present_entity_ids.contains("shovel"));
    }

    #[test]
    fn item_acquisition_and_loss_update_projection() {
        let mut state = state();
        let item = InventoryItem {
            item_id: "key".into(),
            name: "Key".into(),
            description: "A key".into(),
            quantity: 2,
        };
        apply_event(
            &mut state,
            &GameEventKind::ItemAcquired { item: item.clone() },
        );
        apply_event(&mut state, &GameEventKind::ItemAcquired { item });
        apply_event(
            &mut state,
            &GameEventKind::ItemLost {
                item_id: "key".into(),
                quantity: 1,
            },
        );
        assert_eq!(state.items.get("key"), Some(&3));
        assert_eq!(state.investigator.inventory[0].quantity, 3);
    }
}
