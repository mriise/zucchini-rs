// zucchini-rs replacement for Chromium's //base/memory/raw_ref.h: a non-null reference that can
// be stored and reseated, without Chromium's PartitionAlloc protection.

#ifndef BASE_MEMORY_RAW_REF_H_
#define BASE_MEMORY_RAW_REF_H_

#include <memory>
#include <type_traits>

#include "base/memory/raw_ptr.h"

namespace base {

template <class T, RawPtrTraits Traits = RawPtrTraits::kEmpty>
class raw_ref {
 public:
  constexpr explicit raw_ref(T& target) noexcept : ptr_(std::addressof(target)) {}
  raw_ref(const T&&) = delete;

  template <class U, RawPtrTraits OtherTraits>
    requires std::is_convertible_v<U*, T*>
  constexpr raw_ref(const raw_ref<U, OtherTraits>& other) noexcept
      : ptr_(std::addressof(*other)) {}

  constexpr T& operator*() const noexcept { return *ptr_; }
  constexpr T* operator->() const noexcept { return ptr_; }
  constexpr T& get() const noexcept { return *ptr_; }

 private:
  T* ptr_;
};

template <class T>
raw_ref(T&) -> raw_ref<T>;

}  // namespace base

using base::raw_ref;

#endif  // BASE_MEMORY_RAW_REF_H_
