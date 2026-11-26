use std::collections::BTreeSet;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::db::Database;
use crate::kernel::{GameEventKind, GameEventRecord};

pub async fn next_event_seq(db: &Database, session_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM game_events WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_one(&db.pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn append(
    db: &Database,
    event_id: &str,
    session_id: &str,
    turn_id: &str,
    seq: i64,
    event: &GameEventKind,
    module_version: &str,
    caused_by: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO game_events(event_id,session_id,turn_id,seq,event_type,payload_json,module_version,caused_by,created_at) VALUES(?,?,?,?,?,?,?,?,?)",
    )
    .bind(event_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(seq)
    .bind(event_type(event))
    .bind(serde_json::to_string(event).unwrap_or_else(|_| "null".into()))
    .bind(module_version)
    .bind(caused_by)
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list(db: &Database, session_id: &str) -> Result<Vec<GameEventRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
        ),
    >(
        "SELECT event_id,session_id,turn_id,seq,event_type,payload_json,module_version,created_at,caused_by FROM game_events WHERE session_id=? ORDER BY seq ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(
            |(
                event_id,
                session_id,
                turn_id,
                seq,
                _event_type,
                payload,
                module_version,
                created_at,
                caused_by,
            )| {
                serde_json::from_str::<GameEventKind>(&payload)
                    .ok()
                    .map(|event| GameEventRecord {
                        event_id,
                        session_id,
                        turn_id,
                        seq: seq as u64,
                        event,
                        module_version,
                        created_at,
                        caused_by,
                    })
            },
        )
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRecord {
    pub entry_id: String,
    pub turn_id: String,
    pub scene_id: String,
    pub speaker_entity_id: Option<String>,
    pub present_entity_ids: Vec<String>,
    pub audible_entity_ids: Vec<String>,
    pub visible_entity_ids: Vec<String>,
    pub visibility_scope: String,
    pub claim_kind: String,
    pub referenced_event_ids: Vec<String>,
    pub content: String,
    pub created_at: String,
}

pub async fn insert_transcript(
    db: &Database,
    session_id: &str,
    record: &TranscriptRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO transcript_entries(entry_id,session_id,turn_id,scene_id,speaker_entity_id,present_entity_ids_json,audible_entity_ids_json,visible_entity_ids_json,visibility_scope,claim_kind,referenced_event_ids_json,content,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(entry_id) DO NOTHING",
    )
    .bind(&record.entry_id)
    .bind(session_id)
    .bind(&record.turn_id)
    .bind(&record.scene_id)
    .bind(&record.speaker_entity_id)
    .bind(serde_json::to_string(&record.present_entity_ids).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&record.audible_entity_ids).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&record.visible_entity_ids).unwrap_or_else(|_| "[]".into()))
    .bind(&record.visibility_scope)
    .bind(&record.claim_kind)
    .bind(serde_json::to_string(&record.referenced_event_ids).unwrap_or_else(|_| "[]".into()))
    .bind(&record.content)
    .bind(&record.created_at)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_transcripts(
    db: &Database,
    session_id: &str,
) -> Result<Vec<TranscriptRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ),
    >(
        "SELECT entry_id,turn_id,scene_id,speaker_entity_id,present_entity_ids_json,audible_entity_ids_json,visible_entity_ids_json,visibility_scope,claim_kind,referenced_event_ids_json,content,created_at FROM transcript_entries WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                entry_id,
                turn_id,
                scene_id,
                speaker_entity_id,
                present_json,
                audible_json,
                visible_json,
                visibility_scope,
                claim_kind,
                ref_events_json,
                content,
                created_at,
            )| TranscriptRecord {
                entry_id,
                turn_id,
                scene_id,
                speaker_entity_id,
                present_entity_ids: serde_json::from_str(&present_json).unwrap_or_default(),
                audible_entity_ids: serde_json::from_str(&audible_json).unwrap_or_default(),
                visible_entity_ids: serde_json::from_str(&visible_json).unwrap_or_default(),
                visibility_scope,
                claim_kind,
                referenced_event_ids: serde_json::from_str(&ref_events_json).unwrap_or_default(),
                content,
                created_at,
            },
        )
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OocMessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub async fn record_ooc(
    db: &Database,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ooc_messages(id,session_id,role,content,created_at) VALUES(?,?,?,?,?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_ooc(db: &Database, session_id: &str) -> Result<Vec<OocMessageRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id,role,content,created_at FROM ooc_messages WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, role, content, created_at)| OocMessageRow {
            id,
            role,
            content,
            created_at,
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn record_knowledge_acquisition(
    db: &Database,
    id: &str,
    session_id: &str,
    entity_id: &str,
    fact_id: &str,
    source_event_id: &str,
    acquisition_method: &str,
    acquired_at_seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO knowledge_acquisitions(id,session_id,entity_id,fact_id,source_event_id,acquisition_method,acquired_at_seq,created_at) VALUES(?,?,?,?,?,?,?,?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(entity_id)
    .bind(fact_id)
    .bind(source_event_id)
    .bind(acquisition_method)
    .bind(acquired_at_seq)
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_knowledge_facts(
    db: &Database,
    session_id: &str,
    entity_id: &str,
) -> Result<BTreeSet<String>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT fact_id FROM knowledge_acquisitions WHERE session_id=? AND entity_id=?",
    )
    .bind(session_id)
    .bind(entity_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn record_belief(
    db: &Database,
    id: &str,
    session_id: &str,
    entity_id: &str,
    source_transcript_id: Option<&str>,
    claim_summary: &str,
    status: &str,
    confidence: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO npc_beliefs(id,session_id,entity_id,source_transcript_id,claim_summary,belief_status,confidence,updated_at) VALUES(?,?,?,?,?,?,?,?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(entity_id)
    .bind(source_transcript_id)
    .bind(claim_summary)
    .bind(status)
    .bind(confidence)
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_beliefs(
    db: &Database,
    session_id: &str,
) -> Result<Vec<crate::narration::NpcBeliefView>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String, String, f64)>(
        "SELECT entity_id,source_transcript_id,claim_summary,belief_status,confidence FROM npc_beliefs WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(entity_id, source_transcript_id, claim_summary, belief_status, confidence)| {
                crate::narration::NpcBeliefView {
                    entity_id,
                    source_transcript_id,
                    claim_summary,
                    belief_status,
                    confidence,
                }
            },
        )
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectiveDto {
    pub schema_version: u16,
    pub directive_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub narration_generation: i64,
    pub source: String,
    pub directive_type: String,
    pub display_timing: String,
    pub displayed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
    pub payload: Value,
}

#[allow(clippy::too_many_arguments)]
pub async fn save_directive(
    db: &Database,
    directive_id: &str,
    session_id: &str,
    turn_id: &str,
    generation: i64,
    source: &str,
    directive_type: &str,
    timing: &str,
    payload: &Value,
    seq: Option<i64>,
) -> Result<DirectiveDto, sqlx::Error> {
    sqlx::query(
        "INSERT INTO ui_directives(directive_id,session_id,turn_id,narration_generation,source,directive_type,display_timing,payload_json,seq,created_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(directive_id) DO NOTHING",
    )
    .bind(directive_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(generation)
    .bind(source)
    .bind(directive_type)
    .bind(timing)
    .bind(payload.to_string())
    .bind(seq)
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await?;

    Ok(DirectiveDto {
        schema_version: 1,
        directive_id: directive_id.to_string(),
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        narration_generation: generation,
        source: source.to_string(),
        directive_type: directive_type.to_string(),
        display_timing: timing.to_string(),
        displayed: false,
        seq,
        payload: payload.clone(),
    })
}

pub async fn rebind_kernel_directives_generation(
    db: &Database,
    turn_id: &str,
    new_generation: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE ui_directives SET narration_generation=? WHERE turn_id=? AND source='kernel_owned' AND displayed_at IS NULL",
    )
    .bind(new_generation)
    .bind(turn_id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_directives_for_turn(
    db: &Database,
    turn_id: &str,
) -> Result<Vec<DirectiveDto>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
        ),
    >(
        "SELECT directive_id,session_id,turn_id,narration_generation,source,directive_type,display_timing,payload_json,displayed_at,seq FROM ui_directives WHERE turn_id=? ORDER BY COALESCE(seq, rowid) ASC",
    )
    .bind(turn_id)
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                directive_id,
                session_id,
                turn_id,
                narration_generation,
                source,
                directive_type,
                display_timing,
                payload_json,
                displayed_at,
                seq,
            )| DirectiveDto {
                schema_version: 1,
                directive_id,
                session_id,
                turn_id,
                narration_generation,
                source,
                directive_type,
                display_timing,
                displayed: displayed_at.is_some(),
                seq,
                payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
            },
        )
        .collect())
}

pub async fn list_directives_for_session(
    db: &Database,
    session_id: &str,
) -> Result<Vec<DirectiveDto>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
        ),
    >(
        "SELECT directive_id,session_id,turn_id,narration_generation,source,directive_type,display_timing,payload_json,displayed_at,seq FROM ui_directives WHERE session_id=? ORDER BY COALESCE(seq, rowid) ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                directive_id,
                session_id,
                turn_id,
                narration_generation,
                source,
                directive_type,
                display_timing,
                payload_json,
                displayed_at,
                seq,
            )| DirectiveDto {
                schema_version: 1,
                directive_id,
                session_id,
                turn_id,
                narration_generation,
                source,
                directive_type,
                display_timing,
                displayed: displayed_at.is_some(),
                seq,
                payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
            },
        )
        .collect())
}

