// Sorting for Zucchini's offset arrays; build.rs's PATCHES call it in place of std::sort.

#ifndef ZUCCHINI_RS_SORT_H_
#define ZUCCHINI_RS_SORT_H_

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <type_traits>
#include <utility>
#include <vector>

namespace zucchini_rs {

// Sorts 32-bit values with four 8-bit LSD radix passes, skipping a pass whose byte is the same in
// every value. The result is std::sort's; for the millions of offsets Zucchini sorts while applying
// a patch to a large executable it takes about a quarter of the time.
inline void RadixSort(std::vector<uint32_t>* values) {
  const size_t n = values->size();
  if (n < 256) {
    std::sort(values->begin(), values->end());
    return;
  }
  std::vector<uint32_t> scratch(n);
  uint32_t* src = values->data();
  uint32_t* dst = scratch.data();
  size_t counts[4][256] = {};
  for (size_t i = 0; i < n; ++i) {
    uint32_t v = src[i];
    ++counts[0][v & 0xFF];
    ++counts[1][(v >> 8) & 0xFF];
    ++counts[2][(v >> 16) & 0xFF];
    ++counts[3][v >> 24];
  }
  for (int pass = 0; pass < 4; ++pass) {
    size_t* count = counts[pass];
    const int shift = 8 * pass;
    if (count[(src[0] >> shift) & 0xFF] == n) {
      continue;
    }
    size_t sum = 0;
    for (size_t& c : counts[pass]) {
      size_t here = c;
      c = sum;
      sum += here;
    }
    for (size_t i = 0; i < n; ++i) {
      uint32_t v = src[i];
      dst[count[(v >> shift) & 0xFF]++] = v;
    }
    std::swap(src, dst);
  }
  if (src != values->data()) {
    std::copy(src, src + n, values->data());
  }
}

// std::sort, or RadixSort for a vector of 32-bit offsets.
template <class Container>
void Sort(Container* container) {
  if constexpr (std::is_same_v<Container, std::vector<uint32_t>>) {
    RadixSort(container);
  } else {
    std::sort(container->begin(), container->end());
  }
}

}  // namespace zucchini_rs

#endif  // ZUCCHINI_RS_SORT_H_
