# zucchini-rs

Rust bindings to [Zucchini](https://chromium.googlesource.com/chromium/src/+/main/components/zucchini/),
Chromium's binary diff for executables.

| Crate | What it is |
|---|---|
| `zucchini` | Safe API: `generate`, `generate_raw`, `generate_with`, `apply`, `apply_into`, `read_header`, `set_log_level` |
| `zucchini-sys` | Builds Zucchini into a static library and exposes the C API in `shim/zucchini_c.h` |

```rust
let patch = zucchini::generate(&old, &new)?; // diffs PE and ELF code with references resolved
assert_eq!(zucchini::apply(&old, &patch)?, new);
```

Licensed BSD-3-Clause, like Chromium. Chromium's files keep their own license,
`zucchini-sys/chromium/LICENSE`, which every prebuilt archive also carries as `LICENSE.chromium`.

## Where the C++ comes from

`zucchini-sys/chromium` is a git submodule of Chromium's official GitHub mirror
(`github.com/chromium/chromium`, the same commits as chromium.googlesource.com), pinned to one
commit: `32586b65`, Chromium main on 2026-09-14. It is filtered twice over:

- `.gitmodules` marks it `update = none`, so neither `git submodule update` nor cargo clones Chromium.
- `scripts/chromium.sh` fetches only the pinned commit, at depth 1 and without file contents, then
  sparse checks out the paths in `zucchini-sys/chromium-files.txt`: `components/zucchini`,
  Chromium's `LICENSE`, and 24 header-only files from `//base` and `//build`. That is 212 files
  (5.4 MB) plus about 90 MB of git trees, in about 20 seconds.

Everything else Zucchini includes from `//base` (CHECK and logging, bind and callbacks, raw_ptr,
StringPrintf, a few string helpers) would bring 190 more headers and compiled `//base` sources.
`zucchini-sys/shim/` replaces those with small implementations of just the parts Zucchini uses,
plus the three build flag headers GN would generate. The shims are this repository's own code;
nothing comes from Firefox's MPL-2.0 shims.

`build.rs` compiles the `zucchini_lib` sources listed in Chromium's own `BUILD.gn`, so a new pin
needs no source list edits. It also applies the exact text replacements in its `PATCHES` table to
copies of Chromium files; today that is one line of `base/containers/span.h` that needs libstdc++ 15.

## Building

| | From source (default) | Prebuilt (`prebuilt` feature) |
|---|---|---|
| Needs | `scripts/chromium.sh`, clang | network access to the pinned release |
| Links | code compiled on this machine | an archive CI built, tested and attested, accepted only if its SHA-256 is in `zucchini-sys/prebuilt.txt` |

```sh
scripts/chromium.sh                                   # after cloning, and whenever the pin moves
cargo test --workspace                                # from source
cargo test --workspace --features zucchini/prebuilt   # prebuilt
```

Chromium's headers only support clang: clang-cl on Windows (`base/compiler_specific.h` rejects
MSVC's cl) and clang++ elsewhere, with Chromium's own standard flags, `/std:c++23preview` and
`-std=c++23`. CI uses LLVM 21.1.7 from LLVM's release archives, pinned by SHA-256 in
`scripts/install-llvm.sh`, and Xcode's clang on macOS, where LLVM published no archive for that
release. Set `CXX` to use another compiler.

| Variable | Effect |
|---|---|
| `ZUCCHINI_SYS_FROM_SOURCE=1` | Build from source even with the `prebuilt` feature |
| `ZUCCHINI_SYS_PREBUILT_ARCHIVE=<file>` | Read the pinned archive from disk instead of downloading it; still checked |
| `ZUCCHINI_SYS_LIB_DIR=<dir>` | Link the library already in `<dir>`, unchecked |
| `ZUCCHINI_SYS_CHROMIUM_DIR=<dir>` | Build from another Chromium checkout |
| `ZUCCHINI_SYS_DCHECK=1` | Keep Chromium's DCHECKs |
| `ZUCCHINI_SYS_EXPORT_DIR=<dir>` | Also write what a prebuilt archive holds to `<dir>` |

Zucchini is compiled with `-O2` even in debug builds, because unoptimised generation is very slow.

## Behaviour worth knowing

- A failed Chromium CHECK throws to the C API boundary and returns `Error::CheckFailed` where
  Chromium would crash. Of 4,000 single-bit flips in a PE patch, 12 tripped a CHECK; none crashed
  or produced wrong bytes. A CHECK inside a `noexcept` function would still terminate.
- `apply` checks the CRC-32 of the old file and of the result.
- Zucchini cannot patch from or to an empty file, so those calls return `Error::EmptyFile`: it
  writes such a patch, but its own reader rejects it.
- Files are limited to 4 GiB; the patch header stores 32-bit sizes.
- PE and ELF executables are recognised. DEX and ZTF are not enabled, as in Chrome's official
  Windows builds: their disassemblers are compiled but never registered, so patches containing
  them do not apply.
- Patches are not compressed. In the jxsync benchmarks, xz compressed them best.
- Zucchini's log output is off unless `set_log_level` turns it on.

## Prebuilt releases

1. Push a `lib-v*` tag. `.github/workflows/prebuilt.yml` builds every target with the pinned LLVM,
   runs the tests against the exported library, attests each archive's build provenance with
   `actions/attest-build-provenance` (a Sigstore signature bound to the workflow run), and
   publishes a release with the archives and `SHA256SUMS`.
2. Run `scripts/pin-prebuilt.sh <tag>`. It downloads the release, checks `SHA256SUMS`, and runs
   `gh attestation verify` on every archive, requiring this repository's prebuilt workflow, the tag
   as source ref and a GitHub-hosted runner. Only then does it write the sums to
   `zucchini-sys/prebuilt.txt`.
3. Commit `prebuilt.txt`. The `prebuilt` feature now accepts exactly those archives.

Anyone can recheck an archive: `gh attestation verify zucchini-<target>.tar.gz --repo JinxxyVR/zucchini-rs`.

Each archive holds `lib/` (`zucchini.lib` or `libzucchini.a`), `include/zucchini_c.h`, `LICENSE`,
`LICENSE.chromium`, and `BUILD_INFO` (target, Chromium commit, compiler, MSVC CRT).

| Target | Built on | Notes |
|---|---|---|
| `x86_64-pc-windows-msvc` | windows-2025 | Dynamic CRT (/MD); a `crt-static` build must build from source |
| `x86_64-unknown-linux-gnu` | ubuntu-22.04 | Linked there, code needs GLIBCXX_3.4.29 (GCC 11) and glibc 2.34 |
| `aarch64-unknown-linux-gnu` | ubuntu-22.04-arm | As above |
| `aarch64-apple-darwin` | macos-15 | Xcode clang |
| `x86_64-apple-darwin` | macos-15 | Cross compiled, tests not run |

## Moving the Chromium pin

```sh
scripts/chromium.sh <commit or tag>
cargo test --workspace
```

The script fetches and pins the new commit; commit the submodule change with it. If Chromium added a
`//base` dependency, the compiler names the missing header: add it to `chromium-files.txt` when it
is header-only and needs nothing new, otherwise write a shim. If a `PATCHES` entry no longer
matches, `build.rs` stops and names it.

## Status, 2026-09-15

Verified:

- Windows, clang-cl 21.1.7: every test passes from source, through the `prebuilt` feature with a
  pinned local archive (a tampered archive is refused), and through `ZUCCHINI_SYS_LIB_DIR`.
- Linux, LLVM 21.1.7 in Ubuntu 24.04 (libstdc++ 14) and 22.04 (libstdc++ 11) containers: every test
  passes, and Chromium's test pairs give the same patch sizes as on Windows.
- GameAssembly.dll between two VBashi builds (152 to 153 MB): a 46.62 MB patch (53.97 MB raw),
  the same as the Firefox-vendored Zucchini measured earlier, generated in 188 s.
- `scripts/chromium.sh` from a fresh copy: 22 s.

Not run yet: both GitHub workflows, `install-llvm.sh` on a Windows runner, macOS, the prebuilt
download path, `scripts/pin-prebuilt.sh`. Artifact attestations need a public repository or GitHub
Enterprise Cloud.
