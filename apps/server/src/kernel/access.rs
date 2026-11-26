use super::{ClockVisibility, SessionState};
use serde_json::Value;

pub fn project_client_visible(state: &SessionState) -> Value {
    let mut value = serde_json::to_value(state).expect("state serializes");
    if let Some(obj) = value.as_object_mut() {
        if let Some(clocks) = obj.get_mut("clocks").and_then(Value::as_object_mut) {
            clocks.retain(|_, c| {
                c.get("visibility").and_then(Value::as_str) == Some("player_visible")
            });
        }
        obj.remove("rawTruth");
        obj.remove("flags");
        obj.remove("events");
        obj.remove("npcKnownFacts");
        obj.remove("npcHeardEvents");
    }
    value
}

pub fn visible_clock_count(state: &SessionState) -> usize {
    state
        .clocks
        .values()
        .filter(|c| c.visibility == ClockVisibility::PlayerVisible)
        .count()
}
