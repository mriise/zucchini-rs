//! Safe bindings to Zucchini, Chromium's binary diff for executables.
//!
//! Zucchini finds the executables inside the files it compares, disassembles enough of them to
//! resolve references such as relative calls and relocations, and diffs the code with those
//! references normalised. A rebuilt binary therefore patches far smaller than a byte-level diff
//! allows. Bytes that are not part of a recognised executable are diffed as raw data.
//!
//! Patches are not compressed; compress them before storing or sending them. Zucchini cannot patch
//! from or to an empty file, so those calls fail with [`Error::EmptyFile`].
//!
//! This build recognises PE (Windows) and ELF (Linux) executables. Chromium's DEX (Android) and
//! ZTF (a test format) support is not enabled: their disassemblers are compiled but never
//! registered, so a patch Chromium's own tools made with those elements does not apply here.
//!
//! A failed Chromium CHECK returns [`Error::CheckFailed`] instead of crashing the process, and
//! [`apply`] checks the CRC-32 of both the old file and the result. With the optional `crc-fast`
//! feature that CRC-32 runs on crc-fast's SIMD code rather than Chromium's byte-at-a-time loop;
//! [`set_crc32`] picks another implementation. Files are limited to 4 GiB,
//! because the patch header stores 32-bit sizes. Logging stays off until [`set_log_level`] turns
//! it on.
//!
//! Gains are largest between builds of the same PE (x86, x64) or little-endian ELF (x86, x86-64,
//! ARM, AArch64) program, even one stored uncompressed inside a larger file. Mach-O, object
//! files, static libraries and packed or compressed binaries diff as raw bytes; .NET IL
//! assemblies and ARM64 Windows binaries are misread as x86 and x64, so compare those against
//! [`generate_raw`].
//!
//! ```no_run
//! let old = std::fs::read("app-1.0.exe")?;
//! let new = std::fs::read("app-1.1.exe")?;
//! let patch = zucchini::generate(&old, &new)?;
//! assert_eq!(zucchini::apply(&old, &patch)?, new);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::ffi::{CStr, CString, c_int};
use std::fmt;
use std::ptr;
use std::sync::Once;
use std::sync::atomic::{AtomicPtr, Ordering};

use zucchini_sys as sys;

/// Largest file Zucchini diffs or produces; its patch header stores 32-bit sizes.
pub const MAX_LEN: usize = u32::MAX as usize;

/// How [`generate_with`] builds a patch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerateOptions {
    /// Diff both files as raw bytes, without looking for executables.
    pub raw: bool,
    /// Offset where executable detection starts. Skips an outer wrapper, such as a self-extracting
    /// archive, so the executables inside it are found instead.
    pub start_scan_at: u32,
    /// Element matches that replace Zucchini's own, written `old_offset+old_size=new_offset+new_size`
    /// and separated by commas. A non-empty string takes precedence over `raw`. Zucchini does not
    /// report a malformed string: it logs an error and makes a raw patch instead.
    pub imposed_matches: Option<String>,
}

/// The header at the start of every patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatchHeader {
    /// Patch format version, as (major, minor).
    pub version: (u16, u16),
    /// Size of the file the patch applies to.
    pub old_size: u32,
    /// CRC-32 of the file the patch applies to.
    pub old_crc: u32,
    /// Size of the file the patch produces.
    pub new_size: u32,
    /// CRC-32 of the file the patch produces.
    pub new_crc: u32,
}

/// Why a Zucchini call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The patch is truncated, damaged, or in a format version this build does not read.
    InvalidPatch,
    /// Applying: the old file's size or CRC-32 differs from the file the patch was made from.
    /// Generating: Zucchini could not read the old file.
    WrongOldFile,
    /// Applying: the result's CRC-32 differs from the one the patch recorded.
    /// Generating: Zucchini could not read the new file.
    WrongNewFile,
    /// A file is larger than [`MAX_LEN`].
    TooLarge,
    /// Zucchini cannot patch from or to an empty file; send the new file whole instead.
    EmptyFile,
    /// An argument could not be passed to Zucchini, such as imposed matches containing a NUL byte.
    InvalidArgument,
    /// The buffer given to [`apply_into`] is not the size the patch produces.
    BufferSize { expected: usize, actual: usize },
    /// Memory for the patch or the result ran out.
    OutOfMemory,
    /// One of Chromium's internal consistency checks failed; the message names it.
    CheckFailed(String),
    /// Zucchini gave up, or a C++ exception other than a failed check reached the API; the message
    /// is the exception's, or empty.
    Fatal(String),
    /// Any other Zucchini status code.
    Status(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidPatch => f.write_str("not a valid Zucchini patch"),
            Error::WrongOldFile => f.write_str("the old file is not the one the patch was made from"),
            Error::WrongNewFile => f.write_str("the patched result does not match the patch's checksum"),
            Error::TooLarge => write!(f, "a file is larger than {MAX_LEN} bytes"),
            Error::EmptyFile => f.write_str("Zucchini cannot patch from or to an empty file"),
            Error::InvalidArgument => f.write_str("Zucchini rejected an argument"),
            Error::BufferSize { expected, actual } => {
                write!(f, "the output buffer holds {actual} bytes but the patch produces {expected}")
            }
            Error::OutOfMemory => f.write_str("out of memory"),
            Error::CheckFailed(message) => write!(f, "Zucchini internal check failed: {message}"),
            Error::Fatal(message) if message.is_empty() => f.write_str("Zucchini failed"),
            Error::Fatal(message) => write!(f, "Zucchini failed: {message}"),
            Error::Status(code) => write!(f, "Zucchini failed with status {code}"),
        }
    }
}

