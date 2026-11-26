use rand::{rngs::OsRng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub trait DiceRng: Send {
    fn roll_d10(&mut self) -> u8;
}

impl DiceRng for Box<dyn DiceRng> {
    fn roll_d10(&mut self) -> u8 {
        (**self).roll_d10()
    }
}

pub struct OsDiceRng {
    rng: OsRng,
}
impl Default for OsDiceRng {
    fn default() -> Self {
        Self { rng: OsRng }
    }
}
impl DiceRng for OsDiceRng {
    fn roll_d10(&mut self) -> u8 {
        (self.rng.next_u32() % 10) as u8
    }
}

pub struct SeededDiceRng {
    pub rng: ChaCha8Rng,
}
impl SeededDiceRng {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}
impl DiceRng for SeededDiceRng {
    fn roll_d10(&mut self) -> u8 {
        (self.rng.next_u32() % 10) as u8
    }
}

pub fn d100(rng: &mut impl DiceRng, bonus_penalty: i8) -> u32 {
    let units = rng.roll_d10();
    let count = 1 + bonus_penalty.unsigned_abs();
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let raw = 10 * rng.roll_d10() + units;
        values.push(if raw == 0 { 100 } else { raw });
    }
    let value = if bonus_penalty < 0 {
        *values.iter().max().unwrap()
    } else {
        *values.iter().min().unwrap()
    };
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    struct Fixed(Vec<u8>);
    impl DiceRng for Fixed {
        fn roll_d10(&mut self) -> u8 {
            self.0.remove(0)
        }
    }
    #[test]
    fn zero_is_hundred() {
        assert_eq!(d100(&mut Fixed(vec![0, 0]), 0), 100);
    }
    #[test]
    fn bonus_uses_same_units_and_lowest_tens() {
        assert_eq!(d100(&mut Fixed(vec![4, 9, 1]), 1), 14);
    }
    #[test]
    fn penalty_uses_highest_tens() {
        assert_eq!(d100(&mut Fixed(vec![4, 1, 9]), -1), 94);
    }
    #[test]
    fn bonus_orders_zero_as_one_hundred_before_selecting_minimum() {
        assert_eq!(d100(&mut Fixed(vec![0, 0, 3]), 1), 30);
    }
    #[test]
    fn penalty_orders_zero_as_one_hundred_before_selecting_maximum() {
        assert_eq!(d100(&mut Fixed(vec![0, 0, 3]), -1), 100);
    }

    proptest::proptest! {
        #[test]
        fn bonus_zero_candidate_is_never_selected_over_a_nonzero_candidate(tens in 1u8..10) {
            prop_assert_eq!(d100(&mut Fixed(vec![0, 0, tens]), 1), (tens * 10) as u32);
        }

        #[test]
        fn penalty_zero_candidate_remains_the_highest_candidate(tens in 1u8..10) {
            prop_assert_eq!(d100(&mut Fixed(vec![0, 0, tens]), -1), 100);
        }
    }
}
