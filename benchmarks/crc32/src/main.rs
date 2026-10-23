use std::hint::black_box;
use std::time::Instant;

type Checksum = fn(&[u8]) -> u32;

unsafe extern "C" {
    fn benchmark_native(data: *const u8, len: usize) -> u32;
    fn benchmark_callback(data: *const u8, len: usize) -> u32;
    fn benchmark_original(data: *const u8, len: usize) -> u32;
    fn benchmark_avx512_available() -> i32;
    fn benchmark_native_mode(mode: i32);
}

fn direct_rust(data: &[u8]) -> u32 {
    crc_fast::checksum(crc_fast::CrcAlgorithm::Crc32IsoHdlc, data) as u32
}
fn rust_callback(data: &[u8]) -> u32 {
    unsafe { benchmark_callback(data.as_ptr(), data.len()) }
}
fn base_zlib(data: &[u8]) -> u32 {
    unsafe { benchmark_native(data.as_ptr(), data.len()) }
}
fn original(data: &[u8]) -> u32 {
    unsafe { benchmark_original(data.as_ptr(), data.len()) }
}

fn noise(len: usize) -> Vec<u8> {
    let mut state = 1u32;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

fn stats(mut samples: Vec<f64>) -> (f64, f64, f64) {
    samples.sort_by(f64::total_cmp);
    (samples[samples.len() / 2], samples[0], *samples.last().unwrap())
}

#[cfg(target_arch = "x86_64")]
fn cpu() -> String {
    use std::arch::x86_64::__cpuid;
    let mut name = Vec::new();
    for leaf in 0x80000002..=0x80000004 {
        let result = __cpuid(leaf);
        for value in [result.eax, result.ebx, result.ecx, result.edx] {
            name.extend_from_slice(&value.to_le_bytes());
        }
    }
    String::from_utf8_lossy(&name).trim_matches(char::from(0)).trim().to_owned()
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu() -> String {
    std::env::consts::ARCH.to_owned()
}

// Match the former private Rust default callback without an extra Rust function-pointer load.
unsafe extern "C" fn rust_crc(data: *const u8, len: usize) -> u32 {
    let bytes = if len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data, len) } };
    direct_rust(bytes)
}

fn main() {
    println!("CPU: {}", cpu());
    println!("crc-fast: {}", crc_fast::get_calculator_target(crc_fast::CrcAlgorithm::Crc32IsoHdlc));
    let available = unsafe { benchmark_avx512_available() };
    println!("Chromium zlib AVX-512/AVX2 runtime available: {available}");
    // Install the former Rust callback for controlled comparisons. Native checksum calls below
    // bypass this hook and use the actual compiled Zucchini implementation directly.
    unsafe {
        zucchini_sys::zucc_set_crc32(Some(rust_crc));
    }

    let data = noise(177 * 1024 * 1024);
    let methods: [(&str, Checksum); 5] = [
        ("Rust direct", direct_rust),
        ("Rust callback", rust_callback),
        ("native narrow", base_zlib),
        ("native detected", base_zlib),
        ("original", original),
    ];
    for size in [1024, 1024 * 1024, data.len()] {
        let input = &data[..size];
        let expected = direct_rust(input);
        let mut repeats = Vec::new();
        for (index, (_, function)) in methods.iter().enumerate() {
            unsafe { benchmark_native_mode(i32::from(index != 3)) };
            for _ in 0..3 {
                assert_eq!(function(black_box(input)), expected);
            }
            let start = Instant::now();
            black_box(function(black_box(input)));
            let elapsed = start.elapsed().as_secs_f64().max(1e-9);
            repeats.push((0.04 / elapsed).ceil().clamp(1.0, 1_000_000.0) as usize);
        }
        let mut samples = vec![Vec::new(); methods.len()];
        // Rotate method order each round to distribute cache/thermal effects.
        for round in 0..9 {
            for order in 0..methods.len() {
                let index = (round + order) % methods.len();
                unsafe { benchmark_native_mode(i32::from(index != 3)) };
                let function = black_box(methods[index].1);
                let start = Instant::now();
                for _ in 0..repeats[index] {
                    black_box(function(black_box(input)));
                }
                samples[index].push(start.elapsed().as_secs_f64() / repeats[index] as f64);
            }
        }
        for (index, values) in samples.into_iter().enumerate() {
            let (median, min, max) = stats(values);
            println!(
                "CRC bytes={size} method={} median_ms={:.6} min_ms={:.6} max_ms={:.6} GiB_s={:.3} repeats={}",
                methods[index].0,
                median * 1000.0,
                min * 1000.0,
                max * 1000.0,
                size as f64 / median / 1024f64.powi(3),
                repeats[index]
            );
        }
    }
    drop(data);

    let old = noise(16 * 1024 * 1024);
    let mut new = old.clone();
    for offset in [1024, 1024 * 1024, 8 * 1024 * 1024, 15 * 1024 * 1024] {
        new[offset..offset + 64].fill(0xA5);
    }
    println!("Generating 16 MiB raw patch for apply comparison...");
    unsafe {
        zucchini_sys::zucc_set_crc32(None);
        benchmark_native_mode(0);
    }
    let patch = zucchini::generate_raw(&old, &new).unwrap();
    println!("Apply fixture: old={} new={} patch={}", old.len(), new.len(), patch.len());
    let names = ["Rust callback", "native narrow", "native detected", "original"];
    let select = |method| unsafe {
        benchmark_native_mode(i32::from(method == 1));
        zucchini_sys::zucc_set_crc32(match method {
            0 => Some(rust_crc),
            1 | 2 => None,
            3 => Some(benchmark_original),
            _ => unreachable!(),
        });
    };
    for method in 0..4 {
        select(method);
        assert_eq!(zucchini::apply(&old, &patch).unwrap(), new);
    }
    let mut samples = vec![Vec::new(); 4];
    for round in 0..9 {
        for order in 0..4 {
            let method = (round + order) % 4;
            select(method);
            let start = Instant::now();
            for _ in 0..3 {
                black_box(zucchini::apply(black_box(&old), black_box(&patch)).unwrap());
            }
            samples[method].push(start.elapsed().as_secs_f64() / 3.0);
        }
    }
    for (method, values) in samples.into_iter().enumerate() {
        let (median, min, max) = stats(values);
        println!(
            "APPLY method={} median_ms={:.6} min_ms={:.6} max_ms={:.6}",
            names[method],
            median * 1000.0,
            min * 1000.0,
            max * 1000.0
        );
    }
    unsafe {
        zucchini_sys::zucc_set_crc32(None);
        benchmark_native_mode(0);
    }
}
