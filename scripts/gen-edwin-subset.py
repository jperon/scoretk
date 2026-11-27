#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Données de `fonts/edwin/` de MuseScore Studio 4.7.4 (tag v4.7.4, commit
# 7688c005ad963ba09ee715aae53dbc7e8c9d5ef8), Copyright (C) MuseScore Limited,
# sous SIL Open Font License 1.1, converties en Rust (fichier généré) en 2026.
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

"""Génère le sous-ensemble de police de texte Edwin (fichier généré).

Usage : python3 -I scripts/gen-edwin-subset.py <dossier-fonts> <dst.rs>

`<dossier-fonts>` contient `Edwin-Roman.otf` et `Edwin-Bold.otf` tels que
livrés par MuseScore 4.7.4 dans `fonts/edwin/`. Comme `gen-smufl-subset.py`,
le script n'utilise que la bibliothèque standard Python : il réutilise son
lecteur CFF/Type2 (cmap, hmtx, charstrings) et sa conversion cubique →
quadratique à 0.5 font unit. Seuls les codepoints réellement utilisés par les
textes du corpus (ASCII étendu et quelques signes typographiques) sont
conservés. La police n'est jamais exécutée, seulement lue.
"""

from __future__ import annotations

import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SPEC = importlib.util.spec_from_file_location(
    "gen_smufl_subset", os.path.join(HERE, "gen-smufl-subset.py")
)
GEN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEN)

# ASCII imprimable, espace insécable, guillemets et tireurs typographiques,
# signes trouvés dans les paroles/acords du corpus.
CODEPOINTS = frozenset(
    list(range(0x20, 0x7F))
    + [
        0xA0, 0xA9, 0xAB, 0xBB, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6,
        0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1,
        0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD,
        0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8,
        0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3,
        0xF4, 0xF5, 0xF6, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
        0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026, 0x266D,
    ]
)


def parse_font(path: str) -> dict:
    """Lit une OTF CFF : (codepoint → avance, contours, arêtes)."""
    font = open(path, "rb").read()
    tables = GEN.sfnt_tables(font)
    cff = tables["CFF "][0]
    names, pos = GEN.read_index(font, cff + 4)
    names = [name.decode("latin-1") for name in names]
    tops, pos = GEN.read_index(font, pos)
    strings, pos = GEN.read_index(font, pos)
    strings = [value.decode("latin-1") for value in strings]
    glob_subrs, _ = GEN.read_index(font, pos)
    top = GEN.read_dict(tops[0], strings)
    charstrings, _ = GEN.read_index(font, cff + int(top.get(17, [0])[0]))
    count = len(charstrings)
    charset_offset = int(top.get(15, [0])[0])
    fmt = GEN.u8(font, cff + charset_offset)
    charset = []
    pos = cff + charset_offset + 1
    if fmt == 0:
        for _ in range(count - 1):
            charset.append(GEN.sid_name(GEN.u16(font, pos), strings))
            pos += 2
    elif fmt in (1, 2):
        while len(charset) < count - 1:
            sid = GEN.u16(font, pos)
            pos += 2
            n_left = GEN.u8(font, pos) if fmt == 1 else GEN.u16(font, pos)
            pos += 1 if fmt == 1 else 2
            charset.extend(
                GEN.sid_name(sid + offset, strings) for offset in range(n_left + 1)
            )
    else:
        raise ValueError("format de charset CFF inconnu : %d" % fmt)
    glyph_names = [".notdef"] + charset
    private_size = int(top.get(18, [0, 0])[0])
    private_offset = int(top.get(18, [0, 0])[1])
    private_start = cff + private_offset
    private = GEN.read_dict(font[private_start:private_start + private_size], strings)
    local_offset = int(private.get(19, [0])[0])
    local_subrs = []
    if local_offset:
        local_subrs, _ = GEN.read_index(font, private_start + local_offset)
    by_name = GEN.read_cmap(font, tables["cmap"][0], glyph_names)
    reverse = {code: name for name, code in by_name.items()}
    advances = GEN.read_advances(font, tables["hmtx"][0], len(glyph_names))

    result = {}
    for cp in sorted(CODEPOINTS):
        name = reverse.get(cp)
        if name is None or name not in by_name:
            continue
        index = glyph_names.index(name)
        outline = GEN.Outline(local_subrs, glob_subrs)
        outline.run(charstrings[index])
        edges = GEN.glyph_edges(outline.contours)
        points = []
        for contour in outline.contours:
            points.extend((item[1], item[2]) for item in contour)
        for item in edges:
            if item[0] == "M":
                points.append(tuple(item[1]))
            else:
                points.append(tuple(item[-1]))
        if not points:
            continue
        bbox = (
            round(min(point[0] for point in points)),
            round(min(point[1] for point in points)),
            round(max(point[0] for point in points)),
            round(max(point[1] for point in points)),
        )
        result[cp] = (advances[index], bbox, edges)
    return result


