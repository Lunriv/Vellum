use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::{
    validate_module, AcquisitionPath, AlwaysKeyword, ClockDefinition, ClockStage, ClueDefinition,
    EndingCondition, HandoutDefinition, InvestigatorPreset, ModuleError, ModuleIr, NpcDefinition,
    Predicate, ProtectedTruth, ReviewFact, SceneDefinition, SceneEntity, SceneExit, SkillStrength,
    StartingItem, TruthStage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldGenRequest {
    pub prompt: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub scene_count: Option<u8>,
}

fn slugify_id(raw: &str, prefix: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us && !out.is_empty() {
            out.push('_');
            prev_us = true;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.len() >= 3 {
        trimmed
    } else {
        format!("{prefix}_{}", &Uuid::new_v4().simple().to_string()[..8])
    }
}

fn normalize_atmosphere(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "rain_manor" | "candlelight" | "fog" | "san_strain" => raw.trim().to_lowercase(),
        s if s.contains("fog") || s.contains("snow") || s.contains("mist") => "fog".into(),
        s if s.contains("candle") || s.contains("warm") || s.contains("indoor") => {
            "candlelight".into()
        }
        s if s.contains("danger") || s.contains("blood") || s.contains("strain") => {
            "san_strain".into()
        }
        _ => "rain_manor".into(),
    }
}

fn filter_predicate(
    pred: Predicate,
    clue_ids: &BTreeSet<String>,
    clock_ids: &BTreeSet<String>,
) -> Option<Predicate> {
    match pred {
        Predicate::Always(a) => Some(Predicate::Always(a)),
        Predicate::All { all } => {
            let kept: Vec<Predicate> = all
                .into_iter()
                .filter_map(|p| filter_predicate(p, clue_ids, clock_ids))
                .collect();
            if kept.is_empty() {
                None
            } else {
                Some(Predicate::All { all: kept })
            }
        }
        Predicate::Any { any } => {
            let kept: Vec<Predicate> = any
                .into_iter()
                .filter_map(|p| filter_predicate(p, clue_ids, clock_ids))
                .collect();
            if kept.is_empty() {
                None
            } else {
                Some(Predicate::Any { any: kept })
            }
        }
        Predicate::Not { not } => {
            filter_predicate(*not, clue_ids, clock_ids).map(|inner| Predicate::Not {
                not: Box::new(inner),
            })
        }
        Predicate::ClueDiscovered { clue_discovered } => {
            if clue_ids.contains(&clue_discovered) {
                Some(Predicate::ClueDiscovered { clue_discovered })
            } else {
                None
            }
        }
        Predicate::EvidencePresented { evidence_presented } => {
            if clue_ids.contains(&evidence_presented) {
                Some(Predicate::EvidencePresented { evidence_presented })
            } else {
                None
            }
        }
        Predicate::ClockAtLeast { clock_at_least } => {
            if clock_ids.contains(&clock_at_least.clock_id) {
                Some(Predicate::ClockAtLeast { clock_at_least })
            } else {
                None
            }
        }
        other => Some(other),
    }
}

fn collect_topics_from_predicate(pred: &Predicate, out: &mut BTreeSet<String>) {
    match pred {
        Predicate::TopicConfronted { topic_confronted } => {
            if !topic_confronted.trim().is_empty() {
                out.insert(topic_confronted.clone());
            }
        }
        Predicate::All { all } => {
            for p in all {
                collect_topics_from_predicate(p, out);
            }
        }
        Predicate::Any { any } => {
            for p in any {
                collect_topics_from_predicate(p, out);
            }
        }
        Predicate::Not { not } => collect_topics_from_predicate(not, out),
        _ => {}
    }
}

/// Extracts all valid confrontation topic IDs declared by a module's NPC predicates.
pub fn extract_module_topics(module: &ModuleIr) -> Vec<String> {
    let mut topics = BTreeSet::new();
    for npc in &module.npcs {
        for secret in &npc.protected_truths {
            for stage in &secret.stages {
                collect_topics_from_predicate(&stage.predicate, &mut topics);
            }
        }
    }
    topics.into_iter().collect()
}

/// Normalizes minor LLM/import omissions and runs the authoritative static `validate_module` gate.
pub fn sanitize_and_validate_module(
    mut module: ModuleIr,
    forced_module_id: Option<&str>,
) -> Result<ModuleIr, ModuleError> {
    if let Some(id) = forced_module_id {
        module.module_id = slugify_id(id, "world");
    } else if module.module_id.trim().is_empty() {
        module.module_id = slugify_id(&module.title, "world");
    } else {
        module.module_id = slugify_id(&module.module_id, "world");
    }

    if module.version.trim().is_empty() {
        module.version = "1.0.0".into();
    }
    if module.title.trim().is_empty() {
        module.title = "未命名调查世界".into();
    }
    if module.opening_prologue.trim().is_empty() {
        module.opening_prologue = format!("你踏入了「{}」，异样气息正在暗处蔓延。", module.title);
    }

    if module.scenes.is_empty() {
        return Err(ModuleError::Validation(
            "world must contain at least one scene".into(),
        ));
    }

    // Deduplicate scene IDs and normalize atmospheres
    let mut seen_scenes = BTreeSet::new();
    for (idx, scene) in module.scenes.iter_mut().enumerate() {
        let base = if scene.scene_id.trim().is_empty() {
            format!("scene_{}", idx + 1)
        } else {
            slugify_id(&scene.scene_id, "scene")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_scenes.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        scene.scene_id = candidate;
        scene.atmosphere = normalize_atmosphere(&scene.atmosphere);
        if scene.title.trim().is_empty() {
            scene.title = format!("区域 {}", idx + 1);
        }
        if scene.description.trim().is_empty() {
            scene.description = format!("{}里安静得反常。", scene.title);
        }
    }

    if !seen_scenes.contains(&module.initial_scene_id) {
        module.initial_scene_id = module.scenes[0].scene_id.clone();
    }
    let initial_id = module.initial_scene_id.clone();

    // Ensure investigators are valid and unique
    if module.investigators.is_empty() {
        module.investigators.push(default_investigator("inv_lead"));
    }
    let mut seen_invs = BTreeSet::new();
    for (idx, inv) in module.investigators.iter_mut().enumerate() {
        let base = if inv.investigator_id.trim().is_empty() {
            format!("inv_{}", idx + 1)
        } else {
            slugify_id(&inv.investigator_id, "inv")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_invs.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        inv.investigator_id = candidate;
        if inv.name.trim().is_empty() {
            inv.name = format!("调查员 {}", idx + 1);
        }
        if inv.occupation.trim().is_empty() {
            inv.occupation = "自由调查员".into();
        }
        if inv.hp_max <= 0 {
            inv.hp_max = 10;
        }
        if inv.mp_max <= 0 {
            inv.mp_max = 10;
        }
        if inv.san_start <= 0 {
            inv.san_start = 55;
        }
        if inv.luck_start <= 0 {
            inv.luck_start = 50;
        }
        if inv.skills.is_empty() {
            inv.skills.insert("spot_hidden".into(), 65);
            inv.skills.insert("psychology".into(), 55);
            inv.skills.insert("persuade".into(), 50);
            inv.skills.insert("locksmith".into(), 35);
            inv.skills.insert("dodge".into(), 40);
        }
        if inv.characteristics.is_empty() {
            inv.characteristics = BTreeMap::from([
                ("STR".into(), 55),
                ("CON".into(), 55),
                ("SIZ".into(), 55),
                ("DEX".into(), 60),
                ("APP".into(), 50),
                ("INT".into(), 70),
                ("POW".into(), 55),
                ("EDU".into(), 65),
            ]);
        }
        if inv.strengths.is_empty() {
            let mut top: Vec<(String, i32)> =
                inv.skills.iter().map(|(k, v)| (k.clone(), *v)).collect();
            top.sort_by_key(|a| std::cmp::Reverse(a.1));
            for (skill, rating) in top.into_iter().take(3) {
                inv.strengths.push(SkillStrength {
                    label: crate::kernel::skill_display_label(&skill).to_string(),
                    skill,
                    rating,
                });
            }
        }
    }

    // Ensure at least one visible and one hidden clock
    let mut seen_clocks = BTreeSet::new();
    for (idx, clk) in module.clocks.iter_mut().enumerate() {
        let base = if clk.clock_id.trim().is_empty() {
            format!("clock_{}", idx + 1)
        } else {
            slugify_id(&clk.clock_id, "clock")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_clocks.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        clk.clock_id = candidate;
        if clk.visibility != "keeper_hidden" {
            clk.visibility = "player_visible".into();
        }
        if clk.max < 2 {
            clk.max = 6;
        }
        if clk.stages.is_empty() {
            clk.stages = vec![
                ClockStage {
                    at: 0,
                    label: "局势初起".into(),
                },
                ClockStage {
                    at: clk.max / 2,
                    label: "危机升级".into(),
                },
                ClockStage {
                    at: clk.max,
                    label: "事态失控".into(),
                },
            ];
        }
    }
    if !module
        .clocks
        .iter()
        .any(|c| c.visibility == "player_visible")
    {
        let id = "crisis_escalation".to_string();
        seen_clocks.insert(id.clone());
        module.clocks.push(ClockDefinition {
            clock_id: id,
            name: "局势恶化".into(),
            visibility: "player_visible".into(),
            max: 6,
            stages: vec![
                ClockStage {
                    at: 0,
                    label: "尚可掌控".into(),
                },
                ClockStage {
                    at: 3,
                    label: "暗流涌动".into(),
                },
                ClockStage {
                    at: 6,
                    label: "全面失控".into(),
                },
            ],
        });
    }
    if !module
        .clocks
        .iter()
        .any(|c| c.visibility == "keeper_hidden")
    {
        let id = "hidden_countdown".to_string();
        seen_clocks.insert(id.clone());
        module.clocks.push(ClockDefinition {
            clock_id: id,
            name: "幕后倒计时".into(),
            visibility: "keeper_hidden".into(),
            max: 6,
            stages: vec![
                ClockStage {
                    at: 0,
                    label: "尚未察觉".into(),
                },
                ClockStage {
                    at: 3,
                    label: "计划逼近收尾".into(),
                },
                ClockStage {
                    at: 6,
                    label: "幕后图谋达成".into(),
                },
            ],
        });
    }

    // Deduplicate clues
    let mut seen_clues = BTreeSet::new();
    for (idx, clue) in module.clues.iter_mut().enumerate() {
        let base = if clue.clue_id.trim().is_empty() {
            format!("clue_{}", idx + 1)
        } else {
            slugify_id(&clue.clue_id, "clue")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_clues.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        clue.clue_id = candidate;
        clue.scene_ids.retain(|sid| seen_scenes.contains(sid));
        if clue.scene_ids.is_empty() {
            clue.scene_ids.push(initial_id.clone());
        }
    }
    if module.clues.is_empty() {
        let starter_id = "starter_lead".to_string();
        seen_clues.insert(starter_id.clone());
        module.clues.push(ClueDefinition {
            clue_id: starter_id,
            title: "初始异状".into(),
            summary: format!("将你卷入「{}」的第一条异常线索。", module.title),
            source_label: "开场现场".into(),
            is_conclusion: false,
            requires: None,
            acquisition_paths: vec![AcquisitionPath {
                path_id: "start_always".into(),
                description: "开场自带线索".into(),
                condition: Predicate::Always(AlwaysKeyword::Always),
            }],
            scene_ids: vec![initial_id.clone()],
        });
    }

    // Clean clue predicates and ensure conclusions satisfy the anti-softlock rules.
    // Also ensure at most 1 starter clue is unconditionally `Always` in the initial scene so
    // Turn 0 never spoils the entire mystery at once.
    let fallback_skills = [
        "spot_hidden",
        "psychology",
        "locksmith",
        "persuade",
        "science",
        "medicine",
    ];
    let mut starter_always_assigned = false;
    for (c_idx, clue) in module.clues.iter_mut().enumerate() {
        clue.requires = clue
            .requires
            .take()
            .and_then(|p| filter_predicate(p, &seen_clues, &seen_clocks));
        for path in &mut clue.acquisition_paths {
            if let Some(clean) = filter_predicate(path.condition.clone(), &seen_clues, &seen_clocks)
            {
                path.condition = clean;
            } else {
                let skill = fallback_skills[c_idx % fallback_skills.len()];
                path.condition = Predicate::SkillSuccess {
                    skill_success: skill.into(),
                };
            }
            if matches!(path.condition, Predicate::Always(_)) {
                let in_initial = clue.scene_ids.iter().any(|sid| sid == &initial_id);
                if in_initial && !starter_always_assigned {
                    starter_always_assigned = true;
                } else {
                    let skill = fallback_skills[c_idx % fallback_skills.len()];
                    path.condition = Predicate::SkillSuccess {
                        skill_success: skill.into(),
                    };
                }
            }
        }
        if clue.acquisition_paths.is_empty() {
            let cond = if !starter_always_assigned
                && clue.scene_ids.iter().any(|sid| sid == &initial_id)
            {
                starter_always_assigned = true;
                Predicate::Always(AlwaysKeyword::Always)
            } else {
                let skill = fallback_skills[c_idx % fallback_skills.len()];
                Predicate::SkillSuccess {
                    skill_success: skill.into(),
                }
            };
            clue.acquisition_paths.push(AcquisitionPath {
                path_id: format!("path_{}", c_idx + 1),
                description: "现场调查获取".into(),
                condition: cond,
            });
        }
        // Prevent self-cycle or invalid single-skill conclusion soft-lock
        if clue.is_conclusion {
            let has_single_skill = clue
                .acquisition_paths
                .iter()
                .any(|p| matches!(p.condition, Predicate::SkillSuccess { .. }));
            if clue.acquisition_paths.len() < 2 || has_single_skill {
                clue.is_conclusion = false;
            }
        }
    }

    // Deduplicate NPCs and auto-create definitions for any scene NPC missing from module.npcs
    let mut seen_npcs = BTreeSet::new();
    for (idx, npc) in module.npcs.iter_mut().enumerate() {
        let base = if npc.entity_id.trim().is_empty() {
            format!("npc_{}", idx + 1)
        } else {
            slugify_id(&npc.entity_id, "npc")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_npcs.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        npc.entity_id = candidate;
        if npc.name.trim().is_empty() {
            npc.name = format!("在场者 {}", idx + 1);
        }
        if npc.personality.trim().is_empty() {
            npc.personality = "谨慎、戒备，话里有所保留。".into();
        }
        if npc.cover_story.trim().is_empty() {
            npc.cover_story = "这里一切正常，你最好别多管闲事。".into();
        }
    }

    // Clean scene exits & entities
    let mut missing_npcs: Vec<(String, String)> = Vec::new();
    for scene in &mut module.scenes {
        let self_id = scene.scene_id.clone();
        let mut seen_targets = BTreeSet::new();
        scene.exits.retain_mut(|exit| {
            if exit.target_scene_id == self_id || !seen_scenes.contains(&exit.target_scene_id) {
                return false;
            }
            if !seen_targets.insert(exit.target_scene_id.clone()) {
                return false;
            }
            if exit.label.trim().is_empty() {
                exit.label = format!("前往{}", exit.target_scene_id);
            }
            exit.condition = exit
                .condition
                .take()
                .and_then(|p| filter_predicate(p, &seen_clues, &seen_clocks));
            true
        });

        for (eidx, ent) in scene.entities.iter_mut().enumerate() {
            if ent.entity_id.trim().is_empty() {
                ent.entity_id = format!("{}_ent_{}", self_id, eidx + 1);
            } else {
                ent.entity_id = slugify_id(&ent.entity_id, "ent");
            }
            if ent.kind != "npc" {
                ent.kind = "object".into();
            } else if !seen_npcs.contains(&ent.entity_id) {
                seen_npcs.insert(ent.entity_id.clone());
                missing_npcs.push((ent.entity_id.clone(), ent.name.clone()));
            }
            if ent.role.trim().is_empty() {
                ent.role = if ent.kind == "npc" {
                    "witness".into()
                } else {
                    "object".into()
                };
            }
            ent.clue_ids.retain(|cid| seen_clues.contains(cid));
        }
    }

    // Ensure every scene has at least one exit when there are multiple scenes
    if module.scenes.len() > 1 {
        let titles: BTreeMap<String, String> = module
            .scenes
            .iter()
            .map(|s| (s.scene_id.clone(), s.title.clone()))
            .collect();
        let second_id = module
            .scenes
            .iter()
            .find(|s| s.scene_id != initial_id)
            .map(|s| s.scene_id.clone())
            .unwrap_or_else(|| initial_id.clone());
        for scene in &mut module.scenes {
            if scene.exits.is_empty() {
                let target = if scene.scene_id == initial_id {
                    second_id.clone()
                } else {
                    initial_id.clone()
                };
                let label = format!(
                    "前往{}",
                    titles.get(&target).cloned().unwrap_or(target.clone())
                );
                scene.exits.push(SceneExit {
                    target_scene_id: target,
                    label,
                    bidirectional: true,
                    condition: None,
                });
            }
        }
    }

    for (entity_id, name) in missing_npcs {
        module.npcs.push(NpcDefinition {
            entity_id,
            name: if name.trim().is_empty() {
                "神秘目击者".into()
            } else {
                name
            },
            personality: "神情紧绷、说话留有余地。".into(),
            working_knowledge: vec!["熟悉当前场所的日常动向。".into()],
            cover_story: "刚才只是意外，并没有你想得那么复杂。".into(),
            behavioral_tells: vec!["被问到核心疑点时会下意识移开视线。".into()],
            protected_truths: vec![],
            initial_relations: vec![],
        });
    }

    // Clean handouts
    let mut seen_handouts = BTreeSet::new();
    for (idx, h) in module.handouts.iter_mut().enumerate() {
        let base = if h.handout_id.trim().is_empty() {
            format!("handout_{}", idx + 1)
        } else {
            slugify_id(&h.handout_id, "handout")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_handouts.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        h.handout_id = candidate;
        if !matches!(
            h.kind.as_str(),
            "letter" | "diary" | "newspaper" | "telegram" | "report" | "note"
        ) {
            h.kind = "note".into();
        }
        if let Some(cid) = &h.clue_id {
            if !seen_clues.contains(cid) {
                h.clue_id = None;
            }
        }
        h.unlock_condition = h
            .unlock_condition
            .take()
            .and_then(|p| filter_predicate(p, &seen_clues, &seen_clocks));
    }

    // Clean review facts
    let mut seen_facts = BTreeSet::new();
    for (idx, fact) in module.review_facts.iter_mut().enumerate() {
        let base = if fact.fact_id.trim().is_empty() {
            format!("fact_{}", idx + 1)
        } else {
            slugify_id(&fact.fact_id, "fact")
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while !seen_facts.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        fact.fact_id = candidate;
        fact.source_clue_ids.retain(|cid| seen_clues.contains(cid));
    }
    if module.review_facts.is_empty() {
        let default_fact_id = "fact_core".to_string();
        seen_facts.insert(default_fact_id.clone());
        module.review_facts.push(ReviewFact {
            fact_id: default_fact_id,
            title: "核心真相".into(),
            detail: format!("围绕「{}」的真正幕后隐情。", module.title),
            source_clue_ids: module
                .clues
                .first()
                .map(|c| vec![c.clue_id.clone()])
                .unwrap_or_default(),
        });
    }

    // Clean NPC protected_truths predicates & fact references
    for npc in &mut module.npcs {
        for secret in &mut npc.protected_truths {
            for stage in &mut secret.stages {
                if let Some(clean) =
                    filter_predicate(stage.predicate.clone(), &seen_clues, &seen_clocks)
                {
                    stage.predicate = clean;
                } else {
                    stage.predicate = Predicate::Always(AlwaysKeyword::Always);
                }
                stage.releasable_fact_ids.retain(|f| seen_facts.contains(f));
            }
        }
    }

    // Clean endings
    for ending in &mut module.endings {
        if let Some(clean) = filter_predicate(ending.condition.clone(), &seen_clues, &seen_clocks) {
            ending.condition = clean;
        } else {
            ending.condition = Predicate::Always(AlwaysKeyword::Always);
        }
    }
    if module.endings.is_empty() {
        module.endings.push(EndingCondition {
            ending_id: "truth_exposed".into(),
            title: "真相浮出水面".into(),
            summary: format!("你在危机失控前查清了「{}」的幕后隐情。", module.title),
            condition: module
                .clues
                .last()
                .map(|c| Predicate::ClueDiscovered {
                    clue_discovered: c.clue_id.clone(),
                })
                .unwrap_or(Predicate::Always(AlwaysKeyword::Always)),
        });
    }

    // Break any accidental clue dependency cycle if LLM generated one
    if validate_module(&module).is_err() {
        for clue in &mut module.clues {
            clue.requires = None;
            clue.is_conclusion = false;
        }
    }

    validate_module(&module)?;
    Ok(module)
}

fn default_investigator(id: &str) -> InvestigatorPreset {
    InvestigatorPreset {
        investigator_id: id.into(),
        name: "陈默".into(),
        occupation: "独立调查员".into(),
        tagline: "不信巧合，只信现场留下的痕迹".into(),
        biography: "常年承接棘手委托，擅长拆解矛盾口供与环境异常。".into(),
        motivation: "查明事件背后的真实动机并全身而退。".into(),
        manor_connection: "收到了一封指向此地的匿名求助信。".into(),
        hp_max: 11,
        mp_max: 12,
        san_start: 55,
        luck_start: 50,
        strengths: vec![
            SkillStrength {
                skill: "spot_hidden".into(),
                label: "侦查".into(),
                rating: 70,
            },
            SkillStrength {
                skill: "psychology".into(),
                label: "心理学".into(),
                rating: 65,
            },
            SkillStrength {
                skill: "locksmith".into(),
                label: "锁匠".into(),
                rating: 50,
            },
        ],
        starting_items: vec![
            StartingItem {
                item_id: "pocket_flashlight".into(),
                name: "手电筒".into(),
                description: "电池充足的金属手电。".into(),
            },
            StartingItem {
                item_id: "field_notebook".into(),
                name: "调查笔记".into(),
                description: "记录口供与时间线的便携本。".into(),
            },
        ],
        characteristics: BTreeMap::from([
            ("STR".into(), 55),
            ("CON".into(), 60),
            ("SIZ".into(), 55),
            ("DEX".into(), 65),
            ("APP".into(), 50),
            ("INT".into(), 75),
            ("POW".into(), 55),
            ("EDU".into(), 70),
        ]),
        skills: BTreeMap::from([
            ("spot_hidden".into(), 70),
            ("psychology".into(), 65),
            ("locksmith".into(), 50),
            ("persuade".into(), 55),
            ("dodge".into(), 40),
        ]),
    }
}

/// Deterministic mock world builder for offline/test runs.
/// Produces a valid, non-manor 4-scene sandbox directly seeded from the user's prompt.
pub fn build_mock_generated_world(req: &WorldGenRequest) -> ModuleIr {
    let prompt_trim = req.prompt.trim();
    let title = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let short: String = prompt_trim.chars().take(12).collect();
            if short.is_empty() {
                "自定义调查沙盒".into()
            } else {
                short
            }
        });
    let mod_id = format!("gen_{}", &Uuid::new_v4().simple().to_string()[..8]);

    let ir = ModuleIr {
        module_id: mod_id,
        version: "1.0.0".into(),
        title: title.clone(),
        opening_prologue: format!(
            "围绕「{}」的异常正在迅速发酵。你刚踏入现场入口，空气里已经弥漫着不寻常的紧绷感。",
            if prompt_trim.is_empty() {
                title.as_str()
            } else {
                prompt_trim
            }
        ),
        investigators: vec![
            default_investigator("inv_scout"),
            InvestigatorPreset {
                investigator_id: "inv_fixer".into(),
                name: "苏晚".into(),
                occupation: "现场顾问".into(),
                tagline: "擅长在慌乱人群里撬开真话".into(),
                biography: "处理过多次危机善后，习惯同时盯住规则漏洞与人情弱点。".into(),
                motivation: "在局势彻底失控前把幕后黑手逼出水面。".into(),
                manor_connection: "知情线人在失联前把坐标发给了她。".into(),
                hp_max: 10,
                mp_max: 13,
                san_start: 60,
                luck_start: 55,
                strengths: vec![
                    SkillStrength {
                        skill: "persuade".into(),
                        label: "说服".into(),
                        rating: 70,
                    },
                    SkillStrength {
                        skill: "psychology".into(),
                        label: "心理学".into(),
                        rating: 65,
                    },
                    SkillStrength {
                        skill: "medicine".into(),
                        label: "医学".into(),
                        rating: 55,
                    },
                ],
                starting_items: vec![StartingItem {
                    item_id: "tape_recorder".into(),
                    name: "便携录音笔".into(),
                    description: "可用于记录关键口供或制造施压筹码。".into(),
                }],
                characteristics: BTreeMap::from([
                    ("STR".into(), 45),
                    ("CON".into(), 50),
                    ("SIZ".into(), 50),
                    ("DEX".into(), 60),
                    ("APP".into(), 65),
                    ("INT".into(), 75),
                    ("POW".into(), 60),
                    ("EDU".into(), 75),
                ]),
                skills: BTreeMap::from([
                    ("persuade".into(), 70),
                    ("psychology".into(), 65),
                    ("medicine".into(), 55),
                    ("spot_hidden".into(), 55),
                    ("locksmith".into(), 30),
                ]),
            },
        ],
        initial_scene_id: "hub_lobby".into(),
        scenes: vec![
            SceneDefinition {
                scene_id: "hub_lobby".into(),
                title: "接待大厅".into(),
                atmosphere: "fog".into(),
                description: "灯光忽明忽暗，值班台后的登记簿还摊开在昨晚那一页。".into(),
                tags: vec!["hub".into(), "social".into()],
                exits: vec![
                    SceneExit {
                        target_scene_id: "records_office".into(),
                        label: "前往档案室".into(),
                        bidirectional: true,
                        condition: None,
                    },
                    SceneExit {
                        target_scene_id: "service_corridor".into(),
                        label: "前往后勤通道".into(),
                        bidirectional: true,
                        condition: None,
                    },
                    SceneExit {
                        target_scene_id: "restricted_vault".into(),
                        label: "前往封锁禁区".into(),
                        bidirectional: true,
                        condition: Some(Predicate::ClueDiscovered {
                            clue_discovered: "clue_entry_log".into(),
                        }),
                    },
                ],
                entities: vec![
                    SceneEntity {
                        entity_id: "npc_warden".into(),
                        name: "值班主管老周".into(),
                        role: "gatekeeper".into(),
                        kind: "npc".into(),
                        summary: "攥着一串钥匙，反复确认大门是否反锁。".into(),
                        clue_ids: vec!["clue_warden_slip".into()],
                    },
                    SceneEntity {
                        entity_id: "obj_register".into(),
                        name: "值班登记簿".into(),
                        role: "object".into(),
                        kind: "object".into(),
                        summary: "纸页边缘被撕掉半角，留有异常涂改。".into(),
                        clue_ids: vec!["clue_entry_log".into()],
                    },
                ],
            },
            SceneDefinition {
                scene_id: "records_office".into(),
                title: "档案室".into(),
                atmosphere: "candlelight".into(),
                description: "成排铁皮柜上落满灰尘，唯有角落一只带锁抽屉最近被拉动过。".into(),
                tags: vec!["archive".into(), "investigation".into()],
                exits: vec![SceneExit {
                    target_scene_id: "hub_lobby".into(),
                    label: "返回接待大厅".into(),
                    bidirectional: true,
                    condition: None,
                }],
                entities: vec![SceneEntity {
                    entity_id: "obj_locked_cabinet".into(),
                    name: "带锁铁皮柜".into(),
                    role: "object".into(),
                    kind: "object".into(),
                    summary: "锁孔周围有新鲜刮痕，里面似乎塞着隐藏记录。".into(),
                    clue_ids: vec!["clue_hidden_dossier".into()],
                }],
            },
            SceneDefinition {
                scene_id: "service_corridor".into(),
                title: "后勤通道".into(),
                atmosphere: "rain_manor".into(),
                description: "管道滴水作响，一名神色慌张的知情人正躲在储物间门口张望。".into(),
                tags: vec!["backstage".into(), "social".into()],
                exits: vec![
                    SceneExit {
                        target_scene_id: "hub_lobby".into(),
                        label: "返回接待大厅".into(),
                        bidirectional: true,
                        condition: None,
                    },
                    SceneExit {
                        target_scene_id: "restricted_vault".into(),
                        label: "从检修口潜入禁区".into(),
                        bidirectional: false,
                        condition: None,
                    },
                ],
                entities: vec![SceneEntity {
                    entity_id: "npc_witness".into(),
                    name: "后勤技工阿林".into(),
                    role: "witness".into(),
                    kind: "npc".into(),
                    summary: "手指沾着机油，显然目睹了昨晚不该看的一幕。".into(),
                    clue_ids: vec!["clue_witness_account".into()],
                }],
            },
            SceneDefinition {
                scene_id: "restricted_vault".into(),
                title: "封锁禁区".into(),
                atmosphere: "san_strain".into(),
                description: "深处传来低频震动，核心真相与危险正近在咫尺。".into(),
                tags: vec!["danger".into(), "climax".into()],
                exits: vec![SceneExit {
                    target_scene_id: "hub_lobby".into(),
                    label: "撤回接待大厅".into(),
                    bidirectional: true,
                    condition: None,
                }],
                entities: vec![SceneEntity {
                    entity_id: "obj_core_evidence".into(),
                    name: "核心物证箱".into(),
                    role: "object".into(),
                    kind: "object".into(),
                    summary: "封存着决定一切真相的原始记录。".into(),
                    clue_ids: vec!["clue_core_secret".into()],
                }],
            },
        ],
        clues: vec![
            ClueDefinition {
                clue_id: "clue_entry_log".into(),
                title: "被涂改的登记簿".into(),
                summary: "登记簿里少了一个昨夜进入封锁禁区的人名。".into(),
                source_label: "接待大厅".into(),
                is_conclusion: false,
                requires: None,
                acquisition_paths: vec![AcquisitionPath {
                    path_id: "start_log".into(),
                    description: "翻看值班台登记簿".into(),
                    condition: Predicate::Always(AlwaysKeyword::Always),
                }],
                scene_ids: vec!["hub_lobby".into()],
            },
            ClueDefinition {
                clue_id: "clue_warden_slip".into(),
                title: "老周的口供漏洞".into(),
                summary: "老周坚称无人进入禁区，但他的钥匙串少了一把备用钥匙。".into(),
                source_label: "接待大厅对质".into(),
                is_conclusion: false,
                requires: None,
                acquisition_paths: vec![AcquisitionPath {
                    path_id: "psy_warden".into(),
                    description: "识破老周微表情".into(),
                    condition: Predicate::SkillSuccess {
                        skill_success: "psychology".into(),
                    },
                }],
                scene_ids: vec!["hub_lobby".into()],
            },
            ClueDefinition {
                clue_id: "clue_hidden_dossier".into(),
                title: "加密档案袋".into(),
                summary: "记录了冒名顶替者的真实身份与今晚交接时间。".into(),
                source_label: "档案室铁皮柜".into(),
                is_conclusion: false,
                requires: None,
                acquisition_paths: vec![AcquisitionPath {
                    path_id: "lock_cabinet".into(),
                    description: "撬开带锁铁皮柜".into(),
                    condition: Predicate::SkillSuccess {
                        skill_success: "locksmith".into(),
                    },
                }],
                scene_ids: vec!["records_office".into()],
            },
            ClueDefinition {
                clue_id: "clue_witness_account".into(),
                title: "阿林的目击证词".into(),
                summary: "阿林亲眼看见有人半夜把重物拖进封锁禁区。".into(),
                source_label: "后勤通道对话".into(),
                is_conclusion: false,
                requires: None,
                acquisition_paths: vec![AcquisitionPath {
                    path_id: "talk_witness".into(),
                    description: "安抚并询问阿林".into(),
                    condition: Predicate::Always(AlwaysKeyword::Always),
                }],
                scene_ids: vec!["service_corridor".into()],
            },
            ClueDefinition {
                clue_id: "clue_core_secret".into(),
                title: "禁区核心铁证".into(),
                summary: format!("彻底揭开「{}」幕后真相的决定性证据。", title),
                source_label: "封锁禁区".into(),
                is_conclusion: true,
                requires: Some(Predicate::All {
                    all: vec![
                        Predicate::ClueDiscovered {
                            clue_discovered: "clue_entry_log".into(),
                        },
                        Predicate::Any {
                            any: vec![
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_hidden_dossier".into(),
                                },
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_witness_account".into(),
                                },
                            ],
                        },
                    ],
                }),
                acquisition_paths: vec![
                    AcquisitionPath {
                        path_id: "path_dossier".into(),
                        description: "结合加密档案与登记簿".into(),
                        condition: Predicate::All {
                            all: vec![
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_entry_log".into(),
                                },
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_hidden_dossier".into(),
                                },
                            ],
                        },
                    },
                    AcquisitionPath {
                        path_id: "path_witness".into(),
                        description: "结合目击证词与登记簿".into(),
                        condition: Predicate::All {
                            all: vec![
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_entry_log".into(),
                                },
                                Predicate::ClueDiscovered {
                                    clue_discovered: "clue_witness_account".into(),
                                },
                            ],
                        },
                    },
                ],
                scene_ids: vec!["restricted_vault".into()],
            },
        ],
        handouts: vec![HandoutDefinition {
            handout_id: "handout_entry_note".into(),
            kind: "note".into(),
            title: "撕缺一角的登记页".into(),
            canonical: true,
            clue_id: Some("clue_entry_log".into()),
            source_facts: vec!["昨夜 23:40 有一行签名被墨水涂抹".into()],
            presentation_text: format!(
                "【{} · 值班记录】\n23:10 巡检正常\n23:40 [字迹被粗黑墨水涂毁] -> 封锁禁区\n01:05 备用钥匙未归还",
                title
            ),
            unlock_condition: None,
        }],
        npcs: vec![
            NpcDefinition {
                entity_id: "npc_warden".into(),
                name: "值班主管老周".into(),
                personality: "表面镇定、极度护短，习惯用规章制度挡回追问。".into(),
                working_knowledge: vec!["掌握各处房间钥匙与值班排班。".into()],
                cover_story: "昨晚风大线路跳闸，根本没人去过禁区。".into(),
                behavioral_tells: vec!["被问到备用钥匙时会下意识摸腰间钥匙环。".into()],
                protected_truths: vec![ProtectedTruth {
                    secret_id: "warden_complicity".into(),
                    raw_truth: "老周收了封口费，故意删掉了昨夜的登记记录。".into(),
                    tell: Some("他的手腕有墨水渍。".into()),
                    stages: vec![TruthStage {
                        tier: "partial".into(),
                        predicate: Predicate::EvidencePresented {
                            evidence_presented: "clue_hidden_dossier".into(),
                        },
                        releasable_fact_ids: vec!["fact_coverup".into()],
                        truth: "我只是拿钱办事，我真不知道他们把人带进禁区干什么！".into(),
                    }],
                }],
                initial_relations: vec![],
            },
            NpcDefinition {
                entity_id: "npc_witness".into(),
                name: "后勤技工阿林".into(),
                personality: "胆小但还有良心，怕惹祸上身又良心不安。".into(),
                working_knowledge: vec!["熟悉后勤检修通道，知道禁区后门。".into()],
                cover_story: "我昨晚一直在修锅炉，外面什么都没听见。".into(),
                behavioral_tells: vec!["提到禁区时会忍不住瞄一眼通往检修口的铁梯。".into()],
                protected_truths: vec![],
                initial_relations: vec![],
            },
        ],
        clocks: vec![
            ClockDefinition {
                clock_id: "alert_pressure".into(),
                name: "警戒压力".into(),
                visibility: "player_visible".into(),
                max: 6,
                stages: vec![
                    ClockStage {
                        at: 0,
                        label: "表面平静".into(),
                    },
                    ClockStage {
                        at: 3,
                        label: "全员起疑封锁".into(),
                    },
                    ClockStage {
                        at: 6,
                        label: "彻底爆发危机".into(),
                    },
                ],
            },
            ClockDefinition {
                clock_id: "coverup_countdown".into(),
                name: "灭口倒计时".into(),
                visibility: "keeper_hidden".into(),
                max: 6,
                stages: vec![
                    ClockStage {
                        at: 0,
                        label: "尚未转移证据".into(),
                    },
                    ClockStage {
                        at: 4,
                        label: "正在销毁现场".into(),
                    },
                    ClockStage {
                        at: 6,
                        label: "真相彻底掩埋".into(),
                    },
                ],
            },
        ],
        endings: vec![EndingCondition {
            ending_id: "expose_culprit".into(),
            title: "揭穿幕后黑手".into(),
            summary: format!("你在警戒彻底失控前带出核心铁证，破开了「{}」的死局。", title),
            condition: Predicate::ClueDiscovered {
                clue_discovered: "clue_core_secret".into(),
            },
        }],
        review_facts: vec![ReviewFact {
            fact_id: "fact_coverup".into(),
            title: "掩盖真相的交易".into(),
            detail: "值班主管收受贿赂涂改登记簿，掩护真正的幕后人物进入禁区。".into(),
            source_clue_ids: vec!["clue_entry_log".into()],
        }],
    };

    sanitize_and_validate_module(ir, None).expect("mock generated world must satisfy validator")
}

fn normalize_predicate_value(raw: Value, nullable: bool) -> Value {
    let fallback = || {
        if nullable {
            Value::Null
        } else {
            json!("always")
        }
    };
    match raw {
        Value::Null => fallback(),
        Value::Bool(_) => fallback(),
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("always")
                || trimmed.eq_ignore_ascii_case("none")
                || trimmed.eq_ignore_ascii_case("null")
            {
                fallback()
            } else if matches!(
                trimmed,
                "spot_hidden"
                    | "psychology"
                    | "persuade"
                    | "locksmith"
                    | "medicine"
                    | "science"
                    | "intimidate"
                    | "fighting"
                    | "dodge"
                    | "stealth"
                    | "listen"
                    | "library_use"
                    | "charm"
                    | "fast_talk"
                    | "credit_rating"
                    | "occult"
            ) {
                json!({ "skill_success": trimmed })
            } else {
                fallback()
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                return fallback();
            }

            // Handle `{ "type": "skill_success", "skill": "..." }` style predicates often emitted by LLMs
            if let Some(kind) = map.get("type").and_then(Value::as_str) {
                let first_str = |keys: &[&str]| -> Option<String> {
                    keys.iter()
                        .find_map(|k| map.get(*k).and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                };
                match kind.trim().to_lowercase().as_str() {
                    "always" => return json!("always"),
                    "clue_discovered" | "cluediscovered" | "clue" => {
                        if let Some(id) = first_str(&["clueId", "clue_id", "value", "id", "target"])
                        {
                            return json!({ "clue_discovered": id });
                        }
                    }
                    "evidence_presented" | "evidencepresented" | "evidence" => {
                        if let Some(id) =
                            first_str(&["clueId", "clue_id", "evidenceId", "value", "id"])
                        {
                            return json!({ "evidence_presented": id });
                        }
                    }
                    "skill_success" | "skillsuccess" | "skill" | "skill_check" => {
                        if let Some(sk) = first_str(&["skill", "skillId", "value", "name"]) {
                            return json!({ "skill_success": sk });
                        }
                    }
                    "has_item" | "hasitem" | "item" => {
                        if let Some(id) = first_str(&["itemId", "item_id", "value", "id"]) {
                            return json!({ "has_item": id });
                        }
                    }
                    "topic_confronted" | "topicconfronted" | "topic" => {
                        if let Some(t) = first_str(&["topic", "value", "id"]) {
                            return json!({ "topic_confronted": t });
                        }
                    }
                    "event_occurred" | "eventoccurred" | "event" => {
                        if let Some(ev) = first_str(&["event", "tag", "value", "id"]) {
                            return json!({ "event_occurred": ev });
                        }
                    }
                    "clock_at_least" | "clockatleast" | "clock" => {
                        if let Some(cid) = first_str(&["clockId", "clock_id", "id"]) {
                            let val = map
                                .get("value")
                                .or_else(|| map.get("at"))
                                .and_then(Value::as_u64)
                                .unwrap_or(6);
                            return json!({ "clock_at_least": { "clockId": cid, "value": val } });
                        }
                    }
                    _ => {}
                }
            }

            // Normalize canonical & alias single-key predicate objects
            for (key, val) in &map {
                let lower = key.to_lowercase();
                match lower.as_str() {
                    "always" => return json!("always"),
                    "all" | "and" => {
                        if let Some(arr) = val.as_array() {
                            let items: Vec<Value> = arr
                                .iter()
                                .cloned()
                                .map(|item| normalize_predicate_value(item, false))
                                .collect();
                            return if items.is_empty() {
                                fallback()
                            } else {
                                json!({ "all": items })
                            };
                        }
                    }
                    "any" | "or" => {
                        if let Some(arr) = val.as_array() {
                            let items: Vec<Value> = arr
                                .iter()
                                .cloned()
                                .map(|item| normalize_predicate_value(item, false))
                                .collect();
                            return if items.is_empty() {
                                fallback()
                            } else {
                                json!({ "any": items })
                            };
                        }
                    }
                    "not" => {
                        return json!({ "not": normalize_predicate_value(val.clone(), false) });
                    }
                    "clue_discovered" | "cluediscovered" | "has_clue" | "hasclue" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "clue_discovered": s });
                        }
                    }
                    "evidence_presented" | "evidencepresented" | "present_evidence"
                    | "presentevidence" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "evidence_presented": s });
                        }
                    }
                    "skill_success" | "skillsuccess" | "skill_check" | "skillcheck" | "skill" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "skill_success": s });
                        }
                    }
                    "has_item" | "hasitem" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "has_item": s });
                        }
                    }
                    "topic_confronted" | "topicconfronted" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "topic_confronted": s });
                        }
                    }
                    "event_occurred" | "eventoccurred" | "world_tag" | "worldtag" => {
                        if let Some(s) = val.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                            return json!({ "event_occurred": s });
                        }
                    }
                    "clock_at_least" | "clockatleast" | "clock_reached" | "clockreached"
                    | "clock_full" | "clockfull" => {
                        if let Some(obj) = val.as_object() {
                            if let Some(cid) = obj
                                .get("clockId")
                                .or_else(|| obj.get("clock_id"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                let threshold = obj
                                    .get("value")
                                    .or_else(|| obj.get("at"))
                                    .and_then(Value::as_u64)
                                    .unwrap_or(6);
                                return json!({
                                    "clock_at_least": { "clockId": cid, "value": threshold }
                                });
                            }
                        } else if let Some(cid) =
                            val.as_str().map(str::trim).filter(|s| !s.is_empty())
                        {
                            return json!({
                                "clock_at_least": { "clockId": cid, "value": 6 }
                            });
                        }
                    }
                    "npc_relation_at_least" | "npcrelationatleast" => {
                        if let Some(obj) = val.as_object() {
                            let eid = obj
                                .get("entityId")
                                .or_else(|| obj.get("entity_id"))
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let rel = obj
                                .get("relation")
                                .and_then(Value::as_str)
                                .unwrap_or("trust");
                            let v = obj.get("value").and_then(Value::as_i64).unwrap_or(1);
                            if !eid.is_empty() {
                                return json!({
                                    "npc_relation_at_least": {
                                        "entityId": eid,
                                        "relation": rel,
                                        "value": v
                                    }
                                });
                            }
                        }
                    }
                    "flag_equals" | "flagequals" => {
                        if let Some(obj) = val.as_object() {
                            if let (Some(k), Some(v)) =
                                (obj.get("key").and_then(Value::as_str), obj.get("value"))
                            {
                                return json!({ "flag_equals": { "key": k, "value": v } });
                            }
                        }
                    }
                    _ => {}
                }
            }
            fallback()
        }
        _ => fallback(),
    }
}

