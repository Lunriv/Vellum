use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, patch, post},
    Router,
};
use chrono::Utc;
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower_http::cors::CorsLayer;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

use crate::kernel::{GameEventKind, TurnStatus};
use crate::modules::{
    generator::{parse_and_sanitize_module_value, WorldGenRequest},
    ModuleIr,
};
use crate::persistence::{
    db::Database,
    event_repo::{self, EvidenceNoteRow},
    module_repo, session_repo, turn_repo,
};
use crate::runtime::{
    debrief,
    interrupt::{self, InterruptError, ResolveInterruptAction},
    orchestrator, recovery,
};
use crate::stream::buffer::SubscribeStep;
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
struct ErrorBody {
    error: ErrorInner,
}

#[derive(Debug, Serialize, ToSchema)]
struct ErrorInner {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateSessionRequest {
    module_id: String,
    investigator_id: String,
    experience_mode: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateExperienceModeRequest {
    experience_mode: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateTurnRequest {
    input: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct OocRequest {
    question: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateEvidenceNoteRequest {
    note_kind: String,
    title: Option<String>,
    content: Option<String>,
    from_clue_id: Option<String>,
    to_clue_id: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ImportModuleRequest {
    format: String,
    content: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct GenerateModuleRequest {
    prompt: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    tone: Option<String>,
    #[serde(default)]
    scene_count: Option<u8>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vellum AI Keeper API",
        version = "0.1.0",
        description = "Frozen V1 REST & SSE contract for Vellum single-player CoC 7e AI Keeper."
    ),
    components(schemas(
        ErrorBody,
        ErrorInner,
        CreateSessionRequest,
        UpdateExperienceModeRequest,
        CreateTurnRequest,
        OocRequest,
        CreateEvidenceNoteRequest
    ))
)]
struct VellumApiDoc;

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ErrorBody {
            error: ErrorInner {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }),
    )
        .into_response()
}

#[allow(clippy::result_large_err)]
fn require_idem_key(headers: &HeaderMap) -> Result<String, Response> {
    headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "MISSING_IDEMPOTENCY_KEY",
                "Idempotency-Key header is required",
            )
        })
}

fn request_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

#[allow(clippy::result_large_err)]
async fn check_idempotency(
    db: &Database,
    scope: &str,
    command: &str,
    key: &str,
    hash: &str,
) -> Result<Option<Response>, Response> {
    let row = sqlx::query_as::<_, (String, i64, String)>(
        "SELECT request_hash,status_code,response_json FROM idempotency_records WHERE session_scope=? AND command_type=? AND idempotency_key=?",
    )
    .bind(scope)
    .bind(command)
    .bind(key)
    .fetch_optional(&db.pool)
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            "Could not query idempotency record",
        )
    })?;

    if let Some((old_hash, status_code, response_json)) = row {
        if old_hash != hash {
            return Err(error(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_KEY_REUSED",
                "Idempotency-Key was already used with a different request payload",
            ));
        }
        let resp = Response::builder()
            .status(status_code as u16)
            .header("content-type", "application/json")
            .body(Body::from(response_json))
            .unwrap();
        return Ok(Some(resp));
    }

    Ok(None)
}

