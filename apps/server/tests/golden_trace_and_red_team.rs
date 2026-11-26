use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;

use vellum_server::{
    kernel::{
        improvisation::validate_patch, replay_events, ActionContext, DynamicWorldPatch,
        SeededDiceRng, WorldChange,
    },
    models::MockProvider,
    modules::ModuleRegistry,
    narration::{assemble_npc_context, render_npc_prompt, TranscriptEntryView},
    persistence::{db::Database, event_repo, session_repo, turn_repo},
    runtime::orchestrator::{self, build_initial_session_state},
    AppState,
};

#[tokio::test]
async fn golden_trace_deterministic_replay_matches_snapshot() {
    let dir = TempDir::new().unwrap();
    let db_url = format!("sqlite://{}", dir.path().join("golden.sqlite").display());
    let db = Database::connect(&db_url).await.unwrap();
    let modules = Arc::new(ModuleRegistry::load_default().unwrap());
    let module = modules.get("crimson_rain_manor").unwrap().clone();

    let state = AppState::new(db, Arc::new(MockProvider::default()), modules, "mock")
        .with_rng(Box::new(SeededDiceRng::new(42)));

    let sid = "golden-session-1";
    let initial = build_initial_session_state(sid, &module, "investigator_reporter").unwrap();
    session_repo::create(
        &state.db,
        sid,
        &module.module_id,
        &module.version,
        "investigator_reporter",
        "classic",
        Some("t0"),
        &initial,
    )
    .await
    .unwrap();
    turn_repo::create(
        &state.db,
        "t0",
        sid,
        0,
        "B",
        None,
        vellum_server::kernel::TurnStatus::Received,
    )
    .await
    .unwrap();
    orchestrator::run_turn0(state.clone(), sid.into(), "t0".into()).await;

    let script = [
        ("t1", "前往女仆偏厅"),
        ("t2", "检查银托盘背面的刻纹"),
        ("t3", "前往暴雨庭院"),
        ("t4", "找铁铲撬开酒窖后窗"),
    ];
    for (idx, (tid, input)) in script.iter().enumerate() {
        turn_repo::create(
            &state.db,
            tid,
            sid,
            (idx + 1) as i64,
            "B",
            Some(input),
            vellum_server::kernel::TurnStatus::Received,
        )
        .await
        .unwrap();
        orchestrator::run_player_turn(state.clone(), sid.into(), (*tid).into(), (*input).into())
            .await;
    }

    let (_, final_snapshot) = session_repo::load_state(&state.db, sid)
        .await
        .unwrap()
        .unwrap();
    let events = event_repo::list(&state.db, sid).await.unwrap();
    assert!(!events.is_empty());

    let replayed = replay_events(initial, events.iter().map(|r| r.event.clone()));
    assert_eq!(
        replayed.current_scene.scene_id,
        final_snapshot.current_scene.scene_id
    );
    assert_eq!(
        replayed.current_scene.visited,
        final_snapshot.current_scene.visited
    );
    assert_eq!(
        replayed.clues.keys().collect::<Vec<_>>(),
        final_snapshot.clues.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        replayed.handouts.keys().collect::<Vec<_>>(),
        final_snapshot.handouts.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        replayed.clocks.get("manor_anomaly").unwrap().current,
        final_snapshot.clocks.get("manor_anomaly").unwrap().current
    );
    assert_eq!(
        replayed.clocks.get("ritual_countdown").unwrap().current,
        final_snapshot
            .clocks
            .get("ritual_countdown")
            .unwrap()
            .current
    );
}

