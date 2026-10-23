# Native CRC-32: benefits and benchmarks

With `crc-fast`, Zucchini now calls Chromium's existing `base::Crc32`/zlib implementation directly.
It uses the existing AVX-512/VPCLMUL kernel when the CPU and OS support it, SSE on other x86 CPUs,
ARM64 CRC/PMULL on supported platforms, and portable software otherwise. The polynomial, seed,
final complement and patch-header CRC values remain unchanged.

The main benefits over the previous Rust callback are integration and dependencies:

- Both the Rust and C APIs get the accelerated implementation immediately. A C or `zucchini-sys`
  caller no longer depends on a safe Rust API call first installing the default callback.
- The default checksum path needs no Rust FFI callback, callback registration or function-pointer
  dispatch. Its CPU detection stays inside zlib's thread-safe initialization.
- The library workspace drops the Rust `crc-fast` dependency and its unused transitive packages.
  Four existing C sources and the base wrapper from the unchanged Chromium pin go into the same
  archive; no external zlib installation or new crate is needed.
- CPU and OS checks guard the AVX-512 kernel, including VPCLMULQDQ and all required XCR0 state.
  Global AVX flags are unnecessary. The existing ARM and software implementations are reused.

`crc32-hook` remains available for explicit overrides, but `crc-fast` no longer enables it.
Callers using `set_crc32` must request both features; clearing an override restores the compiled
native default. Default/prebuilt configurations keep the original Chromium loop. All new upstream source changes
are patch files selected only by `crc-fast`; zlib sources and ISA defines are also compiled only
with that feature. CI checks that a default export lists only the existing span compatibility patch.

## Results

Measurements below come from the checked-in comparison harness, on an AMD Ryzen 9 7950X,
Windows x64, Rust 1.98.0 and clang-cl 21.1.7. Rust uses release optimization; native code uses
`/O2 /DNDEBUG`. Both Rust crc-fast 1.9.0 and native zlib detect AVX-512/VPCLMUL on this machine.

| Workload | Original byte loop | Former Rust callback | Native SSE | Native AVX-512/VPCLMUL |
|---|---:|---:|---:|---:|
| CRC, 1 KiB | 1,667 ns | 40 ns | 68 ns | 27 ns |
| CRC, 1 MiB | 1.781 ms | 14.55 us | 58.26 us | 14.38 us |
| CRC, 177 MiB | 338.38 ms | 10.74 ms | 15.44 ms | 11.46 ms |
| Apply, 16 MiB raw patch | 66.87 ms | 7.602 ms | 8.667 ms | 7.864 ms |

Native AVX-512 is about 30x faster than the byte loop for the 177 MiB CRC and 8.5x faster for
this apply fixture. The existing Rust callback was already accelerated: native is approximately
7% slower for that CRC and 3% slower for apply in this run, while the sample ranges overlap:

| Workload | Rust callback sample range | Native AVX-512 sample range |
|---|---:|---:|
| CRC, 177 MiB | 9.158-12.442 ms | 9.863-12.114 ms |
| Apply, 16 MiB | 6.603-8.957 ms | 6.770-8.913 ms |

This run demonstrates no large-file or end-to-end speedup over the Rust callback. The reason for
the change is to keep acceleration in the native library and remove the Rust dependency and
callback setup, while retaining comparable throughput. The small-buffer CRC is faster in this
measurement; throughput on other CPUs remains unmeasured. Full samples, repetition counts and
throughput are in [the recorded output](../benchmarks/crc32/results/windows-7950x.txt).

These are synthetic in-memory measurements on one CPU, without file I/O. They do not establish
performance on other processors or architectures. In particular, enabling AVX-512 closes the
native SSE throughput gap on this CPU; removing a callback invoked only once per file is not a
large-file throughput optimization by itself.

## Reproduce

From the repository root, after fetching Chromium and installing clang:

```sh
scripts/chromium.sh
cargo run --manifest-path benchmarks/crc32/Cargo.toml --release --locked --target-dir target/crc32-benchmark
```

Run with other builds and CPU/memory-intensive work idle. The harness prints detected backends,
medians, minimum/maximum samples and throughput. Its separate Cargo workspace and lockfile keep
the Rust comparison dependency out of the library's dependency graph.

All methods receive identical deterministic xorshift data. CRC measurements use three warmups,
nine rounds with rotating method order, and adaptive repetition counts targeting 40 ms per sample.
Every warmup verifies the checksum against Rust's implementation. The original loop is compiled
from the untouched pinned Chromium source under a benchmark-only name.

The apply fixture changes four 64-byte ranges in 16 MiB and produces a 368-byte raw patch. Each
method verifies the full reconstructed output before timing. Nine rotating rounds measure three
applies per sample. Only CRC selection changes; the generation/apply engine and patch stay the same.

The benchmark enables the optional hook to select implementations at runtime. Its Rust C
trampoline matches the former default callback; native checksum timing calls the compiled native
implementation directly. Native apply timing clears the hook and uses its native fallback, so it
still includes the compatibility hook's atomic load and null check. Production `crc-fast` without
`crc32-hook` omits those operations. Benchmark-only controls can disable detected acceleration but
cannot enable an unsupported ISA; they run in a single-threaded process and restore detected flags.
