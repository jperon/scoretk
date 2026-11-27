/* SPDX-License-Identifier: GPL-3.0-only */
/*
 * Copyright (C) 2026 jperon
 *
 * Paste this file into LuaJIT's ffi.cdef: no #include, no macro, only typedefs
 * and prototypes. Grey buffers are 8-bit (0 = black, 255 = white), one byte
 * per pixel, `stride` bytes per row, with stride >= width.
 */

typedef struct st_doc st_doc;

/* Reads a score, engraves it for a width_px x height_px screen and returns a
 * handle. The format is inferred from the extension, then the content, as in
 * the CLI. On failure returns NULL and writes a truncated UTF-8 message into
 * err (which may be NULL when err_len is 0). */
st_doc *st_open(const char *path, int width_px, int height_px, char *err, size_t err_len);

/* Number of engraved pages, or a negative code for a NULL handle. */
int st_page_count(const st_doc *doc);

/* Clears the caller's buffer to white and draws page `page` (0-based).
 * Returns 0 on success, or -1 for a NULL handle, -2 for an out-of-range page,
 * -3 for an invalid buffer/geometry, -4 for an internal error. */
int st_render_page(st_doc *doc, int page, uint8_t *buf, int width, int height, int stride);

/* Releases a handle; NULL is accepted. */
void st_close(st_doc *doc);

/* Static, NUL-terminated version string. */
const char *st_version(void);
