// zucchini-rs replacement for the part of Abseil's absl/strings/ascii.h that Zucchini uses.
// Abseil is Apache-2.0, so the crate carries these one-liners instead of Abseil.

#ifndef ABSL_STRINGS_ASCII_H_
#define ABSL_STRINGS_ASCII_H_

namespace absl {

constexpr bool ascii_isdigit(unsigned char c) {
  return c >= '0' && c <= '9';
}

constexpr bool ascii_isupper(unsigned char c) {
  return c >= 'A' && c <= 'Z';
}

constexpr bool ascii_islower(unsigned char c) {
  return c >= 'a' && c <= 'z';
}

}  // namespace absl

#endif  // ABSL_STRINGS_ASCII_H_
