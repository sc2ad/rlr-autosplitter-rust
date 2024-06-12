#![feature(type_alias_impl_trait, const_async_blocks)]
#![feature(portable_simd)]
#![feature(array_into_iter_constructors)]
#![feature(const_option)]
#![feature(maybe_uninit_array_assume_init)]
#![cfg(target_feature = "simd128")]
#![no_std]

mod data;
mod sigscan;
mod split_type;

use asr::time::Duration;
use asr::Process;
use asr::{future::next_tick, settings::Gui, timer};
use data::GameData;
use split_type::SplitType;

asr::async_main!(nightly);
asr::panic_handler!();

#[cfg(debug_assertions)]
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

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {};
}

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
    #[default = true]
    auto_start: bool,
}

async fn main() {
    let mut settings = Settings::register();

    log!("Loaded settings: {settings:?}");

    let mut splits = [
        SplitType::ExpGained,
        SplitType::ExpGained,
        SplitType::ExpGained,
        SplitType::Level1,
    ]
    .iter();

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
                    let current_exp = data.exp();
                    log!("Initially loaded exp: {current_exp:?}");
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
                    let mut split = splits.next();
                    loop {
                        settings.update();
                        // General loop consists of performing an exp update
                        data.update();
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
                            if data.should_split(*spl) {
                                log!("SPLITTING FOR: {spl:?}");
                                timer::split();
                                split = splits.next();
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
