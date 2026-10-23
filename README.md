# zucchini-rs

[![CI](https://github.com/mriise/zucchini-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mriise/zucchini-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/zucchini.svg)](https://crates.io/crates/zucchini)
[![docs.rs](https://img.shields.io/docsrs/zucchini)](https://docs.rs/zucchini)

Rust bindings to [Zucchini](https://chromium.googlesource.com/chromium/src/+/main/components/zucchini/),
Chromium's binary diff for executables. It resolves the references inside PE and ELF code, so a
rebuilt binary patches far smaller than a byte-level diff allows.

```rust
let patch = zucchini::generate(&old, &new)?;
assert_eq!(zucchini::apply(&old, &patch)?, new);
```

| Crate | What it is |
|---|---|
| `zucchini` | The safe API, documented on [docs.rs](https://docs.rs/zucchini) |
| `zucchini-sys` | Builds Zucchini into a static library and exposes the C API in `shim/zucchini_c.h` |

## Building

```sh
scripts/chromium.sh      # fetches the pinned Chromium files, after cloning and when the pin moves
cargo test --workspace
```

Building from source needs clang.

- [docs/building.md](docs/building.md): the `prebuilt` feature, compilers, environment variables
- [docs/chromium.md](docs/chromium.md): which Chromium files are built, the shims, moving the pin
- [docs/prebuilt-releases.md](docs/prebuilt-releases.md): how archives are built, attested and pinned
- [docs/crc32-benchmarks.md](docs/crc32-benchmarks.md): native CRC benefits and reproducible benchmarks
- [docs/status.md](docs/status.md): what has been verified so far

Licensed BSD-3-Clause, like Chromium. Chromium's files keep their own license,
`zucchini-sys/chromium/LICENSE`, which every prebuilt archive also carries as `LICENSE.chromium`.
Binaries that link `zucchini-sys` must reproduce both notices. The optional native `crc-fast`
feature also uses Chromium's zlib sources under their zlib license, included as `LICENSE.zlib`
in exports built with that feature.
