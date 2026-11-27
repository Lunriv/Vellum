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
    sync::{Arc, RwLock},
};

pub mod generator;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSummary {
    pub module_id: String,
    pub version: String,
    pub title: String,
    pub opening_prologue: String,
    pub source_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_prompt: Option<String>,
    pub scene_count: usize,
    pub npc_count: usize,
    pub investigator_count: usize,
    pub clue_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIr {
    #[serde(default)]
    pub module_id: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub title: String,
    #[serde(default)]
    pub opening_prologue: String,
    #[serde(default)]
    pub investigators: Vec<InvestigatorPreset>,
    #[serde(default)]
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

fn default_version() -> String {
    "1.0.0".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvestigatorPreset {
    #[serde(default)]
    pub investigator_id: String,
    pub name: String,
    #[serde(default)]
    pub occupation: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub biography: String,
    #[serde(default)]
    pub motivation: String,
    #[serde(default)]
    pub manor_connection: String,
    #[serde(default = "default_ten")]
    pub hp_max: i32,
    #[serde(default = "default_ten")]
    pub mp_max: i32,
    #[serde(default = "default_fifty")]
    pub san_start: i32,
    #[serde(default = "default_fifty")]
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

fn default_ten() -> i32 {
    10
}
fn default_fifty() -> i32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillStrength {
    pub skill: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_fifty")]
    pub rating: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartingItem {
    #[serde(default)]
    pub item_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EndingCondition {
    #[serde(default)]
    pub ending_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_always_predicate")]
    pub condition: Predicate,
}

fn default_always_predicate() -> Predicate {
    Predicate::Always(AlwaysKeyword::Always)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFact {
    #[serde(default)]
    pub fact_id: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub source_clue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneDefinition {
    #[serde(default)]
    pub scene_id: String,
    pub title: String,
    #[serde(default = "default_atmosphere")]
    pub atmosphere: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub exits: Vec<SceneExit>,
    #[serde(default)]
    pub entities: Vec<SceneEntity>,
}

fn default_atmosphere() -> String {
    "fog".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneExit {
    pub target_scene_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub bidirectional: bool,
    #[serde(default)]
    pub condition: Option<Predicate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneEntity {
    #[serde(default)]
    pub entity_id: String,
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default = "default_object_kind")]
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub clue_ids: Vec<String>,
}

fn default_object_kind() -> String {
    "object".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClueDefinition {
    #[serde(default)]
    pub clue_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
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
    #[serde(default)]
    pub path_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_always_predicate")]
    pub condition: Predicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandoutDefinition {
    #[serde(default)]
    pub handout_id: String,
    #[serde(default = "default_note_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default = "default_true_bool")]
    pub canonical: bool,
    #[serde(default)]
    pub clue_id: Option<String>,
    #[serde(default)]
    pub source_facts: Vec<String>,
    #[serde(default)]
    pub presentation_text: String,
    #[serde(default)]
    pub unlock_condition: Option<Predicate>,
}

fn default_note_kind() -> String {
    "note".into()
}
fn default_true_bool() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpcDefinition {
    #[serde(default)]
    pub entity_id: String,
    pub name: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub working_knowledge: Vec<String>,
    #[serde(default)]
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
    #[serde(default)]
    pub secret_id: String,
    #[serde(default)]
    pub raw_truth: String,
    #[serde(default)]
    pub tell: Option<String>,
    #[serde(default)]
    pub stages: Vec<TruthStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruthStage {
    #[serde(default = "default_tier")]
    pub tier: String,
    #[serde(default = "default_always_predicate")]
    pub predicate: Predicate,
    #[serde(default)]
    pub releasable_fact_ids: Vec<String>,
    #[serde(default)]
    pub truth: String,
}

fn default_tier() -> String {
    "partial".into()
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
    #[serde(default)]
    pub clock_id: String,
    pub name: String,
    #[serde(default = "default_visible_clock")]
    pub visibility: String,
    #[serde(default = "default_clock_max")]
    pub max: u32,
    #[serde(default)]
    pub stages: Vec<ClockStage>,
}

fn default_visible_clock() -> String {
    "player_visible".into()
}
fn default_clock_max() -> u32 {
    6
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
    #[serde(alias = "entity_id")]
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
    #[serde(alias = "clock_id")]
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

#[derive(Debug, Clone)]
struct RegisteredModule {
    module: ModuleIr,
    source_kind: String,
    source_prompt: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: Arc<RwLock<HashMap<String, RegisteredModule>>>,
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
                    modules.insert(
                        module.module_id.clone(),
                        RegisteredModule {
                            module,
                            source_kind: "builtin".into(),
                            source_prompt: None,
                        },
                    );
                }
            }
        }
        Ok(Self {
            modules: Arc::new(RwLock::new(modules)),
        })
    }

    pub fn load_default() -> Result<Self, ModuleError> {
        let candidates = [
            std::env::var("VELLUM_MODULES_DIR").ok().map(PathBuf::from),
            Some(PathBuf::from("modules")),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules")),
        ];
        for candidate in candidates.into_iter().flatten() {
            if candidate.is_dir() {
                let has_any_module = fs::read_dir(&candidate)
                    .ok()
                    .map(|entries| {
                        entries.filter_map(Result::ok).any(|entry| {
                            let p = entry.path();
                            p.is_dir() && p.join("module.yaml").exists()
                        })
                    })
                    .unwrap_or(false);
                if has_any_module {
                    return Self::load_dir(candidate);
                }
            }
        }
        Err(ModuleError::Validation(
            "could not locate modules directory containing any valid module.yaml".into(),
        ))
    }

    pub fn get(&self, module_id: &str) -> Option<ModuleIr> {
        self.modules
            .read()
            .ok()?
            .get(module_id)
            .map(|r| r.module.clone())
    }

    pub fn is_builtin(&self, module_id: &str) -> bool {
        self.modules
            .read()
            .ok()
            .and_then(|map| map.get(module_id).map(|r| r.source_kind == "builtin"))
            .unwrap_or(false)
    }

    pub fn register(
        &self,
        module: ModuleIr,
        source_kind: impl Into<String>,
        source_prompt: Option<String>,
    ) -> Result<ModuleSummary, ModuleError> {
        validate_module(&module)?;
        let kind = source_kind.into();
        let summary = summarize_module(&module, &kind, source_prompt.as_deref());
        let mut guard = self
            .modules
            .write()
            .map_err(|_| ModuleError::Validation("module registry lock poisoned".into()))?;
        guard.insert(
            module.module_id.clone(),
            RegisteredModule {
                module,
                source_kind: kind,
                source_prompt,
            },
        );
        Ok(summary)
    }

    pub fn remove_custom(&self, module_id: &str) -> Result<bool, String> {
        let mut guard = self
            .modules
            .write()
            .map_err(|_| "module registry lock poisoned".to_string())?;
        if let Some(existing) = guard.get(module_id) {
            if existing.source_kind == "builtin" {
                return Err("内置演示模组不可删除".into());
            }
        }
        Ok(guard.remove(module_id).is_some())
    }

    pub fn list_summaries(&self) -> Vec<ModuleSummary> {
        let Ok(guard) = self.modules.read() else {
            return Vec::new();
        };
        let mut list: Vec<ModuleSummary> = guard
            .values()
            .map(|r| summarize_module(&r.module, &r.source_kind, r.source_prompt.as_deref()))
            .collect();
        // Put user-created / generated worlds first, then built-in demos, stably sorted by title
        list.sort_by(|a, b| {
            let rank = |k: &str| if k == "builtin" { 1 } else { 0 };
            rank(&a.source_kind)
                .cmp(&rank(&b.source_kind))
                .then_with(|| a.title.cmp(&b.title))
        });
        list
    }
}

pub fn summarize_module(
    module: &ModuleIr,
    source_kind: &str,
    source_prompt: Option<&str>,
) -> ModuleSummary {
    ModuleSummary {
        module_id: module.module_id.clone(),
        version: module.version.clone(),
        title: module.title.clone(),
        opening_prologue: module.opening_prologue.clone(),
        source_kind: source_kind.to_string(),
        source_prompt: source_prompt.map(str::to_owned),
        scene_count: module.scenes.len(),
        npc_count: module.npcs.len(),
        investigator_count: module.investigators.len(),
        clue_count: module.clues.len(),
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
