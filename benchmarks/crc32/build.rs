use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let chromium = root.join("zucchini-sys/chromium");
    let target = env::var("TARGET").unwrap();
    let msvc = target.contains("msvc");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .opt_level(2)
        .warnings(false)
        .define("NDEBUG", None)
        .flag(if msvc { "/std:c++23preview" } else { "-std=c++23" })
        .include(root.join("zucchini-sys/shim"))
        .include(&chromium)
        .include(chromium.join("base/allocator/partition_allocator/src"));
    if msvc {
        build.flag("/EHsc");
    }
    if ![
        format!("CXX_{target}"),
        format!("CXX_{}", target.replace('-', "_")),
        "TARGET_CXX".into(),
        "CXX".into(),
    ]
    .iter()
    .any(|key| env::var_os(key).is_some())
    {
        build.compiler(if msvc { "clang-cl" } else { "clang++" });
    }
    // Compile the untouched pinned byte loop under a benchmark-only name.
    let mut original = build.clone();
    original
        .define("CalculateCrc32", "CalculateCrc32Original")
        .file(chromium.join("components/zucchini/crc32.cc"));
    build.objects(original.compile_intermediates()).file("bridge.cc").compile("crc32_benchmark");
    println!("cargo:rerun-if-changed=bridge.cc");
    println!("cargo:rerun-if-changed={}", chromium.join("components/zucchini/crc32.cc").display());
}