fn coerce_string_array(obj: &mut Map<String, Value>, key: &str) {
    if let Some(Value::String(s)) = obj.get(key) {
        let wrapped = Value::Array(vec![Value::String(s.clone())]);
        obj.insert(key.to_string(), wrapped);
    }
}

/// Normalizes minor JSON/YAML schema variances (such as camelCase predicates or string-wrapped
/// lists) before deserializing into `ModuleIr` and passing through `sanitize_and_validate_module`.
pub fn parse_and_sanitize_module_value(
    mut value: Value,
    forced_module_id: Option<&str>,
) -> Result<ModuleIr, String> {
    if let Some(root) = value.as_object_mut() {
        if let Some(scenes) = root.get_mut("scenes").and_then(Value::as_array_mut) {
            for scene in scenes {
                if let Some(s_obj) = scene.as_object_mut() {
                    coerce_string_array(s_obj, "tags");
                    if let Some(exits) = s_obj.get_mut("exits").and_then(Value::as_array_mut) {
                        for exit in exits {
                            if let Some(e_obj) = exit.as_object_mut() {
                                if let Some(cond) = e_obj.remove("condition") {
                                    e_obj.insert(
                                        "condition".into(),
                                        normalize_predicate_value(cond, true),
                                    );
                                }
                            }
                        }
                    }
                    if let Some(entities) = s_obj.get_mut("entities").and_then(Value::as_array_mut)
                    {
                        for ent in entities {
                            if let Some(ent_obj) = ent.as_object_mut() {
                                coerce_string_array(ent_obj, "clueIds");
                            }
                        }
                    }
                }
            }
        }

        if let Some(clues) = root.get_mut("clues").and_then(Value::as_array_mut) {
            for clue in clues {
                if let Some(c_obj) = clue.as_object_mut() {
                    coerce_string_array(c_obj, "sceneIds");
                    if let Some(req) = c_obj.remove("requires") {
                        c_obj.insert("requires".into(), normalize_predicate_value(req, true));
                    }
                    if let Some(paths) = c_obj
                        .get_mut("acquisitionPaths")
                        .and_then(Value::as_array_mut)
                    {
                        for path in paths {
                            if let Some(p_obj) = path.as_object_mut() {
                                let cond = p_obj.remove("condition").unwrap_or(Value::Null);
                                p_obj.insert(
                                    "condition".into(),
                                    normalize_predicate_value(cond, false),
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(handouts) = root.get_mut("handouts").and_then(Value::as_array_mut) {
            for handout in handouts {
                if let Some(h_obj) = handout.as_object_mut() {
                    coerce_string_array(h_obj, "sourceFacts");
                    if let Some(cond) = h_obj.remove("unlockCondition") {
                        h_obj.insert(
                            "unlockCondition".into(),
                            normalize_predicate_value(cond, true),
                        );
                    }
                }
            }
        }

        if let Some(npcs) = root.get_mut("npcs").and_then(Value::as_array_mut) {
            for npc in npcs {
                if let Some(n_obj) = npc.as_object_mut() {
                    coerce_string_array(n_obj, "workingKnowledge");
                    coerce_string_array(n_obj, "behavioralTells");
                    if let Some(truths) = n_obj
                        .get_mut("protectedTruths")
                        .and_then(Value::as_array_mut)
                    {
                        for truth in truths {
                            if let Some(t_obj) = truth.as_object_mut() {
                                if let Some(stages) =
                                    t_obj.get_mut("stages").and_then(Value::as_array_mut)
                                {
                                    for stage in stages {
                                        if let Some(st_obj) = stage.as_object_mut() {
                                            coerce_string_array(st_obj, "releasableFactIds");
                                            let pred =
                                                st_obj.remove("predicate").unwrap_or(Value::Null);
                                            st_obj.insert(
                                                "predicate".into(),
                                                normalize_predicate_value(pred, false),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(endings) = root.get_mut("endings").and_then(Value::as_array_mut) {
            for ending in endings {
                if let Some(e_obj) = ending.as_object_mut() {
                    let cond = e_obj.remove("condition").unwrap_or(Value::Null);
                    e_obj.insert("condition".into(), normalize_predicate_value(cond, false));
                }
            }
        }

        if let Some(facts) = root.get_mut("reviewFacts").and_then(Value::as_array_mut) {
            for fact in facts {
                if let Some(f_obj) = fact.as_object_mut() {
                    coerce_string_array(f_obj, "sourceClueIds");
                }
            }
        }
    }

    let parsed: ModuleIr =
        serde_json::from_value(value).map_err(|e| format!("JSON 反序列化失败：{e}"))?;
    sanitize_and_validate_module(parsed, forced_module_id).map_err(|e| e.to_string())
}

/// Enforces structural richness for AI-generated worlds so the model cannot return a
/// degenerate 1-room / 1-NPC stub when the user requested a multi-room tabletop sandbox.
pub fn validate_generated_world_richness(
    module: &ModuleIr,
    min_scenes: usize,
) -> Result<(), String> {
    let required_scenes = min_scenes.clamp(3, 6);
    if module.scenes.len() < required_scenes {
        return Err(format!(
            "场景数量不足：需要至少 {required_scenes} 个连通场景，实际仅生成 {} 个，请在 scenes 数组中完整输出 {required_scenes} 个不同地点并用 exits 互相连通。",
            module.scenes.len()
        ));
    }
    if module.npcs.len() < 2 {
        return Err(format!(
            "NPC 数量不足：需要至少 2 位立场各异的 NPC，实际仅生成 {} 位，请在 npcs 数组中补充人物并分布到不同场景的 entities 中。",
            module.npcs.len()
        ));
    }
    let connected_scenes = module.scenes.iter().filter(|s| !s.exits.is_empty()).count();
    if connected_scenes < required_scenes {
        return Err(
            "部分场景缺少连通出口 exits，请确保所有场景都通过 exits 连成可探索的拓扑图。".into(),
        );
    }
    if module.clues.len() < 4 {
        return Err(format!(
            "线索数量不足：需要至少 4 条递进线索，实际仅生成 {} 条。",
            module.clues.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_llm_camel_case_and_typed_predicates_into_valid_module() {
        let raw = json!({
            "moduleId": "cyber_clinic",
            "title": "九龙暗诊",
            "initialSceneId": "clinic_lobby",
            "scenes": [
                {
                    "sceneId": "clinic_lobby",
                    "title": "地下候诊区",
                    "description": "霓虹灯管在潮湿天花板下滋滋作响。",
                    "exits": [
                        {
                            "targetSceneId": "memory_lab",
                            "label": "前往记忆舱室",
                            "bidirectional": true,
                            "condition": { "clueDiscovered": "clue_badge" }
                        }
                    ],
                    "entities": [
                        {
                            "entityId": "npc_doc",
                            "name": "义体医生老K",
                            "kind": "npc",
                            "clueIds": ["clue_badge"]
                        }
                    ]
                },
                {
                    "sceneId": "memory_lab",
                    "title": "记忆舱室",
                    "description": "三台休眠舱泛着冷蓝光。",
                    "exits": [
                        { "targetSceneId": "clinic_lobby", "label": "返回候诊区", "bidirectional": true }
                    ]
                }
            ],
            "clues": [
                {
                    "clueId": "clue_badge",
                    "title": "加密门禁卡",
                    "summary": "记录着异常刷卡记录。",
                    "acquisitionPaths": [
                        { "pathId": "p1", "condition": { "always": true } }
                    ],
                    "sceneIds": "clinic_lobby"
                },
                {
                    "clueId": "clue_chip",
                    "title": "被篡改的记忆芯片",
                    "summary": "抹除了昨晚手术室的录像。",
                    "acquisitionPaths": [
                        { "pathId": "p2", "condition": { "type": "skill_success", "skill": "spot_hidden" } }
                    ],
                    "sceneIds": ["memory_lab"]
                }
            ],
            "npcs": [
                {
                    "entityId": "npc_doc",
                    "name": "义体医生老K",
                    "protectedTruths": [
                        {
                            "secretId": "sec_1",
                            "rawTruth": "他把记忆卖给了黑帮。",
                            "stages": [
                                {
                                    "tier": "partial",
                                    "predicate": { "evidencePresented": "clue_chip" },
                                    "truth": "我只是受人雇佣！"
                                }
                            ]
                        }
                    ]
                }
            ],
            "clocks": [
                { "clockId": "lockdown", "name": "全楼封锁", "visibility": "player_visible", "max": 6 }
            ],
            "endings": [
                {
                    "endingId": "e1",
                    "title": "封锁降临",
                    "condition": { "clockAtLeast": { "clock_id": "lockdown", "value": 6 } }
                },
                {
                    "endingId": "e2",
                    "title": "带走铁证",
                    "condition": { "unsupported_custom_pred": "ignored_safely" }
                }
            ]
        });

        let module = parse_and_sanitize_module_value(raw, None)
            .expect("normalized LLM payload should compile cleanly");
        assert_eq!(module.title, "九龙暗诊");
        assert_eq!(module.scenes.len(), 2);
        assert_eq!(module.clues.len(), 2);
        assert!(matches!(
            module.endings[0].condition,
            Predicate::ClockAtLeast { .. }
        ));

        // Verify richness gate rejects a 2-scene / 1-NPC stub when 4 scenes are requested
        assert!(validate_generated_world_richness(&module, 4).is_err());

        // Verify starter `Always` clue cap: at most 1 clue can unlock unconditionally at Turn 0
        let raw_multi_always = json!({
            "title": "极地科考站",
            "initialSceneId": "station_hub",
            "scenes": [
                { "sceneId": "station_hub", "title": "主控舱", "exits": [{"targetSceneId": "lab_bay", "label": "去实验舱"}] },
                { "sceneId": "lab_bay", "title": "冰芯实验舱", "exits": [{"targetSceneId": "station_hub", "label": "回主控舱"}] }
            ],
            "clues": [
                { "clueId": "c1", "title": "开场异状", "sceneIds": ["station_hub"], "acquisitionPaths": [{"pathId": "p1", "condition": "always"}] },
                { "clueId": "c2", "title": "隐藏样本", "sceneIds": ["station_hub"], "acquisitionPaths": [{"pathId": "p2", "condition": "always"}] },
                { "clueId": "c3", "title": "封存日志", "sceneIds": ["lab_bay"], "acquisitionPaths": [{"pathId": "p3", "condition": {"unknown_custom": true}}] }
            ]
        });
        let capped = parse_and_sanitize_module_value(raw_multi_always, None).unwrap();
        let always_count = capped
            .clues
            .iter()
            .filter(|c| {
                c.acquisition_paths
                    .iter()
                    .any(|p| matches!(p.condition, Predicate::Always(_)))
            })
            .count();
        assert_eq!(
            always_count, 1,
            "at most one starter clue should be unconditionally Always"
        );
    }
}
