// C API over Chromium's Zucchini; see zucchini_c.h.

#include "zucchini_c.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <exception>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <optional>
#include <string>

#include "base/logging.h"
#include "components/zucchini/buffer_view.h"
#include "components/zucchini/crc32.h"
#include "components/zucchini/patch_reader.h"
#include "components/zucchini/patch_writer.h"
#include "components/zucchini/zucchini.h"

#ifdef ZUCCHINI_RS_PROGRESS
// Declares the progress functions defined below; patches/progress.patch adds them.
#include "components/zucchini/suffix_array.h"
#endif

static_assert(sizeof(zucc_patch_header) == 20);

struct zucc_patch {
  zucc_patch(zucchini::ConstBufferView old_image, zucchini::ConstBufferView new_image)
      : writer(old_image, new_image) {}

  zucchini::EnsemblePatchWriter writer;
};

namespace {

thread_local std::string g_last_error;

#ifdef ZUCCHINI_RS_PROGRESS
// The progress callback of the C API call running on this thread, and the part of the whole call
// that the current zucchini_rs::ProgressSteps step covers.
struct Progress {
  zucc_progress_fn callback;
  void* context;
  double step;
  double lo = 0;
  double hi = 1;
  // The furthest reported so far, and where the next report is due.
  double done = 0;
  double next = 0;
};

thread_local Progress* g_progress = nullptr;

// Installs a progress callback, or none, for the rest of the scope.
class ProgressCall {
 public:
  ProgressCall(zucc_progress_fn callback, void* context, double step)
      : progress_{callback, context, step}, saved_(g_progress) {
    g_progress = callback ? &progress_ : nullptr;
  }
  ProgressCall(const ProgressCall&) = delete;
  ProgressCall& operator=(const ProgressCall&) = delete;
  ~ProgressCall() { g_progress = saved_; }

  // Reports 0, which the callback may cancel at.
  void Start() { zucchini_rs::ReportProgress(0); }

  // Reports 1, which only a call that has done all its work does, whatever the step.
  void Finish() {
    if (g_progress) {
      g_progress->next = 0;
      Report(1);
    }
  }

  // Calls the callback with done, unless the call has not yet crossed the next multiple of its
  // step since the last report, or has not moved at all.
  static void Report(double done) {
    Progress& progress = *g_progress;
    progress.done = std::max(progress.done, done);
    if (progress.done < progress.next) {
      return;
    }
    double next = std::nextafter(progress.done, 2.0);
    if (progress.step > 0) {
      next = std::max(next, (std::floor(progress.done / progress.step) + 1) * progress.step);
    }
    progress.next = next;
    if (progress.callback(progress.context, progress.done)) {
      throw zucchini_rs::Cancelled();
    }
  }

