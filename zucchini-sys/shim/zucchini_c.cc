// C API over Chromium's Zucchini; see zucchini_c.h.

#include "zucchini_c.h"

#include <atomic>
#include <cstdint>
#include <exception>
#include <limits>
#include <memory>
#include <new>
#include <optional>
#include <string>

#include "base/logging.h"
#include "components/zucchini/buffer_view.h"
#include "components/zucchini/crc32.h"
#include "components/zucchini/patch_reader.h"
#include "components/zucchini/patch_writer.h"
#include "components/zucchini/zucchini.h"

static_assert(sizeof(zucc_patch_header) == 20);

struct zucc_patch {
  zucc_patch(zucchini::ConstBufferView old_image, zucchini::ConstBufferView new_image)
      : writer(old_image, new_image) {}

  zucchini::EnsemblePatchWriter writer;
};

namespace {

thread_local std::string g_last_error;

std::atomic<zucc_crc32_fn> g_crc32{nullptr};

bool TooLarge(size_t size) {
  return size > std::numeric_limits<uint32_t>::max();
}

// Runs body and turns any C++ exception into a result code.
template <typename Body>
int Guard(Body&& body) noexcept {
  g_last_error.clear();
  try {
    return body();
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

}  // namespace

namespace zucchini {

// crc32.cc's own CalculateCrc32, renamed by build.rs's PATCHES.
uint32_t CalculateCrc32Builtin(const uint8_t* first, const uint8_t* last);

uint32_t CalculateCrc32(const uint8_t* first, const uint8_t* last) {
  zucc_crc32_fn crc32 = g_crc32.load(std::memory_order_acquire);
  return crc32 ? crc32(first, static_cast<size_t>(last - first)) : CalculateCrc32Builtin(first, last);
}

}  // namespace zucchini

extern "C" {

int zucc_generate(const uint8_t* old_data, size_t old_len, const uint8_t* new_data, size_t new_len,
                  const zucc_generate_options* options, zucc_patch** patch_out) {
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
    auto patch = std::make_unique<zucc_patch>(old_image, new_image);
    zucchini::status::Code status =
        zucchini::GenerateBuffer(old_image, new_image, generate_options, &patch->writer);
    if (status == zucchini::status::kStatusSuccess) {
      *patch_out = patch.release();
    }
    return status;
  });
}

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
    return zucchini::ApplyBuffer(View(old_data, old_len), *reader,
                                 zucchini::MutableBufferView(new_out, new_len));
  });
}

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

void zucc_set_crc32(zucc_crc32_fn fn) {
  g_crc32.store(fn, std::memory_order_release);
}

}  // extern "C"
