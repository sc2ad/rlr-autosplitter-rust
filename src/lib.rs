#![feature(type_alias_impl_trait, const_async_blocks)]
#![feature(array_into_iter_constructors)]
#![cfg(target_feature = "simd128")]
#![no_std]

use core::mem::MaybeUninit;

use asr::{
    future::{next_tick, retry},
    settings::Gui,
    timer,
    watcher::Watcher,
    Address, FromEndian, MemoryRangeFlags, Process,
};

asr::async_main!(nightly);
asr::panic_handler!();

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

// Problem: Scanning proc memory is SLOW AS SHIT using their API
// Ideas:
// We know we have no mask, we want full equivalence of the pattern
// We know the pattern size is 16 bytes
// We know we are aligned 4
// We know we are always WITHIN a page, not on any boundary
// Ideally, we want to walk the pages of the process that are defined and skip ones that aren't writable completely
// Or directly refer to heap pages and look there or something

// Okay, so, it's WAYY too slow to search every page for a gigantic range.
// It'd be great if we can find a page that will be near us that has some distinguishing properties (maybe some really large size?)
// Notes: 655360 page size is at c4f70000 (goal is 0x260C8C5D77C)
// If we could START at the process heap, that'd be awesome, but I don't know if we get that power...
// Also, the heap is too big, we want to start closer to the front
// Heap is WAYYYYYY too big!!! takes like 1 min even when we are kinda close...

// 651468800
// 455028736
fn find_exp_pattern(process: &Process) -> Option<Address> {
    let mut addr = Address::new(0x26004000000);
    //                               0x260C8C5D77C
    let overall_end = addr.value() + 0x00102000000;
    // Endianness is broken for this value. When we compare, we want to compare against the flipped endian value
    // The actual pattern is: 0x4A000000DF1B0100CA10010000000000
    // but we must write the flipped variant: 0x00000000000110CA00011BDF0000004A
    let signature: u128 = 0x4A000000DF1B0100CA10010000000000.from_be();
    // Array size is 4KB
    let mut buf = [MaybeUninit::uninit(); (4 << 10)];
    for range in process.memory_ranges() {
        // First, get the start and size of the page to see if we should look at it
        if let Ok((chunk_base, chunk_size)) = range.range() {
            // Check the address range against our addr and overall_end
            if chunk_base + chunk_size <= addr || chunk_base.value() > overall_end {
                // This page is out of bounds, ignore it
                continue;
            }
            // Page is in-bounds, check the flags
            if let Ok(flags) = range.flags() {
                if !flags.contains(MemoryRangeFlags::READ | MemoryRangeFlags::WRITE)
                    || flags.contains(MemoryRangeFlags::EXECUTE)
                {
                    // Skip pages that are not important since they have no read/write perms
                    continue;
                }
            } else {
                // Skip pages we can't read flags for
                continue;
            }
            // At this point, read the page into our buffer repeatedly until we have gone through all the size or we have reached the end of our size request
            let chunk_end = chunk_base.value() + chunk_size;
            // We go sequentially in pages, so if we skip a range, capture that.
            addr = addr.value().max(chunk_base.value()).into();
            while addr.value() < chunk_end {
                // We round up to the 4 KiB address boundary as that's a single
                // page, which is safe to read either fully or not at all. We do
                // this to do a single read rather than many small ones as the
                // syscall overhead is a quite high.
                let end = (addr.value() & !((4 << 10) - 1)) + (4 << 10).min(chunk_end);
                let len = end - addr.value();
                let current_read_buf = &mut buf[..len as usize];
                if let Ok(current_read_buf) = process.read_into_uninit_buf(addr, current_read_buf) {
                    // Compare the array data that we just read here.
                    // We want to compare in a fast way, where we convert all 4 byte increments to u128s and then compare equivalence
                    if let Some(offset) = compare_equivalence(signature, current_read_buf) {
                        return Some(addr.add(offset as u64));
                    }
                };
                // Move the address forward, eventually we will be at chunk_end, which will be the next chunk for us to read
                addr = Address::new(end);
                // TODO: Yield here
            }
        }
        // Skip pages we can't read the range for
        // TODO: Yield here
    }
    None
}

fn compare_equivalence(signature: u128, haystack: &[u8]) -> Option<usize> {
    let mut offset = 0;
    // haystack must be aligned 4 at least, we won't have it at the edge of a 4KB page
    while offset < haystack.len() - 15 {
        let ptr: *const u128 =
            unsafe { haystack.as_ptr().byte_offset(offset.try_into().unwrap()) }.cast();
        let val = unsafe { ptr.read_unaligned() };
        if signature == val {
            // Exact equivalence
            return Some(offset);
        }
        offset += 4;
    }
    None
}

async fn main() {
    let mut settings = Settings::register();

    asr::print_limited::<256>(&format_args!("Loaded settings: {settings:?}"));

    let mut exp_addr = None;

    loop {
        let process = Process::wait_attach("SC2_x64.exe").await;
        process
            .until_closes(async {
                asr::print_message("Attached to process!");
                if settings.set_game_time {
                    timer::pause_game_time();
                }
                // TODO: It REALLY shouldn't be this big but alas that's how it is.
                // let search_range = (Address::new(0x20000000000), 0xf000000000);
                loop {
                    settings.update();
                    // EXP address is actually 4 bytes prior
                    if exp_addr.is_none() {
                        exp_addr = Some(retry(|| find_exp_pattern(&process)).await.add_signed(-4));
                        let exp_value = process.read::<i32>(exp_addr.unwrap());
                        asr::print_limited::<256>(&format_args!(
                            "FOUND SIGNATURE: {exp_addr:?} with exp value: {exp_value:?}"
                        ));
                    }

                    // We have found the exp signature, start the timer if requested
                    if settings.auto_start {
                        timer::start();
                    }
                    if settings.set_game_time {
                        timer::resume_game_time();
                    }
                    // let data = Watcher::new();

                    // TODO: Do something on every tick.
                    next_tick().await;
                }
            })
            .await;
    }
}

struct GameData {
    exp_pointer: Option<Address>,
    exp_watcher: Watcher<i32>,
}
impl GameData {}
