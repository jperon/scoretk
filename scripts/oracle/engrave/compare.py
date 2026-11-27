#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
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

"""Compare engraving SVGs from MuseScore 4.7.4 and scoretk, in staff spaces.

Only the Python standard library is used. Both SVGs are read as untrusted
data: no script or external entity from them is executed (ElementTree does not
resolve external entities). Each document's own staff lines define its scale,
so the comparison is independent of page size and coordinate convention.

Usage: python3 -I scripts/oracle/engrave/compare.py <out>
"""

from __future__ import annotations

import json
import re
import statistics
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

SVG = "{http://www.w3.org/2000/svg}"
NUMBERS = re.compile(r"-?\d+(?:\.\d+)?")
TOL = 0.1


def pairs(text: str) -> list[tuple[float, float]]:
    values = [float(value) for value in NUMBERS.findall(text)]
    return [(values[i], values[i + 1]) for i in range(0, len(values) - 1, 2)]


def bbox_of_path(data: str) -> tuple[float, float, float, float] | None:
    """Bounding box from all coordinate pairs of an SVG path."""
    points = pairs(data)
    if not points:
        return None
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return min(xs), min(ys), max(xs), max(ys)

def transform_of(element: ET.Element) -> tuple[float, float, float, float]:
    """(scale_x, scale_y, translate_x, translate_y) of a path transform."""
    transform = element.get("transform", "")
    values = [float(value) for value in NUMBERS.findall(transform)]
    if transform.startswith("matrix") and len(values) >= 6:
        return values[0], values[3], values[4], values[5]
    if transform.startswith("translate") and len(values) >= 2:
        return 1.0, 1.0, values[0], values[1]
    return 1.0, 1.0, 0.0, 0.0


def staff_lines(elements: list[ET.Element]) -> list[tuple[float, float, float, float]]:
    """Staff-line segments: (x_left, x_right, y, line_width)."""
    lines: list[tuple[float, float, float, float]] = []
    for element in elements:
        if "StaffLines" not in element.get("class", ""):
            continue
        width = float(element.get("stroke-width", 0))
        if element.tag == SVG + "polyline":
            for x, y in pairs(element.get("points", "")):
                lines.append((x, x, y, width))
        elif element.tag == SVG + "rect":
            value = element.get("y")
            if value is not None:
                # scoretk draws staff lines as rectangles centred on the
                # staff line; use the centre, not the top edge.
                centre = float(value) + float(element.get("height", 0)) / 2
                x = float(element.get("x", 0))
                lines.append((x, x + float(element.get("width", 0)), centre, width))
    return lines


def staff_geometry(elements: list[ET.Element]) -> tuple[float, float, float]:
    """(origin_x, origin_y, spatium) from the staff-line elements.

    With several systems on a page the median of all consecutive y gaps is
    still the spatium: intra-staff gaps (one sp) outnumber inter-system ones.
    """
    lines = staff_lines(elements)
    ys = sorted({round(line[2], 3) for line in lines})
    spatium = statistics.median(b - a for a, b in zip(ys, ys[1:])) if len(ys) > 1 else 1.0
    if spatium <= 0:
        spatium = 1.0
    origin_x = min((line[0] for line in lines), default=0.0)
    origin_y = min(ys, default=0.0)
    return origin_x, origin_y, spatium


def staff_systems(elements: list[ET.Element], spatium: float) -> list[tuple[float, float, float]]:
    """(x_left, x_right, top_line_y) for each five-line staff system.

    // Port of MuseScore's `System` staff geometry: one system holds five
    lines one spatium apart. Grouping consecutive staff lines by a gap
    smaller than two spatia recovers the systems without reading any layout
    metadata from either SVG.
    """
    groups: list[list[float]] = []
    for y in sorted({round(line[2], 3) for line in staff_lines(elements)}):
        if groups and y - groups[-1][-1] < spatium * 1.6:
            groups[-1].append(y)
        else:
            groups.append([y])
    systems = []
    for group in groups:
        if len(group) < 4:
            continue
        in_group = [line for line in staff_lines(elements)
                    if min(group) - 0.01 <= line[2] <= max(group) + 0.01]
        if in_group:
            systems.append((min(line[0] for line in in_group),
                            max(line[1] for line in in_group), min(group)))
    return systems


def local_position(x: float, y: float, systems: list[tuple[float, float, float]],
                   spatium: float) -> tuple[float, float] | None:
    """Position relative to the staff system that contains an element.

    Vertical comparisons must be local to a system: comparing a note on
    system 3 with one on system 1 measures the system distance, not pitch.
    """
    best = None
    for x0, x1, top in systems:
        if not x0 - 3 * spatium <= x <= x1 + 3 * spatium:
            continue
        distance = abs(y - (top + 2 * spatium))
        if best is None or distance < best[0]:
            best = (distance, x0, top)
    if best is None:
        return None
    return (x - best[1]) / spatium, (y - best[2]) / spatium