 private:
  Progress progress_;
  Progress* saved_;
};

using zucchini_rs::ProgressSteps;
#else
// Without patches/progress.patch, Zucchini reports nothing, and the C API takes no callback.
class ProgressCall {
 public:
  ProgressCall(zucc_progress_fn, void*, double) {}
  void Start() {}
  void Finish() {}
};

struct ProgressSteps {
  void Next(double) {}
};
#endif

// Share of generating that the CRC-32s of both files take, so progress rises evenly.
constexpr double kGenerateCrc32Share = 0.01;

#ifdef ZUCCHINI_RS_CRC32_HOOK
std::atomic<zucc_crc32_fn> g_crc32{nullptr};
// Held while writing g_crc32, so zucc_set_default_crc32 never replaces what zucc_set_crc32 chose.
std::mutex g_crc32_mutex;
bool g_crc32_chosen = false;
#endif

bool TooLarge(size_t size) {
  return size > std::numeric_limits<uint32_t>::max();
}

// Runs body and turns any C++ exception into a result code.
template <typename Body>
int Guard(Body&& body) noexcept {
  g_last_error.clear();
  try {
    return body();
#ifdef ZUCCHINI_RS_PROGRESS
  } catch (const zucchini_rs::Cancelled&) {
    return ZUCC_CANCELLED;
#endif
  } catch (const zucchini_rs::CheckFailure& failure) {
    g_last_error = failure.what();
    return ZUCC_CHECK_FAILED;
  } catch (const std::bad_alloc&) {
    return ZUCC_OUT_OF_MEMORY;
  } catch (const std::exception& error) {
    g_last_error = error.what();
    return ZUCC_FATAL;
  } catch (...) {
    return ZUCC_FATAL;
  }
}

zucchini::ConstBufferView View(const uint8_t* data, size_t size) {
  return zucchini::ConstBufferView(data, size);
}

int Generate(const uint8_t* old_data, size_t old_len, const uint8_t* new_data, size_t new_len,
             const zucc_generate_options* options, zucc_progress_fn progress, void* context,
             double step, zucc_patch** patch_out) {
  if (!patch_out || (!old_data && old_len) || (!new_data && new_len)) {
    return ZUCC_INVALID_PARAM;
  }
  *patch_out = nullptr;
  if (TooLarge(old_len) || TooLarge(new_len)) {
    return ZUCC_TOO_LARGE;
  }
  // Zucchini writes a patch for an empty file, but its own reader rejects that patch.
  if (old_len == 0 || new_len == 0) {
    return ZUCC_EMPTY_FILE;
  }
  return Guard([&]() -> int {
    zucchini::ConstBufferView old_image = View(old_data, old_len);
    zucchini::ConstBufferView new_image = View(new_data, new_len);
    zucchini::GenerateOptions generate_options;
    if (options) {
      generate_options.start_scan_at = options->start_scan_at;
      generate_options.is_raw = options->raw != 0;
      if (options->imposed_matches) {
        generate_options.imposed_matches = options->imposed_matches;
      }
    }
    ProgressCall progress_call(progress, context, step);
    progress_call.Start();
    // The patch writer starts with each file's CRC-32, a sliver of the time generating takes.
    ProgressSteps steps;
    steps.Next(kGenerateCrc32Share);
    auto patch = std::make_unique<zucc_patch>(old_image, new_image);
    steps.Next(1);
    zucchini::status::Code status =
        zucchini::GenerateBuffer(old_image, new_image, generate_options, &patch->writer);
    if (status == zucchini::status::kStatusSuccess) {
      progress_call.Finish();
      *patch_out = patch.release();
    }
    return status;
  });
}

int Apply(const uint8_t* old_data, size_t old_len, const uint8_t* patch, size_t patch_len,
          uint8_t* new_out, size_t new_len, zucc_progress_fn progress, void* context,
          double step) {
  if ((!old_data && old_len) || (!patch && patch_len) || (!new_out && new_len)) {
    return ZUCC_INVALID_PARAM;
  }
  if (TooLarge(old_len) || TooLarge(new_len)) {
    return ZUCC_TOO_LARGE;
  }
  return Guard([&]() -> int {
    std::optional<zucchini::EnsemblePatchReader> reader =
        zucchini::EnsemblePatchReader::Create(View(patch, patch_len));
    if (!reader) {
      return ZUCC_PATCH_READ_ERROR;
    }
    if (reader->header().new_size != new_len) {
      return ZUCC_BUFFER_SIZE;
    }
    ProgressCall progress_call(progress, context, step);
    progress_call.Start();
    zucchini::status::Code status = zucchini::ApplyBuffer(
        View(old_data, old_len), *reader, zucchini::MutableBufferView(new_out, new_len));
    if (status == zucchini::status::kStatusSuccess) {
      progress_call.Finish();
    }
    return status;
  });
}

}  // namespace

#ifdef ZUCCHINI_RS_PROGRESS
namespace zucchini_rs {

ProgressSteps::ProgressSteps() {
  if (g_progress) {
    active_ = true;
    lo_ = g_progress->lo;
    hi_ = g_progress->hi;
  }
}

ProgressSteps::~ProgressSteps() {
  if (active_) {
    g_progress->lo = lo_;
    g_progress->hi = hi_;
  }
}

void ProgressSteps::Next(double end) {
  if (!active_) {
    return;
  }
  g_progress->lo = lo_ + (hi_ - lo_) * begin_;
  g_progress->hi = lo_ + (hi_ - lo_) * end;
  begin_ = end;
  ReportProgress(0);
}

void ReportProgress(double fraction) {
  if (!g_progress) {
    return;
  }
  double span = g_progress->hi - g_progress->lo;
  double done = g_progress->lo + span * std::clamp(fraction, 0.0, 1.0);
  // 1 waits for ProgressCall::Finish, after the last step has succeeded.
  ProgressCall::Report(std::min(done, std::nextafter(1.0, 0.0)));
}

}  // namespace zucchini_rs
#endif

#ifdef ZUCCHINI_RS_CRC32_HOOK
namespace zucchini {

// crc32.cc's own CalculateCrc32, renamed by patches/crc32-hook.patch.
uint32_t CalculateCrc32Builtin(const uint8_t* first, const uint8_t* last);

uint32_t CalculateCrc32(const uint8_t* first, const uint8_t* last) {
  zucc_crc32_fn crc32 = g_crc32.load(std::memory_order_acquire);
  return crc32 ? crc32(first, static_cast<size_t>(last - first)) : CalculateCrc32Builtin(first, last);
}

}  // namespace zucchini
#endif

