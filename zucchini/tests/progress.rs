//! Progress reports and cancellation, on one of Chromium's executable pairs and on raw data.

use std::ops::ControlFlow;
use std::panic;
use std::path::Path;

use zucchini::{
    Error, GenerateOptions, MAX_LEN, apply, apply_into_with_progress, apply_with_progress, generate,
    generate_with, generate_with_progress,
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

/// Checks one run's reports: from 0, never falling, ending at a single 100, through many steps.
fn check_reports(reports: &[u8]) {
    assert_eq!(reports.first(), Some(&0));
    assert!(reports.windows(2).all(|pair| pair[0] <= pair[1]), "reports fell: {reports:?}");
    assert_eq!(reports.iter().filter(|&&percent| percent == 100).count(), 1);
    assert_eq!(reports.last(), Some(&100));
    let mut distinct = reports.to_vec();
    distinct.dedup();
    assert!(distinct.len() > 20, "only {} distinct percentages: {distinct:?}", distinct.len());
}

#[test]
fn generate_reports_progress_and_makes_the_same_patch() {
    let (old, new) = exe_pair();
    for options in [GenerateOptions::default(), GenerateOptions { raw: true, ..GenerateOptions::default() }] {
        let mut reports = Vec::new();
        let patch = generate_with_progress(&old, &new, &options, |percent| {
            reports.push(percent);
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
    let result = apply_with_progress(&old, &patch, MAX_LEN, |percent| {
        reports.push(percent);
        ControlFlow::Continue(())
    })
    .unwrap();
    check_reports(&reports);
    assert_eq!(result, new);

    let mut into = vec![0; new.len()];
    let mut last = 0;
    apply_into_with_progress(&old, &patch, &mut into, |percent| {
        last = percent;
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!((last, into), (100, new));
}

#[test]
fn breaking_cancels_at_any_point() {
    let (old, new) = exe_pair();
    let patch = generate(&old, &new).unwrap();
    for stop_at in [0, 1, 50, 99, 100] {
        let mut calls_after_break = 0;
        let mut broke = false;
        let result = generate_with_progress(&old, &new, &GenerateOptions::default(), |percent| {
            calls_after_break += u32::from(broke);
            broke |= percent >= stop_at;
            if broke { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        assert_eq!(result, Err(Error::Cancelled), "generate stopping at {stop_at}%");
        assert_eq!(calls_after_break, 0);

        let result = apply_with_progress(&old, &patch, MAX_LEN, |percent| {
            if percent >= stop_at { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        assert_eq!(result, Err(Error::Cancelled), "apply stopping at {stop_at}%");
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
        generate_with_progress(&old, &new, &GenerateOptions::default(), |percent| {
            assert!(percent < 30, "stop here");
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
    let patch = generate_with_progress(&old, &new, &GenerateOptions::default(), |percent| {
        reports.push(percent);
        if percent >= 50 && inner == 0 {
            inner += 1;
            let inner_patch =
                generate_with_progress(&small, &old[..20_000], &GenerateOptions::default(), |_| {
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
    assert_eq!(reports.last(), Some(&100));
    assert_eq!(apply(&old, &patch).unwrap(), new);
}
