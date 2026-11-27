-- SPDX-License-Identifier: GPL-3.0-only
--
-- Copyright (C) 2026 jperon
--
-- This program is free software: you can redistribute it and/or modify it
-- under the terms of the GNU General Public License version 3 as published by
-- the Free Software Foundation.
--
-- This program is distributed in the hope that it will be useful, but WITHOUT
-- ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
-- FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
-- more details.
--
-- You should have received a copy of the GNU General Public License along with
-- this program. If not, see the LICENSE file at the root of this repository or
-- <https://www.gnu.org/licenses/gpl-3.0.html>.

-- Smoke test of the delivered header against the host cdylib.
--
--   cd crates/scoretk-engrave-ffi
--   nix-shell -p luajit --run "luajit tests/smoke.lua"
--
-- SCORETK_ENGRAVE_SO and SCORETK_CORPUS may override the two paths.

local ffi = require("ffi")
local here = arg and arg[0] and arg[0]:match("^(.*)/tests/smoke%.lua$") or "."
local root = here .. "/../.."

local header_path = here == "." and "include/scoretk_engrave.h"
  or here .. "/../include/scoretk_engrave.h"
local header = assert(io.open(header_path, "rb"))
ffi.cdef(header:read("*a"))
header:close()

local so = os.getenv("SCORETK_ENGRAVE_SO") or root .. "/target/debug/libscoretk_engrave_ffi.so"
local lib = ffi.load(so)
local corpus = os.getenv("SCORETK_CORPUS") or root .. "/tests/corpus"
local file = corpus .. "/v3/moonlight.mscx"

local width, height = 600, 800
local err = ffi.new("char[256]")
local doc = lib.st_open(file, width, height, err, ffi.sizeof(err))
assert(doc ~= nil, "st_open: " .. ffi.string(err))

local pages = lib.st_page_count(doc)
assert(pages > 0, "no page in " .. file)

local buffer = ffi.new("uint8_t[?]", width * height)
local code = lib.st_render_page(doc, 0, buffer, width, height, width)
assert(code == 0, "st_render_page returned " .. code)

local black = 0
for i = 0, width * height - 1 do
  if buffer[i] < 128 then black = black + 1 end
end
assert(black > 0, "page 0 has no black pixel")

lib.st_close(doc)
lib.st_close(nil)
print(string.format("OK: %s, %d pages, %d dark pixels, version %s", file, pages, black,
  ffi.string(lib.st_version())))
