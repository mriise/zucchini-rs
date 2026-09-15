// zucchini-rs replacement for Chromium's //base/strings/stringprintf.h.

#ifndef BASE_STRINGS_STRINGPRINTF_H_
#define BASE_STRINGS_STRINGPRINTF_H_

#include <cstdarg>
#include <cstdio>
#include <string>

namespace base {

[[gnu::format(printf, 1, 2)]]
inline std::string StringPrintf(const char* format, ...) {
  va_list args;
  va_start(args, format);
  va_list measure;
  va_copy(measure, args);
  int size = std::vsnprintf(nullptr, 0, format, measure);
  va_end(measure);
  std::string result;
  if (size > 0) {
    result.resize(static_cast<size_t>(size));
    std::vsnprintf(result.data(), result.size() + 1, format, args);
  }
  va_end(args);
  return result;
}

}  // namespace base

#endif  // BASE_STRINGS_STRINGPRINTF_H_
