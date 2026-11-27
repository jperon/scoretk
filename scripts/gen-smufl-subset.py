#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Données de `fonts/leland/` de MuseScore Studio 4.7.4 (tag v4.7.4, commit
# 7688c005ad963ba09ee715aae53dbc7e8c9d5ef8), Copyright (C) MuseScore BVBA and
# Klaus Rettinghaus, sous SIL Open Font License 1.1, converties en Rust
# (fichier généré) en 2026.
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
# FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
# more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see the LICENSE file at the root of this repository or
# <https://www.gnu.org/licenses/gpl-3.0.html>.

"""Génère le sous-ensemble SMuFL de `scoretk-smufl` (fichier généré).

Usage : python3 -I scripts/gen-smufl-subset.py <dossier-fonts> <dst.rs>

`<dossier-fonts>` contient `Leland.otf` et `leland_metadata.json` tels que
livrés par MuseScore 4.7.4 dans `fonts/leland/`. Le script n'utilise que la
bibliothèque standard Python : il analyse lui-même la table CFF (TYPE2
charstrings), la table `cmap` et `hmtx`, puis convertit les cubiques en
quadratiques avec une tolérance d'un demi font unit (em = 1000, 1 portée =
250 unités), plus stricte que le seuil d'un unité proposé en P0. Il est donc reproductible sous `python3 -I`, sans fontTools ni
dépendance tierce ; la police n'est jamais exécutée, seulement lue.

La sortie contient : les contours (Move/Line/Quad en coordonnées relatives),
l'avance et la bbox de chaque glyphe, les ancres SMuFL et les
`engravingDefaults` (fixed point 1/1024 portée).
"""

from __future__ import annotations

import json
import os
import struct
import sys

UPEM = 1000
SPATIUM_UNITS = UPEM // 4  # SMuFL : une portée = 1/4 de l'em
TOLERANCE = 0.5            # erreur maximale de Cu2Qu, en font units

# Lesfamilles que l'engraver est susceptible d'émettre (§ 9.4 de
# docs/agents/engraving.md). Les clefs nommées `gClef`/`fClef`/`cClef` et le
# point d'augmentation sont ajoutés explicitement : ils ne commencent pas par
# `clef`/`dot`.
FAMILIES = (
    "notehead", "clef", "accidental", "rest", "flag", "beam", "dot",
    "dynamic", "artic", "slur", "tie", "fermata", "trill", "turn",
    "breath", "caesura", "timeSig", "barline", "bracket", "brace",
    "pedal", "hairpin", "ornament", "fingering", "harmonic", "mordent",
    "arpeggio", "glissando", "repeat",
)
EXPLICIT = frozenset(("augmentationDot", "metAugmentationDot"))


def keep(name: str) -> bool:
    return name.startswith(FAMILIES) or "Clef" in name or name in EXPLICIT


# ---------------------------------------------------------------------------
# Lecture SFNT (std only)
# ---------------------------------------------------------------------------

def u8(buf: bytes, pos: int) -> int:
    return buf[pos]


def u16(buf: bytes, pos: int) -> int:
    return struct.unpack_from(">H", buf, pos)[0]


def i16(buf: bytes, pos: int) -> int:
    return struct.unpack_from(">h", buf, pos)[0]


def u32(buf: bytes, pos: int) -> int:
    return struct.unpack_from(">I", buf, pos)[0]


def sfnt_tables(buf: bytes) -> dict[str, tuple[int, int]]:
    tables = {}
    for index in range(u16(buf, 4)):
        start = 12 + 16 * index
        tag = buf[start:start + 4].decode("latin-1")
        tables[tag] = (u32(buf, start + 8), u32(buf, start + 12))
    return tables


def read_index(buf: bytes, pos: int) -> tuple[list[bytes], int]:
    """INDEX CFF : (éléments, position juste après l'INDEX)."""
    count = u16(buf, pos)
    pos += 2
    if count == 0:
        return [], pos
    off_size = u8(buf, pos)
    pos += 1
    offsets = []
    for _ in range(count + 1):
        value = 0
        for byte in buf[pos:pos + off_size]:
            value = (value << 8) | byte
        pos += off_size
        offsets.append(value)
    start = pos + offsets[0] - 1
    items = [
        buf[start + offsets[i] - 1:start + offsets[i + 1] - 1]
        for i in range(count)
    ]
    return items, pos + offsets[-1] - 1


