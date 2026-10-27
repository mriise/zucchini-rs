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

Optional features patch Zucchini's sources to report progress or run faster; patches and results
are byte-identical with or without them. They are off by default on purpose: the default build, and
every prebuilt library, is Chromium's Zucchini with only the fix it needs to build here, so each
change to Chromium's code is something a user opts into. They need a source build: with `prebuilt`
the build stops and says so. A library linked through `ZUCCHINI_SYS_LIB_DIR` is unchecked, so build
it with the same features.

| Feature (`zucchini`) | Effect (times are for a 177 MB PE's apply) |
|---|---|
| `progress` | `generate_with_progress`, `apply_with_progress` and `apply_into_with_progress` report the fraction done, once per `step` crossed, and can cancel, through the C API's `zucc_*_with_progress` |
| `vector-containers` | `std::vector` instead of `std::deque`: about 1.1 s less with MSVC's standard library; needs contiguous memory for the offset arrays |
| `radix-sort` | radix sorts the offset arrays (turns on `vector-containers`): about 0.25 s less, for a scratch copy of each array while it sorts |
| `crc32-hook` | `zucchini::set_crc32` chooses the whole-file CRC-32, through the C API's `zucc_set_crc32` |
| `crc-fast` | `crc32-hook`, with the crc-fast crate's SIMD CRC-32 as the default: about 0.3 s less per file, since Chromium's loop reads a byte at a time |

```sh
cargo test --workspace --features zucchini/progress,zucchini/radix-sort,zucchini/crc-fast   # every patch
```
