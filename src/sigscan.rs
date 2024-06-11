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

use core::mem;

use asr::{Address, MemoryRangeFlags, Process};

use crate::log;

extern "C" {
    pub fn process_read(
        process: Process,
        address: Address,
        buf_ptr: *mut u8,
        buf_len: usize,
    ) -> bool;
}

// Endianness is broken for this value. When we compare, we want to compare against the flipped endian value
// The actual pattern is: 0x4A000000DF1B0100CA10010000000000
// but we must write the flipped variant: 0x00000000000110CA00011BDF0000004A
// static EXP_PATTERN_FIRST_SCALAR: u64 = 0x00011BDF0000004A;
// static EXP_PATTERN_FIRST: u64x64 = simd::Simd::from_array([EXP_PATTERN_FIRST_SCALAR; 64]);
// static EXP_PATTERN_SECOND_SCALAR: u64 = 0x0110CA;
// static EXP_PATTERN_SECOND: u64x64 = simd::Simd::from_array([EXP_PATTERN_SECOND_SCALAR; 64]);
static EXP_PATTERN_SIGNATURE: u128 = 0x00000000000110CA00011BDF0000004A;
// static EXP_PATTERN_BYTES: [u64; 64] = seq_macro::seq!(N in 0..32 {
//     [
//         #(
//             0x00011BDF0000004A,
//             0x0110CA,
//         )*
//     ]
// });
// static EXP_PATTERN: u64x64 = simd::Simd::from_array(EXP_PATTERN_BYTES);

pub fn find_exp_pattern(process: &Process) -> Option<Address> {
    let mut addr = Address::new(0x20000000000);
    //                               0x260C8C5D77C
    let overall_end = addr.value() + 0x02000000000;
    // Array size is 4KB
    let mut buf = [0u8; (4 << 10)];
    for range in process.memory_ranges() {
        // First, get the start and size of the page to see if we should look at it
        if let Ok((chunk_base, chunk_size)) = range.range() {
            // Check that chunk_size is a multiple of 4096
            if (chunk_size % 4096) != 0 {
                continue;
            }
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

                // TODO: We know that our end scan addr will always be 4KiB page aligned
                if unsafe {
                    process_read(
                        mem::transmute_copy(process),
                        addr,
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                    )
                } {
                    if let Some(offset) = compare_equivalence(&buf) {
                        // Exp offset is -4 off the pattern
                        let result = addr.add(offset as u64).add_signed(-4);
                        log!("Found exp at: {result:?}");
                        return Some(addr.add(offset as u64).add_signed(-4));
                    }
                }
                // Move the address forward, eventually we will be at chunk_end, which will be the next chunk for us to read
                addr = addr.add(4096);
                // TODO: Yield here
            }
        }
        // Skip pages we can't read the range for
        // TODO: Yield here
    }
    None
}

fn compare_equivalence(haystack: &[u8; 4096]) -> Option<usize> {
    let mut result = false;
    // let u64s: &[u64] = unsafe { core::slice::from_raw_parts(haystack.as_ptr().cast(), 4096 / 8) };

    // let a0: u64x64 = simd::Simd::from_slice(&u64s[..64]);
    // let a1: u64x64 = simd::Simd::from_slice(&u64s[64..128]);
    // let a2: u64x64 = simd::Simd::from_slice(&u64s[128..192]);
    // let a3: u64x64 = simd::Simd::from_slice(&u64s[192..256]);
    // let a4: u64x64 = simd::Simd::from_slice(&u64s[256..320]);
    // let a5: u64x64 = simd::Simd::from_slice(&u64s[320..384]);
    // let a6: u64x64 = simd::Simd::from_slice(&u64s[384..448]);
    // let a7: u64x64 = simd::Simd::from_slice(&u64s[448..512]);

    // result |= (a0 & EXP_PATTERN_FIRST).reduce_max() == EXP_PATTERN_FIRST_SCALAR;
    // result |= (a0.)

    let ptr = haystack.as_ptr() as *const u128;
    // let data0: &[u128] = unsafe { std::slice::from_raw_parts(ptr, len / 16)};
    // let data0: *const u128 = unsafe { &haystack.data.as_ptr().cast() };

    // let data: [u128] = bytemuck::cast_slice(haystack);
    for i in (0..4096 - 16).step_by(16) {
        // Unrolled 4 times
        result |= EXP_PATTERN_SIGNATURE == unsafe { ptr.byte_offset(i).read() };
        result |= EXP_PATTERN_SIGNATURE == unsafe { ptr.byte_offset(i + 4).read_unaligned() };
        result |= EXP_PATTERN_SIGNATURE == unsafe { ptr.byte_offset(i + 8).read_unaligned() };
        result |= EXP_PATTERN_SIGNATURE == unsafe { ptr.byte_offset(i + 12).read_unaligned() };
    }
    // Final chunk is just a single check
    result |= EXP_PATTERN_SIGNATURE == unsafe { ptr.byte_offset(4096 - 16).read() };
    if result {
        // Do the slow parse to find the index here since we had a PERFECT match
        let mut offset = 0;
        while offset <= 4096 - 16 {
            let ptr: *const u128 =
                unsafe { haystack.as_ptr().byte_offset(offset.try_into().unwrap()) }.cast();
            let val = unsafe { ptr.read_unaligned() };
            if EXP_PATTERN_SIGNATURE == val {
                // Exact equivalence
                return Some(offset);
            }
            offset += 4;
        }
    }
    None
}
