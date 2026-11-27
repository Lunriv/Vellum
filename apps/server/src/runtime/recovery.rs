use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::kernel::TurnStatus;
use crate::models::NarrationContext;
use crate::persistence::{
    db::Database,
    event_repo::{self, TranscriptRecord},
    session_repo, turn_repo,
};
use crate::runtime::orchestrator;
use crate::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryLease {
    pub turn_id: String,
    pub lease_id: String,
    pub owner_id: String,
    pub generation_id: i64,
    pub expires_at: String,
}

/// Startup consistency pass: NEVER invokes LLMs.
/// Marks pre-commit in-flight turns as `interrupted_before_commit` and
/// committed-but-unfinished `narrating` turns as `narration_pending`.
pub async fn scan_startup(db: &Database) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE turns SET status='interrupted_before_commit', updated_at=? WHERE status IN ('received','routing','proposal_pending')",
    )
    .bind(&now)
    .execute(&db.pool)
    .await?;
    sqlx::query(
        "UPDATE turns SET status='narration_pending', updated_at=? WHERE status='narrating'",
    )
    .bind(&now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// Acquires the unique SQLite CAS RecoveryLease for a `narration_pending` turn
/// (or a `recovering` turn whose previous lease has already expired).
pub async fn try_acquire_lease(
    db: &Database,
    turn_id: &str,
    owner_id: &str,
    ttl: Duration,
) -> Result<Option<RecoveryLease>, sqlx::Error> {
    let now = Utc::now();
    let expires =
        now + ChronoDuration::from_std(ttl).unwrap_or_else(|_| ChronoDuration::seconds(60));
    let now_text = now.to_rfc3339();
    let expires_text = expires.to_rfc3339();

    // Use BEGIN IMMEDIATE so concurrent lease acquisitions serialize cleanly via SQLite busy_timeout
    // instead of hitting SQLite's deferred read-to-write upgrade SQLITE_BUSY deadlock.
    let mut tx = db.pool.begin_with("BEGIN IMMEDIATE").await?;
    let turn_row = sqlx::query_as::<_, (String, i64)>(
        "SELECT status, narration_generation FROM turns WHERE id=?",
    )
    .bind(turn_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((status, current_gen)) = turn_row else {
        tx.rollback().await?;
        return Ok(None);
    };

    if status != "narration_pending" && status != "recovering" {
        tx.rollback().await?;
        return Ok(None);
    }

    let next_gen = current_gen + 1;
    let lease_id = Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO recovery_leases(turn_id,lease_id,owner_id,generation_id,expires_at,updated_at)
         VALUES(?,?,?,?,?,?)
         ON CONFLICT(turn_id) DO UPDATE SET
           lease_id=excluded.lease_id,
           owner_id=excluded.owner_id,
           generation_id=excluded.generation_id,
           expires_at=excluded.expires_at,
           updated_at=excluded.updated_at
         WHERE recovery_leases.expires_at <= excluded.updated_at",
    )
    .bind(turn_id)
    .bind(&lease_id)
    .bind(owner_id)
    .bind(next_gen)
    .bind(&expires_text)
    .bind(&now_text)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(None);
    }

    let updated = sqlx::query(
        "UPDATE turns SET narration_generation=?, status='recovering', updated_at=? WHERE id=? AND status IN ('narration_pending','recovering')",
    )
    .bind(next_gen)
    .bind(&now_text)
    .bind(turn_id)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(None);
    }

    tx.commit().await?;
    Ok(Some(RecoveryLease {
        turn_id: turn_id.to_string(),
        lease_id,
        owner_id: owner_id.to_string(),
        generation_id: next_gen,
        expires_at: expires_text,
    }))
}

pub async fn renew_lease(
    db: &Database,
    lease: &RecoveryLease,
    ttl: Duration,
) -> Result<bool, sqlx::Error> {
    let now = Utc::now();
    let expires =
        now + ChronoDuration::from_std(ttl).unwrap_or_else(|_| ChronoDuration::seconds(60));
    let now_text = now.to_rfc3339();
    let result = sqlx::query(
        "UPDATE recovery_leases SET expires_at=?, updated_at=? WHERE turn_id=? AND lease_id=? AND owner_id=? AND generation_id=? AND expires_at > ?",
    )
    .bind(expires.to_rfc3339())
    .bind(&now_text)
    .bind(&lease.turn_id)
    .bind(&lease.lease_id)
    .bind(&lease.owner_id)
    .bind(lease.generation_id)
    .bind(&now_text)
    .execute(&db.pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn lease_is_current(db: &Database, lease: &RecoveryLease) -> Result<bool, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM recovery_leases WHERE turn_id=? AND lease_id=? AND owner_id=? AND generation_id=? AND expires_at > ?",
    )
    .bind(&lease.turn_id)
    .bind(&lease.lease_id)
    .bind(&lease.owner_id)
    .bind(lease.generation_id)
    .bind(now)
    .fetch_one(&db.pool)
    .await?;
    Ok(count == 1)
}