async fn save_idempotency(
    db: &Database,
    scope: &str,
    command: &str,
    key: &str,
    hash: &str,
    status: StatusCode,
    body: &Value,
) {
    let _ = sqlx::query(
        "INSERT INTO idempotency_records(session_scope,command_type,idempotency_key,request_hash,status_code,response_json,created_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT DO NOTHING",
    )
    .bind(scope)
    .bind(command)
    .bind(key)
    .bind(hash)
    .bind(status.as_u16() as i64)
    .bind(body.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(&db.pool)
    .await;
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/openapi.json", get(openapi))
        .route("/api/modules", get(list_modules))
        .route("/api/modules/generate", post(generate_module))
        .route("/api/modules/import", post(import_module))
        .route("/api/modules/{module_id}", delete(delete_module))
        .route(
            "/api/modules/{module_id}/investigators",
            get(list_investigators),
        )
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/{session_id}", get(get_session))
        .route("/api/sessions/{session_id}/state", get(get_state))
        .route("/api/sessions/{session_id}/mode", patch(change_mode))
        .route("/api/sessions/{session_id}/turns", post(create_turn))
        .route("/api/sessions/{session_id}/turns/{turn_id}", get(get_turn))
        .route(
            "/api/sessions/{session_id}/turns/{turn_id}/events",
            get(turn_events),
        )
        .route(
            "/api/sessions/{session_id}/interrupts/{interrupt_id}/resolve",
            post(resolve_interrupt_handler),
        )
        .route("/api/sessions/{session_id}/ooc", post(ooc_handler))
        .route("/api/sessions/{session_id}/evidence", get(get_evidence))
        .route(
            "/api/sessions/{session_id}/evidence/notes",
            post(create_evidence_note),
        )
        .route(
            "/api/sessions/{session_id}/directives/{directive_id}/ack",
            post(ack_directive),
        )
        .route("/api/sessions/{session_id}/debrief", get(get_debrief))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn openapi() -> Json<Value> {
    let mut doc = serde_json::to_value(VellumApiDoc::openapi()).unwrap_or_else(|_| json!({}));
    if let Some(obj) = doc.as_object_mut() {
        obj.insert(
            "x-action-proposal-schema".into(),
            serde_json::to_value(schemars::schema_for!(crate::kernel::ActionProposal))
                .unwrap_or(Value::Null),
        );
    }
    Json(doc)
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "llmMode": state.llm_mode,
        "database": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

fn investigators_dto(module: &ModuleIr) -> Vec<Value> {
    module
        .investigators
        .iter()
        .map(|inv| {
            json!({
                "investigatorId": inv.investigator_id,
                "name": inv.name,
                "occupation": inv.occupation,
                "tagline": inv.tagline,
                "biography": inv.biography,
                "motivation": inv.motivation,
                "manorConnection": inv.manor_connection,
                "hpMax": inv.hp_max,
                "mpMax": inv.mp_max,
                "sanStart": inv.san_start,
                "luckStart": inv.luck_start,
                "strengths": inv.strengths,
                "startingItems": inv.starting_items,
            })
        })
        .collect()
}

async fn list_modules(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "modules": state.modules.list_summaries(),
    }))
}

async fn generate_module(
    State(state): State<AppState>,
    Json(req): Json<GenerateModuleRequest>,
) -> Response {
    let trimmed = req.prompt.trim();
    if trimmed.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "请先输入世界设定、题材想法或剧本大纲",
        );
    }

    let gen_req = WorldGenRequest {
        prompt: trimmed.to_string(),
        title: req.title,
        tone: req.tone,
        scene_count: req.scene_count,
    };

    let mut module = match state.provider.generate_world(&gen_req).await {
        Ok(m) => m,
        Err(msg) => return error(StatusCode::UNPROCESSABLE_ENTITY, "WORLD_GEN_FAILED", &msg),
    };

    // Never overwrite an existing built-in module ID
    if state.modules.is_builtin(&module.module_id) {
        let suffix = &Uuid::new_v4().simple().to_string()[..6];
        module.module_id = format!("{}_{suffix}", module.module_id);
    }

    let summary =
        match state
            .modules
            .register(module.clone(), "generated", Some(trimmed.to_string()))
        {
            Ok(s) => s,
            Err(e) => {
                return error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "MODULE_VALIDATION_FAILED",
                    &e.to_string(),
                )
            }
        };

    if let Err(e) = module_repo::upsert(&state.db, &module, "generated", Some(trimmed)).await {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        );
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "module": summary,
            "investigators": investigators_dto(&module),
        })),
    )
        .into_response()
}

