use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::kernel::{
    apply_event, clocks::current_stage_label, improvisation::validate_patch, skill_display_label,
    ActionContext, ClockThreshold, ClockVisibility, ClueState, GameEventKind, HandoutState,
    InventoryItem, InvestigatorState, PendingInterrupt, ProgressClock, RollAttempt,
    RollResultLevel, SceneState, SessionState, TurnStatus,
};
use crate::models::{NarrationContext, TurnHistoryItem};
use crate::modules::{evaluate_module_predicate, ModuleIr, SceneDefinition};
use crate::persistence::{
    db::Database,
    event_repo::{self, DirectiveDto, TranscriptRecord},
    session_repo, turn_repo,
};
use crate::AppState;

pub fn legal_transition(from: &TurnStatus, to: &TurnStatus) -> bool {
    use TurnStatus::*;
    matches!(
        (from, to),
        (Received, Routing)
            | (Routing, ProposalPending)
            | (Routing, Narrating)
            | (ProposalPending, AwaitingRoll)
            | (ProposalPending, Committing)
            | (ProposalPending, Failed)
            | (AwaitingRoll, Cancelled)
            | (AwaitingRoll, AwaitingRecovery)
            | (AwaitingRoll, Committing)
            | (AwaitingRecovery, Committing)
            | (AwaitingRecovery, AwaitingPushDescription)
            | (AwaitingPushDescription, AwaitingPushConsequenceAck)
            | (AwaitingPushConsequenceAck, AwaitingPushRoll)
            | (AwaitingPushConsequenceAck, Committing)
            | (AwaitingPushRoll, Committing)
            | (Committing, Narrating)
            | (Narrating, Completed)
            | (Narrating, NarrationPending)
            | (NarrationPending, Recovering)
            | (Recovering, Completed)
            | (Received, InterruptedBeforeCommit)
            | (ProposalPending, InterruptedBeforeCommit)
    )
}

/// Builds the initial authoritative `SessionState` from a validated `ModuleIr` and preset investigator.
pub fn build_initial_session_state(
    session_id: &str,
    module: &ModuleIr,
    investigator_id: &str,
) -> Option<SessionState> {
    let preset = module.find_investigator(investigator_id)?;
    let initial_scene = module.find_scene(&module.initial_scene_id)?;

    let mut items = BTreeMap::new();
    let inventory: Vec<InventoryItem> = preset
        .starting_items
        .iter()
        .map(|it| {
            items.insert(it.item_id.clone(), 1);
            InventoryItem {
                item_id: it.item_id.clone(),
                name: it.name.clone(),
                description: it.description.clone(),
                quantity: 1,
            }
        })
        .collect();

    let investigator = InvestigatorState {
        investigator_id: preset.investigator_id.clone(),
        name: preset.name.clone(),
        occupation: preset.occupation.clone(),
        hp: preset.hp_max,
        hp_max: preset.hp_max,
        mp: preset.mp_max,
        mp_max: preset.mp_max,
        san: preset.san_start,
        san_max: 99,
        luck: preset.luck_start,
        luck_max: 99,
        characteristics: preset.characteristics.clone(),
        skills: preset.skills.clone(),
        inventory,
        conditions: BTreeSet::new(),
    };

    let present_entity_ids: BTreeSet<String> = initial_scene
        .entities
        .iter()
        .map(|e| e.entity_id.clone())
        .collect();
    let mut visited = BTreeSet::new();
    visited.insert(initial_scene.scene_id.clone());

    let current_scene = SceneState {
        scene_id: initial_scene.scene_id.clone(),
        title: initial_scene.title.clone(),
        description: initial_scene.description.clone(),
        atmosphere: initial_scene.atmosphere.clone(),
        visited,
        present_entity_ids,
    };

    let mut clocks = BTreeMap::new();
    for c in &module.clocks {
        let visibility = if c.visibility == "keeper_hidden" {
            ClockVisibility::KeeperHidden
        } else {
            ClockVisibility::PlayerVisible
        };
        clocks.insert(
            c.clock_id.clone(),
            ProgressClock {
                clock_id: c.clock_id.clone(),
                name: c.name.clone(),
                current: 0,
                max: c.max,
                visibility,
                thresholds: c
                    .stages
                    .iter()
                    .map(|s| ClockThreshold {
                        at: s.at,
                        stage_label: s.label.clone(),
                    })
                    .collect(),
                last_reason: None,
                last_caused_by: None,
            },
        );
    }

    let mut npc_relations = BTreeMap::new();
    for npc in &module.npcs {
        for rel in &npc.initial_relations {
            npc_relations.insert(format!("{}:{}", npc.entity_id, rel.relation), rel.value);
        }
    }

    Some(SessionState {
        session_id: session_id.to_string(),
        module_id: module.module_id.clone(),
        module_version: module.version.clone(),
        investigator,
        current_scene,
        clocks,
        clues: BTreeMap::new(),
        handouts: BTreeMap::new(),
        flags: BTreeMap::new(),
        items,
        skill_successes: BTreeSet::new(),
        events: BTreeSet::new(),
        evidence_presented: BTreeSet::new(),
        topics_confronted: BTreeSet::new(),
        npc_heard_events: BTreeSet::new(),
        npc_known_facts: BTreeSet::new(),
        npc_beliefs: BTreeMap::new(),
        npc_relations,
        raw_truth: json!({ "keeperOnly": true }),
        pending_interrupt: None,
        turn_status: Some(TurnStatus::Received),
    })
}

pub fn visible_clocks_dto(state: &SessionState) -> Vec<Value> {
    state
        .clocks
        .values()
        .filter(|c| c.visibility == ClockVisibility::PlayerVisible)
        .map(|c| {
            json!({
                "clockId": c.clock_id,
                "name": c.name,
                "current": c.current,
                "max": c.max,
                "stageLabel": current_stage_label(c),
                "lastReason": c.last_reason,
            })
        })
        .collect()
}

/// Loads the most recent completed turns for multi-turn Keeper conversational memory.
pub async fn load_recent_history(
    db: &Database,
    session_id: &str,
    limit: usize,
) -> Vec<TurnHistoryItem> {
    if limit == 0 {
        return Vec::new();
    }
    let Ok(turns) = turn_repo::list_for_session(db, session_id).await else {
        return Vec::new();
    };
    let Ok(transcripts) = event_repo::list_transcripts(db, session_id).await else {
        return Vec::new();
    };
    let mut narration_by_turn: HashMap<String, String> = HashMap::new();
    for tr in transcripts {
        if tr.claim_kind == "narration" {
            narration_by_turn.insert(tr.turn_id, tr.content);
        }
    }
    let mut items: Vec<TurnHistoryItem> = turns
        .into_iter()
        .filter(|t| matches!(t.status, TurnStatus::Completed))
        .filter_map(|t| {
            narration_by_turn
                .remove(&t.id)
                .map(|narration| TurnHistoryItem {
                    player_input: t.player_input,
                    narration,
                })
        })
        .collect();
    if items.len() > limit {
        items = items.split_off(items.len() - limit);
    }
    items
}

