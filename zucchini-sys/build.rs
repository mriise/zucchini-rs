//! Builds Chromium's Zucchini into a static library, or links a prebuilt one.
//!
//! Source build (the default): compiles the `zucchini_lib` sources Chromium's own BUILD.gn lists,
//! from the sparse checkout in `chromium/` (scripts/chromium.sh fetches it), together with the C API
//! and the //base shims in `shim/`. Chromium's headers only support clang, so this uses clang-cl on
//! Windows and clang++ elsewhere unless CXX names another compiler.
//!
//! Prebuilt (feature `prebuilt`): takes this target's archive from the release pinned in
//! `prebuilt.txt`, refuses it unless its SHA-256 matches the pin, and links it.
//!
//! Environment:
//!   ZUCCHINI_SYS_FROM_SOURCE=1            build from source even with the `prebuilt` feature
//!   ZUCCHINI_SYS_PREBUILT_ARCHIVE=<file>  read the pinned archive from disk instead of downloading
//!   ZUCCHINI_SYS_LIB_DIR=<dir>            link the library already in <dir>, unchecked
//!   ZUCCHINI_SYS_CHROMIUM_DIR=<dir>       Chromium checkout to build from instead of `chromium/`
//!   ZUCCHINI_SYS_DCHECK=1                 keep Chromium's DCHECKs
//!   ZUCCHINI_SYS_EXPORT_DIR=<dir>         also copy the built library, header, licenses and build
//!                                         facts to <dir>; the prebuilt workflow packages that

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use diffy::patch_set::{FileOperation, ParseOptions, PatchSet};
use diffy::{Line, Patch};

const LIB: &str = "zucchini";

/// Unified diffs in `patches/` that build.rs applies, in this order, to copies of Chromium files,
/// with the Cargo feature that turns each on. Each file's header says what it changes and why.
/// Only the libstdc++ fix always applies. The others are opt-in changes to Zucchini's code, for
/// progress reports or speed, and patches and results stay byte-identical with or without them.
/// Each applies with or without the ones before it, except where noted.
const PATCHES: &[(&str, Option<&str>)] = &[
    ("libstdcxx-span.patch", None),
    ("progress.patch", Some("progress")),
    ("vector-containers.patch", Some("vector-containers")),
    // Written against the vector-containers result; the feature turns that one on too.
    ("radix-sort.patch", Some("radix-sort")),
    ("crc32-hook.patch", Some("crc32-hook")),
];