#[test]
fn red_team_npc_provenance_cross_scene_inaudible_bluff_and_protected_truth() {
    let modules = ModuleRegistry::load_default().unwrap();
    let module = modules.get("crimson_rain_manor").unwrap();
    let mut state = build_initial_session_state("rt-1", module, "investigator_reporter").unwrap();

    let butler = module.find_npc("butler_arthur").unwrap();
    let mary = module.find_npc("maid_mary").unwrap();

    // Transcript entries:
    // 1. Speech in manor_foyer heard by butler_arthur ONLY (NOT maid_mary!)
    // 2. Whispered remark in manor_foyer where butler_arthur is present but NOT in audible_entity_ids
    // 3. Missing scene_id metadata -> fail closed (denied for everyone)
    // 4. Player Bluff directed at butler_arthur
    let transcripts = vec![
        TranscriptEntryView {
            entry_id: "e1".into(),
            turn_id: "t1".into(),
            scene_id: "manor_foyer".into(),
            speaker_entity_id: Some("player".into()),
            present_entity_ids: vec!["butler_arthur".into()],
            audible_entity_ids: vec!["butler_arthur".into()],
            visible_entity_ids: vec!["butler_arthur".into()],
            visibility_scope: "scene".into(),
            claim_kind: "dialogue".into(),
            referenced_event_ids: vec![],
            content: "门厅里的公开询问".into(),
        },
        TranscriptEntryView {
            entry_id: "e2".into(),
            turn_id: "t2".into(),
            scene_id: "manor_foyer".into(),
            speaker_entity_id: Some("player".into()),
            present_entity_ids: vec!["butler_arthur".into()],
            audible_entity_ids: vec![], // Butler did NOT hear this!
            visible_entity_ids: vec![],
            visibility_scope: "scene".into(),
            claim_kind: "dialogue".into(),
            referenced_event_ids: vec![],
            content: "调查员背过身低声自语的推理".into(),
        },
        TranscriptEntryView {
            entry_id: "e3".into(),
            turn_id: "t3".into(),
            scene_id: "".into(), // Missing metadata -> deny
            speaker_entity_id: Some("player".into()),
            present_entity_ids: vec!["butler_arthur".into()],
            audible_entity_ids: vec!["butler_arthur".into()],
            visible_entity_ids: vec!["butler_arthur".into()],
            visibility_scope: "public".into(),
            claim_kind: "dialogue".into(),
            referenced_event_ids: vec![],
            content: "缺少场景元数据的发言".into(),
        },
        TranscriptEntryView {
            entry_id: "e4".into(),
            turn_id: "t4".into(),
            scene_id: "manor_foyer".into(),
            speaker_entity_id: Some("player".into()),
            present_entity_ids: vec!["butler_arthur".into()],
            audible_entity_ids: vec!["butler_arthur".into()],
            visible_entity_ids: vec!["butler_arthur".into()],
            visibility_scope: "scene".into(),
            claim_kind: "player_assertion_unverified".into(),
            referenced_event_ids: vec![],
            content: "女仆 Mary 已经把一切告诉我了！".into(),
        },
    ];

    let empty_facts = BTreeSet::new();

    // 1. Cross-scene isolation: Mary in maids_parlor sees NONE of manor_foyer transcripts
    let mary_ctx = assemble_npc_context(mary, &state, None, &transcripts, &[], &empty_facts);
    assert!(mary_ctx.filtered_transcript.is_empty());
    let mary_prompt = render_npc_prompt(&mary_ctx);
    assert!(!mary_prompt.contains("门厅里的公开询问"));
    assert!(!mary_prompt.contains(&mary.protected_truths[0].raw_truth));

    // 2. Audible filtering + Bluff tagging + Protected Truth zero raw_truth leakage
    // Even if player possesses `cellar_ledger_clue`, without ActionContext `evidence_presented` + `topic_confronted`,
    // `partial` truth stage MUST NOT unlock!
    state.clues.insert(
        "cellar_ledger_clue".into(),
        vellum_server::kernel::ClueState {
            clue_id: "cellar_ledger_clue".into(),
            title: "酒窖账本".into(),
            summary: "...".into(),
            source_label: "酒窖".into(),
            discovered_at_turn_id: "t1".into(),
            is_conclusion: false,
        },
    );

    let butler_ctx_before =
        assemble_npc_context(butler, &state, None, &transcripts, &[], &empty_facts);
    let butler_prompt_before = render_npc_prompt(&butler_ctx_before);
    assert!(butler_prompt_before.contains("门厅里的公开询问"));
    assert!(!butler_prompt_before.contains("调查员背过身低声自语的推理"));
    assert!(!butler_prompt_before.contains("缺少场景元数据的发言"));
    assert!(!butler_prompt_before.contains(&butler.protected_truths[0].raw_truth));
    assert!(butler_ctx_before.authorized_disclosures.is_empty());
    assert_eq!(butler_ctx_before.unverified_player_claims.len(), 1);
    assert!(butler_prompt_before.contains("未证实陈述"));

    // 3. When player actively presents `cellar_ledger_clue` and confronts `missing_guest` while Butler is present,
    // `partial` stage unlocks (still never leaking `raw_truth` verbatim!)
    let action_ctx = ActionContext {
        present_entity_ids: ["butler_arthur".to_string()].into_iter().collect(),
        unlocked_clues: ["cellar_ledger_clue".to_string()].into_iter().collect(),
        evidence_presented: ["cellar_ledger_clue".to_string()].into_iter().collect(),
        topics_confronted: ["missing_guest".to_string()].into_iter().collect(),
        event_ids: BTreeSet::new(),
    };
    let butler_ctx_after = assemble_npc_context(
        butler,
        &state,
        Some(&action_ctx),
        &transcripts,
        &[],
        &empty_facts,
    );
    assert_eq!(butler_ctx_after.authorized_disclosures.len(), 1);
    assert_eq!(butler_ctx_after.authorized_disclosures[0].tier, "partial");
    assert!(!render_npc_prompt(&butler_ctx_after).contains(&butler.protected_truths[0].raw_truth));
}

