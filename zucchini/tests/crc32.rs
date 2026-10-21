//! set_crc32 changes the CRC-32 for the whole process, so these checks run in their own test binary
//! and in one test, in order.

#![cfg(feature = "crc32-hook")]

use std::sync::atomic::{AtomicUsize, Ordering};

use zucchini::{Error, apply, generate_raw, read_header, set_crc32};

/// zlib's CRC-32, one bit at a time.
fn reference_crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

static CALLS: AtomicUsize = AtomicUsize::new(0);

fn counted_crc32(data: &[u8]) -> u32 {
    CALLS.fetch_add(1, Ordering::Relaxed);
    reference_crc32(data)
}

#[test]
fn set_crc32_replaces_and_restores_the_crc32() {
    let old: Vec<u8> = (0..300_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    let mut new = old.clone();
    new[50_000..50_100].fill(7);

    // The default writes and checks zlib's CRC-32.
    let patch = generate_raw(&old, &new).unwrap();
    let header = read_header(&patch).unwrap();
    assert_eq!(header.old_crc, reference_crc32(&old));
    assert_eq!(header.new_crc, reference_crc32(&new));
    assert_eq!(apply(&old, &patch).unwrap(), new);

    // A chosen function is the one called: on the old file and on the result.
    set_crc32(Some(counted_crc32));
    assert_eq!(apply(&old, &patch).unwrap(), new);
    assert_eq!(CALLS.load(Ordering::Relaxed), 2);

    // A wrong one fails the old file's check.
    set_crc32(Some(|_| 0));
    assert_eq!(apply(&old, &patch), Err(Error::WrongOldFile));

    set_crc32(None);
    assert_eq!(apply(&old, &patch).unwrap(), new);
    assert_eq!(CALLS.load(Ordering::Relaxed), 2);
}
