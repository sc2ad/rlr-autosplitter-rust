use asr::{future::next_tick, watcher::Watcher, Address, Process};

use crate::{
    log,
    sigscan::find_exp_pattern,
    split_type::{any_boss, Difficulty, SplitType, LARGEST_EXP_DIFFERENCE},
};

const PAD_COUNT: i32 = 19;

pub struct GameData<'a> {
    process: &'a Process,
    exp_pointer: Option<Address>,
    exp_watcher: Watcher<i32>,
    level: SplitType,
    current_pad: i32,
    valid: bool,
    difficulty: Option<Difficulty>,
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
            exp_watcher: Watcher::new(),
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
        self.exp_watcher.pair.map(|pair| pair.current)
    }
    /// Returns the exp difference, if present. If garbage or invalid, None is returned and the state is reset.
    fn update_exp(&mut self) -> Option<i32> {
        if let Some(exp) = self.read_exp() {
            let pair = self.exp_watcher.update_infallible(exp);
            let difference = pair.current - pair.old;
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
                            log!("Crossed pad! Was: {pad}!");
                            self.current_pad += 1;
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
    pub fn update(&mut self) {
        // Check to see if we need to complete a level based off of pad or exp
        if self.current_pad == PAD_COUNT {
            let old_level = self.level;
            self.level = self.level.next();
            let level = self.level;
            log!("Level complete! Was: {old_level:?} now is: {level:?}");
            self.current_pad = 0;
        } else if self.level.is_boss_level() {
            // Check the difference here and increment the level if so
            if let Some(exps) = self.exp_watcher.pair {
                if let Some(diff) = self.difficulty {
                    if exps.current - exps.old == self.level.boss_exp(diff) {
                        let old_level = self.level;
                        self.level = self.level.next();
                        let level = self.level;
                        log!("Boss complete! Was: {old_level:?} now is: {level:?}");
                    }
                }
            }
        }
        // Now, we perform an exp check
        self.update_exp();
        // And then we should perform a split check here and return if we should split
    }
    /// Returns true if the exp gained was exactly equivalent to a split type
    pub fn should_split(&self, split: SplitType) -> bool {
        if let Some(exps) = self.exp_watcher.pair {
            let exp_difference = exps.current - exps.old;
            if let Some(diff) = self.difficulty {
                match split {
                    SplitType::Manual => false,
                    SplitType::ExpGained => exp_difference > 0,
                    SplitType::RawLevelComplete => {
                        any_boss(exp_difference, diff) || self.current_pad == PAD_COUNT
                    }
                    SplitType::LevelComplete => self.current_pad == PAD_COUNT,
                    SplitType::BossComplete => any_boss(exp_difference, diff),
                    // All conventional level splits fit in this call
                    SplitType::Level1
                    | SplitType::Level2
                    | SplitType::Level3
                    | SplitType::Level4
                    | SplitType::Level5
                    | SplitType::Level6
                    | SplitType::Level7
                    | SplitType::Level8
                    | SplitType::Level9 => {
                        // If we now have gotten enough pads to complete the level, we split
                        self.current_pad == PAD_COUNT && exp_difference == split.standard_exp(diff)
                    }
                    SplitType::Bot2000
                    | SplitType::Odin
                    | SplitType::Diablo
                    | SplitType::CowLevel => exp_difference == split.boss_exp(diff),
                }
            } else {
                match split {
                    SplitType::Manual => false,
                    SplitType::ExpGained => exp_difference > 0,
                    _ => panic!(
                        "Split type: {split:?} requires a difficulty, but none was deduced or provided!"
                    ),
                }
            }
        } else {
            false
        }
    }
}