impl std::error::Error for Error {}

/// Which of Zucchini's log messages reach stderr.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    #[default]
    Off,
    Error,
    Warning,
    Info,
}

/// Makes a patch from `old` to `new`, diffing any PE or ELF executables Zucchini finds as code.
pub fn generate(old: &[u8], new: &[u8]) -> Result<Vec<u8>, Error> {
    generate_with(old, new, &GenerateOptions::default())
}

/// Makes a patch from `old` to `new` that treats both as raw bytes.
pub fn generate_raw(old: &[u8], new: &[u8]) -> Result<Vec<u8>, Error> {
    generate_with(old, new, &GenerateOptions { raw: true, ..GenerateOptions::default() })
}

/// Makes a patch from `old` to `new` with explicit options.
pub fn generate_with(old: &[u8], new: &[u8], options: &GenerateOptions) -> Result<Vec<u8>, Error> {
    default_crc32();
    check_len(old)?;
    check_len(new)?;
    let imposed = options
        .imposed_matches
        .as_deref()
        .map(CString::new)
        .transpose()
        .map_err(|_| Error::InvalidArgument)?;
    let c_options = sys::zucc_generate_options {
        start_scan_at: options.start_scan_at,
        raw: c_int::from(options.raw),
        imposed_matches: imposed.as_ref().map_or(ptr::null(), |matches| matches.as_ptr()),
    };

    let mut handle = ptr::null_mut();
    // SAFETY: both slices, `c_options` and the string it points to outlive the call, and `handle`
    // is a valid place for the result.
    status(unsafe {
        sys::zucc_generate(old.as_ptr(), old.len(), new.as_ptr(), new.len(), &c_options, &mut handle)
    })?;
    let patch = Patch(handle);

    // SAFETY: `patch` holds a live patch from zucc_generate.
    let len = unsafe { sys::zucc_patch_size(patch.0) };
    let mut out = zeroed(len)?;
    // SAFETY: `patch` is live and `out` is exactly `len` writable bytes.
    status(unsafe { sys::zucc_patch_write(patch.0, out.as_mut_ptr(), out.len()) })?;
    Ok(out)
}

/// Validates `patch` and returns its header.
pub fn read_header(patch: &[u8]) -> Result<PatchHeader, Error> {
    let mut header = sys::zucc_patch_header::default();
    // SAFETY: `patch` is readable for its length and `header` is a valid place for the result.
    status(unsafe { sys::zucc_read_header(patch.as_ptr(), patch.len(), &mut header) })?;
    Ok(PatchHeader {
        version: (header.major_version, header.minor_version),
        old_size: header.old_size,
        old_crc: header.old_crc,
        new_size: header.new_size,
        new_crc: header.new_crc,
    })
}

/// Applies `patch` to `old` and returns the new file. Both files are checked against the CRC-32s
/// the patch recorded.
///
/// The result is allocated at the header's `new_size` before the old file's CRC-32 is checked, so a
/// crafted patch can ask for up to [`MAX_LEN`] bytes. Use [`apply_bounded`] for untrusted patches
/// to cap that allocation.
pub fn apply(old: &[u8], patch: &[u8]) -> Result<Vec<u8>, Error> {
    apply_bounded(old, patch, MAX_LEN)
}

/// Applies `patch` to `old` like [`apply`], but fails with [`Error::TooLarge`] before allocating
/// when the header's [`PatchHeader::new_size`] exceeds `max_new`.
///
/// A patch names the size of its result in a header field an attacker controls independently of the
/// old file, so [`apply`] can be made to allocate up to [`MAX_LEN`] bytes for a patch that later
/// fails its CRC-32 check. Bounding `new_size` up front caps that work for untrusted patches.
pub fn apply_bounded(old: &[u8], patch: &[u8], max_new: usize) -> Result<Vec<u8>, Error> {
    let header = read_header(patch)?;
    if header.new_size as usize > max_new {
        return Err(Error::TooLarge);
    }
    if old.len() != header.old_size as usize {
        return Err(Error::WrongOldFile);
    }
    let mut new = zeroed(header.new_size as usize)?;
    apply_into(old, patch, &mut new)?;
    Ok(new)
}

