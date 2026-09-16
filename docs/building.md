# Building

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
