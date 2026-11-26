use super::{DynamicWorldPatch, SessionState};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    EmptyId,
    Duplicate,
    ImmutablePast,
    UnlocksUnknownClue,
    ResourceUnavailable,
    MissingAbility,
    PermissionDenied,
    MissingCausality,
    TemporaryEntityIncomplete,
    MissingCausalRecord,
}

pub fn validate_patch(
    patch: &DynamicWorldPatch,
    state: &SessionState,
    submitted: &BTreeSet<String>,
) -> Result<(), PatchError> {
    if patch.patch_id.trim().is_empty() {
        return Err(PatchError::EmptyId);
    }
    if submitted.contains(&patch.patch_id) {
        return Err(PatchError::Duplicate);
    }
    if patch.scope == "immutable_past" || patch.changes.iter().any(|c| c.kind == "immutable_past") {
        return Err(PatchError::ImmutablePast);
    }
    if patch
        .changes
        .iter()
        .any(|c| c.kind == "unlock_core_clue" && !state.clues.contains_key(&c.target_id))
    {
        return Err(PatchError::UnlocksUnknownClue);
    }
    if let Some(cost) = &patch.resource_cost {
        if state.items.get(&cost.resource).copied().unwrap_or(0) < cost.amount {
            return Err(PatchError::ResourceUnavailable);
        }
    }
    if patch.changes.iter().any(|c| {
        c.kind == "assign_ability" && !state.investigator.skills.contains_key(&c.target_id)
    }) {
        return Err(PatchError::MissingAbility);
    }
    if patch.required_permission.is_some() {
        return Err(PatchError::PermissionDenied);
    }
    if patch.causal_event_id.is_none()
        && patch
            .changes
            .iter()
            .any(|c| c.kind.contains("crisis") || c.kind.contains("major"))
    {
        return Err(PatchError::MissingCausality);
    }
    if patch.lifetime == "temporary"
        && (patch.source.trim().is_empty() || patch.scope.trim().is_empty())
    {
        return Err(PatchError::TemporaryEntityIncomplete);
    }
    if patch.changes.iter().any(|c| c.kind == "major_change") && patch.causal_event_id.is_none() {
        return Err(PatchError::MissingCausalRecord);
    }
    Ok(())
}
