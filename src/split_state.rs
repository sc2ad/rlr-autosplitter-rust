use crate::split_type::SplitType;

pub struct SplitState {
    pads_remaining: i32,
    cubes_remaining: i32,
}

impl SplitState {
    pub fn visit_pad(&mut self) {
        self.pads_remaining -= 1;
    }
    pub fn place_cube(&mut self) {
        self.cubes_remaining -= 1;
    }
    pub fn pads(&self) -> i32 {
        self.pads_remaining
    }
    pub fn cubes(&self) -> i32 {
        self.cubes_remaining
    }
    pub fn from_split(full_split: Option<&SplitType>) -> Self {
        if let Some(split) = full_split {
            match split {
                SplitType::PadsCrossed { num } => SplitState {
                    cubes_remaining: 0,
                    pads_remaining: *num,
                },
                SplitType::EnergyCubes { num } => SplitState {
                    cubes_remaining: *num,
                    pads_remaining: 0,
                },
                _ => SplitState {
                    pads_remaining: 0,
                    cubes_remaining: 0,
                },
            }
        } else {
            SplitState {
                pads_remaining: 0,
                cubes_remaining: 0,
            }
        }
    }
}