def read_dict(raw: bytes, strings: list[str]) -> dict[int, list[float]]:
    result: dict[int, list[float]] = {}
    stack: list[float] = []
    pos = 0
    while pos < len(raw):
        byte = raw[pos]
        pos += 1
        if byte <= 21:
            operator = 1200 + raw[pos] if byte == 12 else byte
            if byte == 12:
                pos += 1
            result.setdefault(operator, []).extend(stack)
            stack = []
            if operator == 19:  # Subrs : l'INDEX suit, ce n'est plus un DICT
                break
        elif byte == 28:
            stack.append(i16(raw, pos))
            pos += 2
        elif byte == 29:
            stack.append(struct.unpack_from(">i", raw, pos)[0])
            pos += 4
        elif byte == 30:
            # Nombre réel CFF (BlueValues, StdHW…) : lu pour avancer, puis
            # ignoré — les DICT ne servent ici qu'à trouver les INDEX.
            done = False
            while not done and pos < len(raw):
                pair = raw[pos]
                pos += 1
                for nibble in (pair >> 4, pair & 0x0F):
                    if nibble == 0x0F:
                        done = True
                        break
            stack.append(0.0)
        elif 32 <= byte <= 246:
            stack.append(byte - 139)
        elif 247 <= byte <= 250:
            stack.append((byte - 247) * 256 + raw[pos] + 108)
            pos += 1
        elif 251 <= byte <= 254:
            stack.append(-(byte - 251) * 256 - raw[pos] - 108)
            pos += 1
        elif byte == 255:
            stack.append(struct.unpack_from(">i", raw, pos)[0] / 65536.0)
            pos += 4
    return result


def sid_name(sid: int, strings: list[str]) -> str:
    # Les noms standard SIDs ne font pas partie du sous-ensemble notationnel
    # (vérifié sur la police 4.7.4) : un nom factice suffit à les ignorer.
    if sid < 391:
        return "std%d" % sid
    return strings[sid - 391]


def read_cmap(buf: bytes, start: int, glyph_names: list[str]) -> dict[str, int]:
    """Table cmap (format 4 ou 12) : nom de glyphe → codepoint Unicode."""
    result: dict[str, int] = {}
    for index in range(u16(buf, start + 2)):
        record = start + 4 + 8 * index
        platform, encoding, offset = (
            u16(buf, record), u16(buf, record + 2), u32(buf, record + 4)
        )
        pos = start + offset
        fmt = u16(buf, pos)
        if fmt == 4:
            seg_count = u16(buf, pos + 6) // 2
            ends = pos + 14
            starts = ends + seg_count * 2 + 2
            deltas = starts + seg_count * 2
            ranges = deltas + seg_count * 2
            for seg in range(seg_count):
                end = u16(buf, ends + 2 * seg)
                first = u16(buf, starts + 2 * seg)
                delta = struct.unpack_from(">h", buf, deltas + 2 * seg)[0]
                roffset = u16(buf, ranges + 2 * seg)
                if first == 0xFFFF:
                    continue
                for code in range(first, end + 1):
                    if roffset == 0:
                        glyph = (code + delta) & 0xFFFF
                    else:
                        addr = ranges + 2 * seg + roffset + 2 * (code - first)
                        glyph = u16(buf, addr)
                        if glyph:
                            glyph = (glyph + delta) & 0xFFFF
                    if 0 < glyph < len(glyph_names):
                        result.setdefault(glyph_names[glyph], code)
        elif fmt == 12:
            groups = u32(buf, pos + 12)
            for group in range(groups):
                entry = pos + 16 + 12 * group
                first, last, glyph = (
                    u32(buf, entry), u32(buf, entry + 4), u32(buf, entry + 8)
                )
                if last - first > 0x10000:
                    continue
                for code in range(first, last + 1):
                    index = glyph + code - first
                    if 0 < index < len(glyph_names):
                        result.setdefault(glyph_names[index], code)
    return result


def read_advances(buf: bytes, start: int, count: int) -> list[int]:
    """hmtx : `numHMetrics` avances complètes, puis des lsb seuls."""
    hhea = sfnt_tables(buf)["hhea"][0]
    number = u16(buf, hhea + 34)
    number = min(number, count)
    advances = [u16(buf, start + 4 * i) for i in range(number)]
    if not advances:
        advances = [0]
    return advances + [advances[-1]] * (count - number)


