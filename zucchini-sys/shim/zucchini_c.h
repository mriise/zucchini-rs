/* C API over Chromium's Zucchini, for zucchini-sys.
 *
 * No C++ exception crosses it: a failed Chromium CHECK comes back as ZUCC_CHECK_FAILED, with the
 * message in zucc_last_error(). All functions are safe to call from several threads at once. */

#ifndef ZUCCHINI_RS_ZUCCHINI_C_H_
#define ZUCCHINI_RS_ZUCCHINI_C_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Results 0 to 10 are Zucchini's own status codes (components/zucchini/zucchini.h). */
enum {
  ZUCC_OK = 0,
  ZUCC_INVALID_PARAM = 1,
  ZUCC_PATCH_READ_ERROR = 4,
  ZUCC_PATCH_WRITE_ERROR = 5,
  ZUCC_INVALID_OLD_IMAGE = 6,
  ZUCC_INVALID_NEW_IMAGE = 7,
  ZUCC_FATAL = 10,
  ZUCC_TOO_LARGE = 100,     /* an input is over 4 GiB; patch headers store 32-bit sizes */
  ZUCC_BUFFER_SIZE = 101,   /* the output buffer is not the size the patch produces */
  ZUCC_OUT_OF_MEMORY = 102,
  ZUCC_CHECK_FAILED = 103,  /* a Chromium CHECK failed; see zucc_last_error() */
  ZUCC_EMPTY_FILE = 104,    /* Zucchini cannot patch from or to an empty file */
};

enum {
  ZUCC_LOG_OFF = 0, /* the default */
  ZUCC_LOG_ERROR = 1,
  ZUCC_LOG_WARNING = 2,
  ZUCC_LOG_INFO = 3,
};

/* A generated patch waiting to be written out. */
typedef struct zucc_patch zucc_patch;

typedef struct zucc_generate_options {
  /* Offset where executable detection starts. */
  uint32_t start_scan_at;
  /* Nonzero diffs both files as raw bytes, without looking for executables. */
  int raw;
  /* NULL, or matches that replace Zucchini's own: "old_offset+old_size=new_offset+new_size,...". */
  const char* imposed_matches;
} zucc_generate_options;

typedef struct zucc_patch_header {
  uint16_t major_version;
  uint16_t minor_version;
  uint32_t old_size;
  uint32_t old_crc;
  uint32_t new_size;
  uint32_t new_crc;
} zucc_patch_header;

/* Diffs old against new. On ZUCC_OK, *patch_out holds the patch; free it with zucc_patch_free.
 * options may be NULL for the defaults. Returns ZUCC_EMPTY_FILE if either file is empty. */
int zucc_generate(const uint8_t* old_data, size_t old_len, const uint8_t* new_data, size_t new_len,
                  const zucc_generate_options* options, zucc_patch** patch_out);

/* Serialized size of a generated patch in bytes. */
size_t zucc_patch_size(const zucc_patch* patch);

/* Writes a generated patch into out, which must be exactly zucc_patch_size bytes. */
int zucc_patch_write(const zucc_patch* patch, uint8_t* out, size_t out_len);

void zucc_patch_free(zucc_patch* patch);

/* Validates a patch and reads its header. */
int zucc_read_header(const uint8_t* patch, size_t patch_len, zucc_patch_header* header_out);

/* Applies patch to old, writing the result to new_out, which must be exactly the header's
 * new_size bytes. Checks the CRC-32 of both the old file and the result. */
int zucc_apply(const uint8_t* old_data, size_t old_len, const uint8_t* patch, size_t patch_len,
               uint8_t* new_out, size_t new_len);

/* Message for the calling thread's last ZUCC_CHECK_FAILED or ZUCC_FATAL, or "". Valid until the
 * thread's next call into this API. */
const char* zucc_last_error(void);

/* Chooses which of Zucchini's log messages are written to stderr (a ZUCC_LOG_* value). */
void zucc_set_log_level(int level);

#ifdef __cplusplus
}
#endif

#endif /* ZUCCHINI_RS_ZUCCHINI_C_H_ */
