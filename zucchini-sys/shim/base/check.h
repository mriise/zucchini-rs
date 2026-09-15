// zucchini-rs replacement for Chromium's //base/check.h.
//
// CHECK behaves as in Chromium, except that a failure throws zucchini_rs::CheckFailure instead of
// crashing. The C API catches it and returns ZUCC_CHECK_FAILED, so a damaged patch that trips a
// CHECK fails the call rather than aborting the host process. DCHECK compiles to nothing unless
// DCHECK_IS_ON(), as in Chromium.

#ifndef BASE_CHECK_H_
#define BASE_CHECK_H_

#include <ostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>

#include "base/compiler_specific.h"
#include "base/dcheck_is_on.h"

namespace zucchini_rs {

class CheckFailure : public std::runtime_error {
 public:
  using std::runtime_error::runtime_error;
};

}  // namespace zucchini_rs

namespace logging::internal {

// Gathers a failed CHECK's streamed message, and throws once the statement ends.
class CheckError {
 public:
  CheckError(const char* file, int line, const char* condition) {
    std::string_view name(file);
    name = name.substr(name.find_last_of("/\\") + 1);
    stream_ << name << ":" << line << ": CHECK(" << condition << ") failed ";
  }
  CheckError(const CheckError&) = delete;
  CheckError& operator=(const CheckError&) = delete;
  ~CheckError() noexcept(false) {
    std::string message = stream_.str();
    message.erase(message.find_last_not_of(' ') + 1);
    throw ::zucchini_rs::CheckFailure(message);
  }

  std::ostream& stream() { return stream_; }

 private:
  std::ostringstream stream_;
};

// Ends a streamed expression as void. Its bool constructor type checks a condition that is never
// evaluated.
class VoidifyStream {
 public:
  VoidifyStream() = default;
  explicit VoidifyStream(bool) {}
  void operator&(std::ostream&) {}
};

inline std::ostream& SwallowStream() {
  static std::ostringstream stream;
  return stream;
}

}  // namespace logging::internal

// The switch keeps an `else` after CHECK(...) bound to the caller's `if`.
#define ZUCCHINI_RS_CHECK_IMPL(condition, stream) \
  switch (0)                                      \
  case 0:                                         \
  default:                                        \
    if ((condition) ? true : false) [[likely]];   \
    else stream

#define CHECK(condition, ...) \
  ZUCCHINI_RS_CHECK_IMPL(     \
      condition, ::logging::internal::CheckError(__FILE__, __LINE__, #condition).stream())

#define ZUCCHINI_RS_EAT_STREAM(condition)                     \
  true ? (void)0                                              \
       : ::logging::internal::VoidifyStream(condition) & ::logging::internal::SwallowStream()

#if DCHECK_IS_ON()
#define DCHECK(condition) CHECK(condition)
#else
#define DCHECK(condition) ZUCCHINI_RS_EAT_STREAM(!(condition))
#endif

#endif  // BASE_CHECK_H_
