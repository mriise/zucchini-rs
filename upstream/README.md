# Chromium CRC-32 patches

These patches let Chromium's Zucchini reuse its existing base/zlib CRC implementation and enable
zlib's existing AVX-512/VPCLMUL backend. They target Chromium main at
`b1bf9a57b7c1756033fe69cf49bde125e6603a8c`, whose Zucchini CRC API takes `base::span`.

- [`crc32-base.patch`](crc32-base.patch) replaces the byte loop with
  `~base::Crc32(0xFFFFFFFFU, data)`. The seed and complement preserve CRC-32/ISO-HDLC, including
  empty input. Zucchini already depends on `//base`, which already builds this wrapper and depends
  on zlib, so upstream needs no new dependency edge or callback.
- [`zlib-avx512.patch`](zlib-avx512.patch) enables the existing kernel on Clang x86 builds. It
  checks XSAVE/OSXSAVE/AVX, SSE/PCLMUL, AVX-512F, VPCLMULQDQ and all required XCR0 state bits;
  guards XGETBV and CPUID leaf 7; and explicitly zero-extends the initial vector CRC. Existing
  SSE, ARM and software paths remain available.

The local `zucchini-sys/patches/crc32-native.patch` adapts the base change to our older pinned
raw-pointer API. Its companion zlib patch carries the CPU/OS checks and vector fix. Our standalone
`crc-fast` build compiles the existing base wrapper and four zlib C sources into the Zucchini
archive, using a static `BASE_EXPORT` shim. It needs neither a new Rust crate nor a full base/zlib
build. See [benefits and benchmarks](../docs/crc32-benchmarks.md) for the comparison with the former
Rust callback and a runnable benchmark.

Validation of the upstream patch artifacts on Windows x64 with clang-cl 21.1.7:

- Both patches apply cleanly to the recorded revision.
- Four Zucchini CRC tests and the two added zlib tests pass with detected AVX-512, forced SSE,
  and forced software dispatch. Tests cover CPU/OS feature predicates, 64 alignments, vector
  boundaries, four seeds and incremental updates.
- A standalone runner passes 33,056 comparisons against an independent bitwise reference and the
  original CRC, and all 4,097 input lengths ending immediately before an inaccessible guard page,
  in each dispatch mode. The accelerated results also agree with Rust crc-fast 1.9.0.

These were standalone checks of the actual Chromium sources with GoogleTest 1.16.0. The full
Chromium GN build, zlib suite and presubmit have not been run. The standalone crate's CI separately
validates Linux x64/ARM64, macOS ARM64 and Windows x64; that does not replace Chromium presubmit.

In an authenticated Chromium checkout with depot_tools, apply and validate:

```sh
git apply /path/to/zucchini-rs/upstream/crc32-base.patch
git apply /path/to/zucchini-rs/upstream/zlib-avx512.patch
git cl format
autoninja -C out/Default zucchini_unittests zlib_unittests
out/Default/zucchini_unittests --gtest_filter='Crc32Test.*'
out/Default/zlib_unittests
git diff --check
git add components/zucchini/crc32.cc components/zucchini/crc32_unittest.cc
git add third_party/zlib/BUILD.gn third_party/zlib/cpu_features.c third_party/zlib/cpu_features.h
git add third_party/zlib/crc32_simd.c third_party/zlib/contrib/tests/utils_unittest.cc
git commit -s
git cl presubmit
git cl upload
```

Use `.exe` for the test binaries on Windows and configure `out/Default` first if needed. The
[Chromium contribution guide](https://www.chromium.org/developers/contributing-code/) covers account
setup and review. Suggested CL title: `zucchini: Reuse base CRC with zlib AVX-512 dispatch`.
The independent zlib patch can also be reviewed separately. No Gerrit CL has been uploaded.
