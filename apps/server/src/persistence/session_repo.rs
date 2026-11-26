use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::db::Database;
use crate::kernel::SessionState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetaRow {
    pub session_id: String,
    pub module_id: String,
    pub module_version: String,
    pub investigator_id: String,
    pub experience_mode: String,
    pub status: String,
    pub session_version: i64,
    pub active_turn_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    db: &Database,
    id: &str,
    module_id: &str,
    module_version: &str,
    investigator_id: &str,
    mode: &str,
    active_turn_id: Option<&str>,
    state: &SessionState,
) -> Result<SessionMetaRow, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let state_json = serde_json::to_string(state).unwrap_or_else(|_| "{}".into());
    let mut tx = db.pool.begin().await?;
    sqlx::query(
        "INSERT INTO sessions(id,module_id,module_version,investigator_id,experience_mode,status,session_version,active_turn_id,created_at,updated_at) VALUES(?,?,?,?,?,'active',0,?,?,?)",
    )
    .bind(id)
    .bind(module_id)
    .bind(module_version)
    .bind(investigator_id)
    .bind(mode)
    .bind(active_turn_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO session_snapshots(session_id,session_version,state_json,updated_at) VALUES(?,0,?,?)",
    )
    .bind(id)
    .bind(&state_json)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(SessionMetaRow {
        session_id: id.to_string(),
        module_id: module_id.to_string(),
        module_version: module_version.to_string(),
        investigator_id: investigator_id.to_string(),
        experience_mode: mode.to_string(),
        status: "active".to_string(),
        session_version: 0,
        active_turn_id: active_turn_id.map(str::to_string),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn get_meta(db: &Database, id: &str) -> Result<Option<SessionMetaRow>, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            i64,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT module_id,module_version,investigator_id,experience_mode,status,session_version,active_turn_id,created_at,updated_at FROM sessions WHERE id=?",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await?;

    Ok(row.map(
        |(
            module_id,
            module_version,
            investigator_id,
            experience_mode,
            status,
            session_version,
            active_turn_id,
            created_at,
            updated_at,
        )| SessionMetaRow {
            session_id: id.to_string(),
            module_id,
            module_version,
            investigator_id,
            experience_mode,
            status,
            session_version,
            active_turn_id,
            created_at,
            updated_at,
        },
    ))
}

pub async fn snapshot(db: &Database, id: &str) -> Result<Option<(i64, Value)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT session_version,state_json FROM session_snapshots WHERE session_id=?",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await?;
    Ok(row.map(|(v, s)| (v, serde_json::from_str(&s).unwrap_or(Value::Null))))
}

pub async fn load_state(
    db: &Database,
    id: &str,
) -> Result<Option<(i64, SessionState)>, sqlx::Error> {
    let Some((ver, val)) = snapshot(db, id).await? else {
        return Ok(None);
    };
    let state: SessionState = serde_json::from_value(val).map_err(|e| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        )))
    })?;
    Ok(Some((ver, state)))
}

pub async fn cas_snapshot(
    db: &Database,
    id: &str,
    expected_version: i64,
    state: &SessionState,
    status: Option<&str>,
    active_turn_id: Option<Option<&str>>,
) -> Result<bool, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let state_json = serde_json::to_string(state).unwrap_or_else(|_| "{}".into());
    let mut tx = db.pool.begin().await?;

    let updated = sqlx::query(
        "UPDATE sessions SET session_version=session_version+1, status=COALESCE(?, status), updated_at=? WHERE id=? AND session_version=?",
    )
    .bind(status)
    .bind(&now)
    .bind(id)
    .bind(expected_version)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(false);
    }

    if let Some(turn_opt) = active_turn_id {
        sqlx::query("UPDATE sessions SET active_turn_id=? WHERE id=?")
            .bind(turn_opt)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    let snap = sqlx::query(
        "UPDATE session_snapshots SET session_version=session_version+1, state_json=?, updated_at=? WHERE session_id=? AND session_version=?",
    )
    .bind(&state_json)
    .bind(&now)
    .bind(id)
    .bind(expected_version)
    .execute(&mut *tx)
    .await?;

    if snap.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(false);
    }

    tx.commit().await?;
    Ok(true)
}

pub async fn set_active_turn(
    db: &Database,
    session_id: &str,
    turn_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET active_turn_id=?, updated_at=? WHERE id=?")
        .bind(turn_id)
        .bind(Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

pub async fn update_experience_mode(
    db: &Database,
    session_id: &str,
    mode: &str,
) -> Result<Option<SessionMetaRow>, sqlx::Error> {
    let updated = sqlx::query("UPDATE sessions SET experience_mode=?, updated_at=? WHERE id=?")
        .bind(mode)
        .bind(Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(&db.pool)
        .await?;
    if updated.rows_affected() == 0 {
        return Ok(None);
    }
    get_meta(db, session_id).await
}
