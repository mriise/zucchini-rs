# Chromium CRC-32 changes

Three CLs against Chromium main `0ff6fbee` (2026-10-23), each a `git format-patch` file whose
message is the CL description. They touch two OWNERS sets, so they are reviewed separately:

| Patch | Owners | Depends on | Change |
|---|---|---|---|
| [`1-zlib-avx512-detection.patch`](1-zlib-avx512-detection.patch) | `third_party/zlib` | - | Checks XSAVE/OSXSAVE, CPUID leaf 7, AVX-512F, VPCLMULQDQ and XCR0 before using the AVX-512 CRC kernel; zero-extends its initial CRC. Fixes CMake `ENABLE_SIMD_AVX512` builds; GN builds are unchanged. Adds the `AUTHORS` entry |
| [`2-zlib-enable-avx512.patch`](2-zlib-enable-avx512.patch) | `third_party/zlib` | 1 | Defines `CRC32_SIMD_AVX512_PCLMUL` in GN. gzip decompression +8.2% geomean on Zen 4 ([data](zlib-bench-7950x.txt)) |
| [`3-zucchini-base-crc32.patch`](3-zucchini-base-crc32.patch) | `components/zucchini` | - | `CalculateCrc32` calls `base::Crc32`. 177 MiB CRC 295 ms -> 11.2 ms with SSE alone |

The local `zucchini-sys/patches/crc32-native.patch` and `zlib-avx512.patch` carry the same
source changes, adapted to our older pin. See [benchmarks](../docs/crc32-benchmarks.md).

## Before uploading

- Sign the [Individual CLA](https://cla.developers.google.com/about/google-individual) with the
  email in `AUTHORS`. Patch 1 adds the entry; if patch 3 goes first, move the `AUTHORS` hunk there.
- File a crbug per CL (or one for both zlib CLs) and replace `Bug: TODO_FILE_CRBUG`.
- Follow the [AI coding policy](https://chromium.googlesource.com/chromium/src/+/main/agents/ai_policy.md):
  understand every line, flag AI-assisted parts you are unsure of, and answer reviews yourself.
- Patch 2 only measures AMD Zen 4. Intel AVX-512 data (Ice Lake or later) would strengthen it.

## Validation so far

- All three apply with `git am` to `0ff6fbee`; `git diff --check` is clean.
- The zlib sources build with clang-cl 21 and with GCC 13 (`X86_WINDOWS` and `X86_NOT_WINDOWS`) with
  AVX-512 enabled, so patch 2 needs no `is_clang` guard.
- Standalone GoogleTest runs of the new Zucchini and zlib tests pass with AVX-512, SSE and software
  dispatch, plus 33,056 comparisons against a bitwise reference and guard-page overread checks.

Not yet run: a GN build, `zucchini_unittests`/`zlib_unittests` in Chromium, presubmit and try bots.

## Upload

In a Chromium checkout with depot_tools, for each CL (patch 2 on top of patch 1 as a relation
chain; patch 3 on its own branch):

```sh
git checkout -b zlib-avx512-detection -t origin/main
git am /path/to/zucchini-rs/upstream/1-zlib-avx512-detection.patch
git cl format
autoninja -C out/Default zlib_unittests zucchini_unittests zlib_bench
out/Default/zlib_unittests
git cl presubmit
git cl upload
git cl try
```

Chromium does not use `Signed-off-by`; the CLA covers it. `git cl upload` adds the `Change-Id`.
