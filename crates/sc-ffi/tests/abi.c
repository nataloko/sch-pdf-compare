/* The C ABI harness.
 *
 * Compiled as C and again as C++, with -Wall -Wextra -Werror -pedantic, because
 * a Rust test proves nothing about whether the generated header is usable from
 * the language that actually consumes it.
 *
 * Its main job is rule 3 of the seam: every entry point tolerates a null handle
 * and says so, in both languages, so the frontend needs no null guards of its
 * own. Every one of these calls would be undefined behaviour if that were not
 * true, which is exactly why it is worth a test rather than a comment.
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

static void version_and_status_codes(void) {
    const char *v = sc_version();
    check(v != NULL, "sc_version returns a pointer");
    check(v != NULL && strlen(v) > 0, "sc_version is not empty");

    check(SC_OK == 0, "SC_OK is 0");
    check(SC_PENDING > 0, "SC_PENDING is not an error");
    check(SC_ERR_INVALID < 0, "SC_ERR_INVALID is negative");
    check(SC_ERR_IO < 0, "SC_ERR_IO is negative");
    check(SC_ERR_FORMAT < 0, "SC_ERR_FORMAT is negative");
    check(SC_ERR_NO_PAGE < 0, "SC_ERR_NO_PAGE is negative");
    check(SC_ERR_GEOMETRY < 0, "SC_ERR_GEOMETRY is negative");

    const char *e = sc_last_error();
    check(e != NULL, "sc_last_error is never null");
}

static void null_session_is_survivable(void) {
    ScRectF r;
    ScTile t;
    float fw = 0, fh = 0;
    int32_t iw = 0, ih = 0;

    sc_session_free(NULL); /* a no-op, not a crash */

    check(sc_session_page_count(NULL) == 0, "page_count(NULL) is 0");

    ScPair p = sc_session_pair(NULL, 1);
    check(p.page_a == 0 && p.page_b == 0, "pair(NULL) is the empty pair");

    check(sc_session_page_size(NULL, 1, &fw, &fh) < 0, "page_size(NULL) fails");
    check(sc_session_page_device_size(NULL, 1, 1.0f, &iw, &ih) < 0,
          "page_device_size(NULL) fails");
    check(sc_session_tile(NULL, 1, 1.0f, 0, 0, 8, 8, &t) < 0, "tile(NULL) fails");

    check(sc_session_view_mode(NULL) == SC_VIEW_MODE_OVERLAY,
          "view_mode(NULL) is the default");
    sc_session_set_view_mode(NULL, SC_VIEW_MODE_ONLY_A);
    check(sc_session_tolerance(NULL) == 0, "tolerance(NULL) is 0");
    sc_session_set_tolerance(NULL, 2);
    check(sc_session_page_delta(NULL) == 0, "page_delta(NULL) is 0");
    sc_session_set_page_delta(NULL, 3);

    sc_session_add_ignore_rect(NULL, 0, 0, 1, 1);
    sc_session_clear_ignore_rects(NULL);
    check(sc_session_ignore_rect_count(NULL) == 0, "ignore_rect_count(NULL) is 0");
    check(sc_session_ignore_rect(NULL, 0, &r) < 0, "ignore_rect(NULL) fails");

    check(sc_session_scan_page(NULL, 1) < 0, "scan_page(NULL) fails");
    check(sc_session_change_count(NULL, 1) == -1,
          "change_count(NULL) is -1, meaning not scanned");
    check(sc_session_ignored_count(NULL, 1) == -1, "ignored_count(NULL) is -1");
    check(sc_session_change(NULL, 1, 0, &r) < 0, "change(NULL) fails");

    ScSweepStatus sw;
    check(sc_session_start_sweep(NULL) < 0, "start_sweep(NULL) fails");
    sc_session_stop_sweep(NULL); /* a no-op, not a crash */
    check(sc_session_wakeup_handle(NULL) == -1, "wakeup_handle(NULL) is -1");
    check(sc_session_pump(NULL) < 0, "pump(NULL) fails");
    check(sc_session_sweep_status(NULL, &sw) < 0, "sweep_status(NULL) fails");
    check(sc_session_suggested_count(NULL) == 0, "suggested_count(NULL) is 0");
    check(sc_session_suggested(NULL, 0, &r) < 0, "suggested(NULL) fails");
    check(sc_session_auto_match(NULL) < 0, "auto_match(NULL) fails");
    check(!sc_session_pairing_is_automatic(NULL), "pairing_is_automatic(NULL) is false");
    ScTextChange tc;
    check(sc_session_text_changes(NULL, 1) < 0, "text_changes(NULL) fails");
    check(sc_session_text_change(NULL, 0, &tc) < 0, "text_change(NULL) fails");
    check(sc_session_load_settings(NULL) < 0, "load_settings(NULL) fails");
    check(sc_session_save_settings(NULL) < 0, "save_settings(NULL) fails");
    /* Null out-params are allowed here: a caller may want only one of the two. */
    sc_last_pair(NULL, NULL);
}

static void null_out_params_are_survivable(void) {
    /* A live session is not needed: the argument check has to come first, or a
     * caller who got one thing wrong gets undefined behaviour instead of an
     * error code. */
    check(sc_session_page_size(NULL, 1, NULL, NULL) < 0, "page_size with null outs fails");
    check(sc_session_tile(NULL, 1, 1.0f, 0, 0, 8, 8, NULL) < 0, "tile with a null out fails");
    check(sc_session_change(NULL, 1, 0, NULL) < 0, "change with a null out fails");
    check(sc_session_ignore_rect(NULL, 0, NULL) < 0, "ignore_rect with a null out fails");
    check(sc_session_sweep_status(NULL, NULL) < 0, "sweep_status with a null out fails");
    check(sc_session_suggested(NULL, 0, NULL) < 0, "suggested with a null out fails");
    check(sc_session_text_change(NULL, 0, NULL) < 0, "text_change with a null out fails");
}

static void a_failure_leaves_a_sentence_behind(void) {
    check(sc_session_open(NULL, NULL) == NULL, "opening nothing fails");
    const char *e = sc_last_error();
    check(e != NULL && strlen(e) > 0, "and says why");

    check(sc_session_open("/definitely/not/here.pdf", "/nor/here.pdf") == NULL,
          "opening a missing file fails");
    e = sc_last_error();
    check(e != NULL && strlen(e) > 0, "and says why");
}

int main(void) {
    version_and_status_codes();
    null_session_is_survivable();
    null_out_params_are_survivable();
    a_failure_leaves_a_sentence_behind();

    if (failures == 0) {
        printf("abi: ok\n");
    }
    return failures == 0 ? 0 : 1;
}