async fn import_module(
    State(state): State<AppState>,
    Json(req): Json<ImportModuleRequest>,
) -> Response {
    let raw = req.content.trim();
    if raw.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "导入内容不能为空",
        );
    }

    let format = req.format.trim().to_lowercase();
    let mut module = match format.as_str() {
        "json" => match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => match parse_and_sanitize_module_value(parsed, None) {
                Ok(valid) => valid,
                Err(e) => {
                    return error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MODULE_VALIDATION_FAILED",
                        &e,
                    )
                }
            },
            Err(e) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_MODULE_JSON",
                    &format!("JSON 格式不合法：{e}"),
                )
            }
        },
        "yaml" | "yml" => match serde_yaml::from_str::<Value>(raw) {
            Ok(parsed) => match parse_and_sanitize_module_value(parsed, None) {
                Ok(valid) => valid,
                Err(e) => {
                    return error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MODULE_VALIDATION_FAILED",
                        &e,
                    )
                }
            },
            Err(e) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_MODULE_YAML",
                    &format!("YAML 格式不合法：{e}"),
                )
            }
        },
        "text" => {
            let gen_req = WorldGenRequest {
                prompt: raw.to_string(),
                title: req.title.clone(),
                tone: None,
                scene_count: None,
            };
            match state.provider.generate_world(&gen_req).await {
                Ok(m) => m,
                Err(msg) => {
                    return error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "WORLD_COMPILE_FAILED",
                        &msg,
                    )
                }
            }
        }
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                "format 仅支持 json、yaml 或 text",
            )
        }
    };

    if let Some(title) = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        module.title = title.to_string();
    }
    if state.modules.is_builtin(&module.module_id) {
        let suffix = &Uuid::new_v4().simple().to_string()[..6];
        module.module_id = format!("{}_{suffix}", module.module_id);
    }

    let summary = match state.modules.register(module.clone(), "imported", None) {
        Ok(s) => s,
        Err(e) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MODULE_VALIDATION_FAILED",
                &e.to_string(),
            )
        }
    };

    if let Err(e) = module_repo::upsert(&state.db, &module, "imported", None).await {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        );
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "module": summary,
            "investigators": investigators_dto(&module),
        })),
    )
        .into_response()
}

async fn delete_module(State(state): State<AppState>, Path(module_id): Path<String>) -> Response {
    if state.modules.is_builtin(&module_id) {
        return error(
            StatusCode::FORBIDDEN,
            "BUILTIN_MODULE_PROTECTED",
            "内置演示模组不可删除",
        );
    }
    match state.modules.remove_custom(&module_id) {
        Ok(true) => {
            let _ = module_repo::delete(&state.db, &module_id).await;
            Json(json!({ "ok": true, "moduleId": module_id })).into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "NOT_FOUND", "Module not found"),
        Err(msg) => error(StatusCode::FORBIDDEN, "FORBIDDEN", &msg),
    }
}

async fn list_investigators(
    State(state): State<AppState>,
    Path(module_id): Path<String>,
) -> Response {
    let Some(module) = state.modules.get(&module_id) else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Module not found");
    };

    Json(json!({
        "moduleId": module.module_id,
        "moduleTitle": module.title,
        "openingPrologue": module.opening_prologue,
        "investigators": investigators_dto(&module),
    }))
    .into_response()
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSessionRequest>,
) -> Response {
    let key = match require_idem_key(&headers) {
        Ok(k) => k,
        Err(resp) => return resp,
    };
    let hash = request_hash(&req);

    // Protect global session-creation idempotency with short lock
    let global_lock = state.locks.lock_for("__create_session__").await;
    let guard = global_lock.lock().await;

    match check_idempotency(&state.db, "global", "create_session", &key, &hash).await {
        Ok(Some(resp)) => return resp,
        Err(resp) => return resp,
        Ok(None) => {}
    }

    if !matches!(
        req.experience_mode.as_str(),
        "guided" | "classic" | "immersive"
    ) {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "experienceMode must be guided, classic, or immersive",
        );
    }

    let Some(module) = state.modules.get(&req.module_id) else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Module not found");
    };

    let session_id = Uuid::new_v4().to_string();
    let Some(initial_state) =
        orchestrator::build_initial_session_state(&session_id, &module, &req.investigator_id)
    else {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "investigatorId does not belong to the requested module",
        );
    };

    let turn0_id = Uuid::new_v4().to_string();
    let meta = match session_repo::create(
        &state.db,
        &session_id,
        &module.module_id,
        &module.version,
        &req.investigator_id,
        &req.experience_mode,
        Some(&turn0_id),
        &initial_state,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                &e.to_string(),
            )
        }
    };

    if let Err(e) = turn_repo::create(
        &state.db,
        &turn0_id,
        &session_id,
        0,
        "B",
        None,
        TurnStatus::Received,
    )
    .await
    {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        );
    }

    // Persist Turn 0 authoritative creation events
    let ev1_id = Uuid::new_v4().to_string();
    let _ = event_repo::append(
        &state.db,
        &ev1_id,
        &session_id,
        &turn0_id,
        1,
        &GameEventKind::SessionCreated {
            event_id: ev1_id.clone(),
            session_id: session_id.clone(),
            seq: 1,
            turn_id: turn0_id.clone(),
        },
        &module.version,
        None,
    )
    .await;
    let _ = event_repo::append(
        &state.db,
        &Uuid::new_v4().to_string(),
        &session_id,
        &turn0_id,
        2,
        &GameEventKind::InvestigatorSelected {
            investigator: initial_state.investigator.clone(),
        },
        &module.version,
        Some(&ev1_id),
    )
    .await;

    let body = json!({
        "session": meta,
        "turn0Id": turn0_id,
        "streamUrl": format!("/api/sessions/{session_id}/turns/{turn0_id}/events"),
    });
    save_idempotency(
        &state.db,
        "global",
        "create_session",
        &key,
        &hash,
        StatusCode::CREATED,
        &body,
    )
    .await;

    let _ = state.locks.try_register_active(&session_id).await;
    drop(guard);

    // Launch Turn 0 async opening outside short lock & DB transaction
    let state_clone = state.clone();
    let sid = session_id.clone();
    let t0 = turn0_id.clone();
    tokio::spawn(async move {
        orchestrator::run_turn0(state_clone, sid, t0).await;
    });

    (StatusCode::CREATED, Json(body)).into_response()
}

