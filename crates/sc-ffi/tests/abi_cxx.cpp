/* The same harness as `abi.c`, compiled as C++.
 *
 * A wrapper rather than a LANGUAGE source property: `set_source_files_properties`
 * with TARGET_DIRECTORY applies to the whole directory scope, so it relabelled
 * the C target's copy too and both halves quietly became C++.
 *
 * Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
 */
#include "abi.c" // NOLINT
