//! Progress reports and cancellation, through the C API's `zucc_*_with_progress` functions (the
//! `progress` feature).

use std::any::Any;
use std::ffi::{c_int, c_void};
use std::ops::ControlFlow;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::time::Duration;

use zucchini_sys as sys;

use crate::{Error, GenerateOptions};

/// How often the progress closure runs: when the fraction done crosses a multiple of `step`, but
/// no sooner than `min_interval` after the last call. The default, each percent at most every
/// 100 ms, keeps calls rare enough to cost nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressRate {
    /// 0.01 for each percent, 0 for every move.
    pub step: f64,
    /// [`Duration::ZERO`] for no limit.
    pub min_interval: Duration,
}

impl Default for ProgressRate {
    fn default() -> Self {
        ProgressRate { step: 0.01, min_interval: Duration::from_millis(100) }
    }
}

/// Makes a patch like [`generate_with`](crate::generate_with), calling `progress` with the
/// fraction done as it goes.
///
/// `progress` runs on this thread, as often as `rate` allows, with a fraction from 0 to 1 that
/// rises with each call: 0 first, and 1 only once the patch is made. Zucchini reports between
/// stretches of its work rather than through its tightest loops, so calls can be a second or two
/// apart on large files. Returning [`ControlFlow::Break`] stops the work with
/// [`Error::Cancelled`] at that call. A panic in `progress` stops it too, and resumes once Zucchini
/// has unwound.
pub fn generate_with_progress(
    old: &[u8],
    new: &[u8],
    options: &GenerateOptions,
    rate: ProgressRate,
    mut progress: impl FnMut(f64) -> ControlFlow<()>,
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
                    rate.step,
                    rate.min_interval.as_secs_f64(),
                    handle,
                )
            }
        })
    })
}

/// Applies `patch` to `old` like [`apply_bounded`](crate::apply_bounded) (pass
/// [`MAX_LEN`](crate::MAX_LEN) for [`apply`](crate::apply)'s bound), calling `progress` with the
/// fraction done as [`generate_with_progress`] does.
pub fn apply_with_progress(
    old: &[u8],
    patch: &[u8],
    max_new: usize,
    rate: ProgressRate,
    mut progress: impl FnMut(f64) -> ControlFlow<()>,
) -> Result<Vec<u8>, Error> {
    crate::apply_bounded_inner(old, patch, max_new, |new| {
        apply_into_with_progress(old, patch, new, rate, &mut progress)
    })
}

/// Applies `patch` to `old` like [`apply_into`](crate::apply_into), calling `progress` with the
/// fraction done as [`generate_with_progress`] does.
pub fn apply_into_with_progress(
    old: &[u8],
    patch: &[u8],
    new: &mut [u8],
    rate: ProgressRate,
    mut progress: impl FnMut(f64) -> ControlFlow<()>,
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
                    rate.step,
                    rate.min_interval.as_secs_f64(),
                )
            }
        })
    })
}

/// The context the C API passes back to [`report_progress`].
struct Reporter<'a> {
    progress: &'a mut dyn FnMut(f64) -> ControlFlow<()>,
    panic: Option<Box<dyn Any + Send>>,
}

/// Runs `call` with the C progress callback and context that report to `progress`, and resumes a
/// panic from `progress` once the call has returned.
fn with_progress(
    progress: &mut dyn FnMut(f64) -> ControlFlow<()>,
    call: impl FnOnce(sys::zucc_progress_fn, *mut c_void) -> c_int,
) -> c_int {
    let mut reporter = Reporter { progress, panic: None };
    let code = call(report_progress, ptr::from_mut(&mut reporter).cast());
    if let Some(payload) = reporter.panic {
        panic::resume_unwind(payload);
    }
    code
}

unsafe extern "C" fn report_progress(context: *mut c_void, done: f64) -> c_int {
    // SAFETY: `with_progress` passes a `Reporter` that outlives the call making this one, and the C
    // API calls back only on that call's thread.
    let reporter = unsafe { &mut *context.cast::<Reporter<'_>>() };
    // Zucchini stops at the first nonzero return, so a closure that panicked is not called again.
    match panic::catch_unwind(AssertUnwindSafe(|| (reporter.progress)(done))) {
        Ok(ControlFlow::Continue(())) => 0,
        Ok(ControlFlow::Break(())) => 1,
        Err(payload) => {
            reporter.panic = Some(payload);
            1
        }
    }
}
