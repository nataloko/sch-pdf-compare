/* The C ABI harness.
 *
 * Compiled as C and again as C++, with -Wall -Wextra -Werror -pedantic, because
 * a Rust test proves nothing about whether the generated header is usable from
 * the language that actually consumes it.
 *
 * Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
 */
#include <stdio.h>
#include <string.h>

#include "schcompare.h"

static int failures = 0;

static void check(int ok, const char *what) {
    if (!ok) {
        fprintf(stderr, "FAIL %s\n", what);
        failures++;
    }
}

int main(void) {
    const char *v = sc_version();
    check(v != NULL, "sc_version returns a pointer");
    check(v != NULL && strlen(v) > 0, "sc_version is not empty");

    check(SC_OK == 0, "SC_OK is 0");
    check(SC_PENDING > 0, "SC_PENDING is not an error");
    check(SC_ERR_INVALID < 0, "every failure is negative");

    if (failures == 0) {
        printf("abi: ok\n");
    }
    return failures == 0 ? 0 : 1;
}
