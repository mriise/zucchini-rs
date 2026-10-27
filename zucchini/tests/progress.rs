//! Progress reports and cancellation, on one of Chromium's executable pairs and on raw data.

#![cfg(feature = "progress")]

use std::ops::ControlFlow;
use std::panic;
use std::path::Path;
use std::time::{Duration, Instant};

use zucchini::{
    Error, GenerateOptions, MAX_LEN, ProgressRate, apply, apply_into_with_progress, apply_with_progress,
    generate, generate_with, generate_with_progress,
};

fn testdata(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zucchini-sys/chromium/components/zucchini/testdata")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!("{}: {error}\nRun scripts/chromium.sh to fetch Chromium's test data.", path.display())
    })
}

/// Deterministic pseudo-random bytes, so failures reproduce.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn exe_pair() -> (Vec<u8>, Vec<u8>) {
    (testdata("chrome64_1.exe"), testdata("chrome64_2.exe"))
}

/// Reports every percent, with no time limit, so that small files report many times.
const STEP: ProgressRate = ProgressRate { step: 0.01, min_interval: Duration::ZERO };

/// Reports every move.
const EVERY: ProgressRate = ProgressRate { step: 0.0, min_interval: Duration::ZERO };

/// Checks one run's reports at `STEP`: from 0, each in a later percent than the last, ending at a
/// single 1, through many steps.
fn check_reports(reports: &[f64]) {
    assert_eq!(reports.first(), Some(&0.0));
    let percents: Vec<f64> = reports.iter().map(|done| (done * 100.0).floor()).collect();
    assert!(percents.windows(2).all(|pair| pair[0] < pair[1]), "reports repeat a percent: {reports:?}");
    assert_eq!(reports.iter().filter(|&&done| done >= 1.0).count(), 1);
    assert_eq!(reports.last(), Some(&1.0));
    assert!(reports.len() > 30, "only {} reports: {reports:?}", reports.len());
}

#[test]
fn generate_reports_progress_and_makes_the_same_patch() {
    let (old, new) = exe_pair();
    for options in [GenerateOptions::default(), GenerateOptions { raw: true, ..GenerateOptions::default() }] {
        let mut reports = Vec::new();
        let patch = generate_with_progress(&old, &new, &options, STEP, |done| {
            reports.push(done);
            ControlFlow::Continue(())
        })
        .unwrap();
        check_reports(&reports);
        assert_eq!(patch, generate_with(&old, &new, &options).unwrap());
    }
}

#[test]
fn apply_reports_progress_and_makes_the_same_file() {
    let (old, new) = exe_pair();
    let patch = generate(&old, &new).unwrap();

    let mut reports = Vec::new();
    let result = apply_with_progress(&old, &patch, MAX_LEN, STEP, |done| {
        reports.push(done);
        ControlFlow::Continue(())
    })
    .unwrap();
    check_reports(&reports);
    assert_eq!(result, new);

    let mut into = vec![0; new.len()];
    let mut last = 0.0;
    apply_into_with_progress(&old, &patch, &mut into, STEP, |done| {
        last = done;
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!((last, into), (1.0, new));
}

#[test]
fn a_step_of_zero_reports_every_move_and_a_large_one_only_the_ends() {
    let (old, new) = exe_pair();
    let mut every = Vec::new();
    generate_with_progress(&old, &new, &GenerateOptions::default(), EVERY, |done| {
        every.push(done);
        ControlFlow::Continue(())
    })
    .unwrap();
    assert!(every.windows(2).all(|pair| pair[0] < pair[1]), "reports did not rise: {every:?}");
    assert!(every.len() > 100, "only {} reports", every.len());

    let mut ends = Vec::new();
    let ends_only = ProgressRate { step: 2.0, ..EVERY };
    generate_with_progress(&old, &new, &GenerateOptions::default(), ends_only, |done| {
        ends.push(done);
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(ends, [0.0, 1.0]);
}

#[test]
fn the_default_rate_waits_between_calls_but_always_reports_the_end() {
    let (old, new) = exe_pair();
    let min_interval = ProgressRate::default().min_interval;
    let mut calls = Vec::new();
    generate_with_progress(&old, &new, &GenerateOptions::default(), ProgressRate::default(), |done| {
        calls.push((Instant::now(), done));
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(calls.first().map(|call| call.1), Some(0.0));
    assert_eq!(calls.last().map(|call| call.1), Some(1.0));
    // Every call but the last, which always comes, waits out the interval.
    let spaced = &calls[..calls.len() - 1];
    assert!(spaced.windows(2).all(|pair| pair[1].0 - pair[0].0 >= min_interval), "{calls:?}");
}

#[test]
fn breaking_cancels_at_any_point() {
    let (old, new) = exe_pair();
    let patch = generate(&old, &new).unwrap();
    for stop_at in [0.0, 0.01, 0.5, 0.99, 1.0] {
        let mut calls_after_break = 0;
        let mut broke = false;
        let result = generate_with_progress(&old, &new, &GenerateOptions::default(), STEP, |done| {
            calls_after_break += u32::from(broke);
            broke |= done >= stop_at;
            if broke { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        assert_eq!(result, Err(Error::Cancelled), "generate stopping at {stop_at}");
        assert_eq!(calls_after_break, 0);

        let result = apply_with_progress(&old, &patch, MAX_LEN, STEP, |done| {
            if done >= stop_at { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        assert_eq!(result, Err(Error::Cancelled), "apply stopping at {stop_at}");
    }
    // Nothing is left behind on this thread.
    assert_eq!(apply(&old, &patch).unwrap(), new);
}

#[test]
fn a_panic_in_progress_resumes_after_zucchini_unwinds() {
    let old = noise(300_000, 1);
    let mut new = old.clone();
    new[1_000..2_000].fill(0);
    let caught = panic::catch_unwind(|| {
        generate_with_progress(&old, &new, &GenerateOptions::default(), STEP, |done| {
            assert!(done < 0.3, "stop here");
            ControlFlow::Continue(())
        })
    })
    .unwrap_err();
    assert_eq!(caught.downcast_ref::<&str>(), Some(&"stop here"));
    assert_eq!(apply(&old, &generate(&old, &new).unwrap()).unwrap(), new);
}

#[test]
fn zucchini_can_be_called_from_inside_progress() {
    let old = noise(300_000, 2);
    let mut new = old.clone();
    new[5_000..6_000].fill(0xAB);
    let small = noise(20_000, 3);
    let mut reports = Vec::new();
    let mut inner = 0;
    let patch = generate_with_progress(&old, &new, &GenerateOptions::default(), STEP, |done| {
        reports.push(done);
        if done >= 0.5 && inner == 0 {
            inner += 1;
            let inner_patch =
                generate_with_progress(&small, &old[..20_000], &GenerateOptions::default(), EVERY, |_| {
                    ControlFlow::Continue(())
                })
                .unwrap();
            assert_eq!(apply(&small, &inner_patch).unwrap(), old[..20_000]);
        }
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(inner, 1);
    // The outer call's reports carry on from where they were, not from the inner call's.
    assert!(reports.windows(2).all(|pair| pair[0] <= pair[1]), "{reports:?}");
    assert_eq!(reports.last(), Some(&1.0));
    assert_eq!(apply(&old, &patch).unwrap(), new);
}
