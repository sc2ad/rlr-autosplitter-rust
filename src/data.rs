use asr::{future::next_tick, watcher::Pair, Address, Process};

use crate::{
    log,
    sigscan::find_exp_pattern,
    split_state::SplitState,
    split_type::{any_boss, Difficulty, SplitType, LARGEST_EXP_DIFFERENCE},
};

const PAD_COUNT: i32 = 19;

pub struct GameData<'a> {
    process: &'a Process,
    exp_pointer: Option<Address>,
    current_exp: Option<i32>,
    level: SplitType,
    current_pad: i32,
    valid: bool,
    difficulty: Option<Difficulty>,
}

#[derive(Copy, Clone)]
pub struct StateChange {
    pads: Pair<i32>,
    exps: Pair<Option<i32>>,
    levels: Pair<SplitType>,
    valid: Pair<bool>,
    difficulty: Pair<Option<Difficulty>>,
}

async fn find_and_ret_pattern(process: &Process) -> Address {
    loop {
        match find_exp_pattern(process).await {
            Some(dat) => return dat,
            None => next_tick().await,
        };
    }
}

impl<'a> GameData<'a> {
    pub async fn new(process: &'a Process) -> GameData<'a> {
        // Try to find the address in the process
        Self {
            process,
            exp_pointer: Some(find_and_ret_pattern(process).await),
            current_exp: None,
            level: SplitType::Level1,
            current_pad: 0,
            valid: true,
            difficulty: None,
        }
    }
}
impl GameData<'_> {
    /// Rescans the process for the exp pattern, returns true if still present, false otherwise
    /// This will allow you to rescan for the address after we have either lost it, or in order to reset.
    pub async fn rescan(&mut self) -> bool {
        match find_exp_pattern(self.process).await {
            Some(val) => {
                self.exp_pointer = Some(val);
                true
            }
            None => {
                self.exp_pointer = None;
                false
            }
        }
    }
    fn read_exp(&mut self) -> Option<i32> {
        if let Some(ptr) = self.exp_pointer {
            match self.process.read::<i32>(ptr) {
                // Exp is stored as a multiple of 4096, so compute that here
                Ok(val) => {
                    if val % 4096 != 0 {
                        log!("Invalidating because we read back exp: {val} that was not a multiple of 4096!");
                        self.invalidate();
                        None
                    } else {
                        Some(val / 4096)
                    }
                }
                _ => {
                    log!("Process read failed for exp read!");
                    self.invalidate();
                    None
                }
            }
        } else {
            log!("Exp pointer is invalid for exp read!");
            self.invalidate();
            None
        }
    }
    fn invalidate(&mut self) {
        self.valid = false;
    }
    /// Returns if we are valid or not. If we are not valid, we should make a new instance of this type and rescan.
    /// We may also want to reset the timer and the game time, etc.
    pub fn invalid(&self) -> bool {
        !self.valid
    }
    pub fn exp(&self) -> Option<i32> {
        self.current_exp
    }
    /// Returns the exp difference, if present. If garbage or invalid, None is returned and the state is reset.
    fn update_exp(&mut self) -> Option<i32> {
        if let Some(exp) = self.read_exp() {
            let difference = if let Some(old_exp) = self.current_exp {
                exp - old_exp
            } else {
                log!("Initial exp read as: {exp}");
                0
            };
            self.current_exp = Some(exp);
            if !(0..=LARGEST_EXP_DIFFERENCE).contains(&difference) {
                // Invalid difference
                log!("Resetting state because we read an exp difference: {difference} that makes no sense!");
                self.invalidate();
                return None;
            }
            // If we had enough exp for a pad, we increment our pad counter by 1.
            match self.difficulty {
                Some(diff) => {
                    if self.level.is_normal_level() {
                        // If we have obtained exactly enough exp for a pad, increment our pad counter
                        // TODO: Note that this only works if WE are the ones going through the level
                        if difference
                            == self
                                .level
                                .per_pad_exp(diff)
                                .expect("Level is a normal level, so must have valid pad exp")
                        {
                            let pad = self.current_pad;
                            let new_pad = self.current_pad + 1;
                            log!("Crossed pad! Previous pad was: {pad}, pad just crossed is: {new_pad}!");
                            self.current_pad = new_pad;
                        }
                    }
                }
                None => {
                    // Determine difficulty from exp and set pad accordingly
                    // TODO: Note that this only works if WE are the ones going through the level
                    // TODO: Some compile time check to ensure we capture all difficulties as we iterate
                    if self.level.is_normal_level() {
                        if difference == self.level.standard_exp(Difficulty::Normal) {
                            log!("Determined difficulty to be Normal!");
                            self.difficulty = Some(Difficulty::Normal);
                            self.current_pad = 1;
                        } else if difference == self.level.standard_exp(Difficulty::Hard) {
                            log!("Determined difficulty to be Hard!");
                            self.difficulty = Some(Difficulty::Hard);
                            self.current_pad = 1;
                        } else if difference == self.level.standard_exp(Difficulty::Insane) {
                            log!("Determined difficulty to be Insane!");
                            self.difficulty = Some(Difficulty::Insane);
                            self.current_pad = 1;
                        }
                        // If we cannot match the difficulty, we give up and continue with it as None
                    }
                }
            };
            return Some(difference);
        }
        None
    }
    // TODO: The way this function is written is not conductive to midgame runs or practice.
    // This is because it is assumed that the split is not relevant for the update of this logic.
    // Sometimes, however, the split/game info is useful in telling us things like the difficulty, level, pad count, etc.
    // That's not that easy to do at the moment, so for now, we will leave this assumption in place.
    pub fn update(&mut self) -> StateChange {
        // Initial point is the current state
        let old_level = self.level;
        let old_pad = self.current_pad;
        let old_exp = self.current_exp;
        let old_valid = self.valid;
        let old_diff = self.difficulty;
        // Update our exp
        self.update_exp();
        // Capture new state info
        if !self.invalid() {
            // Check to see if we need to complete a level based off of pad or exp
            if self.current_pad == PAD_COUNT {
                let old_level = self.level;
                self.level = self.level.next();
                let level = self.level;
                log!("Level complete! Was: {old_level:?} now is: {level:?}");
                self.current_pad = 0;
            } else if self.level.is_boss_level() {
                // Check the difference here and increment the level if so
                if let Some(diff) = self.difficulty {
                    match (old_exp, self.current_exp) {
                        (Some(old), Some(current)) => {
                            if current - old == self.level.boss_exp(diff) {
                                let old_level = self.level;
                                self.level = self.level.next();
                                let level = self.level;
                                log!("Boss complete! Was: {old_level:?} now is: {level:?}");
                            }
                        }
                        _ => self.invalidate(),
                    }
                }
            }
        }
        StateChange {
            levels: Pair {
                old: old_level,
                current: self.level,
            },
            pads: Pair {
                old: old_pad,
                current: self.current_pad,
            },
            exps: Pair {
                old: old_exp,
                current: self.current_exp,
            },
            valid: Pair {
                old: old_valid,
                current: self.valid,
            },
            difficulty: Pair {
                old: old_diff,
                current: self.difficulty,
            },
        }
    }
}