# ---------------------------------------------------------------------------
# Type2 charstrings (std only)
# ---------------------------------------------------------------------------

def number(byte: int, code: bytes, pos: int) -> tuple[float, int]:
    if byte == 28:
        return i16(code, pos), pos + 2
    if byte == 29:
        return struct.unpack_from(">i", code, pos)[0], pos + 4
    if 32 <= byte <= 246:
        return byte - 139, pos
    if 247 <= byte <= 250:
        return (byte - 247) * 256 + code[pos] + 108, pos + 1
    if 251 <= byte <= 254:
        return -(byte - 251) * 256 - code[pos] - 108, pos + 1
    if byte == 255:
        return struct.unpack_from(">i", code, pos)[0] / 65536.0, pos + 4
    raise ValueError("byte de charstring inattendu : %d" % byte)


def bias(count: int) -> int:
    if count < 1240:
        return 107
    if count < 33900:
        return 1131
    return 32768


class Outline:
    """Interpréteur TYPE2 : contours en coordonnées absolues (font units)."""

    def __init__(self, local: list[bytes], glob_: list[bytes]):
        self.local = local
        self.global_ = glob_
        self.stack: list[float] = []
        self.x = self.y = 0.0
        self.start_x = self.start_y = 0.0
        self.contours: list[list[tuple]] = []
        self.current: list[tuple] = []
        self.hints = 0
        self.mask_bytes: int | None = None
        self.width_taken = False
        self.depth = 0

    def width(self, extra: int) -> list[float]:
        """Retire l'avance éventuelle en tête de la pile (Type2 §4)."""
        args = self.stack
        if not self.width_taken and len(args) > extra:
            args = args[1:]
        self.width_taken = True
        self.stack = []
        return args

    def pop(self) -> float:
        value = self.stack.pop()
        return value

    def end_path(self) -> None:
        if self.current:
            if (self.current[-1][-2], self.current[-1][-1]) != (self.start_x, self.start_y):
                self.current.append(("L", self.start_x, self.start_y))
            self.contours.append(self.current)
            self.current = []

    def move(self, dx: float, dy: float) -> None:
        self.end_path()
        self.x += dx
        self.y += dy
        self.start_x, self.start_y = self.x, self.y
        self.width_taken = True

    def line(self, dx: float, dy: float) -> None:
        x, y = self.x + dx, self.y + dy
        self.current.append(("L", x, y))
        self.x, self.y = x, y

    def curve(self, dx1: float, dy1: float, dx2: float, dy2: float,
              dx3: float, dy3: float) -> None:
        # TYPE2 : chaque delta de courbe est relatif au point précédent,
        # pas toujours au point de départ (CFF §4, rrcurveto).
        c1 = (self.x + dx1, self.y + dy1)
        c2 = (c1[0] + dx2, c1[1] + dy2)
        end = (c2[0] + dx3, c2[1] + dy3)
        self.current.append(("C", self.x, self.y, *c1, *c2, *end))
        self.x, self.y = end

    def vcurve(self, args: list[float]) -> list[float]:
        dya, dxb, dyb, dxc = args[:4]
        rest = args[4:]
        dyc = rest[0] if len(rest) == 1 else 0.0
        self.curve(0, dya, dxb, dyb, dxc, dyc)
        return rest[1:] if len(rest) == 1 else rest

    def hcurve(self, args: list[float]) -> list[float]:
        dxa, dxb, dyb, dyc = args[:4]
        rest = args[4:]
        dxc = rest[0] if len(rest) == 1 else 0.0
        self.curve(dxa, 0, dxb, dyb, dxc, dyc)
        return rest[1:] if len(rest) == 1 else rest

    def run(self, code: bytes) -> None:
        pos = 0
        while pos < len(code):
            byte = code[pos]
            pos += 1
            if byte == 12:  # escape : hflex/flex, absents de Leland
                pos += 1
                self.stack = []
                continue
            if byte == 28 or byte >= 32:  # 29 = callgsubr, 30/31 = courbes
                value, pos = number(byte, code, pos)
                self.stack.append(value)
                continue
            if byte in (1, 3, 18, 23):  # hstem, vstem, hstemhm, vstemhm
                if not self.width_taken and len(self.stack) % 2:
                    self.stack.pop(0)
                self.width_taken = True
                self.hints += len(self.stack) // 2
                self.stack = []
            elif byte in (19, 20):  # hintmask / cntrmask
                if self.mask_bytes is None:
                    if not self.width_taken and len(self.stack) % 2:
                        self.stack.pop(0)
                    self.width_taken = True
                    self.hints += len(self.stack) // 2
                    self.stack = []
                    self.mask_bytes = (self.hints + 7) // 8
                else:
                    self.stack = []
                pos += self.mask_bytes
            elif byte == 4:  # vmoveto
                args = self.width(1)
                self.move(0, args[0])
            elif byte == 21:  # rmoveto
                args = self.width(2)
                self.move(args[0], args[1])
            elif byte == 6:  # hlineto
                for i, value in enumerate(self.stack):
                    if i % 2 == 0:
                        self.line(value, 0)
                    else:
                        self.line(0, value)
                self.stack = []
            elif byte == 7:  # vlineto
                for i, value in enumerate(self.stack):
                    if i % 2 == 0:
                        self.line(0, value)
                    else:
                        self.line(value, 0)
                self.stack = []
            elif byte == 5:  # rlineto
                args = self.stack
                for i in range(0, len(args) - 1, 2):
                    self.line(args[i], args[i + 1])
                self.stack = []
            elif byte == 8:  # rrcurveto
                args = self.stack
                for i in range(0, len(args) - 5, 6):
                    self.curve(*args[i:i + 6])
                self.stack = []
            elif byte == 22:  # hmoveto
                args = self.width(1)
                self.move(args[0], 0)
            elif byte == 27:  # hhcurveto
                args = self.stack
                if len(args) % 2:
                    dy1 = args[0]
                    args = args[1:]
                else:
                    dy1 = 0
                for i in range(0, len(args) - 3, 4):
                    dxa, dxb, dyb, dxc = args[i:i + 4]
                    self.curve(dxa, dy1, dxb, dyb, dxc, 0)
                    dy1 = 0
                self.stack = []
            elif byte == 31:  # hvcurveto : alterne horizontale / verticale
                args = self.stack
                while args:
                    args = self.hcurve(args)
                    if args:
                        args = self.vcurve(args)
                self.stack = []
            elif byte == 30:  # vhcurveto : alterne verticale / horizontale
                args = self.stack
                while args:
                    args = self.vcurve(args)
                    if args:
                        args = self.hcurve(args)
                self.stack = []
            elif byte == 26:  # vvcurveto
                args = self.stack
                if len(args) % 2:
                    dx1 = args[0]
                    args = args[1:]
                else:
                    dx1 = 0
                for i in range(0, len(args) - 3, 4):
                    dya, dxb, dyb, dyc = args[i:i + 4]
                    self.curve(dx1, dya, dxb, dyb, 0, dyc)
                    dx1 = 0
                self.stack = []
            elif byte == 25:  # rlinecurve
                args = self.stack
                for i in range(0, len(args) - 6, 2):
                    self.line(args[i], args[i + 1])
                self.curve(*args[-6:])
                self.stack = []
            elif byte == 24:  # rcurveline
                args = self.stack
                for i in range(0, len(args) - 2, 6):
                    self.curve(*args[i:i + 6])
                self.line(args[-2], args[-1])
                self.stack = []
            elif byte == 14:  # endchar
                args = self.stack
                if len(args) == 4:
                    raise ValueError("charstring seac (composite) non géré")
                self.width(0)
                self.end_path()
                return
            elif byte in (10, 29):  # callsubr / callgsubr
                if self.depth > 8:
                    raise ValueError("récursion de sous-routine trop profonde")
                index = int(self.pop()) + bias(len(self.local if byte == 10 else self.global_))
                subrs = self.local if byte == 10 else self.global_
                if not 0 <= index < len(subrs):
                    raise ValueError("index de sous-routine hors limites")
                # La pile est partagée avec la sous-routine (TYPE2 §4) : les
                # arguments déjà empilés restent disponibles au retour.
                self.depth += 1
                self.run(subrs[index])
                self.depth -= 1
            elif byte == 11:  # return
                return
            else:
                raise ValueError("opérateur TYPE2 inattendu : %d" % byte)
        self.end_path()