async fn get_session(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    match session_repo::get_meta(&state.db, &session_id).await {
        Ok(Some(meta)) => Json(meta).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found"),
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        ),
    }
}

async fn get_state(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    let Ok(Some(meta)) = session_repo::get_meta(&state.db, &session_id).await else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    };
    let Ok(Some((_, session_state))) = session_repo::load_state(&state.db, &session_id).await
    else {
        return error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Session snapshot not found",
        );
    };
    let module = state.modules.get(&meta.module_id);
    let turns = turn_repo::list_for_session(&state.db, &session_id)
        .await
        .unwrap_or_default();
    let transcripts = event_repo::list_transcripts(&state.db, &session_id)
        .await
        .unwrap_or_default();
    let all_directives = event_repo::list_directives_for_session(&state.db, &session_id)
        .await
        .unwrap_or_default();

    let mut dirs_by_turn: HashMap<String, Vec<event_repo::DirectiveDto>> = HashMap::new();
    for d in all_directives {
        dirs_by_turn.entry(d.turn_id.clone()).or_default().push(d);
    }

    let mut narration_by_turn: HashMap<String, event_repo::TranscriptRecord> = HashMap::new();
    for tr in transcripts {
        if tr.claim_kind == "narration" {
            narration_by_turn.insert(tr.turn_id.clone(), tr);
        }
    }

    let mut transcript_items = Vec::new();
    for t in &turns {
        if matches!(t.status, TurnStatus::Completed) {
            if let Some(tr) = narration_by_turn.get(&t.id) {
                transcript_items.push(json!({
                    "entryId": tr.entry_id,
                    "turnId": t.id,
                    "turnIndex": t.turn_index,
                    "sceneId": tr.scene_id,
                    "playerInput": t.player_input,
                    "narration": tr.content,
                    "narrationGeneration": t.narration_generation,
                    "directives": dirs_by_turn.remove(&t.id).unwrap_or_default(),
                    "createdAt": tr.created_at,
                }));
            }
        }
    }

    let active_interrupt = turn_repo::get_active_interrupt(&state.db, &session_id)
        .await
        .ok()
        .flatten();

    let world_tags: Vec<&String> = session_state.events.iter().collect();
    Json(json!({
        "session": meta,
        "moduleTitle": module.as_ref().map(|m| m.title.as_str()).unwrap_or(&meta.module_id),
        "investigator": session_state.investigator,
        "currentScene": orchestrator::visible_scene_dto(&session_state, module.as_ref()),
        "mapRooms": orchestrator::map_rooms_dto(&session_state, module.as_ref()),
        "mapEdges": orchestrator::map_edges_dto(&session_state, module.as_ref()),
        "worldTags": world_tags,
        "visibleClocks": orchestrator::visible_clocks_dto(&session_state),
        "clues": session_state.clues.values().cloned().collect::<Vec<_>>(),
        "handouts": session_state.handouts.values().cloned().collect::<Vec<_>>(),
        "pendingInterrupt": active_interrupt,
        "transcript": transcript_items,
        "guidedPrompts": orchestrator::guided_prompts_for_scene(&session_state, module.as_ref()),
    }))
    .into_response()
}