fn main() {
    let target = env::var("TARGET").unwrap();
    for var in [
        "ZUCCHINI_SYS_FROM_SOURCE",
        "ZUCCHINI_SYS_PREBUILT_ARCHIVE",
        "ZUCCHINI_SYS_LIB_DIR",
        "ZUCCHINI_SYS_CHROMIUM_DIR",
        "ZUCCHINI_SYS_DCHECK",
        "ZUCCHINI_SYS_EXPORT_DIR",
        "DOCS_RS",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    for var in cxx_vars(&target) {
        println!("cargo:rerun-if-env-changed={var}");
    }

    // docs.rs has neither clang nor network access, and the bindings document without the library.
    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    if let Some(dir) = env::var_os("ZUCCHINI_SYS_LIB_DIR") {
        let dir = Path::new(&dir);
        println!("cargo:rerun-if-changed={}", dir.join(lib_file(&target)).display());
        link(dir, &target);
        return;
    }

    #[cfg(feature = "prebuilt")]
    if !env_flag("ZUCCHINI_SYS_FROM_SOURCE") {
        // Prebuilt archives are built without the optional patches: linking one would quietly drop
        // a speed-up, or fail to link zucc_set_crc32 or the progress functions.
        let features: Vec<&str> = enabled_patches().into_iter().filter_map(|(_, feature)| feature).collect();
        if !features.is_empty() {
            let features = features.join("`, `");
            fail(&format!(
                "the `{features}` features change Zucchini's sources, but prebuilt libraries are \
                 built without them\n  Build from source instead: set ZUCCHINI_SYS_FROM_SOURCE=1, or \
                 drop those features or `prebuilt`."
            ));
        }
        let dir = prebuilt::fetch(&manifest, &out, &target);
        link(&dir.join("lib"), &target);
        return;
    }

    build_from_source(&manifest, &out, &target);
}

fn build_from_source(manifest: &Path, out: &Path, target: &str) {
    let chromium = env::var_os("ZUCCHINI_SYS_CHROMIUM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("chromium"));
    let component = chromium.join("components/zucchini");
    let shim = manifest.join("shim");

    let Ok(gn) = fs::read_to_string(component.join("BUILD.gn")) else {
        fail(&format!(
            "Chromium sources are missing from {}\n  \
             Run scripts/chromium.sh to fetch them, or enable the `prebuilt` feature.",
            chromium.display()
        ))
    };
    let sources = gn_sources(&gn, "zucchini_lib");
    if sources.is_empty() {
        fail("found no zucchini_lib sources in components/zucchini/BUILD.gn; has its layout changed?");
    }

    let patched = out.join("chromium-patched");
    let patches = enabled_patches();
    let patched_files = write_patched_files(&manifest.join("patches"), &patches, &chromium, &patched);
    // A patched source is compiled from its copy; a patched header is found there first.
    let sources: Vec<PathBuf> = sources
        .iter()
        .map(|source| {
            let file = format!("components/zucchini/{source}");
            if patched_files.contains(&file) { patched.join(file) } else { component.join(source) }
        })
        .collect();

    let msvc = target.contains("msvc");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        // The standard flags Chromium's own build passes (build/config/compiler/BUILD.gn).
        .flag(if msvc { "/std:c++23preview" } else { "-std=c++23" })
        // Chromium's code is not ours to fix, so silence warnings; errors still reach cargo.
        .warnings(false)
        .flag("-w")
        // Zucchini is very slow unoptimised, so keep it fast in debug builds of its users too.
        .opt_level(2)
        .include(&shim)
        .include(&patched)
        .include(&chromium)
        .include(chromium.join("base/allocator/partition_allocator/src"))
        .file(shim.join("zucchini_c.cc"))
        .files(&sources);
    if msvc {
        // A failed CHECK throws to the C API boundary (shim/base/check.h), so exceptions stay on.
        build.flag("/EHsc");
    }
    if !env_flag("ZUCCHINI_SYS_DCHECK") {
        build.define("NDEBUG", None);
    }
    if patches.iter().any(|(_, feature)| *feature == Some("progress")) {
        // shim/zucchini_c.cc then defines the progress functions progress.patch declares.
        build.define("ZUCCHINI_RS_PROGRESS", None);
    }
    if patches.iter().any(|(_, feature)| *feature == Some("crc32-hook")) {
        // shim/zucchini_c.cc then defines zucc_set_crc32 and the CalculateCrc32 that calls it.
        build.define("ZUCCHINI_RS_CRC32_HOOK", None);
    }
    if !cxx_vars(target).iter().any(|var| env::var_os(var).is_some()) {
        build.compiler(if msvc { "clang-cl" } else { "clang++" });
    }
    build.compile(LIB);

    for path in [shim, manifest.join("patches"), component, chromium.join("base"), chromium.join("build")] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    if let Some(dir) = env::var_os("ZUCCHINI_SYS_EXPORT_DIR") {
        export(Path::new(&dir), manifest, &chromium, out, target, &build, &patches);
    }
}

/// The PATCHES entries this build applies: those without a feature, and those whose feature is on.
fn enabled_patches() -> Vec<(&'static str, Option<&'static str>)> {
    PATCHES
        .iter()
        .copied()
        .filter(|(_, feature)| {
            feature.is_none_or(|feature| {
                env::var_os(format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"))).is_some()
            })
        })
        .collect()
}

/// Applies `patches` (files in `patch_dir`) to copies of Chromium files, writes the copies into
/// `dir`, which the compiler searches before `chromium`, and returns their paths relative to
/// `chromium`.
fn write_patched_files(
    patch_dir: &Path,
    patches: &[(&str, Option<&str>)],
    chromium: &Path,
    dir: &Path,
) -> Vec<String> {
    let _ = fs::remove_dir_all(dir);
    let mut files: Vec<(String, String)> = Vec::new();
    for (name, _) in patches {
        let path = patch_dir.join(name);
        let patch = fs::read_to_string(&path)
            .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", path.display())));
        apply_patch(name, &patch, chromium, &mut files);
    }
    for (file, text) in &files {
        let patched = dir.join(file);
        fs::create_dir_all(patched.parent().unwrap()).unwrap();
        fs::write(&patched, text).unwrap();
    }
    files.into_iter().map(|(file, _)| file).collect()
}

/// Applies the unified diff `patch` to `files`, (path relative to `chromium`, text) pairs, reading a
/// file from `chromium` the first time a patch changes it. Text before the first file is the patch's
/// description, and `git diff` output reads as well. Every hunk must match at exactly the line it
/// names (line endings aside), so a patch that no longer fits the pinned sources stops the build
/// instead of applying somewhere else.
fn apply_patch(name: &str, patch: &str, chromium: &Path, files: &mut Vec<(String, String)>) {
    let bad = |what: String| -> ! {
        fail(&format!(
            "patches/{name}: {what}\n  The Chromium pin has moved past this patch: regenerate it against the pin."
        ))
    };
    for file_patch in PatchSet::parse(patch, ParseOptions::unidiff()) {
        let file_patch = file_patch.unwrap_or_else(|error| bad(error.to_string()));
        let FileOperation::Modify { original, modified } = file_patch.operation().strip_prefix(1) else {
            bad(format!("only changes to existing files are supported: {:?}", file_patch.operation()))
        };
        if original != modified {
            bad(format!("`--- a/{original}` is not followed by `+++ b/{original}`"));
        }
        let file = original.as_ref();
        let Some(diff) = file_patch.patch().as_text() else { bad(format!("{file}: binary diff")) };

        let old = match files.iter().position(|(patched, _)| patched == file) {
            Some(index) => files.remove(index).1,
            None => {
                let source = chromium.join(file);
                fs::read_to_string(&source)
                    .unwrap_or_else(|error| bad(format!("cannot read {}: {error}", source.display())))
            }
        };
        // Patches hold LF lines; a Windows checkout may hold CRLF ones.
        let crlf = old.contains("\r\n");
        let base = if crlf { old.replace("\r\n", "\n") } else { old };
        // diffy looks elsewhere in the file for a hunk that does not match where it says.
        if let Some(line) = mismatch(&base, diff) {
            bad(format!("{file} line {line} does not match the patch"));
        }
        let text = diffy::apply(&base, diff).unwrap_or_else(|error| bad(format!("{file}: {error}")));
        files.push((file.to_owned(), if crlf { text.replace('\n', "\r\n") } else { text }));
    }
}

/// The first line of `base` (1-based) where a context or removed line of `diff` differs from the
/// line its hunk header puts it at.
fn mismatch(base: &str, diff: &Patch<'_, str>) -> Option<usize> {
    let lines: Vec<&str> = base.split_inclusive('\n').collect();
    for hunk in diff.hunks() {
        let mut at = hunk.old_range().start().saturating_sub(1);
        for line in hunk.lines() {
            if let Line::Context(text) | Line::Delete(text) = line {
                if lines.get(at) != Some(text) {
                    return Some(at + 1);
                }
                at += 1;
            }
        }
    }
    None
}

/// The `.cc` files a static_library lists in BUILD.gn. Reading Chromium's own list keeps the build
/// in step with the sources whenever the pinned commit moves.
fn gn_sources(gn: &str, target: &str) -> Vec<String> {
    let Some(start) = gn.find(&format!("static_library(\"{target}\")")) else {
        return Vec::new();
    };
    let block = &gn[start..];
    let block = &block[..block.find("\n}").unwrap_or(block.len())];
    // gn_sources reads only the first `sources = [ ... ]` list. A pin that appends files with
    // `sources +=` or inside a conditional would drop them silently and surface as a link error, so
    // stop with a clear message instead.
    if block.contains("sources +=") {
        fail(&format!(
            "the {target} target in components/zucchini/BUILD.gn appends sources with `sources +=`, \
             which build.rs does not read; update gn_sources to follow the new layout."
        ));
    }
    let Some(open) = block.find("sources = [") else {
        return Vec::new();
    };
    let list = &block[open..];
    let list = &list[..list.find(']').unwrap_or(list.len())];
    list.split('"').skip(1).step_by(2).filter(|name| name.ends_with(".cc")).map(str::to_owned).collect()
}

/// Copies what a prebuilt archive holds: the library, its C header, the licenses and build facts.
fn export(
    dir: &Path,
    manifest: &Path,
    chromium: &Path,
    out: &Path,
    target: &str,
    build: &cc::Build,
    patches: &[(&str, Option<&str>)],
) {
    let lib = lib_file(target);
    // Only an earlier export is replaced, so a mistyped directory is not wiped.
    let has_files = fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some());
    if has_files && !dir.join("BUILD_INFO").exists() {
        fail(&format!("ZUCCHINI_SYS_EXPORT_DIR {} is not empty and holds no earlier export", dir.display()));
    }
    let _ = fs::remove_dir_all(dir);
    for sub in ["lib", "include"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    copy(&out.join(&lib), &dir.join("lib").join(&lib));
    copy(&manifest.join("shim/zucchini_c.h"), &dir.join("include/zucchini_c.h"));
    copy(&chromium.join("LICENSE"), &dir.join("LICENSE.chromium"));
    let ours = manifest.join("../LICENSE");
    if ours.exists() {
        copy(&ours, &dir.join("LICENSE"));
    }

    let compiler = build.get_compiler();
    let compiler = first_line(Command::new(compiler.path()).arg("--version"));
    let commit = first_line(Command::new("git").arg("-C").arg(chromium).args(["rev-parse", "HEAD"]));
    let crt = if !target.contains("msvc") {
        "not applicable"
    } else if crt_static() {
        "static (/MT)"
    } else {
        "dynamic (/MD)"
    };
    let patches: Vec<&str> = patches.iter().map(|(name, _)| *name).collect();
    let info = format!(
        "target {target}\nchromium {commit}\ncompiler {compiler}\nmsvc_crt {crt}\npatches {}\nzucchini-sys {}\n",
        patches.join(" "),
        env::var("CARGO_PKG_VERSION").unwrap()
    );
    fs::write(dir.join("BUILD_INFO"), info).unwrap();
}

/// The static library's file name for `target`.
fn lib_file(target: &str) -> String {
    if target.contains("msvc") { format!("{LIB}.lib") } else { format!("lib{LIB}.a") }
}

/// Links the static library in `dir` and the C++ runtime a clang-built archive needs.
fn link(dir: &Path, target: &str) {
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static={LIB}");
    if target.contains("msvc") {
        // The MSVC linker pulls in the C++ runtime through the archive's own default-library records.
    } else if target.contains("apple") || target.contains("bsd") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
    }
}

/// The variables cc reads to choose a C++ compiler for `target`.
fn cxx_vars(target: &str) -> [String; 4] {
    [
        format!("CXX_{target}"),
        format!("CXX_{}", target.replace('-', "_")),
        "TARGET_CXX".to_owned(),
        "CXX".to_owned(),
    ]
}

fn crt_static() -> bool {
    env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default().split(',').any(|f| f == "crt-static")
}

fn env_flag(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty() && value != "0")
}

fn first_line(command: &mut Command) -> String {
    match command.output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("").trim().to_owned()
        }
        _ => "unknown".to_owned(),
    }
}

