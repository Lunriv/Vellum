use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::kernel::{
    apply_event, evaluate_recovery_options, roll_check, skill_display_label, GameEventKind,
    PendingInterrupt, RollAttempt, TurnStatus,
};
use crate::persistence::{event_repo, session_repo, turn_repo};
use crate::runtime::orchestrator::{self, build_client_state_patch};
use crate::AppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ResolveInterruptAction {
    Cancel,
    ReviseAction {
        #[serde(rename = "replacementAction")]
        replacement_action: String,
    },
    ConfirmRoll,
    AcceptFailure,
    SpendLuck,
    PushRoll,
    SubmitPushDescription {
        description: String,
    },
    ConfirmPushConsequence,
    AcceptInitialFailure,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveInterruptResponseDto {
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_turn_id: Option<String>,
    pub status: TurnStatus,
    pub pending_interrupt: Option<PendingInterrupt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll: Option<RollAttempt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
}

#[derive(Debug)]
pub enum InterruptError {
    NotFound,
    AlreadyResolved,
    Forbidden(&'static str, String),
    Conflict(&'static str, String),
    Internal(String),
}

pub fn pending_is_active(p: &PendingInterrupt) -> bool {
    matches!(
        p.status,
        TurnStatus::AwaitingRoll
            | TurnStatus::AwaitingRecovery
            | TurnStatus::AwaitingPushDescription
            | TurnStatus::AwaitingPushConsequenceAck
            | TurnStatus::AwaitingPushRoll
    )
}

pub fn resolve_status(current: TurnStatus, action: &str) -> Result<TurnStatus, String> {
    match (current, action) {
        (TurnStatus::AwaitingRoll, "cancel") => Ok(TurnStatus::Cancelled),
        (TurnStatus::AwaitingRoll, "confirm_roll") => Ok(TurnStatus::Committing),
        (TurnStatus::AwaitingRecovery, "accept_failure") => Ok(TurnStatus::Committing),
        (TurnStatus::AwaitingRecovery, "spend_luck") => Ok(TurnStatus::Committing),
        (TurnStatus::AwaitingRecovery, "push_roll") => Ok(TurnStatus::AwaitingPushDescription),
        (TurnStatus::AwaitingPushDescription, "submit_push_description") => {
            Ok(TurnStatus::AwaitingPushConsequenceAck)
        }
        (TurnStatus::AwaitingPushConsequenceAck, "accept_initial_failure") => {
            Ok(TurnStatus::Committing)
        }
        (TurnStatus::AwaitingPushConsequenceAck, "confirm_push_consequence") => {
            Ok(TurnStatus::AwaitingPushRoll)
        }
        (TurnStatus::AwaitingPushRoll, "roll") => Ok(TurnStatus::Committing),
        _ => Err("invalid interrupt action".into()),
    }
}

/// Resolves a Pending Interrupt inside the per-session short command lock.
/// All dice rolls and Luck deductions are persisted atomically BEFORE any narration streams.
pub async fn resolve_interrupt(
    state: AppState,
    session_id: &str,
    interrupt_id: &str,
    action: ResolveInterruptAction,
) -> Result<ResolveInterruptResponseDto, InterruptError> {
    let lock = state.locks.lock_for(session_id).await;
    let guard = lock.lock().await;
    let res = resolve_interrupt_locked(state, session_id, interrupt_id, action).await;
    drop(guard);
    res
}

/// Core interrupt resolver executed while the caller holds the short per-session command lock.
/// Callers must NOT hold the lock across LLM or SSE streaming; all spawned narration runs asynchronously.
pub async fn resolve_interrupt_locked(
    state: AppState,
    session_id: &str,
    interrupt_id: &str,
    action: ResolveInterruptAction,
) -> Result<ResolveInterruptResponseDto, InterruptError> {
    let Some((mut interrupt, resolved_at)) =
        turn_repo::get_interrupt(&state.db, session_id, interrupt_id)
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?
    else {
        return Err(InterruptError::NotFound);
    };

    if resolved_at.is_some() {
        return Err(InterruptError::AlreadyResolved);
    }

    let Some((version, mut session_state)) = session_repo::load_state(&state.db, session_id)
        .await
        .map_err(|e| InterruptError::Internal(e.to_string()))?
    else {
        return Err(InterruptError::NotFound);
    };

    let module_version = session_state.module_version.clone();
    let turn_id = interrupt.turn_id.clone();

    match (&interrupt.status, action) {
        (TurnStatus::AwaitingRoll, ResolveInterruptAction::Cancel) => {
            let ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &ev);
            session_state.turn_status = Some(TurnStatus::Cancelled);

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Cancelled,
                &json!({ "action": "cancel" }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Cancelled)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(None),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version changed concurrently".into(),
                ));
            }
            let buf = state.buffers.get_or_create(&turn_id);
            buf.set_pending_interrupt(None).await;
            buf.complete("cancelled").await;

            Ok(ResolveInterruptResponseDto {
                turn_id,
                replacement_turn_id: None,
                status: TurnStatus::Cancelled,
                pending_interrupt: None,
                roll: None,
                stream_url: None,
            })
        }

        (TurnStatus::AwaitingRoll, ResolveInterruptAction::ReviseAction { replacement_action }) => {
            let trimmed = replacement_action.trim();
            if trimmed.is_empty() {
                return Err(InterruptError::Forbidden(
                    "INVALID_REQUEST",
                    "replacementAction cannot be empty".into(),
                ));
            }

            let ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &ev);

            let new_turn_id = Uuid::new_v4().to_string();
            let next_idx = turn_repo::next_turn_index(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Cancelled,
                &json!({ "action": "revise_action", "replacementTurnId": new_turn_id }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Cancelled)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::create(
                &state.db,
                &new_turn_id,
                session_id,
                next_idx,
                "B",
                Some(trimmed),
                TurnStatus::Received,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            session_state.turn_status = Some(TurnStatus::Received);
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&new_turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version changed concurrently".into(),
                ));
            }
            let _ = state.locks.try_register_active(session_id).await;

            let old_buf = state.buffers.get_or_create(&turn_id);
            old_buf.set_pending_interrupt(None).await;
            old_buf.complete("cancelled").await;

            let state_clone = state.clone();
            let sid = session_id.to_string();
            let tid_for_spawn = new_turn_id.clone();
            let input_for_spawn = trimmed.to_string();
            tokio::spawn(async move {
                orchestrator::run_player_turn(state_clone, sid, tid_for_spawn, input_for_spawn)
                    .await;
            });

            Ok(ResolveInterruptResponseDto {
                turn_id,
                replacement_turn_id: Some(new_turn_id.clone()),
                status: TurnStatus::Received,
                pending_interrupt: None,
                roll: None,
                stream_url: Some(format!(
                    "/api/sessions/{session_id}/turns/{new_turn_id}/events"
                )),
            })
        }

        (TurnStatus::AwaitingRoll, ResolveInterruptAction::ConfirmRoll) => {
            let attempt = {
                let mut rng = state.rng.lock().unwrap();
                roll_check(
                    &mut *rng,
                    format!("chk-{}", interrupt.interrupt_id),
                    interrupt.skill.clone(),
                    interrupt.target_number,
                    interrupt.difficulty,
                    interrupt.bonus_penalty_dice,
                    1,
                    0,
                )
            };

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let roll_ev = GameEventKind::RollResolved {
                attempt: attempt.clone(),
            };
            apply_event(&mut session_state, &roll_ev);
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &roll_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            let is_sanity = interrupt.action_kind == "sanity";
            let is_luck = interrupt.action_kind == "luck";
            let is_combat = interrupt.action_kind == "combat";

            let recovery = evaluate_recovery_options(
                &attempt,
                session_state.investigator.luck.max(0) as u32,
                is_sanity,
                is_luck,
                is_combat,
                interrupt.allow_push,
            );

            let has_recovery_choice =
                !attempt.passed && (recovery.can_spend_luck || recovery.can_push_roll);

            if has_recovery_choice {
                interrupt.status = TurnStatus::AwaitingRecovery;
                interrupt.first_roll = Some(attempt.clone());
                interrupt.recovery = Some(recovery);
                session_state.pending_interrupt = Some(interrupt.clone());
                session_state.turn_status = Some(TurnStatus::AwaitingRecovery);

                turn_repo::save_interrupt(&state.db, &interrupt)
                    .await
                    .map_err(|e| InterruptError::Internal(e.to_string()))?;
                turn_repo::set_status(&state.db, &turn_id, TurnStatus::AwaitingRecovery)
                    .await
                    .map_err(|e| InterruptError::Internal(e.to_string()))?;
                let ok = session_repo::cas_snapshot(
                    &state.db,
                    session_id,
                    version,
                    &session_state,
                    None,
                    Some(Some(&turn_id)),
                )
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
                if !ok {
                    return Err(InterruptError::Conflict(
                        "VERSION_CONFLICT",
                        "Session version conflict".into(),
                    ));
                }
                let buf = state.buffers.get_or_create(&turn_id);
                buf.set_pending_interrupt(Some(interrupt.clone())).await;
                buf.push(
                    "interrupt_required",
                    json!({ "type": "interrupt_required", "interrupt": interrupt }),
                )
                .await;

                return Ok(ResolveInterruptResponseDto {
                    turn_id: turn_id.clone(),
                    replacement_turn_id: None,
                    status: TurnStatus::AwaitingRecovery,
                    pending_interrupt: Some(interrupt),
                    roll: Some(attempt),
                    stream_url: Some(format!("/api/sessions/{session_id}/turns/{turn_id}/events")),
                });
            }

            // Passed OR failed with no recovery options -> resolve interrupt and commit + narrate
            let resolved_ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &resolved_ev);
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq + 1,
                &resolved_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Committing,
                &json!({ "action": "confirm_roll", "passed": attempt.passed }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Committing)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;

            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let _ = state.locks.try_register_active(session_id).await;

            let state_clone = state.clone();
            let sid = session_id.to_string();
            let tid_clone = turn_id.clone();
            let attempt_clone = attempt.clone();
            tokio::spawn(async move {
                orchestrator::complete_resolved_check_turn(
                    state_clone,
                    sid,
                    tid_clone,
                    attempt_clone,
                    false,
                    None,
                )
                .await;
            });

            Ok(ResolveInterruptResponseDto {
                turn_id: turn_id.clone(),
                replacement_turn_id: None,
                status: TurnStatus::Committing,
                pending_interrupt: None,
                roll: Some(attempt),
                stream_url: Some(format!("/api/sessions/{session_id}/turns/{turn_id}/events")),
            })
        }

        (TurnStatus::AwaitingRecovery, ResolveInterruptAction::AcceptFailure)
        | (TurnStatus::AwaitingPushConsequenceAck, ResolveInterruptAction::AcceptInitialFailure) => {
            let first_roll = interrupt
                .first_roll
                .clone()
                .ok_or_else(|| InterruptError::Internal("missing first_roll".into()))?;
            let resolved_ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &resolved_ev);

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &resolved_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Committing,
                &json!({ "action": "accept_failure" }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Committing)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let _ = state.locks.try_register_active(session_id).await;

            let state_clone = state.clone();
            let sid = session_id.to_string();
            let tid_clone = turn_id.clone();
            let roll_clone = first_roll.clone();
            tokio::spawn(async move {
                orchestrator::complete_resolved_check_turn(
                    state_clone,
                    sid,
                    tid_clone,
                    roll_clone,
                    false,
                    None,
                )
                .await;
            });

            Ok(ResolveInterruptResponseDto {
                turn_id: turn_id.clone(),
                replacement_turn_id: None,
                status: TurnStatus::Committing,
                pending_interrupt: None,
                roll: Some(first_roll),
                stream_url: Some(format!("/api/sessions/{session_id}/turns/{turn_id}/events")),
            })
        }

        (TurnStatus::AwaitingRecovery, ResolveInterruptAction::SpendLuck) => {
            let mut roll = interrupt
                .first_roll
                .clone()
                .ok_or_else(|| InterruptError::Internal("missing first_roll".into()))?;
            let eligibility = interrupt.recovery.as_ref().ok_or_else(|| {
                InterruptError::Forbidden("FORBIDDEN_ACTION", "Luck spend is not available".into())
            })?;
            let Some(cost) = eligibility
                .luck_cost_needed
                .filter(|_| eligibility.can_spend_luck)
            else {
                return Err(InterruptError::Forbidden(
                    "FORBIDDEN_ACTION",
                    "Current check is not eligible for Luck spending".into(),
                ));
            };
            if (session_state.investigator.luck as u32) < cost {
                return Err(InterruptError::Forbidden(
                    "INSUFFICIENT_LUCK",
                    "Not enough Luck points".into(),
                ));
            }

            roll.passed = true;
            roll.result_level = match roll.difficulty {
                crate::kernel::CheckDifficulty::Regular => {
                    crate::kernel::RollResultLevel::RegularSuccess
                }
                crate::kernel::CheckDifficulty::Hard => crate::kernel::RollResultLevel::HardSuccess,
                crate::kernel::CheckDifficulty::Extreme => {
                    crate::kernel::RollResultLevel::ExtremeSuccess
                }
            };

            let luck_ev = GameEventKind::LuckSpent { amount: cost };
            let res_ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &luck_ev);
            session_state.skill_successes.insert(roll.skill.clone());
            apply_event(&mut session_state, &res_ev);

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &luck_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq + 1,
                &res_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Committing,
                &json!({ "action": "spend_luck", "luckSpent": cost }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Committing)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let _ = state.locks.try_register_active(session_id).await;

            let state_clone = state.clone();
            let sid = session_id.to_string();
            let tid_clone = turn_id.clone();
            let roll_clone = roll.clone();
            tokio::spawn(async move {
                orchestrator::complete_resolved_check_turn(
                    state_clone,
                    sid,
                    tid_clone,
                    roll_clone,
                    false,
                    Some(cost),
                )
                .await;
            });

            Ok(ResolveInterruptResponseDto {
                turn_id: turn_id.clone(),
                replacement_turn_id: None,
                status: TurnStatus::Committing,
                pending_interrupt: None,
                roll: Some(roll),
                stream_url: Some(format!("/api/sessions/{session_id}/turns/{turn_id}/events")),
            })
        }

        (TurnStatus::AwaitingRecovery, ResolveInterruptAction::PushRoll) => {
            let can_push = interrupt
                .recovery
                .as_ref()
                .map(|r| r.can_push_roll)
                .unwrap_or(false);
            if !can_push {
                return Err(InterruptError::Forbidden(
                    "FORBIDDEN_ACTION",
                    "This check is not eligible for Push the Roll".into(),
                ));
            }
            interrupt.status = TurnStatus::AwaitingPushDescription;
            session_state.pending_interrupt = Some(interrupt.clone());
            session_state.turn_status = Some(TurnStatus::AwaitingPushDescription);

            turn_repo::save_interrupt(&state.db, &interrupt)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::AwaitingPushDescription)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let buf = state.buffers.get_or_create(&turn_id);
            buf.set_pending_interrupt(Some(interrupt.clone())).await;
            buf.push(
                "interrupt_required",
                json!({ "type": "interrupt_required", "interrupt": interrupt }),
            )
            .await;

            Ok(ResolveInterruptResponseDto {
                turn_id,
                replacement_turn_id: None,
                status: TurnStatus::AwaitingPushDescription,
                pending_interrupt: Some(interrupt),
                roll: None,
                stream_url: None,
            })
        }

        (
            TurnStatus::AwaitingPushDescription,
            ResolveInterruptAction::SubmitPushDescription { description },
        ) => {
            let trimmed = description.trim();
            if trimmed.is_empty() {
                return Err(InterruptError::Forbidden(
                    "INVALID_REQUEST",
                    "Push description must explain how you try again".into(),
                ));
            }
            let skill_cn = skill_display_label(&interrupt.skill);
            let visible_clock_name = session_state
                .clocks
                .values()
                .find(|c| c.visibility == crate::kernel::ClockVisibility::PlayerVisible)
                .map(|c| c.name.as_str())
                .unwrap_or("现场危机");
            let consequence = format!(
                "若强行重试「{}」再次失手，你的举动将直接惊动现场并推进「{}」危机进度，且可能损失 1 点理智（二次掷骰无补救机会）。",
                skill_cn, visible_clock_name
            );
            interrupt.push_description = Some(trimmed.to_string());
            interrupt.push_consequence = Some(consequence);
            interrupt.status = TurnStatus::AwaitingPushConsequenceAck;
            session_state.pending_interrupt = Some(interrupt.clone());
            session_state.turn_status = Some(TurnStatus::AwaitingPushConsequenceAck);

            turn_repo::save_interrupt(&state.db, &interrupt)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::AwaitingPushConsequenceAck)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let buf = state.buffers.get_or_create(&turn_id);
            buf.set_pending_interrupt(Some(interrupt.clone())).await;
            buf.push(
                "interrupt_required",
                json!({ "type": "interrupt_required", "interrupt": interrupt }),
            )
            .await;

            Ok(ResolveInterruptResponseDto {
                turn_id,
                replacement_turn_id: None,
                status: TurnStatus::AwaitingPushConsequenceAck,
                pending_interrupt: Some(interrupt),
                roll: None,
                stream_url: None,
            })
        }

        (
            TurnStatus::AwaitingPushConsequenceAck,
            ResolveInterruptAction::ConfirmPushConsequence,
        ) => {
            let push_desc = interrupt
                .push_description
                .clone()
                .unwrap_or_else(|| "强行重试".into());
            let push_accept_ev = GameEventKind::PushRollAccepted {
                description: push_desc,
            };
            apply_event(&mut session_state, &push_accept_ev);

            // Transition explicitly through AwaitingPushRoll and roll 2nd die (attempt_index=2, push_count=1)
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::AwaitingPushRoll)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;

            let mut second_roll = {
                let mut rng = state.rng.lock().unwrap();
                roll_check(
                    &mut *rng,
                    format!("chk-{}", interrupt.interrupt_id),
                    interrupt.skill.clone(),
                    interrupt.target_number,
                    interrupt.difficulty,
                    interrupt.bonus_penalty_dice,
                    2,
                    1,
                )
            };
            second_roll.consequence_ref = interrupt.push_consequence.clone();

            let roll_ev = GameEventKind::RollResolved {
                attempt: second_roll.clone(),
            };
            let res_ev = GameEventKind::InterruptResolved {
                interrupt_id: interrupt_id.to_string(),
            };
            apply_event(&mut session_state, &roll_ev);
            apply_event(&mut session_state, &res_ev);
            session_state.turn_status = Some(TurnStatus::Committing);

            let seq = event_repo::next_event_seq(&state.db, session_id)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq,
                &push_accept_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq + 1,
                &roll_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            event_repo::append(
                &state.db,
                &Uuid::new_v4().to_string(),
                session_id,
                &turn_id,
                seq + 2,
                &res_ev,
                &module_version,
                None,
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;

            turn_repo::resolve_interrupt_record(
                &state.db,
                interrupt_id,
                TurnStatus::Committing,
                &json!({ "action": "confirm_push_consequence", "passed": second_roll.passed }),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            turn_repo::set_status(&state.db, &turn_id, TurnStatus::Committing)
                .await
                .map_err(|e| InterruptError::Internal(e.to_string()))?;

            let ok = session_repo::cas_snapshot(
                &state.db,
                session_id,
                version,
                &session_state,
                None,
                Some(Some(&turn_id)),
            )
            .await
            .map_err(|e| InterruptError::Internal(e.to_string()))?;
            if !ok {
                return Err(InterruptError::Conflict(
                    "VERSION_CONFLICT",
                    "Session version conflict".into(),
                ));
            }
            let _ = state.locks.try_register_active(session_id).await;

            let state_clone = state.clone();
            let sid = session_id.to_string();
            let tid_clone = turn_id.clone();
            let roll_clone = second_roll.clone();
            tokio::spawn(async move {
                orchestrator::complete_resolved_check_turn(
                    state_clone,
                    sid,
                    tid_clone,
                    roll_clone,
                    true,
                    None,
                )
                .await;
            });

            let module = state.modules.get(&session_state.module_id);
            let patch =
                build_client_state_patch(&session_state, version + 1, "active", module.as_ref());
            let buf = state.buffers.get_or_create(&turn_id);
            buf.push(
                "state_delta",
                json!({ "type": "state_delta", "patch": patch }),
            )
            .await;

            Ok(ResolveInterruptResponseDto {
                turn_id: turn_id.clone(),
                replacement_turn_id: None,
                status: TurnStatus::Committing,
                pending_interrupt: None,
                roll: Some(second_roll),
                stream_url: Some(format!("/api/sessions/{session_id}/turns/{turn_id}/events")),
            })
        }

        _ => Err(InterruptError::Forbidden(
            "FORBIDDEN_ACTION",
            format!(
                "Action not allowed while interrupt is in status {:?}",
                interrupt.status
            ),
        )),
    }
}