def esc(text):
    return text.replace("\\", "\\\\").replace('"', '\\"')


def main():
    if len(sys.argv) != 3:
        sys.exit("usage : gen-edwin-subset.py <dossier-fonts> <dst.rs>")
    font_dir, destination = sys.argv[1], sys.argv[2]
    roman = parse_font(os.path.join(font_dir, "Edwin-Roman.otf"))
    bold = parse_font(os.path.join(font_dir, "Edwin-Bold.otf"))

    edge_lines = []
    edge_start = 0

    def emit(style):
        nonlocal edge_start
        lines = []
        for cp, (advance, bbox, edges) in style.items():
            start = edge_start
            cx = cy = 0
            for op in edges:
                if op[0] == "M":
                    cx, cy = round(op[1][0]), round(op[1][1])
                    edge_lines.append("    Edge::Move(%d, %d)," % (cx, cy))
                elif op[0] == "L":
                    x, y = round(op[1][0]), round(op[1][1])
                    edge_lines.append("    Edge::Line(%d, %d)," % (x - cx, y - cy))
                    cx, cy = x, y
                else:
                    q1, q2 = op[1], op[2]
                    qx, qy = round(q1[0]), round(q1[1])
                    x, y = round(q2[0]), round(q2[1])
                    edge_lines.append(
                        "    Edge::Quad(%d, %d, %d, %d),"
                        % (qx - cx, qy - cy, x - cx, y - cy)
                    )
                    cx, cy = x, y
            edge_start += len(edges)
            lines.append(
                '    TextGlyph { codepoint: 0x%04X, advance: %d, bbox: [%d, %d, %d, %d], '
                'edges: (%d, %d) },'
                % (cp, advance, *bbox, start, len(edges))
            )
        return lines

    roman_lines = emit(roman)
    bold_lines = emit(bold)

    header = '''// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Données de `fonts/edwin/` de MuseScore Studio 4.7.4, Copyright (C)
// MuseScore Limited, sous SIL Open Font License 1.1, converties en Rust
// (fichier généré) en 2026.
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

//! Sous-ensemble de police de texte Edwin — **fichier généré, ne pas éditer à
//! la main**.
//!
//! Régénéré par `python3 -I scripts/gen-edwin-subset.py
//! crates/scoretk-smufl/fonts/edwin crates/scoretk-smufl/src/text.rs`.
//! Coordonnées en font units (em = 1000, une portée = 250 unités), telles que
//! `scoretk_smufl::Edge` les interprète pour la police Leland.

use crate::Edge;

/// Un glyphe de texte Edwin : avance, boîte et arêtes dans `TEXT_EDGES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextGlyph {
    pub codepoint: u16,
    /// Avance horizontale en font units.
    pub advance: i16,
    /// Boîte englobante en font units (x_min, y_min, x_max, y_max).
    pub bbox: [i16; 4],
    /// (première arête, nombre d'arêtes) dans `TEXT_EDGES`.
    pub edges: (u16, u16),
}

'''
    with open(destination, "w", encoding="utf-8") as out:
        out.write(header)
        out.write("#[rustfmt::skip]\n")
        out.write("pub const EDWIN_ROMAN: [TextGlyph; %d] = [\n" % len(roman))
        out.write("\n".join(roman_lines) + "\n];\n\n")
        out.write("#[rustfmt::skip]\n")
        out.write("pub const EDWIN_BOLD: [TextGlyph; %d] = [\n" % len(bold))
        out.write("\n".join(bold_lines) + "\n];\n\n")
        out.write("#[allow(clippy::large_const_arrays)]\n#[rustfmt::skip]\n")
        out.write("pub const TEXT_EDGES: [Edge; %d] = [\n" % len(edge_lines))
        out.write("\n".join(edge_lines) + "\n];\n")
    print(
        "Edwin : %d glyphes romains, %d gras, %d arêtes"
        % (len(roman), len(bold), len(edge_lines))
    )


if __name__ == "__main__":
    main()
