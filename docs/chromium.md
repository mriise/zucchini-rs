# Where the C++ comes from

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
plus the three build flag headers GN would generate. The shims are this repository's own code.

`build.rs` compiles the `zucchini_lib` sources listed in Chromium's own `BUILD.gn`, so a new pin
needs no source list edits. It also changes copies of Chromium files, which the compiler finds
before the originals:

- the exact text replacements in its `PATCHES` table: one line of `base/containers/span.h` that
  needs libstdc++ 15, and a rename of `CalculateCrc32` in `crc32.cc`, so the C API's own
  `CalculateCrc32` can call the function `zucc_set_crc32` chose and fall back to Chromium's;
- `std::deque` becomes `std::vector` in every Zucchini file. MSVC's deque stores 16 bytes per
  block (libc++, which Chromium builds with, stores 4 KiB), and Zucchini sorts and searches millions
  of offsets in them: applying a patch to a 177 MB PE took 4.75 s with deques and 3.65 s with
  vectors. Patches and results are byte-identical either way.

## Moving the pin

```sh
scripts/chromium.sh <commit or tag>
cargo test --workspace
```

The script fetches and pins the new commit; commit the submodule change with it. If Chromium added a
`//base` dependency, the compiler names the missing header: add it to `chromium-files.txt` when it
is header-only and needs nothing new, otherwise write a shim. If a `PATCHES` entry no longer
matches, or Zucchini starts adding or removing at the front of a container (which a vector does
slowly), `build.rs` stops and names it.
