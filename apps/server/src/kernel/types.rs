use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Received,
    Routing,
    ProposalPending,
    AwaitingRoll,
    AwaitingRecovery,
    AwaitingPushDescription,
    AwaitingPushConsequenceAck,
    AwaitingPushRoll,
    Committing,
    Narrating,
    NarrationPending,
    Recovering,
    Completed,
    Cancelled,
    Failed,
    InterruptedBeforeCommit,
}

impl TurnStatus {
    pub fn is_active(&self) -> bool {
        !matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::InterruptedBeforeCommit
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    pub item_id: String,
    pub name: String,
    pub description: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvestigatorState {
    pub investigator_id: String,
    pub name: String,
    pub occupation: String,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub san: i32,
    pub san_max: i32,
    pub luck: i32,
    pub luck_max: i32,
    #[serde(default)]
    pub characteristics: BTreeMap<String, i32>,
    #[serde(default)]
    pub skills: BTreeMap<String, i32>,
    #[serde(default)]
    pub inventory: Vec<InventoryItem>,
    #[serde(default)]
    pub conditions: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClockVisibility {
    PlayerVisible,
    KeeperHidden,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressClock {
    pub clock_id: String,
    pub name: String,
    pub current: u32,
    pub max: u32,
    pub visibility: ClockVisibility,
    #[serde(default)]
    pub thresholds: Vec<ClockThreshold>,
    pub last_reason: Option<String>,
    pub last_caused_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClockThreshold {
    pub at: u32,
    pub stage_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClueState {
    pub clue_id: String,
    pub title: String,
    pub summary: String,
    pub source_label: String,
    pub discovered_at_turn_id: String,
    pub is_conclusion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandoutState {
    pub handout_id: String,
    pub kind: String,
    pub title: String,
    pub canonical: bool,
    pub clue_id: Option<String>,
    pub source_facts: Vec<String>,
    pub presentation_text: String,
    pub unlocked_at_turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneState {
    pub scene_id: String,
    pub title: String,
    pub description: String,
    pub atmosphere: String,
    pub visited: BTreeSet<String>,
    pub present_entity_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session_id: String,
    pub module_id: String,
    pub module_version: String,
    pub investigator: InvestigatorState,
    pub current_scene: SceneState,
    #[serde(default)]
    pub clocks: BTreeMap<String, ProgressClock>,
    #[serde(default)]
    pub clues: BTreeMap<String, ClueState>,
    #[serde(default)]
    pub handouts: BTreeMap<String, HandoutState>,
    #[serde(default)]
    pub flags: BTreeMap<String, Value>,
    #[serde(default)]
    pub items: BTreeMap<String, i32>,
    #[serde(default)]
    pub skill_successes: BTreeSet<String>,
    #[serde(default)]
    pub events: BTreeSet<String>,
    #[serde(default)]
    pub evidence_presented: BTreeSet<String>,
    #[serde(default)]
    pub topics_confronted: BTreeSet<String>,
    #[serde(default)]
    pub npc_heard_events: BTreeSet<String>,
    #[serde(default)]
    pub npc_known_facts: BTreeSet<String>,
    #[serde(default)]
    pub npc_beliefs: BTreeMap<String, String>,
    #[serde(default)]
    pub npc_relations: BTreeMap<String, i32>,
    #[serde(default)]
    pub raw_truth: Value,
    #[serde(default)]
    pub pending_interrupt: Option<PendingInterrupt>,
    #[serde(default)]
    pub turn_status: Option<TurnStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RollAttempt {
    pub check_id: String,
    pub attempt_index: u8,
    pub push_count: u8,
    pub skill: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_label: Option<String>,
    pub difficulty: CheckDifficulty,
    pub target_number: u32,
    pub bonus_penalty_dice: i8,
    pub roll_value: u32,
    pub result_level: RollResultLevel,
    pub passed: bool,
    pub consequence_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckDifficulty {
    Regular,
    Hard,
    Extreme,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RollResultLevel {
    CriticalSuccess,
    ExtremeSuccess,
    HardSuccess,
    RegularSuccess,
    Failure,
    Fumble,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingInterrupt {
    pub interrupt_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub action_summary: String,
    pub skill: String,
    #[serde(default)]
    pub skill_label: String,
    pub difficulty: CheckDifficulty,
    pub target_number: u32,
    pub bonus_penalty_dice: i8,
    pub stakes_hint: String,
    #[serde(default = "default_true")]
    pub allow_push: bool,
    #[serde(default)]
    pub action_kind: String,
    pub first_roll: Option<RollAttempt>,
    pub recovery: Option<RecoveryEligibility>,
    pub push_description: Option<String>,
    pub push_consequence: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEligibility {
    pub can_accept_failure: bool,
    pub can_spend_luck: bool,
    pub luck_cost_needed: Option<u32>,
    pub can_push_roll: bool,
    pub push_ineligible_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProposedCheck {
    #[serde(default = "default_check_id")]
    pub check_id: String,
    pub skill: String,
    #[serde(default = "default_difficulty")]
    pub difficulty: CheckDifficulty,
    #[serde(default = "default_target_number")]
    pub target_number: u32,
    #[serde(default)]
    pub bonus_penalty_dice: i8,
    #[serde(default = "default_true")]
    pub allow_push: bool,
    #[serde(default = "default_action_kind")]
    pub action_kind: String,
}

fn default_check_id() -> String {
    "skill_check".into()
}

fn default_difficulty() -> CheckDifficulty {
    CheckDifficulty::Regular
}

fn default_target_number() -> u32 {
    50
}

fn default_action_kind() -> String {
    "investigate".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProposedEffect {
    pub effect_type: String,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub value: Option<i32>,
    #[serde(default)]
    pub payload: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActionProposal {
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub target_entity_ids: Vec<String>,
    #[serde(default = "default_action_type")]
    pub action_type: String,
    #[serde(default)]
    pub proposed_checks: Vec<ProposedCheck>,
    #[serde(default)]
    pub proposed_effects: Vec<ProposedEffect>,
    #[serde(default)]
    pub evidence_to_present: Vec<String>,
    #[serde(default)]
    pub topics_to_confront: Vec<String>,
    #[serde(default)]
    pub proposed_world_patches: Vec<DynamicWorldPatch>,
}

fn default_action_type() -> String {
    "observe".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWorldPatch {
    pub patch_id: String,
    pub source: String,
    pub scope: String,
    pub lifetime: String,
    pub causal_event_id: Option<String>,
    pub changes: Vec<WorldChange>,
    pub resource_cost: Option<ResourceCost>,
    pub required_permission: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorldChange {
    pub kind: String,
    pub target_id: String,
    pub value: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCost {
    pub resource: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GameEventKind {
    SessionCreated {
        event_id: String,
        session_id: String,
        seq: u64,
        turn_id: String,
    },
    InvestigatorSelected {
        investigator: InvestigatorState,
    },
    TurnStarted {
        turn_id: String,
    },
    PlayerActionAccepted {
        input: String,
    },
    ActionProposed {
        proposal: ActionProposal,
    },
    RollRequested {
        check: ProposedCheck,
    },
    RollResolved {
        attempt: RollAttempt,
    },
    LuckSpent {
        amount: u32,
    },
    PushRollAccepted {
        description: String,
    },
    ResourceChanged {
        resource: String,
        delta: i32,
    },
    ItemAcquired {
        item: InventoryItem,
    },
    ItemLost {
        item_id: String,
        quantity: i32,
    },
    ClueUnlocked {
        clue: ClueState,
    },
    HandoutReleased {
        handout: HandoutState,
    },
    ClockAdvanced {
        clock_id: String,
        amount: u32,
        reason: String,
        caused_by: String,
    },
    SceneChanged {
        scene: SceneState,
    },
    WorldPatchApplied {
        patch: DynamicWorldPatch,
    },
    InterruptCreated {
        interrupt: PendingInterrupt,
    },
    InterruptResolved {
        interrupt_id: String,
    },
    TurnCommitted,
    NarrationAccepted {
        text: String,
    },
    NpcKnowledgeAcquired {
        entity_id: String,
        fact_id: String,
    },
    NpcBeliefChanged {
        entity_id: String,
        claim: String,
    },
    NpcRelationChanged {
        entity_id: String,
        relation: String,
        value: i32,
    },
    EvidencePresented {
        clue_id: String,
    },
    TopicConfronted {
        topic_id: String,
    },
    EventOccurred {
        event_id: String,
    },
    FlagSet {
        key: String,
        value: Value,
    },
    ConditionAdded {
        condition: String,
    },
    SessionEnded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameEvent {
    pub event_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub seq: u64,
    pub event_type: String,
    pub payload: Value,
    pub module_version: String,
    pub created_at: String,
    pub caused_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameEventRecord {
    pub event_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub seq: u64,
    pub event: GameEventKind,
    pub module_version: String,
    pub created_at: String,
    pub caused_by: Option<String>,
}
