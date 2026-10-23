// Progress reports and cancellation for Zucchini's generate and apply; patches/progress.patch calls
// these from Zucchini's long loops, and shim/zucchini_c.cc defines them.

#ifndef ZUCCHINI_RS_PROGRESS_H_
#define ZUCCHINI_RS_PROGRESS_H_

#include <cstdint>

namespace zucchini_rs {

// Thrown out of Zucchini when the caller's progress callback cancels; the C API returns
// ZUCC_CANCELLED. Not a std::exception, so nothing mistakes it for a failure.
struct Cancelled {};

// Shares of a step's time that its parts take, so the percentage rises roughly evenly.
// Generating: matching elements, of the whole call.
inline constexpr double kMatchShare = 0.05;
// Generating: the suffix array, of one equivalence map build (about 0.9 for an executable, 0.8 for
// raw data).
inline constexpr double kSuffixArrayShare = 0.85;
// Generating: where each step of a suffix sort ends, as a fraction of the sort: classifying
// suffixes, sorting LMS substrings, labelling them, sorting them recursively. Inducing the full
// order runs to the end.
inline constexpr double kSuffixSortShares[] = {0.03, 0.35, 0.52, 0.62};
// Generating an executable element: disassembling and indexing both images, of the element.
inline constexpr double kElementIndexShare = 0.05;
// Generating an executable element: reference and raw deltas, of the element.
inline constexpr double kElementDeltaShare = 0.02;
// Applying: each file's CRC-32, of the whole call.
inline constexpr double kCrc32Share = 0.08;
// Applying an element: copying equivalences and extra data, and raw deltas, of the element.
inline constexpr double kApplyCopyShare = 0.1;

// Divides the enclosing range of progress reports into consecutive steps, and restores it when
// destroyed. The outermost range is the whole C API call. Does nothing when the call has no
// progress callback.
class ProgressSteps {
 public:
  ProgressSteps();
  ProgressSteps(const ProgressSteps&) = delete;
  ProgressSteps& operator=(const ProgressSteps&) = delete;
  ~ProgressSteps();

  // Starts the next step, which runs from where the last one ended (or 0) to end, both fractions
  // of the enclosing range. Reports the step's start, so it can throw Cancelled.
  void Next(double end);

 private:
  bool active_ = false;
  double lo_ = 0;
  double hi_ = 0;
  double begin_ = 0;
};

// Reports that fraction (0 to 1) of the current step is done, and throws Cancelled if the
// callback asks to stop. Does nothing when the call has no progress callback.
void ReportProgress(double fraction);

// Reports progress through a loop over [0, total) at most about 256 times.
class ProgressLoop {
 public:
  explicit ProgressLoop(uint64_t total) : total_(total), step_(total / 256 + 1) {}

  void At(uint64_t done) {
    if (done >= next_) {
      next_ = done + step_;
      ReportProgress(total_ ? static_cast<double>(done) / static_cast<double>(total_) : 1.0);
    }
  }

 private:
  uint64_t total_;
  uint64_t step_;
  uint64_t next_ = 0;
};

}  // namespace zucchini_rs

#endif  // ZUCCHINI_RS_PROGRESS_H_
