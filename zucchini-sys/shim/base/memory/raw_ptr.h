// zucchini-rs replacement for Chromium's //base/memory/raw_ptr.h.
//
// Chromium's raw_ptr adds use-after-free protection backed by PartitionAlloc. Without
// PartitionAlloc it is a plain pointer, which is what this is.

#ifndef BASE_MEMORY_RAW_PTR_H_
#define BASE_MEMORY_RAW_PTR_H_

namespace base {

enum class RawPtrTraits : unsigned {
  kEmpty = 0,
  kMayDangle = 1 << 0,
  kAllowPtrArithmetic = 1 << 1,
  kExperimental = 1 << 2,
};

template <typename T, RawPtrTraits Traits = RawPtrTraits::kEmpty>
using raw_ptr = T*;

inline constexpr RawPtrTraits DanglingUntriaged = RawPtrTraits::kMayDangle;
inline constexpr RawPtrTraits AllowPtrArithmetic = RawPtrTraits::kAllowPtrArithmetic;
inline constexpr RawPtrTraits CtnExperimental = RawPtrTraits::kExperimental;

}  // namespace base

using base::AllowPtrArithmetic;
using base::CtnExperimental;
using base::DanglingUntriaged;
using base::raw_ptr;

#endif  // BASE_MEMORY_RAW_PTR_H_