fn copy(from: &Path, to: &Path) {
    if let Err(error) = fs::copy(from, to) {
        fail(&format!("cannot copy {} to {}: {error}", from.display(), to.display()));
    }
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}

#[cfg(feature = "prebuilt")]
mod prebuilt {
    use std::env;
    use std::fmt::Display;
    use std::fs;
    use std::path::{Path, PathBuf};

    use sha2::{Digest, Sha256};

    use super::fail;

    /// Largest archive this downloads.
    const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;

    /// Fetches the archive pinned for `target`, checks it and unpacks it; returns where it unpacked.
    pub fn fetch(manifest: &Path, out: &Path, target: &str) -> PathBuf {
        let pins_path = manifest.join("prebuilt.txt");
        println!("cargo:rerun-if-changed={}", pins_path.display());
        let pins = fs::read_to_string(&pins_path)
            .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", pins_path.display())));

        let name = format!("zucchini-{target}.tar.gz");
        let mut release = None;
        let mut expected = None;
        for line in pins.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
            if let Some(url) = line.strip_prefix("release ") {
                release = Some(url.trim().trim_end_matches('/').to_owned());
            } else if let Some((sum, file)) = line.split_once(char::is_whitespace) {
                if file.trim().trim_start_matches('*') == name {
                    expected = Some(sum.to_ascii_lowercase());
                }
            }
        }
        let Some(expected) = expected else {
            fail(&format!(
                "prebuilt.txt pins no library for {target}\n  \
                 Build from source instead: drop the `prebuilt` feature or set ZUCCHINI_SYS_FROM_SOURCE=1."
            ))
        };
        if target.contains("msvc") && super::crt_static() {
            fail(
                "prebuilt Windows libraries use the dynamic CRT, but this build links it statically\n  \
                 Build from source instead: set ZUCCHINI_SYS_FROM_SOURCE=1.",
            );
        }

