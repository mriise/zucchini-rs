//! Exercise the bundled native CRC directly, including runtime fallbacks, in its own process.
#![cfg(feature = "crc-fast")]

use std::ffi::c_ulong;

unsafe extern "C" {
    // Chromium prefixes the public zlib symbols; this does not add a production C API.
    #[link_name = "Cr_z_crc32_z"]
    fn crc32(seed: c_ulong, data: *const u8, len: usize) -> c_ulong;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    static mut Cr_z_x86_cpu_enable_simd: i32;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    static mut x86_cpu_enable_avx512: i32;
    #[cfg(target_arch = "aarch64")]
    static mut Cr_z_arm_cpu_enable_crc32: i32;
    #[cfg(target_arch = "aarch64")]
    static mut Cr_z_arm_cpu_enable_pmull: i32;
}

fn reference(seed: u32, data: &[u8]) -> u32 {
    let mut crc = !seed;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

fn native(seed: u32, data: &[u8]) -> u32 {
    // SAFETY: CPU detection has run, and the slice remains readable throughout the call.
    unsafe { crc32(seed as c_ulong, data.as_ptr(), data.len()) as u32 }
}

fn check(bytes: &[u8]) {
    assert_eq!(native(0, b""), 0);
    assert_eq!(native(0, b"123456789"), 0xCBF4_3926);
    for offset in 0..64 {
        for len in [
            1, 15, 16, 17, 63, 64, 65, 127, 128, 129, 255, 256, 257, 319, 320, 321, 511, 512, 513, 1023,
            1024, 1025, 4095, 4096, 4097,
        ] {
            let data = &bytes[offset..offset + len];
            for seed in [0, 1, 0x1234_5678, !0] {
                let expected = reference(seed, data);
                assert_eq!(native(seed, data), expected, "offset {offset}, len {len}, seed {seed}");
                let (first, last) = data.split_at(len / 2);
                assert_eq!(native(native(seed, first), last), expected);
            }
        }
    }

    // Verify that Zucchini's seed/complement adapter writes compatible patch headers and checks
    // both whole files. This also runs without the optional crc32-hook API.
    let old = &bytes[1..4098];
    let mut new = old.to_vec();
    new[255..321].fill(7);
    let patch = zucchini::generate_raw(old, &new).unwrap();
    let header = zucchini::read_header(&patch).unwrap();
    assert_eq!(header.old_crc, reference(0, old));
    assert_eq!(header.new_crc, reference(0, &new));
    assert_eq!(zucchini::apply(old, &patch).unwrap(), new);
    let mut corrupted = old.to_vec();
    corrupted[256] ^= 1;
    assert_eq!(zucchini::apply(&corrupted, &patch), Err(zucchini::Error::WrongOldFile));
}

#[test]
fn native_crc_matches_reference_and_patch_headers() {
    // SAFETY: zlib documents this initialization call; it uses a thread-safe once primitive.
    assert_eq!(unsafe { crc32(0, std::ptr::null(), 0) }, 0);
    let mut random = 1u32;
    let bytes: Vec<u8> = (0..4161)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u8
        })
        .collect();
    check(&bytes);

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    // SAFETY: this binary has one test, no background native calls, and detection has finished.
    // Only disable detected backends, never enable an unsupported ISA; restore before returning.
    unsafe {
        let avx512 = x86_cpu_enable_avx512;
        let simd = Cr_z_x86_cpu_enable_simd;
        x86_cpu_enable_avx512 = 0;
        if avx512 != 0 {
            check(&bytes);
        }
        Cr_z_x86_cpu_enable_simd = 0;
        if simd != 0 {
            check(&bytes);
        }
        Cr_z_x86_cpu_enable_simd = simd;
        x86_cpu_enable_avx512 = avx512;
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: as above, no concurrent native calls; only disable supported backends.
    unsafe {
        let crc = Cr_z_arm_cpu_enable_crc32;
        let pmull = Cr_z_arm_cpu_enable_pmull;
        Cr_z_arm_cpu_enable_pmull = 0;
        if crc != 0 && pmull != 0 {
            check(&bytes);
        }
        Cr_z_arm_cpu_enable_crc32 = 0;
        if crc != 0 {
            check(&bytes);
        }
        Cr_z_arm_cpu_enable_crc32 = crc;
        Cr_z_arm_cpu_enable_pmull = pmull;
    }
}
