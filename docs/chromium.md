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
needs no source list edits. It applies the unified diffs in `zucchini-sys/patches/` to copies of
Chromium files, which the compiler finds before the originals. Only the first applies by default;
each other one needs a source build and its feature, and patches and results are byte-identical with
or without them:

| Patch | Feature | Change |
|---|---|---|
| `libstdcxx-span.patch` | always | one line of `base/containers/span.h` that needs libstdc++ 15 |
| `vector-containers.patch` | `vector-containers` | `std::vector` instead of `std::deque` for Zucchini's offset arrays. MSVC's deque stores 16 bytes per block (libc++, which Chromium builds with, stores 4 KiB): applying a patch to a 177 MB PE took 4.75 s with deques and 3.65 s with vectors |
| `radix-sort.patch` | `radix-sort` | `SortAndUniquify` and `TargetPool::FilterAndProject` sort through `zucchini_rs::Sort` (`shim/zucchini_rs/sort.h`), which radix sorts vectors of 32-bit offsets: the same order, about 0.25 s less of that apply. Written against the vector-containers result |
| `crc32-hook.patch` | `crc32-hook` | renames `CalculateCrc32` in `crc32.cc`, so the C API's own can call the function `zucc_set_crc32` chose and fall back to Chromium's |

Hunks must apply at exactly the lines they name, with every context line matching, so a patch that
no longer fits the pin stops the build rather than applying somewhere else.

## Moving the pin

```sh
scripts/chromium.sh <commit or tag>
cargo test --workspace
```

The script fetches and pins the new commit; commit the submodule change with it. If Chromium added a
`//base` dependency, the compiler names the missing header: add it to `chromium-files.txt` when it
is header-only and needs nothing new, otherwise write a shim. If a patch no longer applies,
`build.rs` stops and names the file and line; make the same change against the new pin and write the
diff with paths `a/<path>` and `b/<path>` relative to the Chromium checkout. Then run the tests once
more with `--features zucchini/radix-sort,zucchini/crc-fast`, which applies every patch.
