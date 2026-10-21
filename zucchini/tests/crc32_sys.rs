//! A CRC-32 chosen through zucchini-sys directly, before the first patch, stays chosen: crc-fast is
//! only the default. Its own test binary, since the choice is process-wide.

#![cfg(feature = "crc-fast")]

use std::sync::atomic::{AtomicUsize, Ordering};

use zucchini::{apply, generate_raw};

static CALLS: AtomicUsize = AtomicUsize::new(0);

/// zlib's CRC-32, one bit at a time, counting its calls.
unsafe extern "C" fn counted_crc32(data: *const u8, len: usize) -> u32 {
    CALLS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: Zucchini passes a file it holds, readable for `len` bytes.
    let data = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(data, len) } };
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[test]
fn a_crc32_set_through_zucchini_sys_is_not_replaced() {
    // SAFETY: counted_crc32 is thread safe and computes zlib's CRC-32.
    unsafe { zucchini_sys::zucc_set_crc32(Some(counted_crc32)) };
    let old: Vec<u8> = (0..300_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    let mut new = old.clone();
    new[50_000..50_100].fill(7);

    let patch = generate_raw(&old, &new).unwrap();
    let generated = CALLS.load(Ordering::Relaxed);
    assert!(generated > 0);
    assert_eq!(apply(&old, &patch).unwrap(), new);
    assert_eq!(CALLS.load(Ordering::Relaxed), generated + 2);
}