/// Runs lazy narration recovery exclusively for the winning `RecoveryLease` holder.
/// Never re-executes dice rolls or authoritative kernel mutations.
pub async fn run_narration_recovery(
    state: AppState,
    session_id: String,
    turn_id: String,
    lease: RecoveryLease,
    ttl: Duration,
) {
    let buffer = state.buffers.get_or_create(&turn_id);
    let gen_id = lease.generation_id;

    let _ = event_repo::rebind_kernel_directives_generation(&state.db, &turn_id, gen_id).await;
    buffer.reset_narration_for_generation(gen_id).await;

    buffer
        .push(
            "recovery_started",
            json!({
                "type": "recovery_started",
                "turnId": turn_id,
                "generationId": gen_id,
            }),
        )
        .await;
    buffer
        .push(
            "narration_reset",
            json!({
                "type": "narration_reset",
                "turnId": turn_id,
                "generationId": gen_id,
                "clearPartial": true,
            }),
        )
        .await;

    // Re-push any persisted undisplayed kernel-owned directives bound to this generation
    if let Ok(directives) = event_repo::list_directives_for_turn(&state.db, &turn_id).await {
        for d in directives {
            if d.source == "kernel_owned" {
                buffer.record_directive(d.clone()).await;
                buffer
                    .push(
                        "ui_directive",
                        json!({ "type": "ui_directive", "directive": d }),
                    )
                    .await;
            }
        }
    }

    let Ok(Some((_, session_state))) = session_repo::load_state(&state.db, &session_id).await
    else {
        buffer.complete("failed").await;
        return;
    };

    let turn_input = turn_repo::get(&state.db, &turn_id)
        .await
        .ok()
        .flatten()
        .and_then(|t| t.player_input)
        .unwrap_or_else(|| "继续观察当前现场".into());

    let history = orchestrator::load_recent_history(&state.db, &session_id, 6).await;
    let narration_ctx = NarrationContext {
        history: &history,
        outcome_summary: None,
        is_opening: false,
    };

    let chunks = match state
        .provider
        .stream_narration(&turn_input, &session_state, &narration_ctx)
        .await
    {
        Ok(c) => c,
        Err(err) => {
            let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::NarrationPending).await;
            buffer
                .push(
                    "turn_error",
                    json!({
                        "type": "turn_error",
                        "turnId": turn_id,
                        "code": "NARRATION_RECOVERY_FAILED",
                        "message": err,
                    }),
                )
                .await;
            buffer.complete("narration_pending").await;
            return;
        }
    };

    let mut full_text = String::new();
    for chunk in chunks {
        if !lease_is_current(&state.db, &lease).await.unwrap_or(false) {
            return;
        }
        let _ = renew_lease(&state.db, &lease, ttl).await;
        full_text.push_str(&chunk);
        buffer.append_narration(&chunk).await;
        buffer
            .push(
                "narration_delta",
                json!({
                    "type": "narration_delta",
                    "chunk": chunk,
                    "turnId": turn_id,
                    "generationId": gen_id,
                }),
            )
            .await;
    }

    // Final lease check before committing Transcript and Completed status
    if !lease_is_current(&state.db, &lease).await.unwrap_or(false) {
        return;
    }

    let record = TranscriptRecord {
        entry_id: format!("tr-rec-{turn_id}-g{gen_id}"),
        turn_id: turn_id.clone(),
        scene_id: session_state.current_scene.scene_id.clone(),
        speaker_entity_id: None,
        present_entity_ids: session_state
            .current_scene
            .present_entity_ids
            .iter()
            .cloned()
            .collect(),
        audible_entity_ids: session_state
            .current_scene
            .present_entity_ids
            .iter()
            .cloned()
            .collect(),
        visible_entity_ids: session_state
            .current_scene
            .present_entity_ids
            .iter()
            .cloned()
            .collect(),
        visibility_scope: "public".into(),
        claim_kind: "narration".into(),
        referenced_event_ids: vec![],
        content: full_text,
        created_at: Utc::now().to_rfc3339(),
    };
    let _ = event_repo::insert_transcript(&state.db, &session_id, &record).await;
    let _ = session_repo::set_active_turn(&state.db, &session_id, None).await;
    state.locks.clear_active(&session_id).await;
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Completed).await;

    buffer
        .push(
            "narration_complete",
            json!({
                "type": "narration_complete",
                "turnId": turn_id,
                "generationId": gen_id,
            }),
        )
        .await;
    buffer
        .push(
            "turn_complete",
            json!({
                "type": "turn_complete",
                "turnId": turn_id,
            }),
        )
        .await;
    buffer.complete("completed").await;
}
