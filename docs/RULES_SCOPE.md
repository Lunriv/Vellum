# V1 CoC 7e Rules Scope & Deterministic Boundaries

> Synchronized with [ARCHITECTURE.md](ARCHITECTURE.md) v0.3.1 (Section 5 & Section 6.2).
> All rules below are implemented strictly in deterministic Rust code under `apps/server/src/kernel/` and verified by unit and property tests. No copyrighted rulebook prose is bundled in this repository.

## 1. Included in V1

### Core Characteristics & Derived Resources
- Primary characteristics: `STR`, `CON`, `SIZ`, `DEX`, `APP`, `INT`, `POW`, `EDU`, `LUCK` (0–99 scale for V1 investigators).
- Derived resources:
  - `HP_max = floor((CON + SIZ) / 10)`
  - `MP_max = floor(POW / 5)`
  - `SAN_start = POW`, `SAN_max = 99 - CthulhuMythos`
  - `Luck` current / max tracked as mutable investigator resource.
- Skills: map of `skill_id -> rating (1..=99)` (including baseline defaults for common investigative skills).

### Percentile Skill Checks (d100)
- **Difficulty thresholds**:
  - `Regular`: `<= target`
  - `Hard`: `<= floor(target / 2)`
  - `Extreme`: `<= floor(target / 5)`
- **Outcome tiers**:
  - `CriticalSuccess`: `roll == 1`
  - `ExtremeSuccess`: `roll <= floor(target / 5)` (and not Critical)
  - `HardSuccess`: `roll <= floor(target / 2)` (and not Extreme/Critical)
  - `RegularSuccess`: `roll <= target` (and not Hard/Extreme/Critical)
  - `Failure`: `roll > target` (and not Fumble)
  - `Fumble`: `roll == 100`, OR (`target < 50` and `roll >= 96`)
- **Success check**: outcome passes if its tier meets or exceeds required difficulty (and is not `Fumble`).
- **Bonus / Penalty Dice**:
  - Units die (`0..=9`) rolled once; tens die rolled `1 + abs(modifier)` times (`00` + `0` = `100`).
  - Bonus dice pick the lowest resulting d100 value; penalty dice pick the highest resulting d100 value.
  - V1 caps net bonus/penalty dice at `[-2, +2]` unless a scenario rule explicitly specifies otherwise.

### Recovery Options After Initial Failure
Eligibility is computed solely by Kernel rules (never by LLM):
1. **Accept Failure**: Always allowed; triggers Fail Forward / normal failure resolution.
2. **Spend Luck**:
   - Allowed only on eligible non-Sanity, non-Luck, non-Fumble skill checks where `needed = roll_value - threshold_for_required_difficulty` satisfies `1 <= needed <= current_luck`.
   - Not allowed to upgrade into a `CriticalSuccess` (roll 1) or to recover from a `Fumble`.
   - Not allowed after a Pushed roll (`attempt_index > 1`).
   - Luck deduction and upgraded check result commit in the same SQLite transaction.
3. **Push the Roll**:
   - Allowed only on eligible non-Combat, non-Sanity, non-Luck skill checks when `push_count == 0`, scene/check rules allow pushing, and the initial roll is not ineligible by scenario constraint.
   - Enforced multi-step state machine:
     1. Initial failed roll persisted BEFORE display (`AwaitingRecovery`).
     2. Player selects Push -> `AwaitingPushDescription` (must describe a concrete new approach).
     3. Kernel/KP validates plausibility & states explicit escalated failure consequence -> `AwaitingPushConsequenceAck`.
     4. Player may still back out and accept the initial failure, OR confirm risk -> `AwaitingPushRoll`.
     5. Kernel performs second real roll (`attempt_index = 2, push_count = 1`), persisted before narration; direct commit with NO further Luck or Push allowed.

### Sanity Checks & Temporary Insanity (V1 Subset)
- d100 roll against current `SAN` (no Luck spend, no Push).
- Success applies minor loss expression; failure applies major loss expression (`Fumble` applies max loss).
- Single-loss `>= 5` triggers an `INT` check (idea roll): if `INT` check succeeds, investigator grasps the horror and enters **Temporary Insanity** (represented as a structured condition + involuntary narrative guardrail tag, without taking over long-term player agency).
- `SAN` clamped to `[0, SAN_max]`; `SAN == 0` marks investigator permanently incapacitated / triggers scenario end condition.

### Simplified Narrative Combat (V1 Subset)
- Opposed melee / dodge / fight-back resolution using standard difficulty success tiers:
  - Attacker vs Defender Dodge: Defender wins ties.
  - Attacker vs Defender Fight Back: Attacker wins ties (both compare best achieved success tier).
- Basic HP loss, Major Wound flag (single hit `>= floor(HP_max / 2)`), Dying/Unconscious boundary (`HP == 0`).

### Inventory, Resources & Progress Clocks
- Deterministic item acquisition, consumption, and possession checks (`has_item`).
- Progress Clocks: `current / max`, `visibility` (`player_visible` | `keeper_hidden`), threshold triggers, mandatory `reason` + `caused_by` event reference.

## 2. Explicitly Deferred Beyond V1
- Automatic firearm burst/full-auto tables and complex malfunction tables.
- Tactical chase grids and hex/square battle maps.
- Long-term inter-scenario skill improvement / downtime training.
- Full psychotherapy and indefinite/long-term insanity recovery arcs.
- Multi-investigator group combat sequencing.
- Complete occupation catalog, weapon encyclopedia, and pulp/supplement optional rules.

## 3. Resolved Rules Boundaries (Confirmed for V1)
- **Fumble blocks recovery**: A `Fumble` on an investigative skill check unconditionally blocks both `Spend Luck` and `Push the Roll` in V1 (enforced in `apps/server/src/kernel/rules.rs` and verified by `proptest`).
- **Luck spend on Hard/Extreme checks**: When a check explicitly requires Hard or Extreme difficulty and the initial roll is not a `Fumble`, spending Luck to lower the roll to the required difficulty threshold (`floor(target / 2)` or `floor(target / 5)`) is permitted when `1 <= roll - threshold <= current_luck`.
