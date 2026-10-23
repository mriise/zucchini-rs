// Benchmark adapters only; no production API or duplicated SIMD code.
#include <cstddef>
#include <cstdint>
extern "C" {
#include "third_party/zlib/cpu_features.h"
extern int x86_cpu_enable_avx2;
}
namespace zucchini {
uint32_t CalculateCrc32(const uint8_t*, const uint8_t*);
uint32_t CalculateCrc32Builtin(const uint8_t*, const uint8_t*);
uint32_t CalculateCrc32Original(const uint8_t*, const uint8_t*);
}

struct Features { int avx512, avx2, sse, crc, pmull; };
static const Features& Detected() {
  static const Features features = [] {
    cpu_check_features();
    return Features{x86_cpu_enable_avx512, x86_cpu_enable_avx2, x86_cpu_enable_simd,
                    arm_cpu_enable_crc32, arm_cpu_enable_pmull};
  }();
  return features;
}
extern "C" int benchmark_avx512_available() { return Detected().avx512 || Detected().avx2; }

// Single-threaded benchmark controls can disable a detected backend, never enable an
// unsupported ISA. Mode 0 restores detection; mode 1 disables wide folding only.
extern "C" void benchmark_native_mode(int mode) {
  const auto& detected = Detected();
  x86_cpu_enable_avx512 = mode == 0 ? detected.avx512 : 0;
  x86_cpu_enable_avx2 = mode == 0 ? detected.avx2 : 0;
  x86_cpu_enable_simd = detected.sse;
  arm_cpu_enable_crc32 = detected.crc;
  arm_cpu_enable_pmull = mode == 0 ? detected.pmull : 0;
}
extern "C" uint32_t benchmark_native(const uint8_t* data, size_t len) {
  return zucchini::CalculateCrc32Builtin(data, len ? data + len : data);
}
extern "C" uint32_t benchmark_callback(const uint8_t* data, size_t len) {
  return zucchini::CalculateCrc32(data, len ? data + len : data);
}
extern "C" uint32_t benchmark_original(const uint8_t* data, size_t len) {
  return zucchini::CalculateCrc32Original(data, len ? data + len : data);
}
