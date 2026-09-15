// Stands in for the header GN generates for PartitionAlloc's build flags.
//
// zucchini-rs does not use PartitionAlloc, so its allocator features are off. CHECKED_SPAN, which
// asks PartitionAlloc whether a span stays inside its allocation, is off with them; base::span's
// own index and size CHECKs stay on.

#ifndef PARTITION_ALLOC_BUILDFLAGS_H_
#define PARTITION_ALLOC_BUILDFLAGS_H_

#include "partition_alloc/buildflag.h"

#define PA_BUILDFLAG_INTERNAL_BACKUP_REF_PTR_POISON_OOB_PTR() (0)
#define PA_BUILDFLAG_INTERNAL_CHECKED_SPAN() (0)
#define PA_BUILDFLAG_INTERNAL_FORCE_ENABLE_RAW_PTR_EXCLUSION() (0)
#define PA_BUILDFLAG_INTERNAL_IS_CHROMEOS() (0)
#define PA_BUILDFLAG_INTERNAL_IS_COMPONENT_BUILD() (0)
#define PA_BUILDFLAG_INTERNAL_IS_DEBUG() (0)
#define PA_BUILDFLAG_INTERNAL_OFFICIAL() (0)

#if defined(__ANDROID__)
#define PA_BUILDFLAG_INTERNAL_IS_ANDROID() (1)
#else
#define PA_BUILDFLAG_INTERNAL_IS_ANDROID() (0)
#endif

#endif  // PARTITION_ALLOC_BUILDFLAGS_H_
