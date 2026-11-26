use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use vellum_server::{
    build_app,
    kernel::{DiceRng, TurnStatus},
    models::MockProvider,
    modules::ModuleRegistry,
    persistence::{db::Database, event_repo, turn_repo},
    runtime::recovery,
    stream::buffer::{ActiveTurnBuffer, BufferLimits, BufferRegistry, SubscribeStep},
    AppState,
};

struct SequenceRng(std::collections::VecDeque<u8>);
impl SequenceRng {
    fn new(digits: &[u8]) -> Self {
        Self(digits.iter().copied().collect())
    }
}
impl DiceRng for SequenceRng {
    fn roll_d10(&mut self) -> u8 {
        self.0.pop_front().unwrap_or(5)
    }
}

async fn test_app(
    delay: Duration,
    rng: Option<Box<dyn DiceRng>>,
) -> (TempDir, AppState, axum::Router) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("vellum_test.sqlite");
    let url = format!("sqlite://{}", db_path.display());
    let db = Database::connect(&url).await.expect("connect sqlite");
    let modules = Arc::new(ModuleRegistry::load_default().expect("load modules"));
    let provider = Arc::new(MockProvider::with_delay(delay));
    let mut state = AppState::new(db, provider, modules, "mock");
    if let Some(r) = rng {
        state = state.with_rng(r);
    }
    let router = build_app(state.clone());
    (dir, state, router)
}

async fn json_request(
    app: &axum::Router,
    method: Method,
    uri: &str,
    idem_key: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(k) = idem_key {
        builder = builder.header("Idempotency-Key", k);
    }
    let req = if let Some(b) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let val = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, val)
}

