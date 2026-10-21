# Agent guide for zucchini-rs

## Releasing a version

Version bumps happen on `main` after the pull request merges, never inside the pull request.

1. On an up-to-date `main` with CI green, bump both crates together (they share the workspace
   version, and `zucchini` pins `zucchini-sys` exactly):
   `cargo release version patch --workspace --execute --no-confirm` (or `minor`). It warns that
   `zucchini-sys/chromium` is modified: libgit2 misreads the sparse checkout, and `git status`
   shows it clean.
2. Commit with a sign-off: `git commit -s -m "chore: release vX.Y.Z"`.
3. Publish `zucchini-sys` first, then `zucchini`, with `cargo publish -p <crate> --locked`.
   `cargo release publish` refuses over the same submodule warning. A 403 means `cargo login`.
4. Tag `zucchini-sys-vX.Y.Z` and `zucchini-vX.Y.Z` (annotated), then
   `git push origin main zucchini-sys-vX.Y.Z zucchini-vX.Y.Z`.
5. For new prebuilt libraries, push a `lib-vX.Y.Z` tag on the release commit and follow
   [docs/prebuilt-releases.md](docs/prebuilt-releases.md). The crate just published keeps the
   `prebuilt.txt` pin it shipped with, so the new pin goes out with the next release.
