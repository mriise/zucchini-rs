//! Progress reports and cancellation, through the C API's `zucc_*_with_progress` functions (the
//! `progress` feature).

use std::any::Any;
use std::ffi::{c_int, c_void};
use std::ops::ControlFlow;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;

use zucchini_sys as sys;

use crate::{Error, GenerateOptions};

/// Makes a patch like [`generate_with`](crate::generate_with), calling `progress` with the
/// percentage done as it goes.
///
/// `progress` runs often on this thread, with a percentage from 0 to 100 that never falls and often
/// repeats; 100 comes only once the patch is made. Returning [`ControlFlow::Break`] stops the work
/// with [`Error::Cancelled`]. A panic in `progress` stops it too, and resumes once Zucchini has
/// unwound.
pub fn generate_with_progress(
    old: &[u8],
    new: &[u8],
    options: &GenerateOptions,
    mut progress: impl FnMut(u8) -> ControlFlow<()>,
) -> Result<Vec<u8>, Error> {
    crate::generate_inner(old, new, options, |c_options, handle| {
        with_progress(&mut progress, |callback, context| {
            // SAFETY: both slices outlive the call, `generate_inner` keeps `c_options` and the
            // string it points to alive for it, `handle` is a valid place for the result, and
            // `with_progress` keeps `context` alive for the call.
            unsafe {
                sys::zucc_generate_with_progress(
                    old.as_ptr(),
                    old.len(),
                    new.as_ptr(),
                    new.len(),
                    c_options,
                    Some(callback),
                    context,
                    handle,
                )
            }
        })
    })
}

/// Applies `patch` to `old` like [`apply_bounded`](crate::apply_bounded) (pass
/// [`MAX_LEN`](crate::MAX_LEN) for [`apply`](crate::apply)'s bound), calling `progress` with the
/// percentage done as [`generate_with_progress`] does.
pub fn apply_with_progress(
    old: &[u8],
    patch: &[u8],
    max_new: usize,
    mut progress: impl FnMut(u8) -> ControlFlow<()>,
) -> Result<Vec<u8>, Error> {
    crate::apply_bounded_inner(old, patch, max_new, |new| {
        apply_into_with_progress(old, patch, new, &mut progress)
    })
}

/// Applies `patch` to `old` like [`apply_into`](crate::apply_into), calling `progress` with the
/// percentage done as [`generate_with_progress`] does.
pub fn apply_into_with_progress(
    old: &[u8],
    patch: &[u8],
    new: &mut [u8],
    mut progress: impl FnMut(u8) -> ControlFlow<()>,
) -> Result<(), Error> {
    crate::apply_into_inner(patch, new, |new| {
        with_progress(&mut progress, |callback, context| {
            // SAFETY: `old` and `patch` are readable and `new` writable for their lengths, and
            // `with_progress` keeps `context` alive for the call.
            unsafe {
                sys::zucc_apply_with_progress(
                    old.as_ptr(),
                    old.len(),
                    patch.as_ptr(),
                    patch.len(),
                    new.as_mut_ptr(),
                    new.len(),
                    Some(callback),
                    context,
                )
            }
        })
    })
}

/// The context the C API passes back to [`report_progress`].
struct Reporter<'a> {
    progress: &'a mut dyn FnMut(u8) -> ControlFlow<()>,
    panic: Option<Box<dyn Any + Send>>,
}

/// Runs `call` with the C progress callback and context that report to `progress`, and resumes a
/// panic from `progress` once the call has returned.
fn with_progress(
    progress: &mut dyn FnMut(u8) -> ControlFlow<()>,
    call: impl FnOnce(sys::zucc_progress_fn, *mut c_void) -> c_int,
) -> c_int {
    let mut reporter = Reporter { progress, panic: None };
    let code = call(report_progress, ptr::from_mut(&mut reporter).cast());
    if let Some(payload) = reporter.panic {
        panic::resume_unwind(payload);
    }
    code
}

unsafe extern "C" fn report_progress(context: *mut c_void, percent: c_int) -> c_int {
    // SAFETY: `with_progress` passes a `Reporter` that outlives the call making this one, and the C
    // API calls back only on that call's thread.
    let reporter = unsafe { &mut *context.cast::<Reporter<'_>>() };
    // Zucchini stops at the first nonzero return, so a closure that panicked is not called again.
    match panic::catch_unwind(AssertUnwindSafe(|| (reporter.progress)(percent as u8))) {
        Ok(ControlFlow::Continue(())) => 0,
        Ok(ControlFlow::Break(())) => 1,
        Err(payload) => {
            reporter.panic = Some(payload);
            1
        }
    }
}