def parse_svg(path: Path) -> dict:
    root = ET.parse(path).getroot()
    elements = list(root.iter())
    _, _, spatium = staff_geometry(elements)
    scale = 1.0 / spatium
    systems = staff_systems(elements, spatium)

    result: dict[str, list[tuple[float, float]]] = {"Note": [], "Stem": [], "BarLine": []}
    viewbox = root.get("viewBox", "0 0 0 0").replace(",", " ").split()
    result["page_width"] = float(viewbox[2]) if len(viewbox) > 2 else 0.0
    # Bar lines keep their absolute y here: local y repeats on every system,
    # so grouping bar lines by local y would merge all systems into one.
    raw_barlines: list[tuple[float, float, float]] = []
    barline_segments_abs: list[tuple[float, float, float]] = []
    lengths: list[float] = []
    for element in elements:
        class_name = element.get("class", "")
        tag = element.tag
        position = None
        if class_name == "Note" and tag == SVG + "path":
            box = bbox_of_path(element.get("d", ""))
            if box is None:
                continue
            sx, sy, tx, ty = transform_of(element)
            # Both engines' noteheads are compared at their bounding-box
            # centre: MuseScore emits an centred outline, scoretk translates
            # the SMuFL outline, whose origin is the head's left edge.
            center = ((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
            position = (sx * center[0] + tx, sy * center[1] + ty)
        elif class_name in ("Stem", "BarLine"):
            if tag == SVG + "rect":
                x = float(element.get("x", 0)) + float(element.get("width", 0)) / 2
                y0 = float(element.get("y", 0))
                y1 = y0 + float(element.get("height", 0))
            elif tag == SVG + "polyline":
                pts = pairs(element.get("points", ""))
                if not pts:
                    continue
                x = sum(point[0] for point in pts) / len(pts)
                y0 = min(point[1] for point in pts)
                y1 = max(point[1] for point in pts)
            else:
                continue
            position = (x, (y0 + y1) / 2)
        if position is not None:
            local = local_position(*position, systems, spatium)
            if local is not None:
                result[class_name].append(local)
                if class_name == "Stem":
                    lengths.append(abs(y1 - y0) * scale)
                if class_name == "BarLine":
                    raw_barlines.append((position[1], local[0], local[1]))
                    barline_segments_abs.append((position[0], y0, y1))
    result["stem_lengths"] = lengths
    result["spatium"] = spatium
    # Absolute notehead boxes and bar-line positions, used by the horizontal
    # collision check: a head (or an accidental/dot attached to it) that
    # crosses the next bar line is counted on both sides of the comparison.
    result["note_boxes_abs"] = []
    for element in elements:
        if element.get("class", "") != "Note" or element.tag != SVG + "path":
            continue
        box = bbox_of_path(element.get("d", ""))
        if box is None:
            continue
        sx, sy, tx, ty = transform_of(element)
        result["note_boxes_abs"].append(
            (sx * box[0] + tx, sy * box[1] + ty, sx * box[2] + tx, sy * box[3] + ty)
        )
    result["barlines_abs"] = [(x, y) for y, x, _ in raw_barlines]
    result["barline_segments_abs"] = barline_segments_abs
    # Notehead/notehead bbox collisions. Unisons deliberately share one head
    # centre in MuseScore, so coincident centres are not counted. Heads on
    # adjacent staff lines touch by about 0.06 sp in both engines; only an
    # overlap beyond 0.2 sp in both axes is counted as a defect.
    boxes = result["note_boxes_abs"]
    result["note_overlaps"] = sum(
        1
        for i, (ax0, ay0, ax1, ay1) in enumerate(boxes)
        for bx0, by0, bx1, by1 in boxes[i + 1:]
        if min(ax1, bx1) - max(ax0, bx0) > 0.2 * spatium
        and min(ay1, by1) - max(ay0, by0) > 0.2 * spatium
        and abs((ax0 + ax1) - (bx0 + bx1)) > 0.05 * spatium
    )
    # Lyrics are paths in both SVGs; their bounding box is used to detect
    # collisions with a staff's five-line band (the user-visible defect of
    # step 2). Elements that fail to parse are not silently dropped: the
    # element-level matcher below still counts every expected element.
    result["lyric_boxes"] = []
    result["lyric_baselines"] = []
    result["instrument_name_boxes"] = []
    for element in elements:
        class_name = element.get("class", "")
        if class_name not in ("Lyrics", "InstrumentName") or element.tag != SVG + "path":
            continue
        box = bbox_of_path(element.get("d", ""))
        if box is None:
            continue
        sx, sy, tx, ty = transform_of(element)
        target = (
            result["lyric_boxes"] if class_name == "Lyrics" else result["instrument_name_boxes"]
        )
        target.append(
            (sx * box[0] + tx, sy * box[1] + ty, sx * box[2] + tx, sy * box[3] + ty)
        )
        if class_name == "Lyrics":
            # MuseScore writes one path per syllable with no transform; in
            # that case the path's bbox bottom is its baseline. scoretk
            # writes one path per glyph with the baseline in the transform.
            result["lyric_baselines"].append(ty if "transform" in element.attrib else box[3])
    result["system_boxes"] = systems
    # Top line of every five-line staff system, in staff spaces from the
    # page top: the P5 oracle measures page breaks and vertical positions.
    result["systems"] = [system[2] / spatium for system in systems]
    # A multi-staff system shares one x extent: group its five-line staves
    # together so the gaps between adjacent staves can be measured. Staves
    # are five lines one spatium apart (4 sp tall, `STAFF_HEIGHT`). As for
    # systems, a missing or unmatched staff remains in the denominator.
    groups: dict[tuple[float, float], list[float]] = {}
    for x0, x1, top in systems:
        key = (round(x0 / spatium, 1), round(x1 / spatium, 1))
        groups.setdefault(key, []).append(top / spatium)
    result["staff_groups"] = sorted(groups.values(), key=lambda tops: min(tops))
    result["staff_gaps"] = [
        round(top - previous - 4.0, 4)
        for tops in result["staff_groups"]
        for previous, top in zip(tops, tops[1:])
    ]
    # One list of bar-line x positions per staff system, in staff spaces.
    # Consecutive positions delimit a measure; the count per system is the
    # system break pattern. MuseScore draws a double/final bar line as two
    # strokes about 0.6-1.1 sp apart (thin + thick) and scoretk as two
    # rectangles 0.37 sp apart: both are ONE measure boundary, so lines less
    # than 1.5 sp apart are merged at their leftmost stroke (MuseScore draws
    # the thin stroke at the measure's right edge and the thick one after
    # it, `tdraw.cpp:752`). A plain
    # start bar line is not counted either: MuseScore does not draw one at
    # the beginning of a normal system, and counting scoretk's would shift
    # every measure width by one boundary.
    by_system: dict[int, list[float]] = {}
    for absolute_y, local_x, local_y in sorted(raw_barlines):
        # A bar line of a grand staff spans several staves, so its centre y
        # is not on a staff: assign it to the nearest staff system instead
        # of grouping by an absolute y that would create phantom systems.
        key = min(
            range(len(systems)),
            key=lambda index: abs(absolute_y - (systems[index][2] + 2 * spatium)),
            default=0,
        ) if systems else 0
        group = by_system.setdefault(key, [])
        if not group or local_x - group[-1] > 1.5:
            group.append(local_x)
    # Consecutive system groups must be in reading order (top to bottom).
    result["barlines_by_system"] = [by_system[key] for key in sorted(by_system)]
    result["measure_widths"] = [
        round(right - left, 4)
        for bars in result["barlines_by_system"]
        for left, right in zip(bars, bars[1:])
    ]
    return result


def verse_overlaps(parsed: dict) -> int:
    """Pairs of different lyric verse lines whose vertical bands overlap.

    A verse is one baseline; two verses must be separated by the font's line
    height. Each glyph is attached to its baseline (within 0.25 sp), the
    band of a verse is the union of its glyph boxes, and two bands count as
    superposed when their baselines differ by more than 0.6 sp and their y
    ranges overlap by more than 0.1 sp. This catches verses drawn on one
    line even when their syllables never share the same x.
    """
    spatium = parsed.get("spatium", 1.0)
    boxes = parsed.get("lyric_boxes", [])
    baselines = parsed.get("lyric_baselines", [])
    lines: list[tuple[float, float, float]] = []
    for baseline, (_x0, y0, _x1, y1) in zip(baselines, boxes):
        for index, (line_base, top, bottom) in enumerate(lines):
            if abs(baseline - line_base) <= 0.25 * spatium:
                lines[index] = (line_base, min(top, y0), max(bottom, y1))
                break
        else:
            lines.append((baseline, y0, y1))
    return sum(
        1
        for index, (base_a, top_a, bottom_a) in enumerate(lines)
        for base_b, top_b, bottom_b in lines[index + 1:]
        if abs(base_a - base_b) > 0.6 * spatium
        and min(bottom_a, bottom_b) - max(top_a, top_b) > 0.1 * spatium
    )


def instrument_name_clips(parsed: dict) -> tuple[int, int]:
    """(instrument-name glyphs, glyphs starting left of the page margin).

    The staff lines end at the printable area's right edge, so the page's
    left margin is the viewBox width minus that edge. A name starting further
    left is clipped by the printable area.
    """
    spatium = parsed.get("spatium", 1.0)
    systems = parsed.get("system_boxes", [])
    boxes = parsed.get("instrument_name_boxes", [])
    if not systems:
        return len(boxes), 0
    margin = parsed.get("page_width", 0.0) - max(x1 for _x0, x1, _top in systems)
    return len(boxes), sum(x0 < margin - 0.1 * spatium for x0, _y0, _x1, _y1 in boxes)


def barline_spread(parsed: dict) -> tuple[float, int, int]:
    """Bar-line x disagreement between the staves of one system.

    A MuseScore `Segment` is shared by every staff of a system: all staves must
    draw a boundary at the same x (`horizontalspacing.cpp:47-89`,
    `segment.cpp:2640-2697`). Bar lines are grouped by the five-line staff they
    cross, then compared ordinal by ordinal inside each system (staves of a
    system share their left x). Double strokes within 1.5 sp are one boundary.

    Returns (largest spread in staff spaces, boundaries with a spread above
    0.05 sp, boundaries compared).
    """
    spatium = parsed.get("spatium", 1.0)
    systems = parsed.get("system_boxes", [])
    segments = parsed.get("barline_segments_abs", [])
    buckets: dict[tuple[float, float], list[tuple[float, list[float]]]] = {}
    for x0, x1, top in systems:
        # Staves of one system share both edges of their staff lines; systems
        # below them share only the left edge. Grouping by the left edge
        # alone would merge every system of a page into one phantom system.
        key = (round(x0 / spatium, 1), round(x1 / spatium, 1))
        xs: list[float] = []
        for x, y0, y1 in sorted(segments):
            if y1 <= top - spatium / 2 or y0 >= top + 4.5 * spatium:
                continue
            if not xs or x - xs[-1] > 1.5 * spatium:
                xs.append(x)
        buckets.setdefault(key, []).append((top, xs))

    def agreement(first: list[float], second: list[float]) -> float:
        if not first or not second:
            return 0.0
        matched = sum(
            1 for x in first if any(abs(x - other) <= 2 * spatium for other in second)
        )
        return matched / len(first)

    spreads: list[float] = []
    for staves in buckets.values():
        systems_found: list[list[tuple[float, list[float]]]] = []
        for top, xs in sorted(staves):
            for group in systems_found:
                if (
                    top - group[0][0] <= 25 * spatium
                    and agreement(xs, group[0][1]) >= 0.6
                    and agreement(group[0][1], xs) >= 0.6
                ):
                    group.append((top, xs))
                    break
            else:
                systems_found.append([(top, xs)])
        for group in systems_found:
            if len(group) < 2:
                continue
            for boundary in group[0][1]:
                positions = []
                for _top, staff in group:
                    nearest = min(
                        (abs(x - boundary) for x in staff), default=3 * spatium
                    )
                    positions.append(nearest / spatium)
                spreads.append(max(positions))
    return (
        max(spreads, default=0.0),
        sum(spread > 0.05 for spread in spreads),
        len(spreads),
    )


def pair_by_nearest(left: list, right: list, key) -> list[tuple[int | None, int | None]]:
    """One-to-one greedy matching by a distance key, as index pairs.

    Surplus elements get a `None` index so they count as failures instead of
    being silently dropped. Indices rather than values are returned because
    noteheads at the same y are common.
    """
    used: set[int] = set()
    pairs: list[tuple[int | None, int | None]] = []
    for left_index, item in enumerate(left):
        best = min(
            (index for index in range(len(right)) if index not in used),
            key=lambda index: key(item, right[index]),
            default=None,
        )
        if best is None:
            pairs.append((left_index, None))
        else:
            used.add(best)
            pairs.append((left_index, best))
    pairs += [(None, index) for index in range(len(right)) if index not in used]
    return pairs


def page_numbers(path: Path) -> list[int]:
    """Page numbers of `<id>-N.svg` files, in reading order."""
    numbers = []
    for path in path.glob("*-*.svg"):
        stem = path.stem.rsplit("-", 1)[-1]
        if stem.isdigit():
            numbers.append(int(stem))
    return sorted(numbers)


def pages(reference: list[Path], ours: list[Path]) -> dict:
    """Page count, page breaks and staff y positions of a whole score.

    Missing pages and unmatched systems are counted in the denominators:
    a file that fails to engrave cannot improve its own figures.
    """
    same_count = len(reference) == len(ours)
    per_page_equal = 0
    y_total = 0
    y_within = 0
    y_error = 0.0
    y_matched = 0
    gap_total = 0
    gap_within = 0
    gap_error = 0.0
    gap_matched = 0
    gap_ref_sum = 0.0
    gap_ours_sum = 0.0
    overlaps_ref = 0
    overlaps_ours = 0
    lyrics_ref = 0
    lyrics_ours = 0
    lyric_hits_ref = 0
    lyric_hits_ours = 0
    barline_hits_ref = 0
    barline_hits_ours = 0
    spread_max_ref = 0.0
    spread_max_ours = 0.0
    spread_bad_ref = 0
    spread_bad_ours = 0
    spread_total_ref = 0
    spread_total_ours = 0
    names_ref = 0
    names_ours = 0
    name_clips_ref = 0
    name_clips_ours = 0
    verse_hits_ref = 0
    verse_hits_ours = 0
    note_hits_ref = 0
    note_hits_ours = 0

    def barline_crossings(parsed: dict) -> int:
        """Noteheads whose right edge passes the next bar line of their staff.

        Only heads on the same five-line staff as the bar line are compared;
        a bar line of a grand staff is assigned to its nearest staff system.
        """
        spatium = parsed.get("spatium", 1.0)
        systems = parsed.get("system_boxes", [])
        bars = parsed.get("barlines_abs", [])
        hits = 0
        for nx0, ny0, nx1, ny1 in parsed.get("note_boxes_abs", []):
            for x0, x1, top in systems:
                if not (ny1 > top and ny0 < top + 4 * spatium):
                    continue
                hits += sum(
                    1 for bx, by in bars
                    if abs(by - (top + 2 * spatium)) < 2 * spatium
                    and bx >= (nx0 + nx1) / 2 - spatium / 2
                    and bx < nx1 - spatium / 4
                )
        return hits

    def note_collisions(parsed: dict) -> int:
        return parsed.get("note_overlaps", 0)

    def lyric_collisions(parsed: dict) -> tuple[int, int]:
        """(number of lyric glyphs, number intersecting a staff band)."""
        boxes = parsed.get("lyric_boxes", [])
        spatium = parsed.get("spatium", 1.0)
        systems = parsed.get("system_boxes", [])
        hits = 0
        for lx0, ly0, lx1, ly1 in boxes:
            if any(
                lx1 > x0 and lx0 < x1 and ly1 > top and ly0 < top + 4 * spatium
                for x0, x1, top in systems
            ):
                hits += 1
        return len(boxes), hits

    def vertical(ref: dict, our: dict) -> None:
        nonlocal y_total, y_within, y_error, y_matched
        nonlocal gap_total, gap_within, gap_error, gap_matched
        nonlocal gap_ref_sum, gap_ours_sum, overlaps_ref, overlaps_ours
        nonlocal lyrics_ref, lyrics_ours, lyric_hits_ref, lyric_hits_ours
        nonlocal barline_hits_ref, barline_hits_ours
        nonlocal spread_max_ref, spread_max_ours, spread_bad_ref, spread_bad_ours
        nonlocal spread_total_ref, spread_total_ours
        nonlocal names_ref, names_ours, name_clips_ref, name_clips_ours
        nonlocal verse_hits_ref, verse_hits_ours
        nonlocal note_hits_ref, note_hits_ours
        barline_hits_ref += barline_crossings(ref)
        barline_hits_ours += barline_crossings(our)
        ref_max, ref_bad, ref_total = barline_spread(ref)
        our_max, our_bad, our_total = barline_spread(our)
        spread_max_ref = max(spread_max_ref, ref_max)
        spread_max_ours = max(spread_max_ours, our_max)
        spread_bad_ref += ref_bad
        spread_bad_ours += our_bad
        spread_total_ref += ref_total
        spread_total_ours += our_total
        ref_names, ref_clips = instrument_name_clips(ref)
        our_names, our_clips = instrument_name_clips(our)
        names_ref += ref_names
        names_ours += our_names
        name_clips_ref += ref_clips
        name_clips_ours += our_clips
        verse_hits_ref += verse_overlaps(ref)
        verse_hits_ours += verse_overlaps(our)
        note_hits_ref += note_collisions(ref)
        note_hits_ours += note_collisions(our)
        ref_lyrics, ref_hits = lyric_collisions(ref)
        our_lyrics, our_hits = lyric_collisions(our)
        lyrics_ref += ref_lyrics
        lyrics_ours += our_lyrics
        lyric_hits_ref += ref_hits
        lyric_hits_ours += our_hits
        ref_systems = ref["systems"]
        our_systems = our["systems"]
        y_total += abs(len(ref_systems) - len(our_systems))
        for top_ref, top_our in zip(ref_systems, our_systems):
            y_total += 1
            difference = abs(top_ref - top_our)
            y_error += difference
            y_matched += 1
            if difference <= 0.5:
                y_within += 1
        ref_gaps = ref["staff_gaps"]
        our_gaps = our["staff_gaps"]
        # Unmatched adjacent-staff pairs count as failures, not as absent
        # data: a page that fails to engrave cannot improve its own figures.
        gap_total += abs(len(ref_gaps) - len(our_gaps))
        for gap_ref, gap_our in zip(ref_gaps, our_gaps):
            gap_total += 1
            difference = abs(gap_ref - gap_our)
            gap_error += difference
            gap_matched += 1
            if difference <= 0.5:
                gap_within += 1
        gap_ref_sum += sum(ref_gaps)
        gap_ours_sum += sum(our_gaps)
        overlaps_ref += sum(gap < 0 for gap in ref_gaps)
        overlaps_ours += sum(gap < 0 for gap in our_gaps)

    for number in range(min(len(reference), len(ours))):
        ref = parse_svg(reference[number])
        our = parse_svg(ours[number])
        per_page_equal += int(len(ref["systems"]) == len(our["systems"]))
        vertical(ref, our)
    # Pages present on only one side count as missing staff positions and
    # missing adjacent-staff gaps: use the other side's counts.
    for extra in reference[len(ours):] + ours[len(reference):]:
        parsed = parse_svg(extra)
        vertical(parsed, {"systems": [], "staff_gaps": []})
        vertical({"systems": [], "staff_gaps": []}, parsed)
    return {
        "page_count_equal": int(same_count),
        "page_count_ref": len(reference),
        "page_count_ours": len(ours),
        "breaks_equal": per_page_equal,
        "staff_y_total": y_total,
        "staff_y_within": y_within,
        "staff_y_error": y_error / y_matched if y_matched else 0.0,
        "staff_y_matched": y_matched,
        "staff_gap_total": gap_total,
        "staff_gap_within": gap_within,
        "staff_gap_error": gap_error / gap_matched if gap_matched else 0.0,
        "staff_gap_matched": gap_matched,
        "staff_gap_ref_mean": gap_ref_sum / gap_total if gap_total else 0.0,
        "staff_gap_ours_mean": gap_ours_sum / gap_total if gap_total else 0.0,
        "overlaps_ref": overlaps_ref,
        "overlaps_ours": overlaps_ours,
        "lyrics_ref": lyrics_ref,
        "lyrics_ours": lyrics_ours,
        "lyric_hits_ref": lyric_hits_ref,
        "lyric_hits_ours": lyric_hits_ours,
        "barline_hits_ref": barline_hits_ref,
        "barline_hits_ours": barline_hits_ours,
        "spread_max_ref": spread_max_ref,
        "spread_max_ours": spread_max_ours,
        "spread_bad_ref": spread_bad_ref,
        "spread_bad_ours": spread_bad_ours,
        "spread_total_ref": spread_total_ref,
        "spread_total_ours": spread_total_ours,
        "names_ref": names_ref,
        "names_ours": names_ours,
        "name_clips_ref": name_clips_ref,
        "name_clips_ours": name_clips_ours,
        "verse_hits_ref": verse_hits_ref,
        "verse_hits_ours": verse_hits_ours,
        "note_hits_ref": note_hits_ref,
        "note_hits_ours": note_hits_ours,
    }


def compare(reference: dict, ours: dict) -> dict:
    counts = {kind: max(len(reference[kind]), len(ours[kind])) for kind in ("Note", "Stem", "BarLine")}
    within = {kind: 0 for kind in counts}
    within_x = {kind: 0 for kind in counts}
    within_y = {kind: 0 for kind in counts}
    # Horizontal spacing figures: the same one-to-one nearest matching as the
    # both-axes figure, so an unmatched notehead is a failure of the x figure
    # too. The mean error is reported on matched pairs, with their count kept
    # next to it rather than silently dropping unmatched notes.
    note_x_within = 0
    note_x_error = 0.0
    note_x_matched = 0
    for kind in counts:
        matched = pair_by_nearest(
            reference[kind], ours[kind],
            lambda a, b: (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2,
        )
        for left_index, right_index in matched:
            if left_index is None or right_index is None:
                continue
            left, right = reference[kind][left_index], ours[kind][right_index]
            dx = abs(left[0] - right[0])
            dy = abs(left[1] - right[1])
            if dx <= TOL:
                within_x[kind] += 1
            if dy <= TOL:
                within_y[kind] += 1
            if dx <= TOL and dy <= TOL:
                within[kind] += 1
            if kind == "Note":
                note_x_matched += 1
                note_x_error += dx
                if dx <= TOL:
                    note_x_within += 1

    # Vertical figures use y-only matching: horizontal spacing is fixed until
    # P2 and must not perturb the measurement of pitch/ledger/stem length.
    note_pairs = pair_by_nearest(
        reference["Note"], ours["Note"], lambda a, b: abs(a[1] - b[1])
    )
    matched_notes = sum(a is not None and b is not None for a, b in note_pairs)
    vertical = sum(
        a is not None and b is not None
        and abs(reference["Note"][a][1] - ours["Note"][b][1]) <= TOL
        for a, b in note_pairs
    )
    stem_pairs = pair_by_nearest(
        reference["Stem"], ours["Stem"], lambda a, b: abs(a[1] - b[1])
    )
    ref_lengths = reference["stem_lengths"]
    our_lengths = ours["stem_lengths"]
    matched_stems = 0
    stem_length = 0
    for left_index, right_index in stem_pairs:
        if left_index is None or right_index is None:
            continue
        if left_index >= len(ref_lengths) or right_index >= len(our_lengths):
            continue
        matched_stems += 1
        if abs(ref_lengths[left_index] - our_lengths[right_index]) <= TOL:
            stem_length += 1
    ref_widths = reference["measure_widths"]
    our_widths = ours["measure_widths"]
    widths_total = max(len(ref_widths), len(our_widths))
    widths_within = sum(
        abs(left - right) <= TOL
        for left, right in zip(ref_widths, our_widths)
    )
    # A measure present on only one side contributes its whole width as an
    # error and fails the tolerance: no missing measure is excluded here.
    width_error = sum(
        abs(left - right) for left, right in zip(ref_widths, our_widths)
    ) + sum(width for width in ref_widths[len(our_widths):]) \
      + sum(width for width in our_widths[len(ref_widths):])
    total = sum(counts.values())
    return {
        "measure_widths_total": widths_total,
        "measure_widths_within": widths_within,
        "measure_width_error": width_error,
        "note_x_within": note_x_within,
        "note_x_error": note_x_error,
        "note_x_matched": note_x_matched,
        "systems_equal": [len(bars) for bars in reference["barlines_by_system"]]
        == [len(bars) for bars in ours["barlines_by_system"]],
        "counts": counts,
        "within": within,
        "within_x": within_x,
        "within_y": within_y,
        "within_total": sum(within.values()),
        "total": total,
        "share_within_0_1_sp": sum(within.values()) / total if total else 0.0,
        "vertical_pitch": vertical / matched_notes if matched_notes else 0.0,
        "stem_length": stem_length / matched_stems if matched_stems else 0.0,
        "matched_notes": matched_notes,
        "matched_stems": matched_stems,
    }


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit("usage : compare.py <out>")
    out = Path(sys.argv[1])
    corpus = (out / "lots/corpus.txt").read_text(encoding="utf-8").splitlines()
    rows = []
    totals = {
        "files": 0, "failures": 0, "total": 0, "within": 0,
        "matched_notes": 0, "vertical": 0.0, "matched_stems": 0,
        "stem": 0.0, "x": 0, "y": 0, "widths": 0, "widths_total": 0,
        "width_error": 0.0,
        "note_x": 0, "note_x_error": 0.0, "note_x_matched": 0,
        "systems_equal": 0,
        "page_count": 0, "breaks": 0, "pages_total": 0,
        "staff_y": 0, "staff_y_total": 0, "staff_y_error": 0.0, "staff_y_matched": 0,
        "staff_gap": 0, "staff_gap_total": 0, "staff_gap_error": 0.0,
        "staff_gap_matched": 0, "staff_gap_ref": 0.0, "staff_gap_ours": 0.0,
        "overlaps_ref": 0, "overlaps_ours": 0,
        "lyrics_ref": 0, "lyrics_ours": 0,
        "lyric_hits_ref": 0, "lyric_hits_ours": 0,
        "barline_hits_ref": 0, "barline_hits_ours": 0,
        "spread_max_ref": 0.0, "spread_max_ours": 0.0,
        "spread_bad_ref": 0, "spread_bad_ours": 0,
        "spread_total_ref": 0, "spread_total_ours": 0,
        "names_ref": 0, "names_ours": 0, "name_clips_ref": 0, "name_clips_ours": 0,
        "verse_hits_ref": 0, "verse_hits_ours": 0,
        "note_hits_ref": 0, "note_hits_ours": 0,
    }
    for source in corpus:
        if not source.strip():
            continue
        source_path = Path(source)
        identifier = source_path.stem
        ref_pages = sorted(
            (out / "refs").glob(f"{identifier}-*.svg"),
            key=lambda path: int(path.stem.rsplit("-", 1)[-1]),
        )
        our_pages = sorted(
            (out / "ours").glob(f"{identifier}-*.svg"),
            key=lambda path: int(path.stem.rsplit("-", 1)[-1]),
        )
        ours = out / "ours" / f"{identifier}.svg"
        row = {"id": identifier}
        if not ref_pages or not our_pages:
            row["error"] = "missing SVG" if not ref_pages else "missing ours"
            rows.append(row)
            totals["failures"] += 1
            continue
        try:
            row["compare"] = compare(parse_svg(ref_pages[0]), parse_svg(our_pages[0]))
            row["pages"] = pages(ref_pages, our_pages)
        except (ET.ParseError, ValueError, OSError) as error:
            row["error"] = f"{type(error).__name__}: {error}"
            rows.append(row)
            totals["failures"] += 1
            continue
        values = row["compare"]
        totals["files"] += 1
        totals["total"] += values["total"]
        totals["within"] += values["within_total"]
        totals["x"] += sum(values["within_x"].values())
        totals["y"] += sum(values["within_y"].values())
        totals["matched_notes"] += values["matched_notes"]
        totals["vertical"] += values["vertical_pitch"] * values["matched_notes"]
        totals["matched_stems"] += values["matched_stems"]
        totals["stem"] += values["stem_length"] * values["matched_stems"]
        totals["widths"] += values["measure_widths_within"]
        totals["widths_total"] += values["measure_widths_total"]
        totals["width_error"] += values["measure_width_error"]
        totals["note_x"] += values["note_x_within"]
        totals["note_x_error"] += values["note_x_error"]
        totals["note_x_matched"] += values["note_x_matched"]
        totals["systems_equal"] += int(values["systems_equal"])
        page_values = row["pages"]
        totals["page_count"] += page_values["page_count_equal"]
        totals["breaks"] += page_values["breaks_equal"]
        totals["pages_total"] += min(
            page_values["page_count_ref"], page_values["page_count_ours"]
        )
        totals["staff_y"] += page_values["staff_y_within"]
        totals["staff_y_total"] += page_values["staff_y_total"]
        totals["staff_y_error"] += page_values["staff_y_error"] * page_values["staff_y_matched"]
        totals["staff_y_matched"] += page_values["staff_y_matched"]
        totals["staff_gap"] += page_values["staff_gap_within"]
        totals["staff_gap_total"] += page_values["staff_gap_total"]
        totals["staff_gap_error"] += page_values["staff_gap_error"] * page_values["staff_gap_matched"]
        totals["staff_gap_matched"] += page_values["staff_gap_matched"]
        totals["staff_gap_ref"] += page_values["staff_gap_ref_mean"] * page_values["staff_gap_total"]
        totals["staff_gap_ours"] += page_values["staff_gap_ours_mean"] * page_values["staff_gap_total"]
        totals["overlaps_ref"] += page_values["overlaps_ref"]
        totals["overlaps_ours"] += page_values["overlaps_ours"]
        totals["lyrics_ref"] += page_values["lyrics_ref"]
        totals["lyrics_ours"] += page_values["lyrics_ours"]
        totals["lyric_hits_ref"] += page_values["lyric_hits_ref"]
        totals["lyric_hits_ours"] += page_values["lyric_hits_ours"]
        totals["barline_hits_ref"] += page_values["barline_hits_ref"]
        totals["barline_hits_ours"] += page_values["barline_hits_ours"]
        totals["spread_max_ref"] = max(totals["spread_max_ref"], page_values["spread_max_ref"])
        totals["spread_max_ours"] = max(totals["spread_max_ours"], page_values["spread_max_ours"])
        totals["spread_bad_ref"] += page_values["spread_bad_ref"]
        totals["spread_bad_ours"] += page_values["spread_bad_ours"]
        totals["spread_total_ref"] += page_values["spread_total_ref"]
        totals["spread_total_ours"] += page_values["spread_total_ours"]
        totals["names_ref"] += page_values["names_ref"]
        totals["names_ours"] += page_values["names_ours"]
        totals["name_clips_ref"] += page_values["name_clips_ref"]
        totals["name_clips_ours"] += page_values["name_clips_ours"]
        totals["verse_hits_ref"] += page_values["verse_hits_ref"]
        totals["verse_hits_ours"] += page_values["verse_hits_ours"]
        totals["note_hits_ref"] += page_values["note_hits_ref"]
        totals["note_hits_ours"] += page_values["note_hits_ours"]
        rows.append(row)

    (out / "raw.jsonl").write_text(
        "".join(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )
    share = totals["within"] / totals["total"] if totals["total"] else 0.0
    vertical = totals["vertical"] / totals["matched_notes"] if totals["matched_notes"] else 0.0
    stem = totals["stem"] / totals["matched_stems"] if totals["matched_stems"] else 0.0
    share_x = totals["x"] / totals["total"] if totals["total"] else 0.0
    share_y = totals["y"] / totals["total"] if totals["total"] else 0.0
    share_widths = (
        totals["widths"] / totals["widths_total"] if totals["widths_total"] else 0.0
    )
    lines = [
        "# Engraving oracle — P1 skeleton",
        "",
        "Reference: MuseScore Studio 4.7.4 SVG; tolerance 0.1 staff space.",
        "Element coordinates are relative to their own staff system: comparing",
        "absolute y would measure the system distance, not pitch. Horizontal",
        "spacing is fixed until P2. Unmatched elements and failures count in",
        "the denominator of `within_0_1_sp`.",
        "",
        "| Metric | Value |",
        "| --- | ---: |",
        f"| Files measured | {totals['files']} |",
        f"| Failures (included) | {totals['failures']} |",
        f"| Elements compared | {totals['total']} |",
        f"| Within 0.1 sp, both axes | {share:.4f} ({totals['within']}/{totals['total']}) |",
        f"| Within 0.1 sp, x only | {share_x:.4f} |",
        f"| Within 0.1 sp, y only | {share_y:.4f} |",
        f"| Vertical pitch/ledger agreement | {vertical:.4f} "
        f"({totals['matched_notes']} matched noteheads) |",
        f"| Stem-length agreement | {stem:.4f} "
        f"({totals['matched_stems']} matched stems) |",
        f"| Measure widths within 0.1 sp | {share_widths:.4f} "
        f"({totals['widths']}/{totals['widths_total']}) |",
        f"| Measure width mean abs. error, sp (all measures, missing counted) | "
        f"{totals['width_error'] / totals['widths_total'] if totals['widths_total'] else 0.0:.3f} |",
        f"| Note x within 0.1 sp (unmatched counted) | "
        f"{totals['note_x'] / totals['total'] if totals['total'] else 0.0:.4f} "
        f"({totals['note_x']}/{totals['total']}) |",
        f"| Note x mean abs. error, sp (matched noteheads) | "
        f"{totals['note_x_error'] / totals['note_x_matched'] if totals['note_x_matched'] else 0.0:.3f} "
        f"({totals['note_x_matched']} matched) |",
        f"| Files with identical system breaks | "
        f"{totals['systems_equal']}/{totals['files']} |",
        f"| Files with the oracle page count | {totals['page_count']}/{totals['files']} |",
        f"| Pages with identical system breaks | "
        f"{totals['breaks']}/{totals['pages_total']} |",
        f"| Staff system tops within 0.5 sp | "
        f"{totals['staff_y']}/{totals['staff_y_total']} |",
        f"| Staff top mean abs. error, sp (matched positions) | "
        f"{totals['staff_y_error'] / totals['staff_y_matched'] if totals['staff_y_matched'] else 0.0:.3f} |",
        f"| Adjacent-staff gaps within 0.5 sp | "
        f"{totals['staff_gap']}/{totals['staff_gap_total']} |",
        f"| Adjacent-staff gap mean abs. error, sp (matched pairs) | "
        f"{totals['staff_gap_error'] / totals['staff_gap_matched'] if totals['staff_gap_matched'] else 0.0:.3f} |",
        f"| Mean adjacent-staff gap, sp (reference / ours) | "
        f"{totals['staff_gap_ref'] / totals['staff_gap_total'] if totals['staff_gap_total'] else 0.0:.3f}"
        f" / {totals['staff_gap_ours'] / totals['staff_gap_total'] if totals['staff_gap_total'] else 0.0:.3f} |",
        f"| Overlapping adjacent staves (reference / ours) | "
        f"{totals['overlaps_ref']} / {totals['overlaps_ours']} |",
        f"| Lyric glyphs drawn (reference / ours) | "
        f"{totals['lyrics_ref']} / {totals['lyrics_ours']} |",
        f"| Lyric glyphs overlapping a staff (reference / ours) | "
        f"{totals['lyric_hits_ref']} / {totals['lyric_hits_ours']} |",
        f"| Noteheads crossing a bar line (reference / ours) | "
        f"{totals['barline_hits_ref']} / {totals['barline_hits_ours']} |",
        f"| Max bar-line x spread across staves of one system, sp (reference / ours) | "
        f"{totals['spread_max_ref']:.3f} / {totals['spread_max_ours']:.3f} |",
        f"| Boundaries with bar-line x spread > 0.05 sp (reference / ours) | "
        f"{totals['spread_bad_ref']}/{totals['spread_total_ref']} / "
        f"{totals['spread_bad_ours']}/{totals['spread_total_ours']} |",
        f"| Instrument-name glyphs drawn (reference / ours) | "
        f"{totals['names_ref']} / {totals['names_ours']} |",
        f"| Instrument-name glyphs left of the page margin (reference / ours) | "
        f"{totals['name_clips_ref']} / {totals['name_clips_ours']} |",
        f"| Overlapping lyric verse lines (reference / ours) | "
        f"{totals['verse_hits_ref']} / {totals['verse_hits_ours']} |",
        f"| Overlapping notehead pairs (reference / ours) | "
        f"{totals['note_hits_ref']} / {totals['note_hits_ours']} |",
        "",
        "## Failures",
        "",
    ]
    failures = [row for row in rows if "error" in row]
    lines += [f"- `{row['id']}`: {row['error']}" for row in failures] or ["- none"]
    lines.append("")
    (out / "resume.md").write_text("\n".join(lines), encoding="utf-8")
    print("\n".join(lines[:12]))
    return 0


if __name__ == "__main__":
    main()