async fn wait_turn_finished(state: &AppState, turn_id: &str) {
    for _ in 0..80 {
        if let Ok(Some(st)) = turn_repo::status(&state.db, turn_id).await {
            if matches!(
                st,
                TurnStatus::Completed
                    | TurnStatus::AwaitingRoll
                    | TurnStatus::AwaitingRecovery
                    | TurnStatus::Cancelled
                    | TurnStatus::Failed
            ) {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn list_investigators_turn0_idempotency_and_hidden_clock_projection() {
    let (_dir, state, app) = test_app(Duration::ZERO, None).await;

    let (st, inv_body) = json_request(
        &app,
        Method::GET,
        "/api/modules/crimson_rain_manor/investigators",
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(inv_body["investigators"].as_array().unwrap().len(), 3);

    // Missing Idempotency-Key -> 400
    let (st, err) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        None,
        Some(json!({
            "moduleId": "crimson_rain_manor",
            "investigatorId": "investigator_reporter",
            "experienceMode": "classic"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"]["code"], "MISSING_IDEMPOTENCY_KEY");

    // Valid session creation
    let payload = json!({
        "moduleId": "crimson_rain_manor",
        "investigatorId": "investigator_reporter",
        "experienceMode": "classic"
    });
    let (st1, res1) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("idem-create-1"),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED);
    let sid = res1["session"]["sessionId"].as_str().unwrap().to_string();
    let turn0_id = res1["turn0Id"].as_str().unwrap().to_string();

    // Same key + same body -> identical replay
    let (st2, res2) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("idem-create-1"),
        Some(payload),
    )
    .await;
    assert_eq!(st2, StatusCode::CREATED);
    assert_eq!(res1["session"]["sessionId"], res2["session"]["sessionId"]);
    assert_eq!(res1["turn0Id"], res2["turn0Id"]);

    // Same key + different body -> 409 IDEMPOTENCY_KEY_REUSED
    let (st3, err3) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("idem-create-1"),
        Some(json!({
            "moduleId": "crimson_rain_manor",
            "investigatorId": "investigator_doctor",
            "experienceMode": "guided"
        })),
    )
    .await;
    assert_eq!(st3, StatusCode::CONFLICT);
    assert_eq!(err3["error"]["code"], "IDEMPOTENCY_KEY_REUSED");

    wait_turn_finished(&state, &turn0_id).await;

    // Client state MUST expose visible clock only, NEVER keeper_hidden `ritual_countdown`
    let (st_state, state_body) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    assert_eq!(st_state, StatusCode::OK);
    let clocks = state_body["visibleClocks"].as_array().unwrap();
    assert_eq!(clocks.len(), 1);
    assert_eq!(clocks[0]["clockId"], "manor_anomaly");
    assert!(!state_body.to_string().contains("ritual_countdown"));
    assert!(!state_body.to_string().contains("keeperOnly"));
    assert_eq!(state_body["handouts"].as_array().unwrap().len(), 1);
    assert_eq!(state_body["transcript"].as_array().unwrap().len(), 1);

    // Debrief MUST return 403 before session ends
    let (st_deb, err_deb) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/debrief"),
        None,
        None,
    )
    .await;
    assert_eq!(st_deb, StatusCode::FORBIDDEN);
    assert_eq!(err_deb["error"]["code"], "SESSION_NOT_ENDED");
}

#[tokio::test]
async fn double_click_turns_returns_fast_409_during_slow_llm() {
    let (_dir, state, app) = test_app(Duration::from_millis(350), None).await;

    let (_, res) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("s-slow"),
        Some(json!({
            "moduleId": "crimson_rain_manor",
            "investigatorId": "investigator_reporter",
            "experienceMode": "classic"
        })),
    )
    .await;
    let sid = res["session"]["sessionId"].as_str().unwrap();
    let t0 = res["turn0Id"].as_str().unwrap();
    wait_turn_finished(&state, t0).await;

    let (st1, t1) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("turn-key-1"),
        Some(json!({ "input": "前往书房" })),
    )
    .await;
    assert_eq!(st1, StatusCode::ACCEPTED);

    // Second request with a distinct key while slow LLM is running MUST fail fast with 409
    let started = Instant::now();
    let (st2, err2) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("turn-key-2"),
        Some(json!({ "input": "前往女仆偏厅" })),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(st2, StatusCode::CONFLICT);
    assert_eq!(err2["error"]["code"], "TURN_ALREADY_ACTIVE");
    assert!(
        elapsed < Duration::from_millis(200),
        "409 took {:?}, must not queue behind 350ms LLM",
        elapsed
    );

    wait_turn_finished(&state, t1["turnId"].as_str().unwrap()).await;
}

#[tokio::test]
async fn interrupt_recovery_lock_ooc_luck_spend_and_push_flow() {
    // Reporter psychology = 65 -> Hard threshold = 32.
    // SequenceRng digits:
    // Roll 1 (units=4, tens=3 -> 34): fails Hard (34 > 32), needs 2 Luck -> AwaitingRecovery!
    // Roll 2 (units=8, tens=7 -> 78): fails Hard on Push attempt 1 -> AwaitingRecovery!
    // Roll 3 (units=9, tens=8 -> 89): fails Push attempt 2 -> direct commit, NO 2nd recovery!
    let rng = Box::new(SequenceRng::new(&[4, 3, 8, 7, 9, 8]));
    let (_dir, state, app) = test_app(Duration::ZERO, Some(rng)).await;

    let (_, res) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("s-int"),
        Some(json!({
            "moduleId": "crimson_rain_manor",
            "investigatorId": "investigator_reporter",
            "experienceMode": "classic"
        })),
    )
    .await;
    let sid = res["session"]["sessionId"].as_str().unwrap().to_string();
    wait_turn_finished(&state, res["turn0Id"].as_str().unwrap()).await;

    // 1. Trigger Hard Psychology check
    let (st, t1) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("k-check-1"),
        Some(json!({ "input": "审视管家微表情" })),
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let t1_id = t1["turnId"].as_str().unwrap().to_string();
    wait_turn_finished(&state, &t1_id).await;

    let (_, st_body) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    let int1_id = st_body["pendingInterrupt"]["interruptId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(st_body["pendingInterrupt"]["status"], "awaiting_roll");

    // 2. Confirm roll -> rolls 34 vs Hard 32 -> AwaitingRecovery
    let (st_c, c_body) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int1_id}/resolve"),
        Some("k-confirm-1"),
        Some(json!({ "action": "confirm_roll" })),
    )
    .await;
    assert_eq!(st_c, StatusCode::OK);
    assert_eq!(c_body["status"], "awaiting_recovery");
    assert_eq!(c_body["roll"]["rollValue"], 34);
    assert_eq!(c_body["pendingInterrupt"]["recovery"]["canSpendLuck"], true);
    assert_eq!(c_body["pendingInterrupt"]["recovery"]["luckCostNeeded"], 2);

    // 3. During awaiting_recovery: new POST /turns -> 409 INTERRUPT_PENDING
    let (st_blk, err_blk) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("k-blocked"),
        Some(json!({ "input": "前往书房" })),
    )
    .await;
    assert_eq!(st_blk, StatusCode::CONFLICT);
    assert_eq!(err_blk["error"]["code"], "INTERRUPT_PENDING");

    // 4. During awaiting_recovery: cancel MUST be forbidden (no save-scumming!)
    let (st_can, _) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int1_id}/resolve"),
        Some("k-no-cancel"),
        Some(json!({ "action": "cancel" })),
    )
    .await;
    assert_eq!(st_can, StatusCode::FORBIDDEN);

    // 5. OOC MUST work in parallel while locked in awaiting_recovery
    let (st_ooc, ooc_body) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/ooc"),
        None,
        Some(json!({ "question": "我目前还剩多少幸运和理智？" })),
    )
    .await;
    assert_eq!(st_ooc, StatusCode::OK);
    assert!(ooc_body["answer"].as_str().unwrap().contains("50"));

    // 6. Spend Luck (2 points: 50 -> 48) and verify idempotent replay doesn't double-deduct!
    let (st_luck, luck_body) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int1_id}/resolve"),
        Some("k-luck-1"),
        Some(json!({ "action": "spend_luck" })),
    )
    .await;
    assert_eq!(st_luck, StatusCode::OK);
    assert_eq!(luck_body["status"], "committing");
    let (st_luck_dup, _) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int1_id}/resolve"),
        Some("k-luck-1"),
        Some(json!({ "action": "spend_luck" })),
    )
    .await;
    assert_eq!(st_luck_dup, StatusCode::OK);
    wait_turn_finished(&state, &t1_id).await;

    let (_, after_luck) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    assert_eq!(after_luck["investigator"]["luck"], 48);

    // 7. Full Push the Roll flow (fails roll 2 = 78 -> Push -> fails roll 3 = 89 -> direct commit)
    let (_, t2) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("k-check-2"),
        Some(json!({ "input": "审视管家微表情" })),
    )
    .await;
    let t2_id = t2["turnId"].as_str().unwrap().to_string();
    wait_turn_finished(&state, &t2_id).await;

    let (_, st2) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    let int2_id = st2["pendingInterrupt"]["interruptId"]
        .as_str()
        .unwrap()
        .to_string();

    let (_, r2) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int2_id}/resolve"),
        Some("k-p-confirm"),
        Some(json!({ "action": "confirm_roll" })),
    )
    .await;
    assert_eq!(r2["status"], "awaiting_recovery");

    let (_, r_push) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int2_id}/resolve"),
        Some("k-p-push"),
        Some(json!({ "action": "push_roll" })),
    )
    .await;
    assert_eq!(r_push["status"], "awaiting_push_description");

    let (_, r_desc) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int2_id}/resolve"),
        Some("k-p-desc"),
        Some(json!({ "action": "submit_push_description", "description": "走近壁炉紧盯他的手指" })),
    )
    .await;
    assert_eq!(r_desc["status"], "awaiting_push_consequence_ack");
    assert!(r_desc["pendingInterrupt"]["pushConsequence"]
        .as_str()
        .unwrap()
        .contains("庄园异象"));

    let (st_ack, r_ack) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/interrupts/{int2_id}/resolve"),
        Some("k-p-ack"),
        Some(json!({ "action": "confirm_push_consequence" })),
    )
    .await;
    assert_eq!(st_ack, StatusCode::OK);
    assert_eq!(r_ack["status"], "committing");
    assert_eq!(r_ack["roll"]["attemptIndex"], 2);
    assert_eq!(r_ack["roll"]["pushCount"], 1);
    assert!(r_ack["pendingInterrupt"].is_null());

    wait_turn_finished(&state, &t2_id).await;

    // Verify Push failure advanced manor_anomaly visible clock and decreased SAN by 1, while hiding ritual_countdown
    let (_, after_push) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    assert_eq!(after_push["visibleClocks"][0]["current"], 2);
    assert_eq!(after_push["investigator"]["san"], 54);
    assert!(!after_push.to_string().contains("ritual_countdown"));

    // End session and verify Case Debrief unlocks with event traceability & hidden clock
    let (_, t_end) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("k-end"),
        Some(json!({ "input": "揭露真相结案" })),
    )
    .await;
    wait_turn_finished(&state, t_end["turnId"].as_str().unwrap()).await;

    let (st_deb, deb) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/debrief"),
        None,
        None,
    )
    .await;
    assert_eq!(st_deb, StatusCode::OK);
    assert_eq!(deb["spoilerWarning"], true);
    let final_clocks = deb["finalClocks"].as_array().unwrap();
    assert!(final_clocks
        .iter()
        .any(|c| c["clockId"] == "ritual_countdown" && c["visibility"] == "keeper_hidden"));
    let all_event_ids: std::collections::BTreeSet<String> = event_repo::list(&state.db, &sid)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.event_id)
        .collect();
    for item in deb["trueChronology"].as_array().unwrap() {
        for eid in item["sourceEventIds"].as_array().unwrap() {
            assert!(all_event_ids.contains(eid.as_str().unwrap()));
        }
    }
}