pub async fn mark_directive_displayed(
    db: &Database,
    session_id: &str,
    directive_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE ui_directives SET displayed_at=? WHERE session_id=? AND directive_id=? AND displayed_at IS NULL",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(session_id)
    .bind(directive_id)
    .execute(&db.pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceNoteRow {
    pub note_id: String,
    pub note_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_clue_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_clue_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at: String,
}

pub async fn insert_evidence_note(
    db: &Database,
    session_id: &str,
    note: &EvidenceNoteRow,
) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_string(note).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO evidence_notes(note_id,session_id,note_kind,payload_json,created_at,updated_at) VALUES(?,?,?,?,?,?)",
    )
    .bind(&note.note_id)
    .bind(session_id)
    .bind(&note.note_kind)
    .bind(payload)
    .bind(&note.created_at)
    .bind(&note.created_at)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_evidence_notes(
    db: &Database,
    session_id: &str,
) -> Result<Vec<EvidenceNoteRow>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT payload_json FROM evidence_notes WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(session_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|raw| serde_json::from_str::<EvidenceNoteRow>(&raw).ok())
        .collect())
}

fn event_type(event: &GameEventKind) -> &'static str {
    match event {
        GameEventKind::SessionCreated { .. } => "SESSION_CREATED",
        GameEventKind::InvestigatorSelected { .. } => "INVESTIGATOR_SELECTED",
        GameEventKind::TurnStarted { .. } => "TURN_STARTED",
        GameEventKind::PlayerActionAccepted { .. } => "PLAYER_ACTION_ACCEPTED",
        GameEventKind::ActionProposed { .. } => "ACTION_PROPOSED",
        GameEventKind::RollRequested { .. } => "ROLL_REQUESTED",
        GameEventKind::RollResolved { .. } => "ROLL_RESOLVED",
        GameEventKind::LuckSpent { .. } => "LUCK_SPENT",
        GameEventKind::PushRollAccepted { .. } => "PUSH_ROLL_ACCEPTED",
        GameEventKind::ResourceChanged { .. } => "RESOURCE_CHANGED",
        GameEventKind::ItemAcquired { .. } => "ITEM_ACQUIRED",
        GameEventKind::ItemLost { .. } => "ITEM_LOST",
        GameEventKind::ClueUnlocked { .. } => "CLUE_UNLOCKED",
        GameEventKind::HandoutReleased { .. } => "HANDOUT_RELEASED",
        GameEventKind::ClockAdvanced { .. } => "CLOCK_ADVANCED",
        GameEventKind::SceneChanged { .. } => "SCENE_CHANGED",
        GameEventKind::WorldPatchApplied { .. } => "WORLD_PATCH_APPLIED",
        GameEventKind::InterruptCreated { .. } => "INTERRUPT_CREATED",
        GameEventKind::InterruptResolved { .. } => "INTERRUPT_RESOLVED",
        GameEventKind::TurnCommitted => "TURN_COMMITTED",
        GameEventKind::NarrationAccepted { .. } => "NARRATION_ACCEPTED",
        GameEventKind::NpcKnowledgeAcquired { .. } => "NPC_KNOWLEDGE_ACQUIRED",
        GameEventKind::NpcBeliefChanged { .. } => "NPC_BELIEF_CHANGED",
        GameEventKind::NpcRelationChanged { .. } => "NPC_RELATION_CHANGED",
        GameEventKind::EvidencePresented { .. } => "EVIDENCE_PRESENTED",
        GameEventKind::TopicConfronted { .. } => "TOPIC_CONFRONTED",
        GameEventKind::EventOccurred { .. } => "EVENT_OCCURRED",
        GameEventKind::FlagSet { .. } => "FLAG_SET",
        GameEventKind::ConditionAdded { .. } => "CONDITION_ADDED",
        GameEventKind::SessionEnded => "SESSION_ENDED",
    }
}
