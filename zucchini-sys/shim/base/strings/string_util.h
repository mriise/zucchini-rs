// zucchini-rs replacement for the part of Chromium's //base/strings/string_util.h that Zucchini
// uses. The real header pulls in most of //base/strings.

#ifndef BASE_STRINGS_STRING_UTIL_H_
#define BASE_STRINGS_STRING_UTIL_H_

namespace base {

template <typename Char>
constexpr bool IsAsciiDigit(Char c) {
  return c >= '0' && c <= '9';
}

}  // namespace base

#endif  // BASE_STRINGS_STRING_UTIL_H_