pub fn visible_scene_dto(state: &SessionState, module: Option<&ModuleIr>) -> Value {
    let def: Option<&SceneDefinition> =
        module.and_then(|m| m.find_scene(&state.current_scene.scene_id));
    let exits: Vec<Value> = def
        .map(|scene| {
            scene
                .exits
                .iter()
                .map(|exit| {
                    let accessible = exit
                        .condition
                        .as_ref()
                        .map(|cond| evaluate_module_predicate(cond, state, None))
                        .unwrap_or(true);
                    json!({
                        "targetSceneId": exit.target_scene_id,
                        "label": exit.label,
                        "accessible": accessible,
                        "blockedReason": if accessible { Value::Null } else { json!("需要先阅读委托函或取得通行线索") },
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let present_entities: Vec<Value> = def
        .map(|scene| {
            scene
                .entities
                .iter()
                .map(|e| {
                    json!({
                        "entityId": e.entity_id,
                        "name": e.name,
                        "role": e.role,
                        "kind": e.kind,
                        "summary": e.summary,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let tags = def.map(|s| s.tags.clone()).unwrap_or_default();
    let visited_scene_ids: Vec<String> = state.current_scene.visited.iter().cloned().collect();

    json!({
        "sceneId": state.current_scene.scene_id,
        "title": state.current_scene.title,
        "atmosphere": state.current_scene.atmosphere,
        "description": state.current_scene.description,
        "tags": tags,
        "visitedSceneIds": visited_scene_ids,
        "exits": exits,
        "presentEntities": present_entities,
    })
}

/// Projects the architectural room blueprint for the tabletop Manor Battlefield Map.
/// Strictly player-safe: unvisited rooms stay fogged (empty entities) and NEVER expose clues or hidden clocks.
pub fn map_rooms_dto(state: &SessionState, module: Option<&ModuleIr>) -> Vec<Value> {
    let Some(m) = module else {
        return Vec::new();
    };
    let current_id = state.current_scene.scene_id.as_str();
    let current_exits: HashMap<&str, (&str, bool)> = m
        .find_scene(current_id)
        .map(|scene| {
            scene
                .exits
                .iter()
                .map(|exit| {
                    let accessible = exit
                        .condition
                        .as_ref()
                        .map(|cond| evaluate_module_predicate(cond, state, None))
                        .unwrap_or(true);
                    (
                        exit.target_scene_id.as_str(),
                        (exit.label.as_str(), accessible),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    m.scenes
        .iter()
        .map(|scene| {
            let is_current = scene.scene_id == current_id;
            let visited = is_current || state.current_scene.visited.contains(&scene.scene_id);
            let (exit_label, accessible_from_current) = current_exits
                .get(scene.scene_id.as_str())
                .map(|(label, ok)| (Some(*label), *ok))
                .unwrap_or((None, false));
            let known_npcs: Vec<Value> = if visited {
                scene
                    .entities
                    .iter()
                    .filter(|e| e.kind == "npc")
                    .map(|e| {
                        json!({
                            "entityId": e.entity_id,
                            "name": e.name,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            json!({
                "sceneId": scene.scene_id,
                "title": scene.title,
                "atmosphere": scene.atmosphere,
                "visited": visited,
                "isCurrent": is_current,
                "accessibleFromCurrent": accessible_from_current,
                "exitLabelFromCurrent": exit_label,
                "knownNpcs": known_npcs,
            })
        })
        .collect()
}

pub fn guided_prompts_for_scene(scene_id: &str) -> Vec<String> {
    match scene_id {
        "manor_foyer" => vec![
            "审视管家微表情，试探三年前访客的下落".into(),
            "检查墙上的家族肖像是否有暗格".into(),
            "前往书房".into(),
        ],
        "study_room" => vec![
            "撬开书桌锁住的抽屉".into(),
            "翻看书架后的庄园旧剪报".into(),
            "前往女仆偏厅".into(),
        ],
        "maids_parlor" => vec![
            "温和安抚女仆 Mary 并询问昨夜钟声".into(),
            "检查银托盘背面的刻纹".into(),
            "前往暴雨庭院".into(),
        ],
        "rain_courtyard" => vec![
            "查看温室外的泥痕与脚印".into(),
            "找铁铲撬开酒窖后窗".into(),
            "前往酒窖入口".into(),
        ],
        "cellar_entrance" => vec![
            "调查酒窖铁门后的酒窖账本".into(),
            "返回门厅，出示账本与老管家对质揭露真相结案".into(),
        ],
        _ => vec!["观察周围环境".into()],
    }
}

pub fn build_client_state_patch(
    state: &SessionState,
    session_version: i64,
    session_status: &str,
    module: Option<&ModuleIr>,
) -> Value {
    json!({
        "sessionVersion": session_version,
        "sessionStatus": session_status,
        "investigator": state.investigator,
        "currentScene": visible_scene_dto(state, module),
        "mapRooms": map_rooms_dto(state, module),
        "visibleClocks": visible_clocks_dto(state),
        "addedClues": state.clues.values().cloned().collect::<Vec<_>>(),
        "addedHandouts": state.handouts.values().cloned().collect::<Vec<_>>(),
        "pendingInterrupt": state.pending_interrupt,
        "guidedPrompts": guided_prompts_for_scene(&state.current_scene.scene_id),
    })
}

/// Turn 0 automated opening sequence:
/// Emits immediate atmosphere, streams opening prologue, persists & releases the initial
/// commission letter (`after_narration`) + clue, commits state, and clears active_turn_id.
pub async fn run_turn0(state: AppState, session_id: String, turn0_id: String) {
    let buffer = state.buffers.get_or_create(&turn0_id);
    buffer.set_status("narrating").await;
    buffer
        .push(
            "status_hint",
            json!({ "type": "status_hint", "text": "暴雨正敲打庄园铁门，正在展开开篇序幕……", "turnId": turn0_id }),
        )
        .await;

    let Ok(Some((version, mut session_state))) =
        session_repo::load_state(&state.db, &session_id).await
    else {
        buffer.complete("failed").await;
        state.locks.clear_active(&session_id).await;
        return;
    };

    let Some(module) = state.modules.get(&session_state.module_id).cloned() else {
        buffer.complete("failed").await;
        state.locks.clear_active(&session_id).await;
        return;
    };

    let mut seq = event_repo::next_event_seq(&state.db, &session_id)
        .await
        .unwrap_or(1);
    let mut directives_to_emit: Vec<DirectiveDto> = Vec::new();

    // Immediate atmosphere shift
    let atm_payload = json!({
        "theme": session_state.current_scene.atmosphere,
        "sceneTitle": format!("{} · {}", module.title, session_state.current_scene.title),
        "note": "暴雨封锁了通往山脊的旧道",
        "intensity": "subtle",
    });
    if let Ok(d) = event_repo::save_directive(
        &state.db,
        &format!("dir-atm-{turn0_id}"),
        &session_id,
        &turn0_id,
        1,
        "kernel_owned",
        "atmosphere_shift",
        "immediate",
        &atm_payload,
        Some(1),
    )
    .await
    {
        buffer.record_directive(d.clone()).await;
        buffer
            .push(
                "ui_directive",
                json!({ "type": "ui_directive", "directive": d }),
            )
            .await;
    }

    // Unlock initial commission letter clue & canonical handout (after_narration!)
    let mut ref_event_ids = Vec::new();
    if let Some(clue_def) = module.find_clue("commission_letter_clue") {
        let clue = ClueState {
            clue_id: clue_def.clue_id.clone(),
            title: clue_def.title.clone(),
            summary: clue_def.summary.clone(),
            source_label: clue_def.source_label.clone(),
            discovered_at_turn_id: turn0_id.clone(),
            is_conclusion: clue_def.is_conclusion,
        };
        let ev_id = Uuid::new_v4().to_string();
        let ev = GameEventKind::ClueUnlocked { clue: clue.clone() };
        apply_event(&mut session_state, &ev);
        let _ = event_repo::append(
            &state.db,
            &ev_id,
            &session_id,
            &turn0_id,
            seq,
            &ev,
            &module.version,
            None,
        )
        .await;
        seq += 1;
        ref_event_ids.push(ev_id);

        if let Ok(d) = event_repo::save_directive(
            &state.db,
            &format!("dir-clue-{}-{}", clue.clue_id, turn0_id),
            &session_id,
            &turn0_id,
            1,
            "kernel_owned",
            "evidence_unlocked",
            "after_narration",
            &json!({
                "clueId": clue.clue_id,
                "title": clue.title,
                "summary": clue.summary,
                "sourceLabel": clue.source_label,
            }),
            Some(2),
        )
        .await
        {
            directives_to_emit.push(d);
        }
    }

    if let Some(handout_def) = module.find_handout("commission_letter") {
        let handout = HandoutState {
            handout_id: handout_def.handout_id.clone(),
            kind: handout_def.kind.clone(),
            title: handout_def.title.clone(),
            canonical: handout_def.canonical,
            clue_id: handout_def.clue_id.clone(),
            source_facts: handout_def.source_facts.clone(),
            presentation_text: handout_def.presentation_text.clone(),
            unlocked_at_turn_id: turn0_id.clone(),
        };
        let ev_id = Uuid::new_v4().to_string();
        let ev = GameEventKind::HandoutReleased {
            handout: handout.clone(),
        };
        apply_event(&mut session_state, &ev);
        let _ = event_repo::append(
            &state.db,
            &ev_id,
            &session_id,
            &turn0_id,
            seq,
            &ev,
            &module.version,
            None,
        )
        .await;
        ref_event_ids.push(ev_id);

        if let Ok(d) = event_repo::save_directive(
            &state.db,
            &format!("dir-handout-{}-{}", handout.handout_id, turn0_id),
            &session_id,
            &turn0_id,
            1,
            "kernel_owned",
            "diegetic_handout",
            "after_narration",
            &json!({
                "handoutId": handout.handout_id,
                "kind": handout.kind,
                "title": handout.title,
                "canonical": handout.canonical,
                "clueId": handout.clue_id,
                "sourceFacts": handout.source_facts,
                "presentationText": handout.presentation_text,
            }),
            Some(3),
        )
        .await
        {
            directives_to_emit.push(d);
        }
    }

    session_state.turn_status = Some(TurnStatus::Narrating);
    let _ = turn_repo::set_status(&state.db, &turn0_id, TurnStatus::Narrating).await;
    let _ = session_repo::cas_snapshot(
        &state.db,
        &session_id,
        version,
        &session_state,
        None,
        Some(Some(&turn0_id)),
    )
    .await;

    // Emit persisted `after_narration` directives (frontend queues them until narration_complete!)
    for d in directives_to_emit {
        buffer.record_directive(d.clone()).await;
        buffer
            .push(
                "ui_directive",
                json!({ "type": "ui_directive", "directive": d }),
            )
            .await;
    }

    let preset_hook = module
        .find_investigator(&session_state.investigator.investigator_id)
        .map(|p| {
            format!(
                "角色背景：{}；动机：{}；与庄园渊源：{}",
                p.biography, p.motivation, p.manor_connection
            )
        })
        .unwrap_or_default();
    let opening_outcome = format!(
        "开场序幕：{}\n调查员身份：{}「{}」（{}）\n已随身获得线索「无署名委托函」与实体证物，要求查清三年前庄园访客失踪案并留意酒窖。\n老管家 Arthur 正提着黄铜提灯守在壁炉旁迎上前来。",
        module.opening_prologue.trim(),
        session_state.investigator.occupation,
        session_state.investigator.name,
        preset_hook,
    );
    let opening_ctx = NarrationContext {
        history: &[],
        outcome_summary: Some(&opening_outcome),
        is_opening: true,
    };

    let fallback_opening = format!(
        "【守秘人】{}\n\n你以{}「{}」的身份站在{}。老管家 Arthur 提着一盏黄铜提灯迎上前来，微弱灯影映出他克制而警惕的神色。\n\n【老管家 Arthur】「外头暴雨封山，女主人今夜身体不适。阁下既持函而来，不妨先在门厅歇脚——只是酒窖那边湿滑失修，还请莫要独自走近。」\n\n【直觉·侦查】他的礼数无可挑剔，可提灯微晃时，你瞥见他袖口边缘沾着一点尚未干透的深色蜡油。\n\n【抉择】老管家正等着你表明来意，书房与女仆偏厅的廊灯就在前方——你第一步打算怎么做？\n\n【建议行动】\n1. [心理学] 盯住 Arthur 的袖口与神情，直接追问三年前失踪访客的下落\n2. [侦查] 借口欣赏油画，凑近检查壁炉旁的家族肖像\n3. 暂不打草惊蛇，礼貌颔首后转身前往书房",
        module.opening_prologue.trim(),
        session_state.investigator.occupation,
        session_state.investigator.name,
        session_state.current_scene.title
    );

    let chunks = match state
        .provider
        .stream_narration("开场序幕", &session_state, &opening_ctx)
        .await
    {
        Ok(c) if !c.is_empty() => c,
        _ => chunk_text(&fallback_opening),
    };

    let mut opening = String::new();
    for chunk in chunks {
        opening.push_str(&chunk);
        buffer.append_narration(&chunk).await;
        buffer
            .push(
                "narration_delta",
                json!({
                    "type": "narration_delta",
                    "chunk": chunk,
                    "turnId": turn0_id,
                    "generationId": 1,
                }),
            )
            .await;
    }

    let present: Vec<String> = session_state
        .current_scene
        .present_entity_ids
        .iter()
        .cloned()
        .collect();
    let tr = TranscriptRecord {
        entry_id: format!("tr-{turn0_id}"),
        turn_id: turn0_id.clone(),
        scene_id: session_state.current_scene.scene_id.clone(),
        speaker_entity_id: None,
        present_entity_ids: present.clone(),
        audible_entity_ids: present.clone(),
        visible_entity_ids: present,
        visibility_scope: "public".into(),
        claim_kind: "narration".into(),
        referenced_event_ids: ref_event_ids,
        content: opening,
        created_at: Utc::now().to_rfc3339(),
    };
    let _ = event_repo::insert_transcript(&state.db, &session_id, &tr).await;
    let _ = session_repo::set_active_turn(&state.db, &session_id, None).await;
    state.locks.clear_active(&session_id).await;
    let _ = turn_repo::set_status(&state.db, &turn0_id, TurnStatus::Completed).await;

    let patch = build_client_state_patch(&session_state, version + 1, "active", Some(&module));
    buffer
        .push(
            "narration_complete",
            json!({ "type": "narration_complete", "turnId": turn0_id, "generationId": 1 }),
        )
        .await;
    buffer
        .push(
            "state_delta",
            json!({ "type": "state_delta", "patch": patch }),
        )
        .await;
    buffer
        .push(
            "turn_complete",
            json!({ "type": "turn_complete", "turnId": turn0_id }),
        )
        .await;
    buffer.complete("completed").await;
}

/// Executes a player IC turn asynchronously OUTSIDE the short Session Command Lock.
pub async fn run_player_turn(state: AppState, session_id: String, turn_id: String, input: String) {
    let buffer = state.buffers.get_or_create(&turn_id);
    buffer.set_status("routing").await;
    buffer
        .push(
            "status_hint",
            json!({ "type": "status_hint", "text": "守秘人正在审视你的行动……", "turnId": turn_id }),
        )
        .await;

    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Routing).await;

    let Ok(Some((version, mut session_state))) =
        session_repo::load_state(&state.db, &session_id).await
    else {
        buffer.complete("failed").await;
        state.locks.clear_active(&session_id).await;
        return;
    };
    let module = state.modules.get(&session_state.module_id).cloned();

    let history = load_recent_history(&state.db, &session_id, 6).await;

    // Lightweight route: pure low-risk observation -> Path A fast track (read-only, zero authoritative state mutations!)
    let is_path_a = is_low_risk_path_a(&input);
    if is_path_a {
        let _ = turn_repo::set_path_kind(&state.db, &turn_id, "A").await;
        let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Narrating).await;
        buffer.set_status("narrating").await;

        let path_a_ctx = NarrationContext {
            history: &history,
            outcome_summary: Some("纯环境观察回合：未发生房间移动、掷骰或状态变更，请给出细腻现场感、在场人物反应与下一步抉择。"),
            is_opening: false,
        };
        let chunks = match state
            .provider
            .stream_narration(&input, &session_state, &path_a_ctx)
            .await
        {
            Ok(c) => c,
            Err(err) => {
                emit_turn_error(
                    &state,
                    &session_id,
                    &turn_id,
                    &buffer,
                    "NARRATION_ERROR",
                    &err,
                )
                .await;
                return;
            }
        };

        let mut full_text = String::new();
        for chunk in chunks {
            full_text.push_str(&chunk);
            buffer.append_narration(&chunk).await;
            buffer
                .push(
                    "narration_delta",
                    json!({ "type": "narration_delta", "chunk": chunk, "turnId": turn_id, "generationId": 1 }),
                )
                .await;
        }

        persist_turn_transcript(
            &state,
            &session_id,
            &turn_id,
            &session_state,
            "narration",
            vec![],
            &full_text,
        )
        .await;
        let _ = session_repo::set_active_turn(&state.db, &session_id, None).await;
        state.locks.clear_active(&session_id).await;
        let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Completed).await;
        buffer
            .push(
                "narration_complete",
                json!({ "type": "narration_complete", "turnId": turn_id, "generationId": 1 }),
            )
            .await;
        buffer
            .push(
                "turn_complete",
                json!({ "type": "turn_complete", "turnId": turn_id }),
            )
            .await;
        buffer.complete("completed").await;
        return;
    }

    // Path B / C: Proposal -> Kernel Validation -> (Interrupt OR Commit) -> Narration
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::ProposalPending).await;
    buffer.set_status("proposal_pending").await;

    let proposal = match state
        .provider
        .propose_action(&input, &session_state, &history)
        .await
    {
        Ok(p) => p,
        Err(err) => {
            emit_turn_error(
                &state,
                &session_id,
                &turn_id,
                &buffer,
                "PROPOSAL_ERROR",
                &err,
            )
            .await;
            return;
        }
    };

    // If check is proposed -> create and persist PendingInterrupt(AwaitingRoll) BEFORE rolling!
    if let Some(check) = proposal.proposed_checks.first() {
        let _ = turn_repo::set_path_kind(&state.db, &turn_id, "C").await;
        // Kernel owns authoritative character ratings; never let an LLM spoof the player's skill value.
        let authoritative_target = session_state
            .investigator
            .skills
            .get(&check.skill)
            .or_else(|| {
                session_state
                    .investigator
                    .characteristics
                    .get(&check.skill.to_uppercase())
            })
            .copied()
            .map(|v| v.clamp(1, 99) as u32)
            .unwrap_or_else(|| check.target_number.clamp(1, 99));
        let clamped_dice = check.bonus_penalty_dice.clamp(-2, 2);
        let interrupt = PendingInterrupt {
            interrupt_id: format!("int-{}", Uuid::new_v4()),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            status: TurnStatus::AwaitingRoll,
            action_summary: input.clone(),
            skill: check.skill.clone(),
            skill_label: skill_display_label(&check.skill).to_string(),
            difficulty: check.difficulty,
            target_number: authoritative_target,
            bonus_penalty_dice: clamped_dice,
            stakes_hint: "检定结果将直接影响现场进展；掷骰前可撤回或修改行动。".into(),
            allow_push: check.allow_push,
            action_kind: check.action_kind.clone(),
            first_roll: None,
            recovery: None,
            push_description: None,
            push_consequence: None,
        };

        let ev = GameEventKind::InterruptCreated {
            interrupt: interrupt.clone(),
        };
        apply_event(&mut session_state, &ev);
        let seq = event_repo::next_event_seq(&state.db, &session_id)
            .await
            .unwrap_or(1);
        let _ = event_repo::append(
            &state.db,
            &Uuid::new_v4().to_string(),
            &session_id,
            &turn_id,
            seq,
            &ev,
            &session_state.module_version,
            None,
        )
        .await;
        let _ = turn_repo::save_interrupt(&state.db, &interrupt).await;
        let _ = session_repo::cas_snapshot(
            &state.db,
            &session_id,
            version,
            &session_state,
            None,
            Some(Some(&turn_id)),
        )
        .await;
        state.locks.clear_active(&session_id).await;
        let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::AwaitingRoll).await;

        buffer.set_status("awaiting_roll").await;
        buffer.set_pending_interrupt(Some(interrupt.clone())).await;
        let patch =
            build_client_state_patch(&session_state, version + 1, "active", module.as_ref());
        buffer
            .push(
                "interrupt_required",
                json!({ "type": "interrupt_required", "interrupt": interrupt }),
            )
            .await;
        buffer
            .push(
                "state_delta",
                json!({ "type": "state_delta", "patch": patch }),
            )
            .await;
        return;
    }

    // Kernel validation & authoritative state commit
    let _ = turn_repo::set_path_kind(&state.db, &turn_id, "B").await;
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Committing).await;
    buffer.set_status("committing").await;

    let mut next_seq = event_repo::next_event_seq(&state.db, &session_id)
        .await
        .unwrap_or(1);
    let mut ref_event_ids = Vec::new();
    let mut directives_to_emit: Vec<DirectiveDto> = Vec::new();
    let mut directive_seq: i64 = 1;
    let mut session_ended = false;
    let mut claim_kind = "narration";
    let mut extra_narration_notes: Vec<String> = Vec::new();

    // Validate Off-Rail DynamicWorldPatches through Kernel 9-rule validator
    let mut applied_patch_ids: BTreeSet<String> = session_state
        .events
        .iter()
        .filter(|e| e.starts_with("patch:"))
        .cloned()
        .collect();
    for patch in &proposal.proposed_world_patches {
        if validate_patch(patch, &session_state, &applied_patch_ids).is_ok() {
            applied_patch_ids.insert(patch.patch_id.clone());
            let ev_id = Uuid::new_v4().to_string();
            let ev = GameEventKind::WorldPatchApplied {
                patch: patch.clone(),
            };
            apply_event(&mut session_state, &ev);
            let _ = event_repo::append(
                &state.db,
                &ev_id,
                &session_id,
                &turn_id,
                next_seq,
                &ev,
                &session_state.module_version,
                None,
            )
            .await;
            next_seq += 1;
            ref_event_ids.push(ev_id);
        }
    }

    // Build Turn ActionContext for evidence_presented / topic_confronted
    let mut action_ctx = ActionContext {
        present_entity_ids: session_state.current_scene.present_entity_ids.clone(),
        unlocked_clues: session_state.clues.keys().cloned().collect(),
        evidence_presented: BTreeSet::new(),
        topics_confronted: BTreeSet::new(),
        event_ids: session_state.events.clone(),
    };
    for clue_id in &proposal.evidence_to_present {
        if session_state.clues.contains_key(clue_id) {
            action_ctx.evidence_presented.insert(clue_id.clone());
            let ev_id = Uuid::new_v4().to_string();
            let ev = GameEventKind::EvidencePresented {
                clue_id: clue_id.clone(),
            };
            apply_event(&mut session_state, &ev);
            let _ = event_repo::append(
                &state.db,
                &ev_id,
                &session_id,
                &turn_id,
                next_seq,
                &ev,
                &session_state.module_version,
                None,
            )
            .await;
            next_seq += 1;
            ref_event_ids.push(ev_id);
        }
    }
    for topic_id in &proposal.topics_to_confront {
        action_ctx.topics_confronted.insert(topic_id.clone());
        let ev_id = Uuid::new_v4().to_string();
        let ev = GameEventKind::TopicConfronted {
            topic_id: topic_id.clone(),
        };
        apply_event(&mut session_state, &ev);
        let _ = event_repo::append(
            &state.db,
            &ev_id,
            &session_id,
            &turn_id,
            next_seq,
            &ev,
            &session_state.module_version,
            None,
        )
        .await;
        next_seq += 1;
        ref_event_ids.push(ev_id);
    }

    // Apply proposed effects through deterministic checks
    let mut outcome_notes: Vec<String> = Vec::new();
    for eff in &proposal.proposed_effects {
        match eff.effect_type.as_str() {
            "move_scene" => {
                if let (Some(target_id), Some(m)) = (eff.target_id.as_deref(), module.as_ref()) {
                    if let Some(scene_def) = m.find_scene(target_id) {
                        let prev_title = session_state.current_scene.title.clone();
                        let new_scene = SceneState {
                            scene_id: scene_def.scene_id.clone(),
                            title: scene_def.title.clone(),
                            description: scene_def.description.clone(),
                            atmosphere: scene_def.atmosphere.clone(),
                            visited: session_state.current_scene.visited.clone(),
                            present_entity_ids: scene_def
                                .entities
                                .iter()
                                .map(|e| e.entity_id.clone())
                                .collect(),
                        };
                        let ev_id = Uuid::new_v4().to_string();
                        let ev = GameEventKind::SceneChanged { scene: new_scene };
                        apply_event(&mut session_state, &ev);
                        outcome_notes.push(format!(
                            "场景移动：从「{}」来到「{}」（{}）",
                            prev_title, scene_def.title, scene_def.description
                        ));
                        let _ = event_repo::append(
                            &state.db,
                            &ev_id,
                            &session_id,
                            &turn_id,
                            next_seq,
                            &ev,
                            &session_state.module_version,
                            None,
                        )
                        .await;
                        next_seq += 1;
                        ref_event_ids.push(ev_id);

                        if let Ok(d) = event_repo::save_directive(
                            &state.db,
                            &format!("dir-atm-{}-{}", scene_def.scene_id, turn_id),
                            &session_id,
                            &turn_id,
                            1,
                            "kernel_owned",
                            "atmosphere_shift",
                            "immediate",
                            &json!({
                                "theme": scene_def.atmosphere,
                                "sceneTitle": scene_def.title,
                                "note": scene_def.description,
                                "intensity": "subtle",
                            }),
                            Some(directive_seq),
                        )
                        .await
                        {
                            directive_seq += 1;
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
            }
            "player_bluff" => {
                claim_kind = "player_assertion_unverified";
                if let Some(npc_id) = eff.target_id.as_deref() {
                    let tr_id = format!("tr-player-{turn_id}");
                    let _ = event_repo::record_belief(
                        &state.db,
                        &Uuid::new_v4().to_string(),
                        &session_id,
                        npc_id,
                        Some(&tr_id),
                        &input,
                        "suspected",
                        0.45,
                    )
                    .await;
                    let ev_id = Uuid::new_v4().to_string();
                    let ev = GameEventKind::NpcBeliefChanged {
                        entity_id: npc_id.to_string(),
                        claim: input.clone(),
                    };
                    apply_event(&mut session_state, &ev);
                    outcome_notes.push(
                        "诈唬试探：玩家抛出了未经证实的说辞，在场人物神色动摇并产生怀疑，但尚未彻底崩盘。"
                            .into(),
                    );
                    let _ = event_repo::append(
                        &state.db,
                        &ev_id,
                        &session_id,
                        &turn_id,
                        next_seq,
                        &ev,
                        &session_state.module_version,
                        None,
                    )
                    .await;
                    next_seq += 1;
                    ref_event_ids.push(ev_id);
                }
            }
            "unlock_clue" => {
                if let (Some(clue_id), Some(m)) = (eff.target_id.as_deref(), module.as_ref()) {
                    if let Some(note) = unlock_clue_and_handout(
                        &state,
                        &session_id,
                        &turn_id,
                        m,
                        clue_id,
                        &mut session_state,
                        &mut next_seq,
                        &mut directive_seq,
                        &mut ref_event_ids,
                        &mut directives_to_emit,
                    )
                    .await
                    {
                        outcome_notes.push(note);
                    }
                }
            }
            "record_event" => {
                if let Some(ev_name) = eff.target_id.as_deref() {
                    let ev_id = Uuid::new_v4().to_string();
                    let ev = GameEventKind::EventOccurred {
                        event_id: ev_name.to_string(),
                    };
                    apply_event(&mut session_state, &ev);
                    outcome_notes.push(format!("世界变化已生效：{ev_name}"));
                    let _ = event_repo::append(
                        &state.db,
                        &ev_id,
                        &session_id,
                        &turn_id,
                        next_seq,
                        &ev,
                        &session_state.module_version,
                        None,
                    )
                    .await;
                    next_seq += 1;
                    ref_event_ids.push(ev_id);
                }
            }
            "advance_clock" => {
                if let Some(clock_id) = eff.target_id.as_deref() {
                    let amount = eff.value.unwrap_or(1).max(1) as u32;
                    let reason = eff
                        .payload
                        .as_ref()
                        .and_then(|p| p.get("reason"))
                        .and_then(Value::as_str)
                        .unwrap_or("局势随时间推进")
                        .to_string();
                    advance_clock_and_maybe_emit(
                        &state,
                        &session_id,
                        &turn_id,
                        clock_id,
                        amount,
                        &reason,
                        &mut session_state,
                        &mut next_seq,
                        &mut directive_seq,
                        &mut ref_event_ids,
                        &mut directives_to_emit,
                    )
                    .await;
                    // Strictly only narrate PlayerVisible clocks! Never expose KeeperHidden clocks.
                    if let Some(clk) = session_state
                        .clocks
                        .get(clock_id)
                        .filter(|c| c.visibility == ClockVisibility::PlayerVisible)
                    {
                        outcome_notes.push(format!(
                            "可见危机时钟「{}」推进至 {}/{}（{}）",
                            clk.name, clk.current, clk.max, reason
                        ));
                    }
                }
            }
            "end_session" => {
                session_ended = true;
                outcome_notes.push("本局调查已正式进入结案收尾阶段。".into());
                let ev_id = Uuid::new_v4().to_string();
                let ev = GameEventKind::SessionEnded;
                apply_event(&mut session_state, &ev);
                let _ = event_repo::append(
                    &state.db,
                    &ev_id,
                    &session_id,
                    &turn_id,
                    next_seq,
                    &ev,
                    &session_state.module_version,
                    None,
                )
                .await;
                next_seq += 1;
                ref_event_ids.push(ev_id);
            }
            _ => {}
        }
    }

    // Evaluate multi-stage Protected Truths for present NPCs
    if let Some(m) = module.as_ref() {
        for npc in &m.npcs {
            if !session_state
                .current_scene
                .present_entity_ids
                .contains(&npc.entity_id)
            {
                continue;
            }
            for secret in &npc.protected_truths {
                for stage in &secret.stages {
                    if evaluate_module_predicate(
                        &stage.predicate,
                        &session_state,
                        Some(&action_ctx),
                    ) {
                        extra_narration_notes.push(format!(
                            "{}在压力下松口低声吐露：「{}」",
                            npc.name, stage.truth
                        ));
                        for fid in &stage.releasable_fact_ids {
                            let key = format!("player:{fid}");
                            if !session_state.npc_known_facts.contains(&key) {
                                let ev_id = Uuid::new_v4().to_string();
                                let ev = GameEventKind::NpcKnowledgeAcquired {
                                    entity_id: "player".into(),
                                    fact_id: fid.clone(),
                                };
                                apply_event(&mut session_state, &ev);
                                let _ = event_repo::append(
                                    &state.db,
                                    &ev_id,
                                    &session_id,
                                    &turn_id,
                                    next_seq,
                                    &ev,
                                    &session_state.module_version,
                                    None,
                                )
                                .await;
                                let _ = event_repo::record_knowledge_acquisition(
                                    &state.db,
                                    &Uuid::new_v4().to_string(),
                                    &session_id,
                                    "player",
                                    fid,
                                    &ev_id,
                                    "told_by_entity",
                                    next_seq,
                                )
                                .await;
                                next_seq += 1;
                                ref_event_ids.push(ev_id);
                            }
                        }
                    }
                }
            }
        }

        // Auto-unlock conclusions if their `requires` predicate is now satisfied
        let conclusion_ids: Vec<String> = m
            .clues
            .iter()
            .filter(|c| c.is_conclusion && !session_state.clues.contains_key(&c.clue_id))
            .filter(|c| {
                c.requires
                    .as_ref()
                    .map(|req| evaluate_module_predicate(req, &session_state, Some(&action_ctx)))
                    .unwrap_or(false)
            })
            .map(|c| c.clue_id.clone())
            .collect();
        for cid in conclusion_ids {
            if let Some(note) = unlock_clue_and_handout(
                &state,
                &session_id,
                &turn_id,
                m,
                &cid,
                &mut session_state,
                &mut next_seq,
                &mut directive_seq,
                &mut ref_event_ids,
                &mut directives_to_emit,
            )
            .await
            {
                outcome_notes.push(note);
            }
        }
    }
    outcome_notes.extend(extra_narration_notes);

    // Commit authoritative state BEFORE narration stream
    let new_session_status = if session_ended { "ended" } else { "active" };
    session_state.turn_status = Some(TurnStatus::Narrating);
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Narrating).await;
    let _ = session_repo::cas_snapshot(
        &state.db,
        &session_id,
        version,
        &session_state,
        Some(new_session_status),
        Some(Some(&turn_id)),
    )
    .await;

    // Persist and push after_narration directives (frontend holds until narration_complete)
    for d in directives_to_emit {
        buffer.record_directive(d.clone()).await;
        buffer
            .push(
                "ui_directive",
                json!({ "type": "ui_directive", "directive": d }),
            )
            .await;
    }

    // Stream narration AFTER state commit
    let outcome_joined = outcome_notes.join("\n");
    let narration_ctx = NarrationContext {
        history: &history,
        outcome_summary: if outcome_joined.is_empty() {
            None
        } else {
            Some(outcome_joined.as_str())
        },
        is_opening: false,
    };
    let chunks = match state
        .provider
        .stream_narration(&input, &session_state, &narration_ctx)
        .await
    {
        Ok(c) => c,
        Err(err) => {
            emit_narration_pending_error(
                &state,
                &session_id,
                &turn_id,
                &buffer,
                "NARRATION_ERROR",
                &err,
            )
            .await;
            return;
        }
    };

    let mut full_text = String::new();
    for chunk in chunks {
        full_text.push_str(&chunk);
        buffer.append_narration(&chunk).await;
        buffer
            .push(
                "narration_delta",
                json!({ "type": "narration_delta", "chunk": chunk, "turnId": turn_id, "generationId": 1 }),
            )
            .await;
    }

    // Record player assertion transcript separately if Bluff
    if claim_kind == "player_assertion_unverified" {
        let present: Vec<String> = session_state
            .current_scene
            .present_entity_ids
            .iter()
            .cloned()
            .collect();
        let _ = event_repo::insert_transcript(
            &state.db,
            &session_id,
            &TranscriptRecord {
                entry_id: format!("tr-player-{turn_id}"),
                turn_id: turn_id.clone(),
                scene_id: session_state.current_scene.scene_id.clone(),
                speaker_entity_id: Some("player".into()),
                present_entity_ids: present.clone(),
                audible_entity_ids: present.clone(),
                visible_entity_ids: present,
                visibility_scope: "scene".into(),
                claim_kind: "player_assertion_unverified".into(),
                referenced_event_ids: vec![],
                content: input.clone(),
                created_at: Utc::now().to_rfc3339(),
            },
        )
        .await;
    }

    persist_turn_transcript(
        &state,
        &session_id,
        &turn_id,
        &session_state,
        "narration",
        ref_event_ids,
        &full_text,
    )
    .await;

    let _ = session_repo::set_active_turn(&state.db, &session_id, None).await;
    state.locks.clear_active(&session_id).await;
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Completed).await;

    let patch = build_client_state_patch(
        &session_state,
        version + 1,
        new_session_status,
        module.as_ref(),
    );
    buffer
        .push(
            "narration_complete",
            json!({ "type": "narration_complete", "turnId": turn_id, "generationId": 1 }),
        )
        .await;
    buffer
        .push(
            "state_delta",
            json!({ "type": "state_delta", "patch": patch }),
        )
        .await;
    buffer
        .push(
            "turn_complete",
            json!({ "type": "turn_complete", "turnId": turn_id }),
        )
        .await;
    buffer.complete("completed").await;
}

/// Completes a turn after a PendingInterrupt resolves into Committing (pass, fail, Luck spend, or Push).
pub async fn complete_resolved_check_turn(
    state: AppState,
    session_id: String,
    turn_id: String,
    final_roll: RollAttempt,
    was_pushed: bool,
    luck_spent: Option<u32>,
) {
    let buffer = state.buffers.get_or_create(&turn_id);
    buffer.reopen("committing").await;
    buffer.set_pending_interrupt(None).await;

    let Ok(Some((version, mut session_state))) =
        session_repo::load_state(&state.db, &session_id).await
    else {
        buffer.complete("failed").await;
        state.locks.clear_active(&session_id).await;
        return;
    };
    let module = state.modules.get(&session_state.module_id).cloned();

    let mut next_seq = event_repo::next_event_seq(&state.db, &session_id)
        .await
        .unwrap_or(1);
    let mut ref_event_ids = Vec::new();
    let mut directives_to_emit = Vec::new();
    let mut directive_seq: i64 = 10;

    // Immediate inline_dice summary directive
    if let Ok(d) = event_repo::save_directive(
        &state.db,
        &format!("dir-dice-{}-a{}", turn_id, final_roll.attempt_index),
        &session_id,
        &turn_id,
        1,
        "kernel_owned",
        "inline_dice",
        "immediate",
        &serde_json::to_value(&final_roll).unwrap_or(Value::Null),
        Some(directive_seq),
    )
    .await
    {
        directive_seq += 1;
        buffer.record_directive(d.clone()).await;
        buffer
            .push(
                "ui_directive",
                json!({ "type": "ui_directive", "directive": d }),
            )
            .await;
    }

    if let Some(cost) = luck_spent {
        if let Ok(d) = event_repo::save_directive(
            &state.db,
            &format!("dir-luck-{turn_id}"),
            &session_id,
            &turn_id,
            1,
            "kernel_owned",
            "character_state_delta",
            "immediate",
            &json!({
                "deltas": [{ "label": format!("幸运 -{}", cost) }],
                "reason": "消耗幸运改判检定结果",
            }),
            Some(directive_seq),
        )
        .await
        {
            directive_seq += 1;
            buffer.record_directive(d.clone()).await;
            buffer
                .push(
                    "ui_directive",
                    json!({ "type": "ui_directive", "directive": d }),
                )
                .await;
        }
    }

    // Apply outcome effects
    let mut roll_notes: Vec<String> = Vec::new();
    if final_roll.passed {
        if let Some(m) = module.as_ref() {
            let clue_to_unlock = match final_roll.skill.as_str() {
                "psychology" => Some("butler_contradiction_clue"),
                "locksmith" => {
                    if session_state.current_scene.scene_id == "cellar_entrance" {
                        Some("cellar_ledger_clue")
                    } else {
                        Some("bloodstained_cuff_clue")
                    }
                }
                "spot_hidden" => {
                    if session_state.current_scene.scene_id == "rain_courtyard" {
                        Some("courtyard_tracks_clue")
                    } else {
                        Some("manor_clipping_clue")
                    }
                }
                _ => None,
            };
            if let Some(cid) = clue_to_unlock {
                if let Some(note) = unlock_clue_and_handout(
                    &state,
                    &session_id,
                    &turn_id,
                    m,
                    cid,
                    &mut session_state,
                    &mut next_seq,
                    &mut directive_seq,
                    &mut ref_event_ids,
                    &mut directives_to_emit,
                )
                .await
                {
                    roll_notes.push(note);
                }
            }
        }
    } else {
        // Fail Forward: advance visible anomaly clock (and escalated SAN loss if pushed)
        let amount = if was_pushed { 2 } else { 1 };
        let reason = if was_pushed {
            "孤注一掷再次失手，引来庄园深处剧烈异动"
        } else {
            "检定失利拖延了时间，庄园里的异常愈发明显"
        };
        advance_clock_and_maybe_emit(
            &state,
            &session_id,
            &turn_id,
            "manor_anomaly",
            amount,
            reason,
            &mut session_state,
            &mut next_seq,
            &mut directive_seq,
            &mut ref_event_ids,
            &mut directives_to_emit,
        )
        .await;
        if let Some(clk) = session_state.clocks.get("manor_anomaly") {
            roll_notes.push(format!(
                "可见危机时钟「{}」推进至 {}/{}（{}）",
                clk.name, clk.current, clk.max, reason
            ));
        }
        advance_clock_and_maybe_emit(
            &state,
            &session_id,
            &turn_id,
            "ritual_countdown",
            1,
            "调查受阻令幕后仪式推进了一步",
            &mut session_state,
            &mut next_seq,
            &mut directive_seq,
            &mut ref_event_ids,
            &mut directives_to_emit,
        )
        .await;

        if was_pushed {
            session_state.investigator.san = (session_state.investigator.san - 1).max(0);
            roll_notes.push("孤注一掷二次失败带来精神冲击，调查员理智 -1。".into());
            let ev_id = Uuid::new_v4().to_string();
            let ev = GameEventKind::ResourceChanged {
                resource: "san".into(),
                delta: -1,
            };
            let _ = event_repo::append(
                &state.db,
                &ev_id,
                &session_id,
                &turn_id,
                next_seq,
                &ev,
                &session_state.module_version,
                None,
            )
            .await;
            ref_event_ids.push(ev_id);
            if let Ok(d) = event_repo::save_directive(
                &state.db,
                &format!("dir-san-{turn_id}"),
                &session_id,
                &turn_id,
                1,
                "kernel_owned",
                "character_state_delta",
                "after_narration",
                &json!({
                    "deltas": [{ "label": "理智 -1" }],
                    "reason": "孤注一掷失败带来的精神冲击",
                }),
                Some(directive_seq),
            )
            .await
            {
                directives_to_emit.push(d);
            }
        }
    }

    session_state.turn_status = Some(TurnStatus::Narrating);
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Narrating).await;
    let _ = session_repo::cas_snapshot(
        &state.db,
        &session_id,
        version,
        &session_state,
        None,
        Some(Some(&turn_id)),
    )
    .await;

    for d in directives_to_emit {
        buffer.record_directive(d.clone()).await;
        buffer
            .push(
                "ui_directive",
                json!({ "type": "ui_directive", "directive": d }),
            )
            .await;
    }

    let skill_cn = final_roll
        .skill_label
        .as_deref()
        .unwrap_or_else(|| skill_display_label(&final_roll.skill));
    let result_cn = match final_roll.result_level {
        RollResultLevel::CriticalSuccess => "大成功",
        RollResultLevel::ExtremeSuccess => "极难成功",
        RollResultLevel::HardSuccess => "困难成功",
        RollResultLevel::RegularSuccess => "常规成功",
        RollResultLevel::Failure => "失败",
        RollResultLevel::Fumble => "大失败",
    };
    let turn_input = turn_repo::get(&state.db, &turn_id)
        .await
        .ok()
        .flatten()
        .and_then(|t| t.player_input)
        .unwrap_or_else(|| format!("执行{}检定", skill_cn));
    let history = load_recent_history(&state.db, &session_id, 6).await;

    let mut summary_lines = vec![format!(
        "检定已由确定性内核裁定：{}检定掷出 d100 = {}（目标值 {}，结果：{}）",
        skill_cn, final_roll.roll_value, final_roll.target_number, result_cn
    )];
    if let Some(cost) = luck_spent {
        summary_lines.push(format!("玩家消耗 {} 点幸运惊险改判为成功。", cost));
    } else if was_pushed {
        summary_lines.push(if final_roll.passed {
            "玩家顶着升级风险孤注一掷，二次掷骰成功破局。".into()
        } else {
            "玩家孤注一掷二次掷骰仍宣告失手，局势急转直下。".into()
        });
    }
    summary_lines.extend(roll_notes);
    let outcome_summary = summary_lines.join("\n");

    let fallback_prose = if final_roll.passed {
        if let Some(cost) = luck_spent {
            format!(
                "【守秘人】千钧一发之际，你凭直觉（消耗 {} 点幸运）稳住了手脚。{}检定惊险通过，关键破绽终于落入你的眼中。\n\n【抉择】趁着眼前局势刚刚松动，你要如何顺势追击？\n\n【建议行动】\n1. 顺着刚发现的破绽继续追问细节\n2. 检查周围是否还有可相互印证的物证\n3. 暂作记录，转往相邻房间扩大排查",
                cost, skill_cn
            )
        } else if was_pushed {
            format!(
                "【守秘人】顶着升级的风险孤注一掷（掷出 {}），你的{}尝试终于奏效，撬开了此前紧闭的突破口。\n\n【抉择】突破口已经敞开，下一步你打算怎么利用这份收获？\n\n【建议行动】\n1. 趁热打铁继续深挖眼前的线索\n2. 带着刚到手的发现向在场人物当面求证\n3. 立刻转移至下一个关键房间",
                final_roll.roll_value, skill_cn
            )
        } else {
            format!(
                "【守秘人】你的{}检定干脆利落（掷出 {} / 目标 {}，{}），在烛影晃动间捕捉到了决定性的线索。\n\n【抉择】真相的拼图多了一块，接下来你想从哪一步切入？\n\n【建议行动】\n1. 顺着刚捕捉到的细节继续追问\n2. 细查房间内其余可疑物件\n3. 带着线索前往其他房间交叉印证",
                skill_cn, final_roll.roll_value, final_roll.target_number, result_cn
            )
        }
    } else if was_pushed {
        format!(
            "【守秘人】孤注一掷终判失手（掷出 {}）。工具发出的脆响在雨夜长廊里回荡，烛火猛然一暗，庄园深处传来一声低沉的钟鸣。\n\n【抉择】骚动已经惊动了庄园深处，你是咬牙留在原地还是立刻换一条路？\n\n【建议行动】\n1. 稳住心神，试着从另一处线索绕行\n2. 离开当前房间，避免正面暴露\n3. 先向在场人物试探刚才的钟声",
            final_roll.roll_value
        )
    } else {
        format!(
            "【守秘人】这次{}尝试未能如愿（掷出 {}），你在受挫的同时察觉到走廊温度骤降——时间并不站在你这边，或许还有其他绕行之法。\n\n【抉择】正面试探受挫，你要换个切入点还是去别的房间找突破口？\n\n【建议行动】\n1. 观察房间内是否有可替代的明面线索\n2. 换一种温和或迂回的方式继续交涉\n3. 先前往相邻房间寻找新的工具或证词",
            skill_cn, final_roll.roll_value
        )
    };

    let roll_ctx = NarrationContext {
        history: &history,
        outcome_summary: Some(&outcome_summary),
        is_opening: false,
    };
    let chunks = match state
        .provider
        .stream_narration(&turn_input, &session_state, &roll_ctx)
        .await
    {
        Ok(c) if !c.is_empty() => c,
        _ => chunk_text(&fallback_prose),
    };

    let mut prose = String::new();
    for chunk in chunks {
        prose.push_str(&chunk);
        buffer.append_narration(&chunk).await;
        buffer
            .push(
                "narration_delta",
                json!({ "type": "narration_delta", "chunk": chunk, "turnId": turn_id, "generationId": 1 }),
            )
            .await;
    }

    persist_turn_transcript(
        &state,
        &session_id,
        &turn_id,
        &session_state,
        "narration",
        ref_event_ids,
        &prose,
    )
    .await;

    let _ = session_repo::set_active_turn(&state.db, &session_id, None).await;
    state.locks.clear_active(&session_id).await;
    let _ = turn_repo::set_status(&state.db, &turn_id, TurnStatus::Completed).await;

    let patch = build_client_state_patch(&session_state, version + 1, "active", module.as_ref());
    buffer
        .push(
            "narration_complete",
            json!({ "type": "narration_complete", "turnId": turn_id, "generationId": 1 }),
        )
        .await;
    buffer
        .push(
            "state_delta",
            json!({ "type": "state_delta", "patch": patch }),
        )
        .await;
    buffer
        .push(
            "turn_complete",
            json!({ "type": "turn_complete", "turnId": turn_id }),
        )
        .await;
    buffer.complete("completed").await;
}

#[allow(clippy::too_many_arguments)]
async fn unlock_clue_and_handout(
    state: &AppState,
    session_id: &str,
    turn_id: &str,
    module: &ModuleIr,
    clue_id: &str,
    session_state: &mut SessionState,
    next_seq: &mut i64,
    directive_seq: &mut i64,
    ref_event_ids: &mut Vec<String>,
    directives_to_emit: &mut Vec<DirectiveDto>,
) -> Option<String> {
    if session_state.clues.contains_key(clue_id) {
        return None;
    }
    let clue_def = module.find_clue(clue_id)?;
    let clue = ClueState {
        clue_id: clue_def.clue_id.clone(),
        title: clue_def.title.clone(),
        summary: clue_def.summary.clone(),
        source_label: clue_def.source_label.clone(),
        discovered_at_turn_id: turn_id.to_string(),
        is_conclusion: clue_def.is_conclusion,
    };
    let clue_note = format!("新解锁线索「{}」：{}", clue.title, clue.summary);
    let ev_id = Uuid::new_v4().to_string();
    let ev = GameEventKind::ClueUnlocked { clue: clue.clone() };
    apply_event(session_state, &ev);
    let _ = event_repo::append(
        &state.db,
        &ev_id,
        session_id,
        turn_id,
        *next_seq,
        &ev,
        &module.version,
        None,
    )
    .await;
    *next_seq += 1;
    ref_event_ids.push(ev_id);

    if let Ok(d) = event_repo::save_directive(
        &state.db,
        &format!("dir-clue-{clue_id}-{turn_id}"),
        session_id,
        turn_id,
        1,
        "kernel_owned",
        "evidence_unlocked",
        "after_narration",
        &json!({
            "clueId": clue.clue_id,
            "title": clue.title,
            "summary": clue.summary,
            "sourceLabel": clue.source_label,
        }),
        Some(*directive_seq),
    )
    .await
    {
        *directive_seq += 1;
        directives_to_emit.push(d);
    }

    // Release any canonical handout bound to this clue
    for h_def in &module.handouts {
        if h_def.clue_id.as_deref() == Some(clue_id)
            && !session_state.handouts.contains_key(&h_def.handout_id)
        {
            let handout = HandoutState {
                handout_id: h_def.handout_id.clone(),
                kind: h_def.kind.clone(),
                title: h_def.title.clone(),
                canonical: h_def.canonical,
                clue_id: h_def.clue_id.clone(),
                source_facts: h_def.source_facts.clone(),
                presentation_text: h_def.presentation_text.clone(),
                unlocked_at_turn_id: turn_id.to_string(),
            };
            let h_ev_id = Uuid::new_v4().to_string();
            let h_ev = GameEventKind::HandoutReleased {
                handout: handout.clone(),
            };
            apply_event(session_state, &h_ev);
            let _ = event_repo::append(
                &state.db,
                &h_ev_id,
                session_id,
                turn_id,
                *next_seq,
                &h_ev,
                &module.version,
                None,
            )
            .await;
            *next_seq += 1;
            ref_event_ids.push(h_ev_id);

            if let Ok(d) = event_repo::save_directive(
                &state.db,
                &format!("dir-handout-{}-{turn_id}", handout.handout_id),
                session_id,
                turn_id,
                1,
                "kernel_owned",
                "diegetic_handout",
                "after_narration",
                &json!({
                    "handoutId": handout.handout_id,
                    "kind": handout.kind,
                    "title": handout.title,
                    "canonical": handout.canonical,
                    "clueId": handout.clue_id,
                    "sourceFacts": handout.source_facts,
                    "presentationText": handout.presentation_text,
                }),
                Some(*directive_seq),
            )
            .await
            {
                *directive_seq += 1;
                directives_to_emit.push(d);
            }
        }
    }
    Some(clue_note)
}

#[allow(clippy::too_many_arguments)]
async fn advance_clock_and_maybe_emit(
    state: &AppState,
    session_id: &str,
    turn_id: &str,
    clock_id: &str,
    amount: u32,
    reason: &str,
    session_state: &mut SessionState,
    next_seq: &mut i64,
    directive_seq: &mut i64,
    ref_event_ids: &mut Vec<String>,
    directives_to_emit: &mut Vec<DirectiveDto>,
) {
    let Some(prev) = session_state.clocks.get(clock_id).cloned() else {
        return;
    };
    let ev_id = Uuid::new_v4().to_string();
    let ev = GameEventKind::ClockAdvanced {
        clock_id: clock_id.to_string(),
        amount,
        reason: reason.to_string(),
        caused_by: turn_id.to_string(),
    };
    apply_event(session_state, &ev);
    let _ = event_repo::append(
        &state.db,
        &ev_id,
        session_id,
        turn_id,
        *next_seq,
        &ev,
        &session_state.module_version,
        Some(turn_id),
    )
    .await;
    *next_seq += 1;
    ref_event_ids.push(ev_id);

    // Strictly emit `clock_pulse` ONLY for `PlayerVisible` clocks; NEVER expose `KeeperHidden`!
    if let Some(updated) = session_state.clocks.get(clock_id) {
        if updated.visibility == ClockVisibility::PlayerVisible {
            if let Ok(d) = event_repo::save_directive(
                &state.db,
                &format!("dir-clock-{clock_id}-{turn_id}"),
                session_id,
                turn_id,
                1,
                "kernel_owned",
                "clock_pulse",
                "after_narration",
                &json!({
                    "clockId": updated.clock_id,
                    "name": updated.name,
                    "previousValue": prev.current,
                    "currentValue": updated.current,
                    "maxValue": updated.max,
                    "stageLabel": current_stage_label(updated),
                    "reason": reason,
                }),
                Some(*directive_seq),
            )
            .await
            {
                *directive_seq += 1;
                directives_to_emit.push(d);
            }
        }
    }
}

fn is_low_risk_path_a(input: &str) -> bool {
    let trimmed = input.trim();
    let read_only_markers = ["看看周围", "观察周围", "环顾四周", "听听雨声", "再看看"];
    let state_change_markers = [
        "前往",
        "去",
        "回到",
        "返回",
        "调查",
        "查看",
        "检查",
        "撬",
        "开锁",
        "微表情",
        "心理学",
        "侦查",
        "出示",
        "对质",
        "质问",
        "铁铲",
        "后窗",
        "结案",
        "揭露",
        "诈唬",
        "已经把一切告诉我",
        "询问",
        "翻看",
        "托盘",
        "袖口",
        "账本",
        "剪报",
    ];
    read_only_markers.iter().any(|m| trimmed.contains(m))
        && !state_change_markers.iter().any(|m| trimmed.contains(m))
}

async fn persist_turn_transcript(
    state: &AppState,
    session_id: &str,
    turn_id: &str,
    session_state: &SessionState,
    claim_kind: &str,
    ref_event_ids: Vec<String>,
    content: &str,
) {
    let present: Vec<String> = session_state
        .current_scene
        .present_entity_ids
        .iter()
        .cloned()
        .collect();
    let _ = event_repo::insert_transcript(
        &state.db,
        session_id,
        &TranscriptRecord {
            entry_id: format!("tr-{turn_id}"),
            turn_id: turn_id.to_string(),
            scene_id: session_state.current_scene.scene_id.clone(),
            speaker_entity_id: None,
            present_entity_ids: present.clone(),
            audible_entity_ids: present.clone(),
            visible_entity_ids: present,
            visibility_scope: "public".into(),
            claim_kind: claim_kind.into(),
            referenced_event_ids: ref_event_ids,
            content: content.to_string(),
            created_at: Utc::now().to_rfc3339(),
        },
    )
    .await;
}

async fn emit_narration_pending_error(
    state: &AppState,
    session_id: &str,
    turn_id: &str,
    buffer: &crate::stream::buffer::ActiveTurnBuffer,
    code: &str,
    message: &str,
) {
    // Authoritative state is already committed; preserve `narration_pending` and `active_turn_id`
    // so lazy SSE RecoveryLease can complete narration without re-rolling dice or re-mutating state.
    state.locks.clear_active(session_id).await;
    let _ = turn_repo::set_status(&state.db, turn_id, TurnStatus::NarrationPending).await;
    buffer
        .push(
            "turn_error",
            json!({
                "type": "turn_error",
                "turnId": turn_id,
                "code": code,
                "message": message,
            }),
        )
        .await;
    buffer.complete("narration_pending").await;
}

async fn emit_turn_error(
    state: &AppState,
    session_id: &str,
    turn_id: &str,
    buffer: &crate::stream::buffer::ActiveTurnBuffer,
    code: &str,
    message: &str,
) {
    let _ = session_repo::set_active_turn(&state.db, session_id, None).await;
    state.locks.clear_active(session_id).await;
    let _ = turn_repo::set_status(&state.db, turn_id, TurnStatus::Failed).await;
    buffer
        .push(
            "turn_error",
            json!({
                "type": "turn_error",
                "turnId": turn_id,
                "code": code,
                "message": message,
            }),
        )
        .await;
    buffer.complete("failed").await;
}

fn chunk_text(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![];
    }
    chars
        .chunks(24)
        .map(|slice| slice.iter().collect::<String>())
        .collect()
}
