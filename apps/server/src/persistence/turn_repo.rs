use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::db::Database;
use crate::kernel::{PendingInterrupt, TurnStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRow {
    pub id: String,
    pub session_id: String,
    pub turn_index: i64,
    pub path_kind: String,
    pub status: TurnStatus,
    pub player_input: Option<String>,
    pub narration_generation: i64,
    pub committed_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn status_to_db(status: TurnStatus) -> String {
    serde_json::to_string(&status)
        .unwrap_or_else(|_| "\"failed\"".into())
        .trim_matches('"')
        .to_string()
}

fn status_from_db(raw: &str) -> TurnStatus {
    serde_json::from_str(&format!("\"{raw}\"")).unwrap_or(TurnStatus::Failed)
}

pub async fn create(
    db: &Database,
    id: &str,
    session_id: &str,
    index: i64,
    path: &str,
    input: Option<&str>,
    status: TurnStatus,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO turns(id,session_id,turn_index,path_kind,status,player_input,narration_generation,created_at,updated_at) VALUES(?,?,?,?,?,?,1,?,?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(index)
    .bind(path)
    .bind(status_to_db(status))
    .bind(input)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn next_turn_index(db: &Database, session_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(turn_index), -1) + 1 FROM turns WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_one(&db.pool)
    .await
}

pub async fn get(db: &Database, id: &str) -> Result<Option<TurnRow>, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            String,
            Option<String>,
            i64,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id,session_id,turn_index,path_kind,status,player_input,narration_generation,committed_at,completed_at,created_at,updated_at FROM turns WHERE id=?",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await?;

    Ok(row.map(
        |(
            id,
            session_id,
            turn_index,
            path_kind,
            status,
            player_input,
            narration_generation,
            committed_at,
            completed_at,
            created_at,
            updated_at,
        )| TurnRow {
            id,
            session_id,
            turn_index,
            path_kind,
            status: status_from_db(&status),
            player_input,
            narration_generation,
            committed_at,
            completed_at,
            created_at,
            updated_at,
        },
    ))
}

pub async fn list_for_session(
    db: &Database,
    session_id: &str,
) -> Result<Vec<TurnRow>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            String,
            Option<String>,
            i64,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id,session_id,turn_index,path_kind,status,player_input,narration_generation,committed_at,completed_at,created_at,updated_at FROM turns WHERE session_id=? ORDER BY turn_index ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                session_id,
                turn_index,
                path_kind,
                status,
                player_input,
                narration_generation,
                committed_at,
                completed_at,
                created_at,
                updated_at,
            )| TurnRow {
                id,
                session_id,
                turn_index,
                path_kind,
                status: status_from_db(&status),
                player_input,
                narration_generation,
                committed_at,
                completed_at,
                created_at,
                updated_at,
            },
        )
        .collect())
}

pub async fn status(db: &Database, id: &str) -> Result<Option<TurnStatus>, sqlx::Error> {
    let row = sqlx::query_scalar::<_, String>("SELECT status FROM turns WHERE id=?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await?;
    Ok(row.map(|s| status_from_db(&s)))
}

pub async fn set_status(db: &Database, id: &str, status: TurnStatus) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let committed_at = if matches!(status, TurnStatus::Narrating) {
        Some(now.clone())
    } else {
        None
    };
    let completed_at = if matches!(status, TurnStatus::Completed) {
        Some(now.clone())
    } else {
        None
    };
    sqlx::query(
        "UPDATE turns SET status=?, committed_at=COALESCE(committed_at, ?), completed_at=COALESCE(completed_at, ?), updated_at=? WHERE id=?",
    )
    .bind(status_to_db(status))
    .bind(committed_at)
    .bind(completed_at)
    .bind(&now)
    .bind(id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn set_path_kind(db: &Database, id: &str, path_kind: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE turns SET path_kind=?, updated_at=? WHERE id=?")
        .bind(path_kind)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

pub async fn save_interrupt(
    db: &Database,
    interrupt: &PendingInterrupt,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let payload_json = serde_json::to_string(interrupt).unwrap_or_else(|_| "{}".into());
    let first_roll_json = interrupt
        .first_roll
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok());
    let push_conseq_json = interrupt
        .push_consequence
        .as_ref()
        .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()));
    sqlx::query(
        "INSERT INTO pending_interrupts(id,session_id,turn_id,status,check_payload_json,first_roll_json,push_description,push_consequence_json,created_at) VALUES(?,?,?,?,?,?,?,?,?)
         ON CONFLICT(id) DO UPDATE SET status=excluded.status, check_payload_json=excluded.check_payload_json, first_roll_json=excluded.first_roll_json, push_description=excluded.push_description, push_consequence_json=excluded.push_consequence_json",
    )
    .bind(&interrupt.interrupt_id)
    .bind(&interrupt.session_id)
    .bind(&interrupt.turn_id)
    .bind(status_to_db(interrupt.status))
    .bind(payload_json)
    .bind(first_roll_json)
    .bind(&interrupt.push_description)
    .bind(push_conseq_json)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn resolve_interrupt_record(
    db: &Database,
    interrupt_id: &str,
    status: TurnStatus,
    resolution: &Value,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE pending_interrupts SET status=?, resolution_json=?, resolved_at=? WHERE id=?",
    )
    .bind(status_to_db(status))
    .bind(resolution.to_string())
    .bind(now)
    .bind(interrupt_id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn get_interrupt(
    db: &Database,
    session_id: &str,
    interrupt_id: &str,
) -> Result<Option<(PendingInterrupt, Option<String>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT check_payload_json, resolved_at FROM pending_interrupts WHERE session_id=? AND id=?",
    )
    .bind(session_id)
    .bind(interrupt_id)
    .fetch_optional(&db.pool)
    .await?;

    Ok(row.and_then(|(json, resolved_at)| {
        serde_json::from_str::<PendingInterrupt>(&json)
            .ok()
            .map(|p| (p, resolved_at))
    }))
}

pub async fn get_active_interrupt(
    db: &Database,
    session_id: &str,
) -> Result<Option<PendingInterrupt>, sqlx::Error> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT check_payload_json FROM pending_interrupts WHERE session_id=? AND resolved_at IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&db.pool)
    .await?;

    Ok(row.and_then(|json| serde_json::from_str::<PendingInterrupt>(&json).ok()))
}
