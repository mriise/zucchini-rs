// zucchini-rs replacement for Chromium's //base/memory/raw_span.h: a plain base::span, since
// raw_ptr is a plain pointer here.

#ifndef BASE_MEMORY_RAW_SPAN_H_
#define BASE_MEMORY_RAW_SPAN_H_

#include <cstddef>

#include "base/containers/span.h"
#include "base/memory/raw_ptr.h"

namespace base {

template <typename T, RawPtrTraits Traits = RawPtrTraits::kEmpty, size_t Extent = dynamic_extent>
using raw_span = span<T, Extent>;

}  // namespace base

#endif  // BASE_MEMORY_RAW_SPAN_H_
