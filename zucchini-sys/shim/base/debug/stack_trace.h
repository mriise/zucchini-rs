// zucchini-rs replacement for Chromium's //base/debug/stack_trace.h. Zucchini only logs a stack
// trace beside patch read errors, so this one records nothing.

#ifndef BASE_DEBUG_STACK_TRACE_H_
#define BASE_DEBUG_STACK_TRACE_H_

#include <string>

namespace base::debug {

class StackTrace {
 public:
  std::string ToString() const { return "(no stack trace)"; }
};

}  // namespace base::debug

#endif  // BASE_DEBUG_STACK_TRACE_H_
