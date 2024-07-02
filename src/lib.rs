#![feature(type_alias_impl_trait, const_async_blocks)]
#![feature(portable_simd)]
#![feature(array_into_iter_constructors)]
#![feature(maybe_uninit_array_assume_init)]
#![cfg(target_feature = "simd128")]
#![no_std]

mod data;
mod sigscan;
mod split_state;
mod split_type;

use asr::time::Duration;
use asr::Process;
use asr::{future::next_tick, settings::Gui, timer};
use data::GameData;
use split_state::SplitState;
use split_type::SplitType;

asr::async_main!(nightly);
asr::panic_handler!();

// #[cfg(debug_assertions)]
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        let mut buf = ::asr::arrayvec::ArrayString::<1024>::new();
        let _ = ::core::fmt::Write::write_fmt(
            &mut buf,
            ::core::format_args!($($arg)*),
        );
        ::asr::print_message(&buf);
    }};
}

// #[cfg(not(debug_assertions))]
// #[macro_export]
// macro_rules! log {
//     ($($arg:tt)*) => {};
// }

#[macro_export]
macro_rules! dbg {
    // Copy of ::std::dbg! but for no_std with redirection to log!
    () => {
        $crate::log!("[{}:{}]", ::core::file!(), ::core::line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::log!("[{}:{}] {} = {:#?}",
                    ::core::file!(), ::core::line!(), ::core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

#[derive(Gui, Debug)]
struct Settings {
    /// Automattcally reset the timer when RLR4 has ended
    #[default = false]
    auto_reset: bool,
    /// Automatically set the game time in livesplit to match the IGT in game
    #[default = false]
    set_game_time: bool,
    /// Automatically start the timer
    #[default = false]
    auto_start: bool,
}

async fn main() {
    let mut settings = Settings::register();

    log!("Loaded settings: {settings:?}");
    asr::set_tick_rate(30.0);

    // TODO: Back with a settings structure of some kind
    let all_splits = [
        SplitType::Level1,
        SplitType::Level2,
        SplitType::Level3,
        SplitType::Bot2000,
        SplitType::Level4,
        SplitType::Level5,
        SplitType::Level6,
        SplitType::Odin,
        SplitType::Level7,
        SplitType::Level8,
        SplitType::Level9,
        SplitType::Diablo,
    ];
    let mut split_iter = all_splits.iter();

    loop {
        let process = Process::wait_attach("SC2_x64.exe").await;
        process
            .until_closes(async {
                log!("Attached to process!");
                loop {
                    // This outer loop happens whenever we decide to reset the timer
                    if settings.set_game_time {
                        timer::pause_game_time();
                        timer::set_game_time(Duration::ZERO);
                        // TODO: Figure out how to set game_time within the critical loop to something from the game
                    }
                    // Try to make a gamedata instance
                    let mut data = GameData::new(&process).await;
                    // Set tick rate back to something fast enough to catch cases
                    asr::set_tick_rate(120.0);
                    settings.update();
                    // Now that we have a game data instance, first immediately try to start the timer as needed
                    if settings.set_game_time {
                        log!("RESUMING GAME TIME");
                        timer::resume_game_time();
                    }
                    if settings.auto_start {
                        log!("STARTING THE TIMER!");
                        timer::start();
                    }
                    // When we reset, we reset counting the splits
                    split_iter = all_splits.iter();
                    let mut split = split_iter.next();
                    // Form the split state with the options from this current split, if present.
                    let mut split_state = SplitState::from_split(split);
                    // TODO: Depending on if our run type has a set difficulty or not, force a certain difficulty instead of deducing it
                    // Here and also within the split_iter.next() call
                    loop {
                        settings.update();
                        // General loop consists of performing an exp update
                        let state = data.update();
                        // Check to see if we invalidated in some way, if so, reset as needed and break to our outer loop
                        if data.invalid() {
                            if settings.auto_reset {
                                log!("RESETTING THE TIMER!");
                                timer::reset();
                            }
                            if settings.set_game_time {
                                log!("PAUSING THE GAME TIME!");
                                timer::pause_game_time();
                            }
                            break;
                        }
                        // Then check our upcoming split to see if we should split
                        // TODO: Keep the split info in a settings file somehow
                        if let Some(spl) = split {
                            if state.should_split(&mut split_state, *spl) {
                                log!("SPLITTING FOR: {spl:?}");
                                timer::split();
                                split = split_iter.next();
                                // Form the next state with the next split options
                                split_state = SplitState::from_split(split);
                            }
                        }

                        // TODO: At some cadence, decide to rescan and determine if we should reset (or invalidate)

                        next_tick().await;
                    }
                }
            })
            .await;
    }
}

// TODO: Handle the case where we undo a split if possible (undoing a level skip after we did a level is not saveable)
// TODO: Handle the case where we skip a split
