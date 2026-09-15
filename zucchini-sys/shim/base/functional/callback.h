// zucchini-rs replacement for Chromium's //base/functional/callback.h: RepeatingCallback over
// std::function. Zucchini only stores callbacks and runs them on the thread that made them.

#ifndef BASE_FUNCTIONAL_CALLBACK_H_
#define BASE_FUNCTIONAL_CALLBACK_H_

#include <cstddef>
#include <functional>
#include <type_traits>
#include <utility>

namespace base {

template <typename Signature>
class RepeatingCallback;

template <typename R, typename... Args>
class RepeatingCallback<R(Args...)> {
 public:
  RepeatingCallback() = default;
  RepeatingCallback(std::nullptr_t) {}

  template <typename F>
    requires(!std::is_same_v<std::remove_cvref_t<F>, RepeatingCallback>)
  RepeatingCallback(F&& function) : function_(std::forward<F>(function)) {}

  bool is_null() const { return !function_; }
  explicit operator bool() const { return static_cast<bool>(function_); }

  R Run(Args... args) const { return function_(std::forward<Args>(args)...); }

 private:
  std::function<R(Args...)> function_;
};

}  // namespace base

#endif  // BASE_FUNCTIONAL_CALLBACK_H_