async fn change_mode(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<UpdateExperienceModeRequest>,
) -> Response {
    if !matches!(
        req.experience_mode.as_str(),
        "guided" | "classic" | "immersive"
    ) {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "Invalid experienceMode",
        );
    }
    match session_repo::update_experience_mode(&state.db, &session_id, &req.experience_mode).await {
        Ok(Some(meta)) => Json(meta).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found"),
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        ),
    }
}

async fn create_turn(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<CreateTurnRequest>,
) -> Response {
    let key = match require_idem_key(&headers) {
        Ok(k) => k,
        Err(resp) => return resp,
    };
    let trimmed = req.input.trim();
    if trimmed.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "Action input cannot be empty",
        );
    }
    let hash = request_hash(&req);

    // Millisecond-only short critical section; NEVER held across LLM or SSE!
    let lock = state.locks.lock_for(&session_id).await;
    let guard = lock.lock().await;

    match check_idempotency(&state.db, &session_id, "create_turn", &key, &hash).await {
        Ok(Some(resp)) => return resp,
        Err(resp) => return resp,
        Ok(None) => {}
    }

    let Ok(Some(meta)) = session_repo::get_meta(&state.db, &session_id).await else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    };
    if meta.status == "ended" {
        return error(
            StatusCode::CONFLICT,
            "SESSION_ENDED",
            "Session has already ended",
        );
    }

    // Reject immediately if there is an unresolved PendingInterrupt (even AwaitingRoll!)
    if let Ok(Some(pending)) = turn_repo::get_active_interrupt(&state.db, &session_id).await {
        if interrupt::pending_is_active(&pending) {
            return error(
                StatusCode::CONFLICT,
                "INTERRUPT_PENDING",
                "Resolve the pending interrupt before submitting a new turn",
            );
        }
    }

    // Reject immediately if DB records a non-terminal active turn (e.g. NarrationPending after restart)
    if let Some(active_tid) = meta.active_turn_id.as_deref() {
        if let Ok(Some(active_turn)) = turn_repo::get(&state.db, active_tid).await {
            if !matches!(
                active_turn.status,
                TurnStatus::Completed
                    | TurnStatus::Cancelled
                    | TurnStatus::Failed
                    | TurnStatus::InterruptedBeforeCommit
            ) {
                return error(
                    StatusCode::CONFLICT,
                    "TURN_ALREADY_ACTIVE",
                    "An active or recovering turn must finish before submitting a new turn",
                );
            }
        }
    }

    // Reject immediately if an active IC turn is already running (never queue behind LLM!)
    if !state.locks.try_register_active(&session_id).await {
        return error(
            StatusCode::CONFLICT,
            "TURN_ALREADY_ACTIVE",
            "Another turn is currently active for this session",
        );
    }

    let turn_id = Uuid::new_v4().to_string();
    let index = turn_repo::next_turn_index(&state.db, &session_id)
        .await
        .unwrap_or(1);

    if let Err(e) = turn_repo::create(
        &state.db,
        &turn_id,
        &session_id,
        index,
        "B",
        Some(trimmed),
        TurnStatus::Received,
    )
    .await
    {
        state.locks.clear_active(&session_id).await;
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        );
    }
    let _ = session_repo::set_active_turn(&state.db, &session_id, Some(&turn_id)).await;

    let body = json!({
        "turnId": turn_id,
        "turnIndex": index,
        "status": "received",
        "streamUrl": format!("/api/sessions/{session_id}/turns/{turn_id}/events"),
    });
    save_idempotency(
        &state.db,
        &session_id,
        "create_turn",
        &key,
        &hash,
        StatusCode::ACCEPTED,
        &body,
    )
    .await;

    // Release short lock BEFORE spawning async turn orchestrator
    drop(guard);

    let state_clone = state.clone();
    let sid = session_id.clone();
    let tid = turn_id.clone();
    let input_owned = trimmed.to_string();
    tokio::spawn(async move {
        orchestrator::run_player_turn(state_clone, sid, tid, input_owned).await;
    });

    (StatusCode::ACCEPTED, Json(body)).into_response()
}

