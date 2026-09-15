//! Raw bindings to the C API in `shim/zucchini_c.h`, over Chromium's Zucchini binary diff.
//!
//! Use the `zucchini` crate for a safe interface.

#![no_std]
#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int};
use core::marker::{PhantomData, PhantomPinned};

// Results 0 to 10 are Zucchini's own status codes; 100 and up come from the C API.
pub const ZUCC_OK: c_int = 0;
pub const ZUCC_INVALID_PARAM: c_int = 1;
pub const ZUCC_PATCH_READ_ERROR: c_int = 4;
pub const ZUCC_PATCH_WRITE_ERROR: c_int = 5;
pub const ZUCC_INVALID_OLD_IMAGE: c_int = 6;
pub const ZUCC_INVALID_NEW_IMAGE: c_int = 7;
pub const ZUCC_FATAL: c_int = 10;
pub const ZUCC_TOO_LARGE: c_int = 100;
pub const ZUCC_BUFFER_SIZE: c_int = 101;
pub const ZUCC_OUT_OF_MEMORY: c_int = 102;
pub const ZUCC_CHECK_FAILED: c_int = 103;
pub const ZUCC_EMPTY_FILE: c_int = 104;

pub const ZUCC_LOG_OFF: c_int = 0;
pub const ZUCC_LOG_ERROR: c_int = 1;
pub const ZUCC_LOG_WARNING: c_int = 2;
pub const ZUCC_LOG_INFO: c_int = 3;

/// A generated patch waiting to be written out.
#[repr(C)]
pub struct zucc_patch {
    _data: (),
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct zucc_generate_options {
    pub start_scan_at: u32,
    pub raw: c_int,
    pub imposed_matches: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct zucc_patch_header {
    pub major_version: u16,
    pub minor_version: u16,
    pub old_size: u32,
    pub old_crc: u32,
    pub new_size: u32,
    pub new_crc: u32,
}

unsafe extern "C" {
    pub fn zucc_generate(
        old_data: *const u8,
        old_len: usize,
        new_data: *const u8,
        new_len: usize,
        options: *const zucc_generate_options,
        patch_out: *mut *mut zucc_patch,
    ) -> c_int;
    pub fn zucc_patch_size(patch: *const zucc_patch) -> usize;
    pub fn zucc_patch_write(patch: *const zucc_patch, out: *mut u8, out_len: usize) -> c_int;
    pub fn zucc_patch_free(patch: *mut zucc_patch);
    pub fn zucc_read_header(patch: *const u8, patch_len: usize, header_out: *mut zucc_patch_header) -> c_int;
    pub fn zucc_apply(
        old_data: *const u8,
        old_len: usize,
        patch: *const u8,
        patch_len: usize,
        new_out: *mut u8,
        new_len: usize,
    ) -> c_int;
    pub fn zucc_last_error() -> *const c_char;
    pub fn zucc_set_log_level(level: c_int);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn struct_layouts_match_the_c_header() {
        assert_eq!(size_of::<zucc_patch_header>(), 20);
        assert_eq!(size_of::<zucc_generate_options>(), 8 + size_of::<*const c_char>());
    }
}
