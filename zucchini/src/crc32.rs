//! The whole-file CRC-32 Zucchini computes, through the C API's `zucc_set_crc32` (the `crc32-hook`
//! feature).

use std::ptr;
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

/// Installs crc-fast the first time a patch is generated or applied, unless a CRC-32 was already
/// chosen, through [`set_crc32`] or `zucchini_sys::zucc_set_crc32`.
#[cfg(feature = "crc-fast")]
pub(crate) fn install_default() {
    use std::sync::Once;

    static DEFAULT: Once = Once::new();
    // SAFETY: crc32_fast_from_c is thread safe, and crc-fast's CRC-32/ISO-HDLC is zlib's CRC-32.
    DEFAULT.call_once(|| unsafe { sys::zucc_set_default_crc32(Some(crc32_fast_from_c)) });
}

/// The function crc32_from_c calls, a `fn(&[u8]) -> u32`; null until one is installed.
static CRC32: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

#[cfg(feature = "crc-fast")]
fn set_default() {
    // SAFETY: crc32_fast_from_c is thread safe, and crc-fast's CRC-32/ISO-HDLC is zlib's CRC-32.
    unsafe { sys::zucc_set_crc32(Some(crc32_fast_from_c)) }
}

#[cfg(not(feature = "crc-fast"))]
fn set_default() {
    // SAFETY: None restores Chromium's own loop.
    unsafe { sys::zucc_set_crc32(None) }
}

/// What zucc_set_crc32 is given for crc-fast.
#[cfg(feature = "crc-fast")]
unsafe extern "C" fn crc32_fast_from_c(data: *const u8, len: usize) -> u32 {
    // SAFETY: Zucchini passes a file it holds, readable for `len` bytes.
    let data = unsafe { file(data, len) };
    crc_fast::checksum(crc_fast::CrcAlgorithm::Crc32IsoHdlc, data) as u32
}

/// What zucc_set_crc32 is given for a chosen function: calls the function in CRC32, which is always
/// stored before this is installed and never cleared.
unsafe extern "C" fn crc32_from_c(data: *const u8, len: usize) -> u32 {
    // SAFETY: Zucchini passes a file it holds, readable for `len` bytes.
    let data = unsafe { file(data, len) };
    // SAFETY: CRC32 only ever holds a `fn(&[u8]) -> u32`, and holds one whenever this runs.
    let crc32 = unsafe { std::mem::transmute::<*mut (), fn(&[u8]) -> u32>(CRC32.load(Ordering::Acquire)) };
    crc32(data)
}

/// The `len` bytes at `data`, which may be null when `len` is 0.
///
/// # Safety
/// `data` must be readable for `len` bytes while the slice is in use.
unsafe fn file<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        // SAFETY: the caller vouches for `len` readable bytes.
        unsafe { std::slice::from_raw_parts(data, len) }
    }
}
