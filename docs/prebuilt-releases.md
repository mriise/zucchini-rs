# Prebuilt releases

1. Push a `lib-v*` tag. `.github/workflows/prebuilt.yml` builds every target with the pinned LLVM,
   runs the tests against the exported library, attests each archive's build provenance with
   `actions/attest-build-provenance` (a Sigstore signature bound to the workflow run), and
   publishes a release with the archives and `SHA256SUMS`.
2. Run `scripts/pin-prebuilt.sh <tag>`. It downloads the release, checks `SHA256SUMS`, and runs
   `gh attestation verify` on every archive, requiring this repository's prebuilt workflow, the tag
   as source ref and a GitHub-hosted runner. Only then does it write the sums to
   `zucchini-sys/prebuilt.txt`.
3. Commit `prebuilt.txt`. The `prebuilt` feature now accepts exactly those archives, unless their
   `BUILD_INFO` lacks a patch that `build.rs` always applies.

Anyone can recheck an archive: `gh attestation verify zucchini-<target>.tar.gz --repo mriise/zucchini-rs`.

Each archive holds `lib/` (`zucchini.lib` or `libzucchini.a`), `include/zucchini_c.h`, `LICENSE`,
`LICENSE.chromium`, and `BUILD_INFO` (target, Chromium commit, compiler, MSVC CRT).

| Target | Built on | Notes |
|---|---|---|
| `x86_64-pc-windows-msvc` | windows-2025 | Dynamic CRT (/MD); a `crt-static` build must build from source |
| `x86_64-unknown-linux-gnu` | ubuntu-22.04 | Linked there, code needs GLIBCXX_3.4.29 (GCC 11) and glibc 2.34 |
| `aarch64-unknown-linux-gnu` | ubuntu-22.04-arm | As above |
| `aarch64-apple-darwin` | macos-15 | Xcode clang |
| `x86_64-apple-darwin` | macos-15 | Cross compiled, tests not run |
