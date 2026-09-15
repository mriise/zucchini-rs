use zucchini::{
    Error, GenerateOptions, apply, apply_into, generate, generate_raw, generate_with, read_header,
};

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

/// `old` with an overwrite, an insertion and a truncation. Needs at least 150 KB.
fn edited(old: &[u8]) -> Vec<u8> {
    let mut new = old.to_vec();
    new[1_000..1_064].fill(0xAB);
    new.splice(100_000..100_000, noise(3_000, 99));
    new.truncate(new.len() - 20_000);
    new
}

#[test]
fn raw_patch_round_trips_and_stays_small() {
    let old = noise(400_000, 1);
    let new = edited(&old);
    let patch = generate_raw(&old, &new).unwrap();
    assert_eq!(apply(&old, &patch).unwrap(), new);
    assert!(patch.len() < new.len() / 10, "patch is {} bytes", patch.len());
}

#[test]
fn ensemble_patch_of_data_without_executables_round_trips() {
    let old = noise(400_000, 2);
    let new = edited(&old);
    let patch = generate(&old, &new).unwrap();
    assert_eq!(apply(&old, &patch).unwrap(), new);
}

#[test]
fn identical_files_make_a_tiny_patch() {
    let data = noise(200_000, 3);
    let patch = generate(&data, &data).unwrap();
    assert!(patch.len() < 1_000, "patch is {} bytes", patch.len());
    assert_eq!(apply(&data, &patch).unwrap(), data);
}

#[test]
fn empty_files_are_refused() {
    // Zucchini writes patches for these, but its own reader rejects them.
    let data = noise(10_000, 4);
    for (old, new) in [(&[][..], &data[..]), (&data[..], &[][..]), (&[][..], &[][..])] {
        assert_eq!(generate(old, new), Err(Error::EmptyFile));
        assert_eq!(generate_raw(old, new), Err(Error::EmptyFile));
    }
}

#[test]
fn header_describes_both_files() {
    let old = noise(150_000, 5);
    let new = edited(&old);
    let header = read_header(&generate(&old, &new).unwrap()).unwrap();
    assert_eq!((header.old_size as usize, header.new_size as usize), (old.len(), new.len()));
    assert_ne!(header.old_crc, header.new_crc);
}

#[test]
fn rejects_the_wrong_old_file() {
    let old = noise(150_000, 6);
    let new = edited(&old);
    let patch = generate_raw(&old, &new).unwrap();
    let mut same_size = old.clone();
    same_size[70_000] ^= 1;
    assert_eq!(apply(&same_size, &patch), Err(Error::WrongOldFile));
    assert_eq!(apply(&old[1..], &patch), Err(Error::WrongOldFile));
}

#[test]
fn rejects_data_that_is_not_a_patch() {
    assert_eq!(read_header(b"not a patch"), Err(Error::InvalidPatch));
    assert_eq!(apply(b"old", b""), Err(Error::InvalidPatch));
}

#[test]
fn damaged_patches_fail_without_crashing() {
    let old = noise(150_000, 7);
    let new = edited(&old);
    for patch in [generate_raw(&old, &new).unwrap(), generate(&old, &new).unwrap()] {
        let mut failures = 0;
        let mut check = |damaged: &[u8]| match apply(&old, damaged) {
            Ok(result) => assert!(result == new, "a damaged patch produced other bytes without an error"),
            Err(_) => failures += 1,
        };
        for cut in (0..patch.len()).step_by((patch.len() / 200).max(1)) {
            check(&patch[..cut]);
        }
        for i in 0..600 {
            let mut damaged = patch.clone();
            let at = (i * 7_919) % damaged.len();
            damaged[at] ^= 1 << (i % 8);
            check(&damaged);
        }
        assert!(failures > 200, "only {failures} damaged patches were rejected");
    }
}

#[test]
fn apply_into_checks_the_buffer_size() {
    let old = noise(150_000, 8);
    let new = edited(&old);
    let patch = generate_raw(&old, &new).unwrap();
    let mut short = vec![0; new.len() - 1];
    assert_eq!(
        apply_into(&old, &patch, &mut short),
        Err(Error::BufferSize { expected: new.len(), actual: new.len() - 1 })
    );
    let mut exact = vec![0; new.len()];
    apply_into(&old, &patch, &mut exact).unwrap();
    assert_eq!(exact, new);
}

#[test]
fn imposed_matches_containing_nul_are_rejected() {
    let options = GenerateOptions { imposed_matches: Some("0+1\0=0+1".into()), ..Default::default() };
    assert_eq!(generate_with(b"a", b"a", &options), Err(Error::InvalidArgument));
}

#[test]
fn threads_diff_at_the_same_time() {
    let threads: Vec<_> = (0..4)
        .map(|seed| {
            std::thread::spawn(move || {
                let old = noise(300_000, 10 + seed);
                let new = edited(&old);
                let patch = generate(&old, &new).unwrap();
                assert_eq!(apply(&old, &patch).unwrap(), new);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}
