use super::{CheckDifficulty, DiceRng, InvestigatorState, RollAttempt, RollResultLevel};

pub fn threshold(target: u32, difficulty: CheckDifficulty) -> u32 {
    match difficulty {
        CheckDifficulty::Regular => target,
        CheckDifficulty::Hard => target / 2,
        CheckDifficulty::Extreme => target / 5,
    }
}

pub fn classify_roll(target: u32, _difficulty: CheckDifficulty, roll: u32) -> RollResultLevel {
    if roll == 1 {
        return RollResultLevel::CriticalSuccess;
    }
    if roll == 100 || (target < 50 && roll >= 96) {
        return RollResultLevel::Fumble;
    }
    if roll <= target / 5 {
        RollResultLevel::ExtremeSuccess
    } else if roll <= target / 2 {
        RollResultLevel::HardSuccess
    } else if roll <= target {
        RollResultLevel::RegularSuccess
    } else {
        RollResultLevel::Failure
    }
}

fn result_rank(level: &RollResultLevel) -> u8 {
    match level {
        RollResultLevel::Fumble | RollResultLevel::Failure => 0,
        RollResultLevel::RegularSuccess => 1,
        RollResultLevel::HardSuccess => 2,
        RollResultLevel::ExtremeSuccess => 3,
        RollResultLevel::CriticalSuccess => 4,
    }
}

fn required_rank(difficulty: &CheckDifficulty) -> u8 {
    match difficulty {
        CheckDifficulty::Regular => 1,
        CheckDifficulty::Hard => 2,
        CheckDifficulty::Extreme => 3,
    }
}

pub fn passes_difficulty(level: &RollResultLevel, difficulty: &CheckDifficulty) -> bool {
    !matches!(level, RollResultLevel::Fumble) && result_rank(level) >= required_rank(difficulty)
}

#[allow(clippy::too_many_arguments)]
pub fn roll_check(
    rng: &mut impl DiceRng,
    check_id: String,
    skill: String,
    target: u32,
    difficulty: CheckDifficulty,
    bonus_penalty_dice: i8,
    attempt_index: u8,
    push_count: u8,
) -> RollAttempt {
    let value = super::dice::d100(rng, bonus_penalty_dice.clamp(-2, 2));
    let result_level = classify_roll(target, difficulty, value);
    let passed = passes_difficulty(&result_level, &difficulty);
    RollAttempt {
        check_id,
        attempt_index,
        push_count,
        skill: skill.clone(),
        skill_label: Some(skill_display_label(&skill).into()),
        difficulty,
        target_number: target,
        bonus_penalty_dice,
        roll_value: value,
        result_level,
        passed,
        consequence_ref: None,
    }
}

pub fn skill_display_label(skill: &str) -> &'static str {
    match skill {
        "spot_hidden" => "侦查",
        "psychology" => "心理学",
        "persuade" => "说服",
        "locksmith" => "锁匠",
        "medicine" => "医学",
        "science" => "科学",
        "intimidate" => "恐吓",
        "fighting" | "fighting_brawl" => "格斗",
        "dodge" => "闪避",
        "listen" => "聆听",
        "library_use" => "图书馆使用",
        "stealth" => "潜行",
        "occult" => "神秘学",
        _ => "调查技能",
    }
}

