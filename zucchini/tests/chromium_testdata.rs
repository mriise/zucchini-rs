//! Diffs the executable pairs Chromium keeps in components/zucchini/testdata.

use std::path::Path;

fn testdata(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zucchini-sys/chromium/components/zucchini/testdata")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!("{}: {error}\nRun scripts/chromium.sh to fetch Chromium's test data.", path.display())
    })
}

fn check_pair(old_name: &str, new_name: &str) {
    let old = testdata(old_name);
    let new = testdata(new_name);
    let ensemble = zucchini::generate(&old, &new).unwrap();
    let raw = zucchini::generate_raw(&old, &new).unwrap();
    assert_eq!(zucchini::apply(&old, &ensemble).unwrap(), new);
    assert_eq!(zucchini::apply(&old, &raw).unwrap(), new);
    println!(
        "{old_name} to {new_name}: new file {} bytes, ensemble patch {}, raw patch {}",
        new.len(),
        ensemble.len(),
        raw.len()
    );
    assert!(ensemble.len() < raw.len(), "ensemble patch {} is not smaller than raw {}", ensemble.len(), raw.len());
}

#[test]
fn setup_exe() {
    check_pair("setup1.exe", "setup2.exe");
}

#[test]
fn chrome64_exe() {
    check_pair("chrome64_1.exe", "chrome64_2.exe");
}