def bezier(p0, p1, p2, p3, t):
    u = 1.0 - t
    return (
        u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0],
        u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1],
    )


def quad(p0, p1, p2, t):
    u = 1.0 - t
    return (
        u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0],
        u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1],
    )


def to_quads(p0, p1, p2, p3, depth=0):
    """Cu2Qu simple : un point de contrôle ajusté, subdivision si besoin."""
    q1 = ((3 * p1[0] + 3 * p2[0] - p0[0] - p3[0]) / 4.0,
          (3 * p1[1] + 3 * p2[1] - p0[1] - p3[1]) / 4.0)
    rounded = (round(q1[0]), round(q1[1]))
    error = 0.0
    for step in range(1, 8):
        t = step / 8.0
        c = bezier(p0, p1, p2, p3, t)
        q = quad(p0, rounded, p3, t)
        error = max(error, abs(c[0] - q[0]), abs(c[1] - q[1]))
    if error <= TOLERANCE or depth >= 12:
        return [("Q", rounded, p3)]
    a = ((p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2)
    b = ((p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2)
    c = ((p2[0] + p3[0]) / 2, (p2[1] + p3[1]) / 2)
    d = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
    e = ((b[0] + c[0]) / 2, (b[1] + c[1]) / 2)
    mid = ((d[0] + e[0]) / 2, (d[1] + e[1]) / 2)
    return to_quads(p0, a, d, mid, depth + 1) + \
           to_quads(mid, e, c, p3, depth + 1)


def on_segment(p0, point, p1):
    """Vrai si `point` est à moins d'un font unit du segment p0→p1."""
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    length = max(1.0, (dx * dx + dy * dy) ** 0.5)
    if abs(dx * (point[1] - p0[1]) - dy * (point[0] - p0[0])) / length > TOLERANCE:
        return False
    projection = (dx * (point[0] - p0[0]) + dy * (point[1] - p0[1])) / (length * length)
    return -0.01 <= projection <= 1.01


def glyph_edges(contours):
    """Aplatit en commandes Move / Line / Quad relatives au point courant."""
    ops = []
    for contour in contours:
        start = contour[0][1:3]
        ops.append(("M", start))
        cx, cy = start
        segments = []
        # Le premier élément d'un contour CFF est déjà un segment (avec son
        # point initial), pas seulement le point de départ : l'omettre
        # remplace le premier arc par une corde droite (le « o » d'Edwin
        # apparaissait barré).
        for item in contour:
            if item[0] == "L":
                p1 = (round(item[1]), round(item[2]))
                if p1 != (cx, cy):
                    segments.append(("L", p1))
                cx, cy = p1
            else:
                _, x0, y0, c1x, c1y, c2x, c2y, x1, y1 = item
                p0 = (round(x0), round(y0))
                c1 = (round(c1x), round(c1y))
                c2 = (round(c2x), round(c2y))
                p1 = (round(x1), round(y1))
                if on_segment(p0, c1, p1) and on_segment(p0, c2, p1):
                    if p1 != (cx, cy):
                        segments.append(("L", p1))
                else:
                    for _, q1, q2 in to_quads(p0, c1, c2, p1):
                        segments.append(("Q", q1, q2))
                cx, cy = p1
        if segments:
            ops.extend(segments)
    return ops


# ---------------------------------------------------------------------------
# Génération du fichier Rust
# ---------------------------------------------------------------------------

def esc(text):
    return text.replace("\\", "\\\\").replace('"', '\\"')


def main():
    if len(sys.argv) != 3:
        sys.exit("usage : gen-smufl-subset.py <dossier-fonts> <dst.rs>")
    font_dir, destination = sys.argv[1], sys.argv[2]
    font = open(os.path.join(font_dir, "Leland.otf"), "rb").read()
    metadata = json.load(open(os.path.join(font_dir, "leland_metadata.json"), encoding="utf-8"))
    tables = sfnt_tables(font)

    cff_start = tables["CFF "][0]
    names, pos = read_index(font, cff_start + 4)
    names = [name.decode("latin-1") for name in names]
    tops, pos = read_index(font, pos)
    strings, pos = read_index(font, pos)
    strings = [value.decode("latin-1") for value in strings]
    glob_subrs, _ = read_index(font, pos)  # GlobalSubr INDEX
    top = read_dict(tops[0], strings)
    charstrings, _ = read_index(font, cff_start + int(top.get(17, [0])[0]))
    glyph_count = len(charstrings)
    charset_offset = int(top.get(15, [0])[0])
    charstrings_offset = int(top.get(17, [0])[0])
    # CFF Top DICT : l'opérateur Private porte (size, offset) dans cet ordre.
    private_size = int(top.get(18, [0, 0])[0])
    private_offset = int(top.get(18, [0, 0])[1])
    fmt = u8(font, cff_start + charset_offset)
    charset = []
    pos = cff_start + charset_offset + 1
    if fmt == 0:
        for _ in range(glyph_count - 1):
            charset.append(sid_name(u16(font, pos), strings))
            pos += 2
    elif fmt in (1, 2):
        while len(charset) < glyph_count - 1:
            sid = u16(font, pos)
            pos += 2
            n_left = u8(font, pos) if fmt == 1 else u16(font, pos)
            pos += 1 if fmt == 1 else 2
            for offset in range(n_left + 1):
                charset.append(sid_name(sid + offset, strings))
    else:
        raise ValueError("format de charset CFF inconnu : %d" % fmt)
    # Le glyphe 0 (`.notdef`) n'est pas dans le charset CFF.
    glyph_names = [".notdef"] + charset
    assert len(glyph_names) == glyph_count, (len(glyph_names), glyph_count)

    private_start = cff_start + private_offset
    private = read_dict(font[private_start:private_start + private_size], strings)
    local_offset = int(private.get(19, [0])[0])
    local_subrs = []
    if local_offset:
        local_subrs, _ = read_index(font, private_start + local_offset)

    codepoints = read_cmap(font, tables["cmap"][0], glyph_names)
    advances = read_advances(font, tables["hmtx"][0], len(glyph_names))
    bboxes = metadata["glyphBBoxes"]
    anchors_meta = metadata["glyphsWithAnchors"]

    selected = [i for i, name in enumerate(glyph_names) if keep(name)]
    selected.sort(key=lambda i: glyph_names[i])

    edge_lines: list[str] = []
    glyph_lines: list[str] = []
    anchor_lines: list[str] = []
    anchor_names: list[str] = []
    edge_start = 0
    anchor_start = 0
    n_lines = n_quads = 0

    for index in selected:
        name = glyph_names[index]
        outline = Outline(local_subrs, glob_subrs)
        if index >= len(charstrings):
            raise ValueError("charstring absente pour %s" % name)
        outline.run(charstrings[index])
        ops = glyph_edges(outline.contours)
        for op in ops:
            if op[0] == "M":
                CURRENT[0], CURRENT[1] = round(op[1][0]), round(op[1][1])
                edge_lines.append("    Edge::Move(%d, %d)," % (CURRENT[0], CURRENT[1]))
            elif op[0] == "L":
                x, y = round(op[1][0]), round(op[1][1])
                edge_lines.append("    Edge::Line(%d, %d)," % (
                    x - CURRENT[0], y - CURRENT[1]))
                CURRENT[0], CURRENT[1] = x, y
                n_lines += 1
            else:
                q1, q2 = op[1], op[2]
                x, y = round(q2[0]), round(q2[1])
                edge_lines.append("    Edge::Quad(%d, %d, %d, %d)," % (
                    round(q1[0]) - CURRENT[0], round(q1[1]) - CURRENT[1],
                    x - CURRENT[0], y - CURRENT[1]))
                CURRENT[0], CURRENT[1] = x, y
                n_quads += 1
        # remise à zéro du point courant à chaque glyphe
        CURRENT[0] = CURRENT[1] = 0
        edge_len = len(ops)
        for anchor_name, value in sorted(anchors_meta.get(name, {}).items()):
            if anchor_name not in anchor_names:
                anchor_names.append(anchor_name)
            anchor_lines.append("    Anchor { name: %d, x: %d, y: %d }," % (
                anchor_names.index(anchor_name),
                round(value[0] * SPATIUM_UNITS), round(value[1] * SPATIUM_UNITS)))
        box = bboxes.get(name, {"bBoxSW": [0, 0], "bBoxNE": [0, 0]})
        bbox = (
            round(box["bBoxSW"][0] * SPATIUM_UNITS),
            round(box["bBoxSW"][1] * SPATIUM_UNITS),
            round(box["bBoxNE"][0] * SPATIUM_UNITS),
            round(box["bBoxNE"][1] * SPATIUM_UNITS),
        )
        glyph_lines.append(
            '    Glyph { name: "%s", codepoint: 0x%04X, advance: %d, bbox: [%d, %d, %d, %d], '
            'edges: (%d, %d), anchors: (%d, %d) },' % (
                esc(name), codepoints.get(name, 0), advances[index], *bbox,
                edge_start, edge_len, anchor_start,
                len(anchors_meta.get(name, {}))))
        edge_start += edge_len
        anchor_start += len(anchors_meta.get(name, {}))

    defaults = []
    for key, value in sorted(metadata["engravingDefaults"].items()):
        if isinstance(value, (int, float)):
            defaults.append('    Default { name: "%s", value: %d },' % (
                esc(key), round(value * 1024)))

    header = '''// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Données de `fonts/leland/` de MuseScore Studio 4.7.4, Copyright (C)
// MuseScore BVBA and Klaus Rettinghaus, sous SIL Open Font License 1.1,
// converties en Rust (fichier généré) en 2026.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Sous-ensemble SMuFL Leland — **fichier généré, ne pas éditer à la main**.
//!
//! Régénéré par `python3 -I scripts/gen-smufl-subset.py
//! crates/scoretk-smufl/fonts/leland crates/scoretk-smufl/src/glyphs.rs`.
//! Coordonnées en font units (em = 1000, une portée = 250 unités) ; les
//! `Default` sont en fixed point 1/1024 de portée.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub name: &'static str,
    pub codepoint: u16,
    pub advance: i16,
    pub bbox: [i16; 4],
    /// (première arête, nombre d'arêtes) dans `EDGES`.
    pub edges: (u16, u16),
    /// (première ancre, nombre d'ancres) dans `ANCHORS`.
    pub anchors: (u16, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Déplacement absolu (début d'un contour).
    Move(i16, i16),
    /// Droite relative au point courant.
    Line(i16, i16),
    /// Quadratique : contrôle et extrémité relatifs au point courant.
    Quad(i16, i16, i16, i16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub name: u8,
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Default {
    pub name: &'static str,
    /// Valeur en fixed point 1/1024 de portée.
    pub value: i16,
}

'''
    with open(destination, "w", encoding="utf-8") as out:
        out.write(header)
        out.write("pub const UPEM: i16 = %d;\n" % UPEM)
        out.write("pub const SPATIUM_UNITS: i16 = %d;\n" % SPATIUM_UNITS)
        out.write("#[rustfmt::skip]\n")
        out.write("pub const GLYPHS: [Glyph; %d] = [\n" % len(selected))
        out.write("\n".join(glyph_lines) + "\n];\n\n")
        out.write("#[allow(clippy::large_const_arrays)]\n")
        out.write("#[rustfmt::skip]\n")
        out.write("pub const EDGES: [Edge; %d] = [\n" % len(edge_lines))
        out.write("\n".join(edge_lines) + "\n];\n\n")
        out.write("#[rustfmt::skip]\n")
        out.write("pub const ANCHORS: [Anchor; %d] = [\n" % len(anchor_lines))
        out.write("\n".join(anchor_lines) + "\n];\n\n")
        out.write("#[rustfmt::skip]\n")
        out.write("pub const ANCHOR_NAMES: [&str; %d] = [\n" % len(anchor_names))
        out.write("\n".join('    "%s",' % esc(n) for n in anchor_names) + "\n];\n\n")
        out.write("#[rustfmt::skip]\n")
        out.write("pub const DEFAULTS: [Default; %d] = [\n" % len(defaults))
        out.write("\n".join(defaults) + "\n];\n")
    print("glyphes : %d, arêtes : %d (%d quadratiques, %d droites), "
          "ancres : %d (%d noms), defaults : %d" % (
              len(selected), len(edge_lines), n_quads, n_lines,
              len(anchor_lines), len(anchor_names), len(defaults)))


CURRENT = [0, 0]

if __name__ == "__main__":
    main()
