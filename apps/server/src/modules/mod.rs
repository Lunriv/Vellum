//! Local Module IR loader and static integrity checks.
//!
//! Module files are author-owned data.  They are parsed into a deliberately
//! small, closed representation before being handed to the runtime.  The
//! validator checks references and graph properties without executing any
//! user supplied expression.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIr {
    pub module_id: String,
    pub version: String,
    pub title: String,
    pub opening_prologue: String,
    pub investigators: Vec<InvestigatorPreset>,
    pub initial_scene_id: String,
    #[serde(default)]
    pub endings: Vec<EndingCondition>,
    #[serde(default)]
    pub review_facts: Vec<ReviewFact>,
    #[serde(default)]
    pub scenes: Vec<SceneDefinition>,
    #[serde(default)]
    pub clues: Vec<ClueDefinition>,
    #[serde(default)]
    pub handouts: Vec<HandoutDefinition>,
    #[serde(default)]
    pub npcs: Vec<NpcDefinition>,
    #[serde(default)]
    pub clocks: Vec<ClockDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvestigatorPreset {
    pub investigator_id: String,
    pub name: String,
    pub occupation: String,
    pub tagline: String,
    pub biography: String,
    pub motivation: String,
    pub manor_connection: String,
    pub hp_max: i32,
    pub mp_max: i32,
    pub san_start: i32,
    pub luck_start: i32,
    #[serde(default)]
    pub strengths: Vec<SkillStrength>,
    #[serde(default)]
    pub starting_items: Vec<StartingItem>,
    #[serde(default)]
    pub characteristics: BTreeMap<String, i32>,
    #[serde(default)]
    pub skills: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillStrength {
    pub skill: String,
    pub label: String,
    pub rating: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartingItem {
    pub item_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EndingCondition {
    pub ending_id: String,
    pub title: String,
    pub summary: String,
    pub condition: Predicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFact {
    pub fact_id: String,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub source_clue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneDefinition {
    pub scene_id: String,
    pub title: String,
    pub atmosphere: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub exits: Vec<SceneExit>,
    #[serde(default)]
    pub entities: Vec<SceneEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneExit {
    pub target_scene_id: String,
    pub label: String,
    #[serde(default)]
    pub bidirectional: bool,
    pub condition: Option<Predicate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneEntity {
    pub entity_id: String,
    pub name: String,
    pub role: String,
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub clue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClueDefinition {
    pub clue_id: String,
    pub title: String,
    pub summary: String,
    pub source_label: String,
    #[serde(default)]
    pub is_conclusion: bool,
    #[serde(default)]
    pub requires: Option<Predicate>,
    #[serde(default)]
    pub acquisition_paths: Vec<AcquisitionPath>,
    #[serde(default)]
    pub scene_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionPath {
    pub path_id: String,
    pub description: String,
    pub condition: Predicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandoutDefinition {
    pub handout_id: String,
    pub kind: String,
    pub title: String,
    pub canonical: bool,
    pub clue_id: Option<String>,
    #[serde(default)]
    pub source_facts: Vec<String>,
    pub presentation_text: String,
    #[serde(default)]
    pub unlock_condition: Option<Predicate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpcDefinition {
    pub entity_id: String,
    pub name: String,
    pub personality: String,
    #[serde(default)]
    pub working_knowledge: Vec<String>,
    pub cover_story: String,
    #[serde(default)]
    pub behavioral_tells: Vec<String>,
    #[serde(default)]
    pub protected_truths: Vec<ProtectedTruth>,
    #[serde(default)]
    pub initial_relations: Vec<NpcRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedTruth {
    pub secret_id: String,
    pub raw_truth: String,
    #[serde(default)]
    pub tell: Option<String>,
    #[serde(default)]
    pub stages: Vec<TruthStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruthStage {
    pub tier: String,
    pub predicate: Predicate,
    #[serde(default)]
    pub releasable_fact_ids: Vec<String>,
    pub truth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpcRelation {
    pub entity_id: String,
    pub relation: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClockDefinition {
    pub clock_id: String,
    pub name: String,
    pub visibility: String,
    pub max: u32,
    #[serde(default)]
    pub stages: Vec<ClockStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClockStage {
    pub at: u32,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlwaysKeyword {
    #[serde(rename = "always")]
    Always,
}

/// A restricted predicate language used by the module validator and kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Predicate {
    All {
        all: Vec<Predicate>,
    },
    Any {
        any: Vec<Predicate>,
    },
    Not {
        not: Box<Predicate>,
    },
    HasItem {
        has_item: String,
    },
    SkillSuccess {
        skill_success: String,
    },
    NpcRelationAtLeast {
        npc_relation_at_least: RelationRequirement,
    },
    FlagEquals {
        flag_equals: FlagRequirement,
    },
    ClockAtLeast {
        clock_at_least: ClockRequirement,
    },
    ClueDiscovered {
        clue_discovered: String,
    },
    EvidencePresented {
        evidence_presented: String,
    },
    TopicConfronted {
        topic_confronted: String,
    },
    EventOccurred {
        event_occurred: String,
    },
    Always(AlwaysKeyword),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelationRequirement {
    pub entity_id: String,
    pub relation: String,
    pub value: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FlagRequirement {
    pub key: String,
    pub value: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClockRequirement {
    pub clock_id: String,
    pub value: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("module validation failed: {0}")]
    Validation(String),
}

fn read_yaml<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<T, ModuleError> {
    let path = dir.join(name);
    let text = fs::read_to_string(&path).map_err(|source| ModuleError::Io {
        path: path.clone(),
        source,
    })?;
    serde_yaml::from_str(&text).map_err(|source| ModuleError::Parse { path, source })
}

/// Load all six module files from a directory and validate the resulting IR.
pub fn load_module(dir: impl AsRef<Path>) -> Result<ModuleIr, ModuleError> {
    let dir = dir.as_ref();
    let mut module: ModuleIr = read_yaml(dir, "module.yaml")?;
    module.scenes = read_yaml(dir, "scenes.yaml")?;
    module.clues = read_yaml(dir, "clues.yaml")?;
    module.handouts = read_yaml(dir, "handouts.yaml")?;
    module.npcs = read_yaml(dir, "npcs.yaml")?;
    module.clocks = read_yaml(dir, "clocks.yaml")?;
    validate_module(&module)?;
    Ok(module)
}

/// Evaluates a Module IR predicate against authoritative SessionState and optional turn-action context.
/// Turn-action predicates (`EvidencePresented`, `TopicConfronted`) require an explicit `ActionContext`
/// (having a clue in inventory/clues is NOT equivalent to presenting it in the current turn).
pub fn evaluate_module_predicate(
    predicate: &Predicate,
    state: &crate::kernel::SessionState,
    action: Option<&crate::kernel::ActionContext>,
) -> bool {
    match predicate {
        Predicate::Always(_) => true,
        Predicate::All { all } => all
            .iter()
            .all(|p| evaluate_module_predicate(p, state, action)),
        Predicate::Any { any } => any
            .iter()
            .any(|p| evaluate_module_predicate(p, state, action)),
        Predicate::Not { not } => !evaluate_module_predicate(not, state, action),
        Predicate::HasItem { has_item } => {
            state.items.get(has_item).copied().unwrap_or(0) > 0
                || state
                    .investigator
                    .inventory
                    .iter()
                    .any(|item| item.item_id == *has_item && item.quantity > 0)
        }
        Predicate::SkillSuccess { skill_success } => state.skill_successes.contains(skill_success),
        Predicate::NpcRelationAtLeast {
            npc_relation_at_least,
        } => {
            let key = format!(
                "{}:{}",
                npc_relation_at_least.entity_id, npc_relation_at_least.relation
            );
            state
                .npc_relations
                .get(&key)
                .or_else(|| state.npc_relations.get(&npc_relation_at_least.entity_id))
                .copied()
                .unwrap_or(0)
                >= npc_relation_at_least.value
        }
        Predicate::FlagEquals { flag_equals } => {
            state.flags.get(&flag_equals.key) == Some(&flag_equals.value)
        }
        Predicate::ClockAtLeast { clock_at_least } => state
            .clocks
            .get(&clock_at_least.clock_id)
            .map(|c| c.current >= clock_at_least.value)
            .unwrap_or(false),
        Predicate::ClueDiscovered { clue_discovered } => state.clues.contains_key(clue_discovered),
        Predicate::EvidencePresented { evidence_presented } => action
            .map(|ctx| {
                ctx.unlocked_clues.contains(evidence_presented)
                    && ctx.evidence_presented.contains(evidence_presented)
            })
            .unwrap_or(false),
        Predicate::TopicConfronted { topic_confronted } => action
            .map(|ctx| ctx.topics_confronted.contains(topic_confronted))
            .unwrap_or(false),
        Predicate::EventOccurred { event_occurred } => state.events.contains(event_occurred),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: HashMap<String, ModuleIr>,
}

impl ModuleRegistry {
    pub fn load_dir(root: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let root = root.as_ref();
        let mut modules = HashMap::new();
        if root.exists() {
            for entry in fs::read_dir(root).map_err(|source| ModuleError::Io {
                path: root.to_path_buf(),
                source,
            })? {
                let entry = entry.map_err(|source| ModuleError::Io {
                    path: root.to_path_buf(),
                    source,
                })?;
                let path = entry.path();
                if path.is_dir() && path.join("module.yaml").exists() {
                    let module = load_module(&path)?;
                    modules.insert(module.module_id.clone(), module);
                }
            }
        }
        Ok(Self { modules })
    }

    pub fn load_default() -> Result<Self, ModuleError> {
        let candidates = [
            std::env::var("VELLUM_MODULES_DIR").ok().map(PathBuf::from),
            Some(PathBuf::from("modules")),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules")),
        ];
        for candidate in candidates.into_iter().flatten() {
            if candidate
                .join("crimson_rain_manor")
                .join("module.yaml")
                .exists()
            {
                return Self::load_dir(candidate);
            }
        }
        Err(ModuleError::Validation(
            "could not locate modules directory containing crimson_rain_manor".into(),
        ))
    }

    pub fn get(&self, module_id: &str) -> Option<&ModuleIr> {
        self.modules.get(module_id)
    }
}

impl ModuleIr {
    pub fn find_investigator(&self, investigator_id: &str) -> Option<&InvestigatorPreset> {
        self.investigators
            .iter()
            .find(|inv| inv.investigator_id == investigator_id)
    }

    pub fn find_scene(&self, scene_id: &str) -> Option<&SceneDefinition> {
        self.scenes.iter().find(|s| s.scene_id == scene_id)
    }

    pub fn find_clue(&self, clue_id: &str) -> Option<&ClueDefinition> {
        self.clues.iter().find(|c| c.clue_id == clue_id)
    }

    pub fn find_handout(&self, handout_id: &str) -> Option<&HandoutDefinition> {
        self.handouts.iter().find(|h| h.handout_id == handout_id)
    }

    pub fn find_npc(&self, entity_id: &str) -> Option<&NpcDefinition> {
        self.npcs.iter().find(|n| n.entity_id == entity_id)
    }
}

pub fn validate_module(module: &ModuleIr) -> Result<(), ModuleError> {
    if module.module_id.trim().is_empty() || module.version.trim().is_empty() {
        return Err(ModuleError::Validation(
            "module_id and version are required".into(),
        ));
    }
    if module.investigators.is_empty() {
        return Err(ModuleError::Validation(
            "at least one investigator is required".into(),
        ));
    }
    ensure_unique(
        module
            .investigators
            .iter()
            .map(|x| x.investigator_id.as_str()),
        "investigator",
    )?;
    ensure_unique(module.scenes.iter().map(|x| x.scene_id.as_str()), "scene")?;
    ensure_unique(module.clues.iter().map(|x| x.clue_id.as_str()), "clue")?;
    ensure_unique(
        module.handouts.iter().map(|x| x.handout_id.as_str()),
        "handout",
    )?;
    ensure_unique(module.npcs.iter().map(|x| x.entity_id.as_str()), "npc")?;
    ensure_unique(module.clocks.iter().map(|x| x.clock_id.as_str()), "clock")?;
    let scene_ids: BTreeSet<_> = module.scenes.iter().map(|s| s.scene_id.as_str()).collect();
    if !scene_ids.contains(module.initial_scene_id.as_str()) {
        return Err(ModuleError::Validation(format!(
            "initial scene '{}' does not exist",
            module.initial_scene_id
        )));
    }
    let clue_ids: BTreeSet<_> = module.clues.iter().map(|c| c.clue_id.as_str()).collect();
    let npc_ids: BTreeSet<_> = module.npcs.iter().map(|n| n.entity_id.as_str()).collect();
    let clock_ids: BTreeSet<_> = module.clocks.iter().map(|c| c.clock_id.as_str()).collect();
    for scene in &module.scenes {
        for exit in &scene.exits {
            if !scene_ids.contains(exit.target_scene_id.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "scene '{}' exits to missing scene '{}'",
                    scene.scene_id, exit.target_scene_id
                )));
            }
        }
        for entity in &scene.entities {
            if entity.kind == "npc" && !npc_ids.contains(entity.entity_id.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "scene '{}' references missing NPC '{}'",
                    scene.scene_id, entity.entity_id
                )));
            }
            for clue in &entity.clue_ids {
                if !clue_ids.contains(clue.as_str()) {
                    return Err(ModuleError::Validation(format!(
                        "entity '{}' references missing clue '{}'",
                        entity.entity_id, clue
                    )));
                }
            }
        }
    }
    for clue in &module.clues {
        for path in &clue.acquisition_paths {
            validate_predicate_refs(&path.condition, &clue_ids, &clock_ids)?;
        }
        if let Some(condition) = &clue.requires {
            validate_predicate_refs(condition, &clue_ids, &clock_ids)?;
        }
        for scene_id in &clue.scene_ids {
            if !scene_ids.contains(scene_id.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "clue '{}' references missing scene '{}'",
                    clue.clue_id, scene_id
                )));
            }
        }
        if clue.is_conclusion && clue.acquisition_paths.len() < 2 {
            return Err(ModuleError::Validation(format!(
                "conclusion '{}' needs at least two acquisition paths",
                clue.clue_id
            )));
        }
        if clue.is_conclusion
            && clue
                .acquisition_paths
                .iter()
                .any(|p| is_single_skill_check(&p.condition))
        {
            return Err(ModuleError::Validation(format!(
                "conclusion '{}' is soft-locked by one ordinary check",
                clue.clue_id
            )));
        }
    }
    for handout in &module.handouts {
        if let Some(clue) = &handout.clue_id {
            if !clue_ids.contains(clue.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "handout '{}' references missing clue '{}'",
                    handout.handout_id, clue
                )));
            }
        }
        if let Some(condition) = &handout.unlock_condition {
            validate_predicate_refs(condition, &clue_ids, &clock_ids)?;
        }
    }
    for npc in &module.npcs {
        for truth in &npc.protected_truths {
            for stage in &truth.stages {
                validate_predicate_refs(&stage.predicate, &clue_ids, &clock_ids)?;
                for fact in &stage.releasable_fact_ids {
                    if !module.review_facts.iter().any(|f| f.fact_id == *fact) {
                        return Err(ModuleError::Validation(format!(
                            "NPC '{}' stage references missing fact '{}'",
                            npc.entity_id, fact
                        )));
                    }
                }
            }
        }
    }
    for ending in &module.endings {
        validate_predicate_refs(&ending.condition, &clue_ids, &clock_ids)?;
    }
    for fact in &module.review_facts {
        for clue in &fact.source_clue_ids {
            if !clue_ids.contains(clue.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "review fact '{}' references missing clue '{}'",
                    fact.fact_id, clue
                )));
            }
        }
    }
    detect_clue_cycles(module)?;
    Ok(())
}

fn ensure_unique<'a>(ids: impl Iterator<Item = &'a str>, kind: &str) -> Result<(), ModuleError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || !seen.insert(id) {
            return Err(ModuleError::Validation(format!(
                "duplicate or empty {kind} id '{id}'"
            )));
        }
    }
    Ok(())
}

fn validate_predicate_refs(
    predicate: &Predicate,
    clues: &BTreeSet<&str>,
    clocks: &BTreeSet<&str>,
) -> Result<(), ModuleError> {
    match predicate {
        Predicate::All { all } => {
            for item in all {
                validate_predicate_refs(item, clues, clocks)?
            }
        }
        Predicate::Any { any } => {
            for item in any {
                validate_predicate_refs(item, clues, clocks)?
            }
        }
        Predicate::Not { not } => validate_predicate_refs(not, clues, clocks)?,
        Predicate::ClueDiscovered { clue_discovered }
        | Predicate::EvidencePresented {
            evidence_presented: clue_discovered,
        } => {
            if !clues.contains(clue_discovered.as_str()) {
                return Err(ModuleError::Validation(format!(
                    "predicate references missing clue '{clue_discovered}'"
                )));
            }
        }
        Predicate::ClockAtLeast { clock_at_least }
            if !clocks.contains(clock_at_least.clock_id.as_str()) =>
        {
            return Err(ModuleError::Validation(format!(
                "predicate references missing clock '{}'",
                clock_at_least.clock_id
            )));
        }
        _ => {}
    }
    Ok(())
}

fn predicate_clues(predicate: &Predicate, out: &mut BTreeSet<String>) {
    match predicate {
        Predicate::All { all } => {
            for item in all {
                predicate_clues(item, out)
            }
        }
        Predicate::Any { any } => {
            for item in any {
                predicate_clues(item, out)
            }
        }
        Predicate::Not { not } => predicate_clues(not, out),
        Predicate::ClueDiscovered { clue_discovered }
        | Predicate::EvidencePresented {
            evidence_presented: clue_discovered,
        } => {
            out.insert(clue_discovered.clone());
        }
        _ => {}
    }
}

fn is_single_skill_check(predicate: &Predicate) -> bool {
    matches!(predicate, Predicate::SkillSuccess { .. })
}

fn detect_clue_cycles(module: &ModuleIr) -> Result<(), ModuleError> {
    let mut graph: HashMap<String, BTreeSet<String>> = HashMap::new();
    for clue in &module.clues {
        let mut deps = BTreeSet::new();
        if let Some(predicate) = &clue.requires {
            predicate_clues(predicate, &mut deps);
        }
        for path in &clue.acquisition_paths {
            predicate_clues(&path.condition, &mut deps);
        }
        graph.insert(clue.clue_id.clone(), deps);
    }
    fn visit(
        id: &str,
        graph: &HashMap<String, BTreeSet<String>>,
        visiting: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
    ) -> bool {
        if done.contains(id) {
            return false;
        }
        if !visiting.insert(id.to_string()) {
            return true;
        }
        if let Some(deps) = graph.get(id) {
            for dep in deps {
                if visit(dep, graph, visiting, done) {
                    return true;
                }
            }
        }
        visiting.remove(id);
        done.insert(id.to_string());
        false
    }
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for id in graph.keys() {
        if visit(id, &graph, &mut visiting, &mut done) {
            return Err(ModuleError::Validation(
                "clue dependency graph contains a cycle".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_scene_reference() {
        let module = ModuleIr {
            module_id: "x".into(),
            version: "1".into(),
            title: "x".into(),
            opening_prologue: "x".into(),
            investigators: vec![test_investigator()],
            initial_scene_id: "start".into(),
            endings: vec![],
            review_facts: vec![],
            scenes: vec![SceneDefinition {
                scene_id: "start".into(),
                title: "x".into(),
                atmosphere: "fog".into(),
                description: "x".into(),
                tags: vec![],
                exits: vec![SceneExit {
                    target_scene_id: "missing".into(),
                    label: "x".into(),
                    bidirectional: false,
                    condition: None,
                }],
                entities: vec![],
            }],
            clues: vec![],
            handouts: vec![],
            npcs: vec![],
            clocks: vec![],
        };
        assert!(validate_module(&module).is_err());
    }

    #[test]
    fn loads_crimson_rain_manor_fixture() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/crimson_rain_manor");
        let module = load_module(dir).expect("fixture should satisfy the static validator");
        assert_eq!(module.module_id, "crimson_rain_manor");
        assert_eq!(module.scenes.len(), 5);
        assert_eq!(module.npcs.len(), 3);
        assert!(
            module
                .clues
                .iter()
                .filter(|clue| clue.is_conclusion)
                .count()
                >= 2
        );
        assert!(module
            .clocks
            .iter()
            .any(|clock| clock.visibility == "keeper_hidden"));
    }
    fn test_investigator() -> InvestigatorPreset {
        InvestigatorPreset {
            investigator_id: "i".into(),
            name: "i".into(),
            occupation: "x".into(),
            tagline: "x".into(),
            biography: "x".into(),
            motivation: "x".into(),
            manor_connection: "x".into(),
            hp_max: 10,
            mp_max: 10,
            san_start: 50,
            luck_start: 50,
            strengths: vec![],
            starting_items: vec![],
            characteristics: BTreeMap::new(),
            skills: BTreeMap::new(),
        }
    }
}
