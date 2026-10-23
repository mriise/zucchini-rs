# Chromium CRC-32 changes

Four CLs against Chromium main `0ff6fbee` (2026-10-23), each a `git format-patch` file whose
message is the CL description:

| Patch | Owners | Depends on | Change |
|---|---|---|---|
| [`1-zlib-avx512-detection.patch`](1-zlib-avx512-detection.patch) | `third_party/zlib` | - | Checks XSAVE/OSXSAVE, AVX-512F, VPCLMULQDQ and XCR0 before the AVX-512 CRC kernel; zero-extends its initial CRC. Fixes CMake `ENABLE_SIMD_AVX512` builds. Adds the `AUTHORS` entry |
| [`2-zlib-avx2-vpclmul.patch`](2-zlib-avx2-vpclmul.patch) | `third_party/zlib` | 1 | AVX2 VPCLMULQDQ kernel for Zen 3 and Intel parts without AVX-512, enabled in GN. CRC 2x SSE4.2 |
| [`3-zlib-enable-avx512.patch`](3-zlib-enable-avx512.patch) | `third_party/zlib` | 2 | Defines `CRC32_SIMD_AVX512_PCLMUL` in GN |
| [`4-zucchini-base-crc32.patch`](4-zucchini-base-crc32.patch) | `components/zucchini` | - | `CalculateCrc32` calls `base::Crc32`: 177 MiB in 11.2 ms instead of 295 ms |

Measurements are in [`zlib-bench-7950x.txt`](zlib-bench-7950x.txt). `zucchini-sys/patches/` carries
the same source changes for our pin.

## Before uploading

- Sign the [Individual CLA](https://cla.developers.google.com/about/google-individual) with the
  email in `AUTHORS`. If patch 4 goes first, move the `AUTHORS` hunk there.
- File crbugs and replace `Bug: TODO_FILE_CRBUG`.
- Follow the [AI coding policy](https://chromium.googlesource.com/chromium/src/+/main/agents/ai_policy.md):
  understand every line, flag AI-assisted parts you are unsure of, and answer reviews yourself.
- Patch 3 has only AMD Zen 4 data; its gain over patch 2 there is within noise.

## Validation so far

- The patches apply with `git am`; `git diff --check` and clang-format are clean.
- zlib builds with clang-cl 21 and GCC 13 for every combination of the AVX2 and AVX-512 defines.
- The new zlib tests pass with GoogleTest 1.16.0 in AVX-512, AVX2 and SSE builds. A harness of
  831,408 checks compares each kernel and forced `crc32_z` tier with a bitwise reference, including
  inputs that end before a guard page.

Not yet run: a GN build, `zlib_unittests`/`zucchini_unittests` in Chromium, presubmit and try bots.

## Upload

In a Chromium checkout with depot_tools; patches 1-3 form a relation chain:

```sh
git checkout -b zlib-avx512-detection -t origin/main
git am /path/to/zucchini-rs/upstream/1-zlib-avx512-detection.patch
git cl format
autoninja -C out/Default zlib_unittests zucchini_unittests
out/Default/zlib_unittests
git cl presubmit
git cl upload
git cl try
```

Chromium does not use `Signed-off-by`; the CLA covers it. `git cl upload` adds the `Change-Id`.
