// zucchini-rs replacement for Chromium's //base/functional/bind.h.
//
// BindRepeating binds leading arguments to a function, member function or lambda. As in Chromium,
// bound values are copied and reach the target as const lvalues, and Unretained binds an object
// pointer without ownership.

#ifndef BASE_FUNCTIONAL_BIND_H_
#define BASE_FUNCTIONAL_BIND_H_

#include <functional>
#include <tuple>
#include <type_traits>
#include <utility>

#include "base/functional/callback.h"

namespace base {

template <typename T>
class UnretainedWrapper {
 public:
  explicit UnretainedWrapper(T* ptr) : ptr_(ptr) {}
  T* get() const { return ptr_; }

 private:
  T* ptr_;
};

template <typename T>
UnretainedWrapper<T> Unretained(T* ptr) {
  return UnretainedWrapper<T>(ptr);
}

namespace internal {

template <typename T>
const T& Unwrap(const T& value) {
  return value;
}

template <typename T>
T* Unwrap(const UnretainedWrapper<T>& wrapper) {
  return wrapper.get();
}

template <typename Functor, typename... Bound>
class BindState {
 public:
  explicit BindState(Functor functor, Bound... bound)
      : functor_(std::move(functor)), bound_(std::move(bound)...) {}

  template <typename... Args>
  decltype(auto) operator()(Args&&... args) const {
    return std::apply(
        [&](const Bound&... bound) -> decltype(auto) {
          return std::invoke(functor_, Unwrap(bound)..., std::forward<Args>(args)...);
        },
        bound_);
  }

  template <typename... Args>
  decltype(auto) Run(Args&&... args) const {
    return (*this)(std::forward<Args>(args)...);
  }

 private:
  Functor functor_;
  std::tuple<Bound...> bound_;
};

}  // namespace internal

template <typename Functor, typename... Bound>
auto BindRepeating(Functor&& functor, Bound&&... bound) {
  return internal::BindState<std::decay_t<Functor>, std::decay_t<Bound>...>(
      std::forward<Functor>(functor), std::forward<Bound>(bound)...);
}

}  // namespace base

#endif  // BASE_FUNCTIONAL_BIND_H_