#[tokio::test]
async fn sse_multi_subscriber_catchup_and_expired_cursor_stream_reset() {
    let limits = BufferLimits {
        max_events: 3,
        max_buffer_bytes: 64 * 1024,
        max_narration_bytes: 64 * 1024,
        ttl: Duration::from_secs(60),
    };
    let buf = Arc::new(ActiveTurnBuffer::new("turn-sse-1", limits));

    // Late subscriber catch-up + concurrent subscriber zero-loss
    let b1 = buf.clone();
    let b2 = buf.clone();
    let sub1 = tokio::spawn(async move {
        let mut collected = Vec::new();
        let mut cursor = 0u64;
        loop {
            match b1.next_batch(cursor).await {
                SubscribeStep::Events { events, terminal } => {
                    for e in events {
                        cursor = e.seq;
                        collected.push(e.seq);
                    }
                    if terminal {
                        break;
                    }
                }
                SubscribeStep::StreamReset { seq, terminal, .. } => {
                    cursor = seq;
                    if terminal {
                        break;
                    }
                }
            }
        }
        collected
    });

    buf.push("narration_delta", json!({ "chunk": "A" })).await;
    buf.append_narration("A").await;
    buf.push("narration_delta", json!({ "chunk": "B" })).await;
    buf.append_narration("B").await;
    buf.push("narration_delta", json!({ "chunk": "C" })).await;
    buf.append_narration("C").await;

    // Second subscriber joins late with Last-Event-ID = 1 -> catches up 2 & 3
    let sub2 = tokio::spawn(async move {
        match b2.next_batch(1).await {
            SubscribeStep::Events { events, .. } => {
                events.into_iter().map(|e| e.seq).collect::<Vec<_>>()
            }
            _ => vec![],
        }
    });

    assert_eq!(sub2.await.unwrap(), vec![2, 3]);

    // Push 2 more events -> evicts seq 1 & 2 (max_events = 3 leaves seq 3,4,5)
    buf.push("narration_delta", json!({ "chunk": "D" })).await;
    buf.append_narration("D").await;
    buf.push("turn_complete", json!({ "turnId": "turn-sse-1" }))
        .await;
    buf.complete("completed").await;

    // Stale cursor (0) MUST receive StreamReset with full snapshot, never silently drop!
    match buf.next_batch(0).await {
        SubscribeStep::StreamReset { seq, payload, .. } => {
            assert_eq!(seq, 5);
            assert_eq!(payload["type"], "stream_reset");
            assert_eq!(payload["snapshot"]["narrationSnapshot"], "ABCD");
        }
        _ => panic!("expected StreamReset for evicted cursor"),
    }

    let s1 = sub1.await.unwrap();
    assert!(!s1.is_empty());
    assert_eq!(*s1.last().unwrap(), 5);
}

