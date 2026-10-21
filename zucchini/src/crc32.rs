//! The whole-file CRC-32 Zucchini computes, through the C API's `zucc_set_crc32` (the `crc32-hook`
//! feature).

use std::ptr;
use std::sync::Once;
use std::sync::atomic::{AtomicPtr, Ordering};

use zucchini_sys as sys;

/// Chooses the CRC-32 Zucchini computes over both whole files when it generates or applies a patch.
/// `None` goes back to the default: crc-fast with the `crc-fast` feature, otherwise Chromium's
/// byte-at-a-time loop.
///
/// The choice applies to the whole process, including calls already running on other threads, so
/// `crc32` must return what zlib's `crc32` does (CRC-32/ISO-HDLC); anything else makes every apply
/// fail with [`Error::WrongOldFile`](crate::Error::WrongOldFile). A panic inside it aborts the
/// process, since it is called from C++.
pub fn set_crc32(crc32: Option<fn(&[u8]) -> u32>) {
    install_default();
    match crc32 {
        Some(crc32) => {
            CRC32.store(crc32 as *mut (), Ordering::Release);
            // SAFETY: crc32_from_c is thread safe and computes the CRC-32 with the function just
            // stored, which the caller vouches for.
            unsafe { sys::zucc_set_crc32(Some(crc32_from_c)) }
        }
        None => set_default(),
    }
}

/// Installs the default CRC-32 the first time a patch is generated or applied, unless
/// [`set_crc32`] chose one first.
pub(crate) fn install_default() {
    static DEFAULT: Once = Once::new();
    DEFAULT.call_once(set_default);
}

/// The function crc32_from_c calls, a `fn(&[u8]) -> u32`; null until one is installed.
static CRC32: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

#[cfg(feature = "crc-fast")]
fn set_default() {
    CRC32.store(crc32_fast as fn(&[u8]) -> u32 as *mut (), Ordering::Release);
    // SAFETY: crc32_from_c is thread safe, and crc-fast's CRC-32/ISO-HDLC is zlib's CRC-32.
    unsafe { sys::zucc_set_crc32(Some(crc32_from_c)) }
}

#[cfg(not(feature = "crc-fast"))]
fn set_default() {
    // SAFETY: None restores Chromium's own loop.
    unsafe { sys::zucc_set_crc32(None) }
}

#[cfg(feature = "crc-fast")]
fn crc32_fast(data: &[u8]) -> u32 {
    crc_fast::checksum(crc_fast::CrcAlgorithm::Crc32IsoHdlc, data) as u32
}

/// What zucc_set_crc32 is given: calls the function in CRC32, which is always stored before this is
/// installed and never cleared.
unsafe extern "C" fn crc32_from_c(data: *const u8, len: usize) -> u32 {
    let data = if len == 0 {
        &[][..]
    } else {
        // SAFETY: Zucchini passes a file it holds, readable for `len` bytes.
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    // SAFETY: CRC32 only ever holds a `fn(&[u8]) -> u32`, and holds one whenever this runs.
    let crc32 = unsafe { std::mem::transmute::<*mut (), fn(&[u8]) -> u32>(CRC32.load(Ordering::Acquire)) };
    crc32(data)
}