async fn get_turn(
    State(state): State<AppState>,
    Path((session_id, turn_id)): Path<(String, String)>,
) -> Response {
    let Ok(Some(turn)) = turn_repo::get(&state.db, &turn_id).await else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Turn not found");
    };
    if turn.session_id != session_id {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Turn not found");
    }

    let transcripts = event_repo::list_transcripts(&state.db, &session_id)
        .await
        .unwrap_or_default();
    let narration_text = transcripts
        .into_iter()
        .find(|t| t.turn_id == turn_id && t.claim_kind == "narration")
        .map(|t| t.content)
        .unwrap_or_default();
    let directives = event_repo::list_directives_for_turn(&state.db, &turn_id)
        .await
        .unwrap_or_default();
    let pending_interrupt = turn_repo::get_active_interrupt(&state.db, &session_id)
        .await
        .ok()
        .flatten()
        .filter(|p| p.turn_id == turn_id);

    Json(json!({
        "turnId": turn.id,
        "sessionId": turn.session_id,
        "turnIndex": turn.turn_index,
        "status": turn.status,
        "playerInput": turn.player_input,
        "narrationGeneration": turn.narration_generation,
        "narrationText": narration_text,
        "pendingInterrupt": pending_interrupt,
        "directives": directives,
    }))
    .into_response()
}

