//! Pure in-memory context assembly, NPC Knowledge Provenance filtering,
//! Bluff isolation, and Protected Truth stage authorization.
//!
//! Boundary rule: this module must NEVER import `sqlx` or `crate::persistence`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::kernel::{ActionContext, SessionState};
use crate::modules::{evaluate_module_predicate, NpcDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEntryView {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpcBeliefView {
    pub entity_id: String,
    pub source_transcript_id: Option<String>,
    pub claim_summary: String,
    pub belief_status: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnverifiedPlayerClaim {
    pub source_transcript_id: String,
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedTruthDisclosure {
    pub secret_id: String,
    pub tier: String,
    pub truth: String,
    pub releasable_fact_ids: Vec<String>,
}

/// Safe prompt context for a single NPC.
/// Strictly omits any unauthorized `ProtectedTruth.raw_truth` and filters out
/// transcripts the NPC did not legitimately hear/see or acquire through provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpcContextPacket {
    pub entity_id: String,
    pub name: String,
    pub personality: String,
    pub working_knowledge: Vec<String>,
    pub cover_story: String,
    pub behavioral_tells: Vec<String>,
    pub authorized_disclosures: Vec<AuthorizedTruthDisclosure>,
    pub filtered_transcript: Vec<TranscriptEntryView>,
    pub unverified_player_claims: Vec<UnverifiedPlayerClaim>,
    pub subjective_beliefs: Vec<NpcBeliefView>,
}

/// Assemble a strictly filtered cognitive packet for one NPC.
///
/// Security invariants:
/// - `raw_truth` is NEVER copied into `NpcContextPacket`.
/// - Transcript entries require non-empty `scene_id` and explicit audience/visual membership
///   (`audible_entity_ids` or `visible_entity_ids`), or an already-acquired canonical event/fact.
/// - `player_assertion_unverified` claims heard by this NPC are separated into
///   `unverified_player_claims` with an explicit disclaimer and never treated as world facts.
pub fn assemble_npc_context(
    npc: &NpcDefinition,
    state: &SessionState,
    action_ctx: Option<&ActionContext>,
    transcripts: &[TranscriptEntryView],
    beliefs: &[NpcBeliefView],
    acquired_fact_ids: &BTreeSet<String>,
) -> NpcContextPacket {
    let mut authorized_disclosures = Vec::new();
    let npc_present_in_action = action_ctx
        .map(|ctx| ctx.present_entity_ids.contains(&npc.entity_id))
        .unwrap_or(false);

    for secret in &npc.protected_truths {
        for stage in &secret.stages {
            let scoped_action = if npc_present_in_action {
                action_ctx
            } else {
                None
            };
            if evaluate_module_predicate(&stage.predicate, state, scoped_action) {
                authorized_disclosures.push(AuthorizedTruthDisclosure {
                    secret_id: secret.secret_id.clone(),
                    tier: stage.tier.clone(),
                    truth: stage.truth.clone(),
                    releasable_fact_ids: stage.releasable_fact_ids.clone(),
                });
            }
        }
    }

    let mut filtered_transcript = Vec::new();
    let mut unverified_player_claims = Vec::new();

    for entry in transcripts {
        if entry.scene_id.trim().is_empty() {
            // Missing scene metadata -> fail closed (deny).
            continue;
        }
        let heard_directly = entry
            .audible_entity_ids
            .iter()
            .any(|id| id == &npc.entity_id);
        let saw_directly = entry
            .visible_entity_ids
            .iter()
            .any(|id| id == &npc.entity_id);
        let acquired_via_event = entry.referenced_event_ids.iter().any(|ev| {
            state
                .npc_heard_events
                .contains(&format!("{}:{ev}", npc.entity_id))
                || acquired_fact_ids.contains(ev)
        });

        let allowed = heard_directly || saw_directly || acquired_via_event;

        if !allowed {
            continue;
        }

        if entry.claim_kind == "player_assertion_unverified" && heard_directly {
            unverified_player_claims.push(UnverifiedPlayerClaim {
                source_transcript_id: entry.entry_id.clone(),
                label: "玩家声称的未证实陈述（非世界权威事实）".into(),
                content: entry.content.clone(),
            });
        }

        filtered_transcript.push(entry.clone());
    }

    let subjective_beliefs = beliefs
        .iter()
        .filter(|b| b.entity_id == npc.entity_id)
        .cloned()
        .collect();

    let mut working_knowledge = npc.working_knowledge.clone();
    for fact_id in acquired_fact_ids {
        working_knowledge.push(format!("verified_fact:{fact_id}"));
    }

    NpcContextPacket {
        entity_id: npc.entity_id.clone(),
        name: npc.name.clone(),
        personality: npc.personality.clone(),
        working_knowledge,
        cover_story: npc.cover_story.clone(),
        behavioral_tells: npc.behavioral_tells.clone(),
        authorized_disclosures,
        filtered_transcript,
        unverified_player_claims,
        subjective_beliefs,
    }
}

/// Render an NPC context packet into a prompt string for inspection and red-team testing.
pub fn render_npc_prompt(packet: &NpcContextPacket) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "NPC: {} ({})\nPersonality: {}\nCover Story: {}\n",
        packet.name, packet.entity_id, packet.personality, packet.cover_story
    ));
    if !packet.behavioral_tells.is_empty() {
        out.push_str("Behavioral Tells:\n");
        for tell in &packet.behavioral_tells {
            out.push_str(&format!("- {tell}\n"));
        }
    }
    if !packet.working_knowledge.is_empty() {
        out.push_str("Authorized Working Knowledge:\n");
        for item in &packet.working_knowledge {
            out.push_str(&format!("- {item}\n"));
        }
    }
    if !packet.authorized_disclosures.is_empty() {
        out.push_str("Authorized Stage Disclosures:\n");
        for stage in &packet.authorized_disclosures {
            out.push_str(&format!("- [{}] {}\n", stage.tier, stage.truth));
        }
    }
    if !packet.unverified_player_claims.is_empty() {
        out.push_str("Unverified Player Claims (NOT canonical facts):\n");
        for claim in &packet.unverified_player_claims {
            out.push_str(&format!("- {}: {}\n", claim.label, claim.content));
        }
    }
    if !packet.filtered_transcript.is_empty() {
        out.push_str("Directly Heard/Seen History:\n");
        for entry in &packet.filtered_transcript {
            out.push_str(&format!("- [{}] {}\n", entry.scene_id, entry.content));
        }
    }
    out
}