extern "C" {

int zucc_generate(const uint8_t* old_data, size_t old_len, const uint8_t* new_data, size_t new_len,
                  const zucc_generate_options* options, zucc_patch** patch_out) {
  return Generate(old_data, old_len, new_data, new_len, options, nullptr, nullptr, 0, patch_out);
}

#ifdef ZUCCHINI_RS_PROGRESS
int zucc_generate_with_progress(const uint8_t* old_data, size_t old_len, const uint8_t* new_data,
                                size_t new_len, const zucc_generate_options* options,
                                zucc_progress_fn progress, void* context, double step,
                                zucc_patch** patch_out) {
  return Generate(old_data, old_len, new_data, new_len, options, progress, context, step, patch_out);
}
#endif

size_t zucc_patch_size(const zucc_patch* patch) {
  size_t size = 0;
  if (patch) {
    Guard([&]() -> int {
      size = patch->writer.SerializedSize();
      return ZUCC_OK;
    });
  }
  return size;
}

int zucc_patch_write(const zucc_patch* patch, uint8_t* out, size_t out_len) {
  if (!patch || (!out && out_len)) {
    return ZUCC_INVALID_PARAM;
  }
  return Guard([&]() -> int {
    if (patch->writer.SerializedSize() != out_len) {
      return ZUCC_BUFFER_SIZE;
    }
    return patch->writer.SerializeInto(zucchini::MutableBufferView(out, out_len))
               ? ZUCC_OK
               : ZUCC_PATCH_WRITE_ERROR;
  });
}

void zucc_patch_free(zucc_patch* patch) {
  Guard([&]() -> int {
    delete patch;
    return ZUCC_OK;
  });
}

int zucc_read_header(const uint8_t* patch, size_t patch_len, zucc_patch_header* header_out) {
  if (!header_out || (!patch && patch_len)) {
    return ZUCC_INVALID_PARAM;
  }
  return Guard([&]() -> int {
    std::optional<zucchini::EnsemblePatchReader> reader =
        zucchini::EnsemblePatchReader::Create(View(patch, patch_len));
    if (!reader) {
      return ZUCC_PATCH_READ_ERROR;
    }
    const zucchini::PatchHeader& header = reader->header();
    header_out->major_version = header.major_version;
    header_out->minor_version = header.minor_version;
    header_out->old_size = header.old_size;
    header_out->old_crc = header.old_crc;
    header_out->new_size = header.new_size;
    header_out->new_crc = header.new_crc;
    return ZUCC_OK;
  });
}

int zucc_apply(const uint8_t* old_data, size_t old_len, const uint8_t* patch, size_t patch_len,
               uint8_t* new_out, size_t new_len) {
  return Apply(old_data, old_len, patch, patch_len, new_out, new_len, nullptr, nullptr, 0);
}

#ifdef ZUCCHINI_RS_PROGRESS
int zucc_apply_with_progress(const uint8_t* old_data, size_t old_len, const uint8_t* patch,
                             size_t patch_len, uint8_t* new_out, size_t new_len,
                             zucc_progress_fn progress, void* context, double step) {
  return Apply(old_data, old_len, patch, patch_len, new_out, new_len, progress, context, step);
}
#endif

const char* zucc_last_error(void) {
  return g_last_error.c_str();
}

void zucc_set_log_level(int level) {
  logging::LogSeverity min_severity = logging::LOGGING_FATAL + 1;
  switch (level) {
    case ZUCC_LOG_ERROR:
      min_severity = logging::LOGGING_ERROR;
      break;
    case ZUCC_LOG_WARNING:
      min_severity = logging::LOGGING_WARNING;
      break;
    case ZUCC_LOG_INFO:
      min_severity = logging::LOGGING_INFO;
      break;
    default:
      break;
  }
  logging::internal::g_min_severity.store(min_severity, std::memory_order_relaxed);
}

#ifdef ZUCCHINI_RS_CRC32_HOOK
void zucc_set_crc32(zucc_crc32_fn fn) {
  std::lock_guard<std::mutex> lock(g_crc32_mutex);
  g_crc32_chosen = true;
  g_crc32.store(fn, std::memory_order_release);
}

void zucc_set_default_crc32(zucc_crc32_fn fn) {
  std::lock_guard<std::mutex> lock(g_crc32_mutex);
  if (!g_crc32_chosen) {
    g_crc32.store(fn, std::memory_order_release);
  }
}
#endif

}  // extern "C"
