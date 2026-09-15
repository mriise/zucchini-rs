// zucchini-rs replacement for Chromium's //base/check_op.h.
//
// Each macro compares with its plain operator, which is also what Chromium's own CHECK_op expands
// to in builds that do not stream the operand values.

#ifndef BASE_CHECK_OP_H_
#define BASE_CHECK_OP_H_

#include "base/check.h"

#define CHECK_EQ(val1, val2, ...) CHECK((val1) == (val2))
#define CHECK_NE(val1, val2, ...) CHECK((val1) != (val2))
#define CHECK_LE(val1, val2, ...) CHECK((val1) <= (val2))
#define CHECK_LT(val1, val2, ...) CHECK((val1) < (val2))
#define CHECK_GE(val1, val2, ...) CHECK((val1) >= (val2))
#define CHECK_GT(val1, val2, ...) CHECK((val1) > (val2))

#define DCHECK_EQ(val1, val2) DCHECK((val1) == (val2))
#define DCHECK_NE(val1, val2) DCHECK((val1) != (val2))
#define DCHECK_LE(val1, val2) DCHECK((val1) <= (val2))
#define DCHECK_LT(val1, val2) DCHECK((val1) < (val2))
#define DCHECK_GE(val1, val2) DCHECK((val1) >= (val2))
#define DCHECK_GT(val1, val2) DCHECK((val1) > (val2))

#endif  // BASE_CHECK_OP_H_