impl StateChange {
    /// Returns true if the exp gained was exactly equivalent to a split type
    pub fn should_split(&self, split_state: &mut SplitState, split: SplitType) -> bool {
        if !self.valid.current {
            false
        } else if let Some(diff) = self.difficulty.current {
            match (self.exps.old, self.exps.current) {
                (Some(old), Some(current)) => {
                    let exp_difference = current - old;
                    let raw_level_change = self.levels.old != self.levels.current;
                    // TODO: For some of these, we could try doing EITHER an exp change OR tracking the level from the beginning.
                    // If we track from the beginning, things are cleaner, but that may not always be possible.
                    // For example, we could be practicing some splits, or we may have opened the timer in the middle of a game.
                    // So, it's probably safer to err on the side of exp difference wherever possible, as opposed to the logical level transitions.
                    match split {
                        SplitType::Manual => false,
                        SplitType::ExpGained => exp_difference > 0,
                        SplitType::RawLevelComplete => raw_level_change,
                        SplitType::LevelComplete => {
                            raw_level_change && self.levels.old.is_normal_level()
                        }
                        SplitType::BossComplete => any_boss(exp_difference, diff),
                        // All individual level splits fit in this call
                        SplitType::Level1
                        | SplitType::Level2
                        | SplitType::Level3
                        | SplitType::Level4
                        | SplitType::Level5
                        | SplitType::Level6
                        | SplitType::Level7
                        | SplitType::Level8
                        | SplitType::Level9
                        | SplitType::Bot2000
                        | SplitType::Odin
                        | SplitType::Diablo
                        | SplitType::CowLevel => {
                            (split.is_boss_level() && split.boss_exp(diff) == exp_difference)
                                || (raw_level_change && self.levels.old == split)
                        }
                        SplitType::Bot2000Cube => {
                            self.levels.current == SplitType::Bot2000
                                && exp_difference == SplitType::Bot2000.cube_exp(diff)
                        }
                        SplitType::OdinCube => {
                            self.levels.current == SplitType::Odin
                                && exp_difference == SplitType::Odin.cube_exp(diff)
                        }
                        SplitType::PadCrossed => self.pads.old != self.pads.current,
                        // Discriminated types
                        SplitType::PadCrossedForLevel { raw_level } => {
                            self.levels.old == SplitType::from_raw_level(raw_level)
                                && self.pads.old != self.pads.current
                        }
                        SplitType::CompleteForLevel { raw_level } => {
                            raw_level_change
                                && self.levels.old == SplitType::from_raw_level(raw_level)
                        }
                        SplitType::PadsCrossed { .. } => {
                            if self.pads.old != self.pads.current {
                                split_state.visit_pad();
                            }
                            split_state.pads() == 0
                        }
                        SplitType::EnergyCubes { .. } => {
                            if (self.levels.current == SplitType::Bot2000
                                || self.levels.current == SplitType::Odin)
                                && exp_difference == self.levels.current.cube_exp(diff)
                            {
                                split_state.place_cube();
                            }
                            split_state.cubes() == 0
                        }
                    }
                }
                _ => false,
            }
        } else {
            false
        }
    }
}
