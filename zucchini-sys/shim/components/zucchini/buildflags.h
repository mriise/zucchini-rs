// Stands in for the header GN generates for //components/zucchini:buildflags.
//
// These choose which executable formats this build understands, and therefore which patches it
// can apply: PE and ELF on, DEX and ZTF off, as in Chrome's official Windows builds.

#ifndef COMPONENTS_ZUCCHINI_BUILDFLAGS_H_
#define COMPONENTS_ZUCCHINI_BUILDFLAGS_H_

#include "build/buildflag.h"

#define BUILDFLAG_INTERNAL_ENABLE_DEX() (0)
#define BUILDFLAG_INTERNAL_ENABLE_ELF() (1)
#define BUILDFLAG_INTERNAL_ENABLE_WIN() (1)
#define BUILDFLAG_INTERNAL_ENABLE_ZTF() (0)

#endif  // COMPONENTS_ZUCCHINI_BUILDFLAGS_H_