#[tokio::test]
async fn red_team_bluff_never_pollutes_knowledge_acquisitions() {
    let dir = TempDir::new().unwrap();
    let db_url = format!("sqlite://{}", dir.path().join("bluff.sqlite").display());
    let db = Database::connect(&db_url).await.unwrap();
    let modules = Arc::new(ModuleRegistry::load_default().unwrap());
    let module = modules.get("crimson_rain_manor").unwrap().clone();
    let state = AppState::new(
        db,
        Arc::new(MockProvider::with_delay(Duration::ZERO)),
        modules,
        "mock",
    );

    let sid = "bluff-sess";
    let initial = build_initial_session_state(sid, &module, "investigator_reporter").unwrap();
    session_repo::create(
        &state.db,
        sid,
        &module.module_id,
        &module.version,
        "investigator_reporter",
        "classic",
        None,
        &initial,
    )
    .await
    .unwrap();
    turn_repo::create(
        &state.db,
        "t-bluff",
        sid,
        1,
        "B",
        Some("女仆已经把一切告诉我了"),
        vellum_server::kernel::TurnStatus::Received,
    )
    .await
    .unwrap();

    orchestrator::run_player_turn(
        state.clone(),
        sid.into(),
        "t-bluff".into(),
        "女仆已经把一切告诉我了".into(),
    )
    .await;

    let beliefs = event_repo::list_beliefs(&state.db, sid).await.unwrap();
    assert_eq!(beliefs.len(), 1);
    assert!(beliefs[0].source_transcript_id.is_some());

    // Canonical knowledge_acquisitions MUST remain completely empty!
    let butler_facts = event_repo::list_knowledge_facts(&state.db, sid, "butler_arthur")
        .await
        .unwrap();
    assert!(butler_facts.is_empty());
}

#[test]
fn red_team_off_rail_patch_validation() {
    let modules = ModuleRegistry::load_default().unwrap();
    let module = modules.get("crimson_rain_manor").unwrap();
    let state = build_initial_session_state("patch-1", module, "investigator_reporter").unwrap();
    let applied = BTreeSet::new();

    // Forbidden: attempting to rewrite immutable_past
    let bad_patch = DynamicWorldPatch {
        patch_id: "p-bad".into(),
        source: "player_improvisation".into(),
        scope: "manor_foyer".into(),
        lifetime: "session".into(),
        causal_event_id: Some("ev-1".into()),
        changes: vec![WorldChange {
            kind: "immutable_past".into(),
            target_id: "missing_guest".into(),
            value: json!("never_happened"),
        }],
        resource_cost: None,
        required_permission: None,
    };
    assert!(validate_patch(&bad_patch, &state, &applied).is_err());

    // Valid scoped temporary improvisation
    let good_patch = DynamicWorldPatch {
        patch_id: "p-good".into(),
        source: "player_improvisation".into(),
        scope: "rain_courtyard".into(),
        lifetime: "session".into(),
        causal_event_id: Some("ev-2".into()),
        changes: vec![WorldChange {
            kind: "temporary_tag".into(),
            target_id: "courtyard_window_opened".into(),
            value: json!(true),
        }],
        resource_cost: None,
        required_permission: None,
    };
    assert!(validate_patch(&good_patch, &state, &applied).is_ok());
}
