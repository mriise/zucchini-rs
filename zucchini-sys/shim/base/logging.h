// zucchini-rs replacement for Chromium's //base/logging.h.
//
// LOG and LOG_IF write to stderr at or above the level zucc_set_log_level sets, and are silent by
// default, so a library user does not get Zucchini's progress chatter.

#ifndef BASE_LOGGING_H_
#define BASE_LOGGING_H_

#include <atomic>
#include <cstdio>
#include <sstream>
#include <string_view>

#include "base/check.h"
#include "base/check_op.h"

namespace logging {

using LogSeverity = int;
inline constexpr LogSeverity LOGGING_INFO = 0;
inline constexpr LogSeverity LOGGING_WARNING = 1;
inline constexpr LogSeverity LOGGING_ERROR = 2;
inline constexpr LogSeverity LOGGING_FATAL = 3;

namespace internal {

// Lowest severity written; anything above LOGGING_FATAL silences logging.
inline std::atomic<LogSeverity> g_min_severity{LOGGING_FATAL + 1};

class LogMessage {
 public:
  LogMessage(const char* file, int line, LogSeverity severity) {
    static constexpr const char* kNames[] = {"INFO", "WARNING", "ERROR", "FATAL"};
    std::string_view name(file);
    name = name.substr(name.find_last_of("/\\") + 1);
    stream_ << "zucchini " << kNames[severity] << " " << name << ":" << line << ": ";
  }
  LogMessage(const LogMessage&) = delete;
  LogMessage& operator=(const LogMessage&) = delete;
  ~LogMessage() {
    stream_ << '\n';
    std::fputs(stream_.str().c_str(), stderr);
  }

  std::ostream& stream() { return stream_; }

 private:
  std::ostringstream stream_;
};

}  // namespace internal

inline bool ShouldCreateLogMessage(LogSeverity severity) {
  return severity >= internal::g_min_severity.load(std::memory_order_relaxed);
}

}  // namespace logging

#define LOG_IS_ON(severity) (::logging::ShouldCreateLogMessage(::logging::LOGGING_##severity))

#define LAZY_STREAM(stream, condition) \
  !(condition) ? (void)0 : ::logging::internal::VoidifyStream() & (stream)

#define LOG(severity)                                                                          \
  LAZY_STREAM(                                                                                 \
      ::logging::internal::LogMessage(__FILE__, __LINE__, ::logging::LOGGING_##severity).stream(), \
      LOG_IS_ON(severity))

#define LOG_IF(severity, condition)                                                            \
  LAZY_STREAM(                                                                                 \
      ::logging::internal::LogMessage(__FILE__, __LINE__, ::logging::LOGGING_##severity).stream(), \
      LOG_IS_ON(severity) && (condition))

#endif  // BASE_LOGGING_H_