        let bytes = match env::var_os("ZUCCHINI_SYS_PREBUILT_ARCHIVE") {
            Some(path) => fs::read(&path).unwrap_or_else(|error| {
                fail(&format!("cannot read {}: {error}", Path::new(&path).display()))
            }),
            None => {
                let Some(release) = release else { fail("prebuilt.txt has no `release` line") };
                download(&format!("{release}/{name}"))
            }
        };
        let actual: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        if actual != expected {
            fail(&format!(
                "{name} does not match its pin in prebuilt.txt\n  expected sha256 {expected}\n  got      sha256 {actual}"
            ));
        }

        let dir = out.join("prebuilt");
        let _ = fs::remove_dir_all(&dir);
        tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
            .unpack(&dir)
            .unwrap_or_else(|error| fail(&format!("cannot unpack {name}: {error}")));
        dir
    }

    fn download(url: &str) -> Vec<u8> {
        let mut response = ureq::get(url).call().unwrap_or_else(|error| download_failed(url, error));
        response
            .body_mut()
            .with_config()
            .limit(MAX_ARCHIVE)
            .read_to_vec()
            .unwrap_or_else(|error| download_failed(url, error))
    }

    fn download_failed(url: &str, error: impl Display) -> ! {
        fail(&format!("cannot download {url}: {error}"))
    }
}