pub fn evaluate_recovery_options(
    attempt: &RollAttempt,
    current_luck: u32,
    is_sanity: bool,
    is_luck: bool,
    is_combat: bool,
    allow_push: bool,
) -> super::RecoveryEligibility {
    let fumble = matches!(attempt.result_level, RollResultLevel::Fumble);
    let required = threshold(attempt.target_number, attempt.difficulty);
    let needed = attempt.roll_value.saturating_sub(required);
    let can_luck = !attempt.passed
        && !fumble
        && attempt.attempt_index == 1
        && !is_sanity
        && !is_luck
        && needed >= 1
        && needed <= current_luck;
    let push_reason = if attempt.attempt_index != 1 || attempt.push_count >= 1 {
        Some("push_limit_reached".into())
    } else if is_combat {
        Some("combat_check".into())
    } else if is_sanity {
        Some("sanity_check".into())
    } else if is_luck {
        Some("luck_check".into())
    } else if fumble {
        Some("fumble".into())
    } else if !allow_push {
        Some("scene_disallows_push".into())
    } else {
        None
    };
    super::RecoveryEligibility {
        can_accept_failure: true,
        can_spend_luck: can_luck,
        luck_cost_needed: can_luck.then_some(needed),
        can_push_roll: push_reason.is_none(),
        push_ineligible_reason: push_reason,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanityOutcome {
    pub loss: u32,
    pub temporary_insanity: bool,
    pub permanent_incapacitation: bool,
}
pub fn sanity_check(
    rng: &mut impl DiceRng,
    investigator: &mut InvestigatorState,
    loss_on_success: u32,
    loss_on_failure: u32,
    int_target: u32,
) -> SanityOutcome {
    let roll = super::dice::d100(rng, 0);
    let loss = if roll <= investigator.san as u32 {
        loss_on_success
    } else if roll == 100 {
        loss_on_failure.max(loss_on_success)
    } else {
        loss_on_failure
    };
    investigator.san = investigator.san.saturating_sub(loss as i32);
    let temporary = loss >= 5 && super::dice::d100(rng, 0) <= int_target;
    if temporary {
        investigator.conditions.insert("temporary_insanity".into());
    }
    SanityOutcome {
        loss,
        temporary_insanity: temporary,
        permanent_incapacitation: investigator.san == 0,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatOutcome {
    pub attacker_success: bool,
    pub defender_success: bool,
    pub attacker_wins: bool,
    pub damage: u32,
    pub major_wound: bool,
    pub defender_unconscious: bool,
}
pub fn opposed_combat(
    attacker: &RollAttempt,
    defender: &RollAttempt,
    fight_back: bool,
    defender_hp: u32,
    defender_hp_max: u32,
    damage: u32,
) -> CombatOutcome {
    let rank = |r: &RollResultLevel| match r {
        RollResultLevel::Fumble => 0,
        RollResultLevel::Failure => 1,
        RollResultLevel::RegularSuccess => 2,
        RollResultLevel::HardSuccess => 3,
        RollResultLevel::ExtremeSuccess => 4,
        RollResultLevel::CriticalSuccess => 5,
    };
    let a = rank(&attacker.result_level);
    let d = rank(&defender.result_level);
    let attacker_wins = if fight_back { a >= d } else { a > d };
    let dealt = attacker_wins
        && attacker.result_level != RollResultLevel::Failure
        && attacker.result_level != RollResultLevel::Fumble;
    CombatOutcome {
        attacker_success: attacker.passed,
        defender_success: defender.passed,
        attacker_wins: dealt,
        damage: if dealt { damage } else { 0 },
        major_wound: dealt && damage >= defender_hp_max / 2,
        defender_unconscious: dealt && damage >= defender_hp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn thresholds_and_fumble() {
        assert_eq!(threshold(60, CheckDifficulty::Hard), 30);
        assert_eq!(
            classify_roll(40, CheckDifficulty::Regular, 97),
            RollResultLevel::Fumble
        );
        assert_eq!(
            classify_roll(60, CheckDifficulty::Regular, 96),
            RollResultLevel::Failure
        );
        assert_eq!(
            classify_roll(60, CheckDifficulty::Regular, 1),
            RollResultLevel::CriticalSuccess
        );
    }

    #[test]
    fn classification_uses_original_target_before_difficulty() {
        assert_eq!(
            classify_roll(60, CheckDifficulty::Hard, 31),
            RollResultLevel::RegularSuccess
        );
        assert!(!passes_difficulty(
            &classify_roll(60, CheckDifficulty::Hard, 31),
            &CheckDifficulty::Hard
        ));
        assert_eq!(
            classify_roll(60, CheckDifficulty::Extreme, 30),
            RollResultLevel::HardSuccess
        );
        assert!(!passes_difficulty(
            &classify_roll(60, CheckDifficulty::Extreme, 30),
            &CheckDifficulty::Extreme
        ));
        assert!(passes_difficulty(
            &classify_roll(60, CheckDifficulty::Extreme, 12),
            &CheckDifficulty::Extreme
        ));
    }

    proptest::proptest! {
        #[test]
        fn ordinary_tiers_are_independent_of_requested_difficulty(
            target in 2u32..99,
            roll in 2u32..95,
        ) {
            let regular = classify_roll(target, CheckDifficulty::Regular, roll);
            let hard = classify_roll(target, CheckDifficulty::Hard, roll);
            let extreme = classify_roll(target, CheckDifficulty::Extreme, roll);
            prop_assert_eq!(regular, hard);
            prop_assert_eq!(hard, extreme);
        }

        #[test]
        fn fumbles_never_pass_any_difficulty(target in 1u32..49) {
            let level = classify_roll(target, CheckDifficulty::Extreme, 96);
            prop_assert_eq!(level, RollResultLevel::Fumble);
            prop_assert!(!passes_difficulty(&level, &CheckDifficulty::Regular));
            prop_assert!(!passes_difficulty(&level, &CheckDifficulty::Hard));
            prop_assert!(!passes_difficulty(&level, &CheckDifficulty::Extreme));
        }
    }
}
