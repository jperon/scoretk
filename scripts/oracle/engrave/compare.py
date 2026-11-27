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
    # Bar lines keep their absolute y here: local y repeats on every system,
    # so grouping bar lines by local y would merge all systems into one.
    raw_barlines: list[tuple[float, float, float]] = []
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
    result["stem_lengths"] = lengths
    result["spatium"] = spatium
    # Lyrics are paths in both SVGs; their bounding box is used to detect
    # collisions with a staff's five-line band (the user-visible defect of
    # step 2). Elements that fail to parse are not silently dropped: the
    # element-level matcher below still counts every expected element.
    result["lyric_boxes"] = []
    for element in elements:
        if element.get("class", "") != "Lyrics" or element.tag != SVG + "path":
            continue
        box = bbox_of_path(element.get("d", ""))
        if box is None:
            continue
        sx, sy, tx, ty = transform_of(element)
        result["lyric_boxes"].append(
            (sx * box[0] + tx, sy * box[1] + ty, sx * box[2] + tx, sy * box[3] + ty)
        )
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