#[tokio::test]
async fn startup_scan_and_concurrent_recovery_lease_cas() {
    let (_dir, state, app) = test_app(Duration::ZERO, None).await;
    let reg = Arc::new(BufferRegistry::default());
    let state = state.with_buffers(reg);

    // Seed session + two crashed turns (one pre-commit, one post-commit narrating)
    let module = state.modules.get("crimson_rain_manor").unwrap();
    let initial = vellum_server::runtime::orchestrator::build_initial_session_state(
        "sess-rec",
        module,
        "investigator_reporter",
    )
    .unwrap();
    vellum_server::persistence::session_repo::create(
        &state.db,
        "sess-rec",
        "crimson_rain_manor",
        &module.version,
        "investigator_reporter",
        "classic",
        Some("t-narr"),
        &initial,
    )
    .await
    .unwrap();
    turn_repo::create(
        &state.db,
        "t-pre",
        "sess-rec",
        1,
        "B",
        Some("未提交行动"),
        TurnStatus::ProposalPending,
    )
    .await
    .unwrap();
    turn_repo::create(
        &state.db,
        "t-narr",
        "sess-rec",
        2,
        "B",
        Some("已提交行动"),
        TurnStatus::Narrating,
    )
    .await
    .unwrap();

    // Startup scan must mark statuses cleanly without invoking LLMs
    recovery::scan_startup(&state.db).await.unwrap();
    assert_eq!(
        turn_repo::status(&state.db, "t-pre").await.unwrap(),
        Some(TurnStatus::InterruptedBeforeCommit)
    );
    assert_eq!(
        turn_repo::status(&state.db, "t-narr").await.unwrap(),
        Some(TurnStatus::NarrationPending)
    );

    // Even after restart (empty in-memory active lock set), submitting a new turn while a
    // NarrationPending active_turn_id exists must fail fast with 409 TURN_ALREADY_ACTIVE
    let (st_blocked, blocked_body) = json_request(
        &app,
        Method::POST,
        "/api/sessions/sess-rec/turns",
        Some("k-blocked-by-pending-narration"),
        Some(json!({ "input": "前往书房" })),
    )
    .await;
    assert_eq!(st_blocked, StatusCode::CONFLICT);
    assert_eq!(blocked_body["error"]["code"], "TURN_ALREADY_ACTIVE");

    // Race two clients trying to acquire the RecoveryLease simultaneously
    let db1 = state.db.clone();
    let db2 = state.db.clone();
    let (r1, r2) = tokio::join!(
        recovery::try_acquire_lease(&db1, "t-narr", "client-a", Duration::from_secs(30)),
        recovery::try_acquire_lease(&db2, "t-narr", "client-b", Duration::from_secs(30)),
    );
    let l1 = r1.unwrap();
    let l2 = r2.unwrap();
    assert_ne!(
        l1.is_some(),
        l2.is_some(),
        "exactly one client must win the SQLite CAS RecoveryLease"
    );
    let winner = l1.or(l2).unwrap();
    assert_eq!(winner.generation_id, 2);

    // Run recovery and verify `narration_reset` + `generationId = 2` emitted
    recovery::run_narration_recovery(
        state.clone(),
        "sess-rec".into(),
        "t-narr".into(),
        winner,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(
        turn_repo::status(&state.db, "t-narr").await.unwrap(),
        Some(TurnStatus::Completed)
    );
    let buf = state.buffers.get("t-narr").unwrap();
    match buf.next_batch(0).await {
        SubscribeStep::Events { events, .. } => {
            assert!(events.iter().any(|e| {
                e.event_type == "narration_reset"
                    && e.payload["generationId"] == 2
                    && e.payload["clearPartial"] == true
            }));
        }
        _ => panic!("expected recovery events"),
    }
}

#[tokio::test]
async fn concurrent_interrupt_resolve_same_idempotency_key_rolls_once() {
    let (_dir, state, app) = test_app(
        Duration::ZERO,
        Some(Box::new(SequenceRng::new(&[7, 5, 1, 2]))),
    )
    .await;

    let (_, created) = json_request(
        &app,
        Method::POST,
        "/api/sessions",
        Some("k-race-sess"),
        Some(json!({
            "moduleId": "crimson_rain_manor",
            "investigatorId": "investigator_reporter",
            "experienceMode": "classic"
        })),
    )
    .await;
    let sid = created["session"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let t0 = created["turn0Id"].as_str().unwrap().to_string();
    wait_turn_finished(&state, &t0).await;

    let (_, t1) = json_request(
        &app,
        Method::POST,
        &format!("/api/sessions/{sid}/turns"),
        Some("k-race-t1"),
        Some(json!({ "input": "审视管家微表情" })),
    )
    .await;
    let t1_id = t1["turnId"].as_str().unwrap().to_string();
    wait_turn_finished(&state, &t1_id).await;

    let (_, st) = json_request(
        &app,
        Method::GET,
        &format!("/api/sessions/{sid}/state"),
        None,
        None,
    )
    .await;
    let int_id = st["pendingInterrupt"]["interruptId"]
        .as_str()
        .unwrap()
        .to_string();

    let url = format!("/api/sessions/{sid}/interrupts/{int_id}/resolve");
    let (res_a, res_b) = tokio::join!(
        json_request(
            &app,
            Method::POST,
            &url,
            Some("k-same-confirm"),
            Some(json!({ "action": "confirm_roll" })),
        ),
        json_request(
            &app,
            Method::POST,
            &url,
            Some("k-same-confirm"),
            Some(json!({ "action": "confirm_roll" })),
        ),
    );

    assert_eq!(res_a.0, StatusCode::OK);
    assert_eq!(res_b.0, StatusCode::OK);
    assert_eq!(res_a.1["roll"]["rollValue"], res_b.1["roll"]["rollValue"]);
    assert_eq!(res_a.1["status"], res_b.1["status"]);

    let roll_events = event_repo::list(&state.db, &sid)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| {
            matches!(
                e.event,
                vellum_server::kernel::GameEventKind::RollResolved { .. }
            )
        })
        .count();
    assert_eq!(
        roll_events, 1,
        "concurrent identical idempotent confirm_roll must execute exactly one dice roll"
    );
}
