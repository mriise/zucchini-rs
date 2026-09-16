# Status, 2026-09-15

Verified:

- Windows, clang-cl 21.1.7: every test passes from source, through the `prebuilt` feature with a
  pinned local archive (a tampered archive is refused), and through `ZUCCHINI_SYS_LIB_DIR`.
- Linux, LLVM 21.1.7 in Ubuntu 24.04 (libstdc++ 14) and 22.04 (libstdc++ 11) containers: every test
  passes, and Chromium's test pairs give the same patch sizes as on Windows.
- `scripts/chromium.sh` from a fresh copy: 22 s.
- GitHub Actions: CI passes on all four targets, and the prebuilt workflow builds and attests every
  archive.

Not run yet: the prebuilt download path and `scripts/pin-prebuilt.sh`, which need a `lib-v*`
release. Artifact attestations need a public repository or GitHub Enterprise Cloud.