async fn turn_events(
    State(state): State<AppState>,
    Path((session_id, turn_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Ok(Some(turn)) = turn_repo::get(&state.db, &turn_id).await else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Turn not found");
    };
    if turn.session_id != session_id {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Turn not found");
    }

    let last_event_id: u64 = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    // Lazy Narration Recovery: triggered ONLY when turn is NarrationPending or expired Recovering
    if matches!(
        turn.status,
        TurnStatus::NarrationPending | TurnStatus::Recovering
    ) {
        let owner_id = format!("sse-{}", Uuid::new_v4());
        let ttl = Duration::from_secs(30);
        if let Ok(Some(lease)) =
            recovery::try_acquire_lease(&state.db, &turn_id, &owner_id, ttl).await
        {
            let state_clone = state.clone();
            let sid = session_id.clone();
            let tid = turn_id.clone();
            tokio::spawn(async move {
                recovery::run_narration_recovery(state_clone, sid, tid, lease, ttl).await;
            });
        }
    }

    // If turn already completed and no live buffer exists (e.g. after restart / TTL eviction),
    // replay directly from persisted Transcript + Directives (NEVER re-invoke LLM!)
    if matches!(turn.status, TurnStatus::Completed) && state.buffers.get(&turn_id).is_none() {
        let transcripts = event_repo::list_transcripts(&state.db, &session_id)
            .await
            .unwrap_or_default();
        let text = transcripts
            .into_iter()
            .find(|t| t.turn_id == turn_id && t.claim_kind == "narration")
            .map(|t| t.content)
            .unwrap_or_default();
        let directives = event_repo::list_directives_for_turn(&state.db, &turn_id)
            .await
            .unwrap_or_default();
        let gen_id = turn.narration_generation;
        let tid = turn_id.clone();

        let replay_stream = async_stream_from_persisted(tid, gen_id, text, directives);
        return Sse::new(replay_stream)
            .keep_alive(KeepAlive::default())
            .into_response();
    }

    let buffer = state.buffers.get_or_create(&turn_id);
    let stream = live_buffer_sse_stream(buffer, last_event_id);
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn async_stream_from_persisted(
    turn_id: String,
    generation_id: i64,
    narration: String,
    directives: Vec<event_repo::DirectiveDto>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let mut frames: Vec<(u64, &'static str, Value)> = Vec::new();
    let mut seq = 1u64;
    for d in directives {
        frames.push((
            seq,
            "ui_directive",
            json!({ "type": "ui_directive", "directive": d }),
        ));
        seq += 1;
    }
    if !narration.is_empty() {
        frames.push((
            seq,
            "narration_delta",
            json!({
                "type": "narration_delta",
                "chunk": narration,
                "turnId": turn_id,
                "generationId": generation_id,
            }),
        ));
        seq += 1;
    }
    frames.push((
        seq,
        "narration_complete",
        json!({
            "type": "narration_complete",
            "turnId": turn_id,
            "generationId": generation_id,
        }),
    ));
    seq += 1;
    frames.push((
        seq,
        "turn_complete",
        json!({ "type": "turn_complete", "turnId": turn_id }),
    ));

    futures_util::stream::iter(frames.into_iter().map(|(id, ev_type, payload)| {
        Ok(Event::default()
            .id(id.to_string())
            .event(ev_type)
            .json_data(payload)
            .unwrap())
    }))
}

fn live_buffer_sse_stream(
    buffer: std::sync::Arc<crate::stream::buffer::ActiveTurnBuffer>,
    initial_cursor: u64,
) -> impl Stream<Item = Result<Event, Infallible>> {
    struct StateHolder {
        buffer: std::sync::Arc<crate::stream::buffer::ActiveTurnBuffer>,
        cursor: u64,
        pending: std::collections::VecDeque<Event>,
        done: bool,
    }

    futures_util::stream::unfold(
        StateHolder {
            buffer,
            cursor: initial_cursor,
            pending: std::collections::VecDeque::new(),
            done: false,
        },
        |mut st| async move {
            loop {
                if let Some(ev) = st.pending.pop_front() {
                    return Some((Ok(ev), st));
                }
                if st.done {
                    return None;
                }
                match st.buffer.next_batch(st.cursor).await {
                    SubscribeStep::StreamReset {
                        seq,
                        payload,
                        terminal,
                    } => {
                        st.cursor = seq;
                        if terminal {
                            st.done = true;
                        }
                        let ev = Event::default()
                            .id(seq.to_string())
                            .event("stream_reset")
                            .json_data(payload)
                            .unwrap();
                        st.pending.push_back(ev);
                    }
                    SubscribeStep::Events { events, terminal } => {
                        for item in events {
                            st.cursor = st.cursor.max(item.seq);
                            let ev = Event::default()
                                .id(item.seq.to_string())
                                .event(item.event_type)
                                .json_data(item.payload)
                                .unwrap();
                            st.pending.push_back(ev);
                        }
                        if terminal {
                            st.done = true;
                        }
                        if st.pending.is_empty() && st.done {
                            return None;
                        }
                    }
                }
            }
        },
    )
}

async fn resolve_interrupt_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, interrupt_id)): Path<(String, String)>,
    Json(action): Json<ResolveInterruptAction>,
) -> Response {
    let key = match require_idem_key(&headers) {
        Ok(k) => k,
        Err(resp) => return resp,
    };
    let hash = request_hash(&action);
    let command = format!("resolve_interrupt:{interrupt_id}");

    // Hold short per-session command lock across idempotency check -> state transition -> idempotency save
    let lock = state.locks.lock_for(&session_id).await;
    let _guard = lock.lock().await;

    match check_idempotency(&state.db, &session_id, &command, &key, &hash).await {
        Ok(Some(resp)) => return resp,
        Err(resp) => return resp,
        Ok(None) => {}
    }

    match interrupt::resolve_interrupt_locked(state.clone(), &session_id, &interrupt_id, action)
        .await
    {
        Ok(res) => {
            let body = serde_json::to_value(&res).unwrap_or(Value::Null);
            save_idempotency(
                &state.db,
                &session_id,
                &command,
                &key,
                &hash,
                StatusCode::OK,
                &body,
            )
            .await;
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(InterruptError::NotFound) => {
            error(StatusCode::NOT_FOUND, "NOT_FOUND", "Interrupt not found")
        }
        Err(InterruptError::AlreadyResolved) => error(
            StatusCode::CONFLICT,
            "INTERRUPT_ALREADY_RESOLVED",
            "Interrupt has already been resolved",
        ),
        Err(InterruptError::Forbidden(code, msg)) => error(StatusCode::FORBIDDEN, code, &msg),
        Err(InterruptError::Conflict(code, msg)) => error(StatusCode::CONFLICT, code, &msg),
        Err(InterruptError::Internal(msg)) => {
            error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &msg)
        }
    }
}