/// Applies `patch` to `old`, writing the new file into `new`, which must be exactly the patch's
/// [`PatchHeader::new_size`].
pub fn apply_into(old: &[u8], patch: &[u8], new: &mut [u8]) -> Result<(), Error> {
    default_crc32();
    // SAFETY: `old` and `patch` are readable and `new` writable for their lengths.
    let code = unsafe {
        sys::zucc_apply(old.as_ptr(), old.len(), patch.as_ptr(), patch.len(), new.as_mut_ptr(), new.len())
    };
    if code == sys::ZUCC_BUFFER_SIZE {
        let expected = read_header(patch)?.new_size as usize;
        return Err(Error::BufferSize { expected, actual: new.len() });
    }
    status(code)
}

/// Chooses which of Zucchini's log messages are written to stderr. Logging is off by default.
pub fn set_log_level(level: LogLevel) {
    let level = match level {
        LogLevel::Off => sys::ZUCC_LOG_OFF,
        LogLevel::Error => sys::ZUCC_LOG_ERROR,
        LogLevel::Warning => sys::ZUCC_LOG_WARNING,
        LogLevel::Info => sys::ZUCC_LOG_INFO,
    };
    // SAFETY: accepts any value and only stores it.
    unsafe { sys::zucc_set_log_level(level) }
}

/// Chooses the CRC-32 Zucchini computes over both whole files when it generates or applies a patch.
/// `None` goes back to the default: crc-fast with the `crc-fast` feature, otherwise Chromium's
/// byte-at-a-time loop.
///
/// The choice applies to the whole process, including calls already running on other threads, so
/// `crc32` must return what zlib's `crc32` does (CRC-32/ISO-HDLC); anything else makes every apply
/// fail with [`Error::WrongOldFile`]. A panic inside it aborts the process, since it is called
/// from C++.
pub fn set_crc32(crc32: Option<fn(&[u8]) -> u32>) {
    default_crc32();
    match crc32 {
        Some(crc32) => {
            CRC32.store(crc32 as *mut (), Ordering::Release);
            // SAFETY: crc32_from_c is thread safe and computes the CRC-32 with the function just
            // stored, which the caller vouches for.
            unsafe { sys::zucc_set_crc32(Some(crc32_from_c)) }
        }
        None => install_default_crc32(),
    }
}

/// The function crc32_from_c calls, a `fn(&[u8]) -> u32`; null until one is installed.
static CRC32: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// Installs the default CRC-32 the first time a patch is generated or applied, unless
/// [`set_crc32`] chose one first.
fn default_crc32() {
    static DEFAULT: Once = Once::new();
    DEFAULT.call_once(install_default_crc32);
}

#[cfg(feature = "crc-fast")]
fn install_default_crc32() {
    CRC32.store(crc32_fast as fn(&[u8]) -> u32 as *mut (), Ordering::Release);
    // SAFETY: crc32_from_c is thread safe, and crc-fast's CRC-32/ISO-HDLC is zlib's CRC-32.
    unsafe { sys::zucc_set_crc32(Some(crc32_from_c)) }
}

#[cfg(not(feature = "crc-fast"))]
fn install_default_crc32() {
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

/// Owns a generated patch until it has been written out.
struct Patch(*mut sys::zucc_patch);

impl Drop for Patch {
    fn drop(&mut self) {
        // SAFETY: the pointer came from zucc_generate and is freed only here.
        unsafe { sys::zucc_patch_free(self.0) }
    }
}

fn check_len(data: &[u8]) -> Result<(), Error> {
    if data.len() > MAX_LEN { Err(Error::TooLarge) } else { Ok(()) }
}

/// A zero-filled buffer, or `OutOfMemory` instead of aborting when `len` cannot be allocated.
fn zeroed(len: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(len).map_err(|_| Error::OutOfMemory)?;
    buffer.resize(len, 0);
    Ok(buffer)
}

fn status(code: c_int) -> Result<(), Error> {
    match code {
        sys::ZUCC_OK => Ok(()),
        sys::ZUCC_INVALID_PARAM => Err(Error::InvalidArgument),
        sys::ZUCC_PATCH_READ_ERROR => Err(Error::InvalidPatch),
        sys::ZUCC_INVALID_OLD_IMAGE => Err(Error::WrongOldFile),
        sys::ZUCC_INVALID_NEW_IMAGE => Err(Error::WrongNewFile),
        sys::ZUCC_TOO_LARGE => Err(Error::TooLarge),
        sys::ZUCC_EMPTY_FILE => Err(Error::EmptyFile),
        sys::ZUCC_OUT_OF_MEMORY => Err(Error::OutOfMemory),
        sys::ZUCC_CHECK_FAILED => Err(Error::CheckFailed(last_error())),
        sys::ZUCC_FATAL => Err(Error::Fatal(last_error())),
        other => Err(Error::Status(other)),
    }
}

fn last_error() -> String {
    // SAFETY: zucc_last_error returns a NUL-terminated string owned by this thread, valid until the
    // thread's next call into the C API.
    unsafe { CStr::from_ptr(sys::zucc_last_error()) }.to_string_lossy().into_owned()
}
