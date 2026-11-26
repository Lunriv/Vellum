use super::{ClockThreshold, ClockVisibility, ProgressClock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockAdvance {
    pub previous: u32,
    pub current: u32,
    pub stage_label: String,
}

pub fn advance_clock(
    clock: &mut ProgressClock,
    amount: u32,
    reason: impl Into<String>,
    caused_by: impl Into<String>,
) -> Result<ClockAdvance, String> {
    let reason = reason.into();
    if reason.trim().is_empty() {
        return Err("clock advance requires reason".into());
    }
    let caused_by = caused_by.into();
    if caused_by.trim().is_empty() {
        return Err("clock advance requires caused_by".into());
    }
    let previous = clock.current;
    clock.current = clock.current.saturating_add(amount).min(clock.max);
    clock.last_reason = Some(reason);
    clock.last_caused_by = Some(caused_by);
    Ok(ClockAdvance {
        previous,
        current: clock.current,
        stage_label: resolve_stage(clock),
    })
}
pub fn resolve_stage(clock: &ProgressClock) -> String {
    clock
        .thresholds
        .iter()
        .filter(|t| t.at <= clock.current)
        .max_by_key(|t| t.at)
        .map(|t| t.stage_label.clone())
        .unwrap_or_else(|| "未开始".into())
}
pub fn current_stage_label(clock: &ProgressClock) -> String {
    resolve_stage(clock)
}
pub fn visible_clock(clock: &ProgressClock) -> bool {
    clock.visibility == ClockVisibility::PlayerVisible
}
pub fn default_thresholds(max: u32) -> Vec<ClockThreshold> {
    vec![
        ClockThreshold {
            at: 0,
            stage_label: "未开始".into(),
        },
        ClockThreshold {
            at: max,
            stage_label: "完成".into(),
        },
    ]
}