async fn ooc_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<OocRequest>,
) -> Response {
    let question = req.question.trim();
    if question.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "OOC question cannot be empty",
        );
    }

    // Read-only parallel query: works even during PendingInterrupt! Never mutates authoritative state.
    let Ok(Some((_, session_state))) = session_repo::load_state(&state.db, &session_id).await
    else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    };

    let module = state.modules.get(&session_state.module_id);
    let recent_history = orchestrator::load_recent_history(&state.db, &session_id, 4).await;
    let answer = match state
        .provider
        .answer_ooc(question, &session_state, module.as_ref(), &recent_history)
        .await
    {
        Ok(a) => a,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, "OOC_PROVIDER_ERROR", &e),
    };

    let _ = event_repo::record_ooc(
        &state.db,
        &Uuid::new_v4().to_string(),
        &session_id,
        "player",
        question,
    )
    .await;
    let _ = event_repo::record_ooc(
        &state.db,
        &Uuid::new_v4().to_string(),
        &session_id,
        "assistant",
        &answer,
    )
    .await;

    let history = event_repo::list_ooc(&state.db, &session_id)
        .await
        .unwrap_or_default();
    Json(json!({
        "answer": answer,
        "history": history,
    }))
    .into_response()
}

async fn get_evidence(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    let Ok(Some((_, session_state))) = session_repo::load_state(&state.db, &session_id).await
    else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    };
    let notes = event_repo::list_evidence_notes(&state.db, &session_id)
        .await
        .unwrap_or_default();

    Json(json!({
        "clues": session_state.clues.values().cloned().collect::<Vec<_>>(),
        "handouts": session_state.handouts.values().cloned().collect::<Vec<_>>(),
        "notes": notes,
    }))
    .into_response()
}

async fn create_evidence_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(req): Json<CreateEvidenceNoteRequest>,
) -> Response {
    let key = match require_idem_key(&headers) {
        Ok(k) => k,
        Err(resp) => return resp,
    };
    let hash = request_hash(&req);

    let lock = state.locks.lock_for(&session_id).await;
    let _guard = lock.lock().await;

    match check_idempotency(&state.db, &session_id, "create_evidence_note", &key, &hash).await {
        Ok(Some(resp)) => return resp,
        Err(resp) => return resp,
        Ok(None) => {}
    }

    if session_repo::get_meta(&state.db, &session_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    }

    let note = EvidenceNoteRow {
        note_id: Uuid::new_v4().to_string(),
        note_kind: req.note_kind,
        title: req.title,
        content: req.content,
        from_clue_id: req.from_clue_id,
        to_clue_id: req.to_clue_id,
        label: req.label,
        created_at: Utc::now().to_rfc3339(),
    };

    if let Err(e) = event_repo::insert_evidence_note(&state.db, &session_id, &note).await {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        );
    }

    let body = serde_json::to_value(&note).unwrap_or(Value::Null);
    save_idempotency(
        &state.db,
        &session_id,
        "create_evidence_note",
        &key,
        &hash,
        StatusCode::CREATED,
        &body,
    )
    .await;

    (StatusCode::CREATED, Json(body)).into_response()
}

async fn ack_directive(
    State(state): State<AppState>,
    Path((session_id, directive_id)): Path<(String, String)>,
) -> Response {
    let _ = event_repo::mark_directive_displayed(&state.db, &session_id, &directive_id).await;
    Json(json!({ "ok": true })).into_response()
}

async fn get_debrief(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    let Ok(Some(meta)) = session_repo::get_meta(&state.db, &session_id).await else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Session not found");
    };
    if meta.status != "ended" {
        return error(
            StatusCode::FORBIDDEN,
            "SESSION_NOT_ENDED",
            "Case Debrief is only available after the session has ended",
        );
    }
    let Ok(Some((_, session_state))) = session_repo::load_state(&state.db, &session_id).await
    else {
        return error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Session snapshot not found",
        );
    };
    let Some(module) = state.modules.get(&meta.module_id) else {
        return error(StatusCode::NOT_FOUND, "NOT_FOUND", "Module not found");
    };
    let events = event_repo::list(&state.db, &session_id)
        .await
        .unwrap_or_default();

    let dto = debrief::build_case_debrief(&session_id, &session_state, &events, &module);
    Json(dto).into_response()
}
