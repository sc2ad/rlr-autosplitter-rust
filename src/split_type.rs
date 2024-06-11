#[derive(Debug, Clone, Copy)]
pub enum SplitType {
    Manual = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Bot2000 = 4,
    Level4 = 5,
    Level5 = 6,
    Level6 = 7,
    Odin = 8,
    Level7 = 9,
    Level8 = 10,
    Level9 = 11,
    Diablo = 12,
    CowLevel = 13,
    ExpGained = 100,
    RawLevelComplete,
    LevelComplete,
    BossComplete,
    // TODO: Add splits for energy feeding for b2k, odin (+ healing), diablo chaser hit 1 and 3
    // TODO: Once we move away from JUST exp, add splits for b2k rocks, odin flood phases, diablo p1/p2 on insane, etc.
}

/// Values for the difficulties are used as the multipliers
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Difficulty {
    Normal = 1,
    Hard = 2,
    Insane = 3,
}

// Largest EXP difference is diablo on insane win for a total of 900 exp in one tick
pub const LARGEST_EXP_DIFFERENCE: i32 = 300 * 3;

impl SplitType {
    /// Returns true if we need the difficulty to be computed for the current split type
    pub fn need_difficulty(&self) -> bool {
        !matches!(self, SplitType::Manual | SplitType::ExpGained)
    }
    pub fn is_normal_level(&self) -> bool {
        matches!(
            self,
            SplitType::Level1
                | SplitType::Level2
                | SplitType::Level3
                | SplitType::Level4
                | SplitType::Level5
                | SplitType::Level6
                | SplitType::Level7
                | SplitType::Level8
                | SplitType::Level9
        )
    }
    pub fn is_boss_level(&self) -> bool {
        matches!(
            self,
            SplitType::Bot2000 | SplitType::Odin | SplitType::Diablo | SplitType::CowLevel
        )
    }
    /// Returns the amount of exp granted for a single pad, or None otherwise
    pub fn per_pad_exp(&self, difficulty: Difficulty) -> Option<i32> {
        match self {
            // Boss levels
            SplitType::Bot2000 => Some(75 * difficulty as i32),
            SplitType::Odin => Some(150 * difficulty as i32),
            SplitType::Diablo => Some(300 * difficulty as i32),
            SplitType::CowLevel => Some(100 * difficulty as i32),
            // Standard levels
            // Level 9 on insane mode breaks from the pattern
            SplitType::Level9 => Some(
                if difficulty == Difficulty::Insane {
                    13
                } else {
                    12
                } * difficulty as i32,
            ),
            SplitType::Level1
            | SplitType::Level2
            | SplitType::Level3
            | SplitType::Level4
            | SplitType::Level5
            | SplitType::Level6
            | SplitType::Level7
            | SplitType::Level8 => Some((1 + *self as i32) * difficulty as i32),
            SplitType::Manual
            | SplitType::ExpGained
            | SplitType::BossComplete
            | SplitType::RawLevelComplete
            | SplitType::LevelComplete => None,
        }
    }
    /// Returns the boss completion exp, panicking if the current level type is not a boss level
    pub fn boss_exp(&self, difficulty: Difficulty) -> i32 {
        match self {
            SplitType::Bot2000 | SplitType::Odin | SplitType::Diablo | SplitType::CowLevel => {
                self.per_pad_exp(difficulty).expect("unreachable")
            }
            _ => panic!("boss_exp cannot be called with: {self:?} for difficulty: {difficulty:?}"),
        }
    }
    pub fn standard_exp(&self, difficulty: Difficulty) -> i32 {
        match self {
            SplitType::Level1
            | SplitType::Level2
            | SplitType::Level3
            | SplitType::Level4
            | SplitType::Level5
            | SplitType::Level6
            | SplitType::Level7
            | SplitType::Level8
            | SplitType::Level9 => self.per_pad_exp(difficulty).expect("unreachable"),
            _ => panic!(
                "standard_exp cannot be called with: {self:?} for difficulty: {difficulty:?}"
            ),
        }
    }
    /// Modifies the current to be the next raw level, if there is one that exists
    pub fn next(&self) -> SplitType {
        match self {
            SplitType::Level1 => SplitType::Level2,
            SplitType::Level2 => SplitType::Level3,
            SplitType::Level3 => SplitType::Bot2000,
            SplitType::Bot2000 => SplitType::Level4,
            SplitType::Level4 => SplitType::Level5,
            SplitType::Level5 => SplitType::Level6,
            SplitType::Level6 => SplitType::Odin,
            SplitType::Odin => SplitType::Level7,
            SplitType::Level7 => SplitType::Level8,
            SplitType::Level8 => SplitType::Level9,
            SplitType::Level9 => SplitType::Diablo,
            SplitType::Diablo => SplitType::CowLevel,
            // Loop back around for cowlevel
            SplitType::CowLevel => SplitType::Level1,
            _ => panic!("Cannot call next on a non-level split-type: {self:?}"),
        }
    }
}
/// Returns true if any boss level was just completed for this difficulty
pub fn any_boss(difference: i32, difficulty: Difficulty) -> bool {
    SplitType::Bot2000.boss_exp(difficulty) == difference
        || SplitType::Odin.boss_exp(difficulty) == difference
        || SplitType::Diablo.boss_exp(difficulty) == difference
        || SplitType::CowLevel.boss_exp(difficulty) == difference
}
