// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Ottavas à l'export : `<octave-shift>` et correction de la hauteur écrite.
//!
//! Portage de `ExportMusicXml::ottava` et de la correction d'octave de
//! `pitch2xml` (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! MuseScore exporte la hauteur **écrite** des notes. Le `<pitch>` du `.mscx`
//! est la hauteur sonnante ; sous une ottava, la hauteur affichée est décalée
//! d'une (ou deux) octaves. Le décalage se lit dans
//! `note->ppitch() - note->pitch()` : `8va` ajoute une octave écrite, `8vb`
//! la retire.

use scoretk_core::model::{Score, Spanner, SpannerType, StaffId, Tick};

/// Nombre d'octaves dont la hauteur écrite est décalée par une ottava.
fn written_shift(kind: SpannerType) -> i32 {
    match kind {
        SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveUp) => 1,
        SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveDown) => -1,
        SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthUp) => 2,
        SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthDown) => -2,
        _ => 0,
    }
}

/// Taille MusicXML (`size`) d'une ottava.
fn size(kind: SpannerType) -> Option<&'static str> {
    match kind {
        SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveUp)
        | SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveDown) => Some("8"),
        SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthUp)
        | SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthDown) => Some("15"),
        _ => None,
    }
}

/// Type de départ d'un `<octave-shift>` (`down` pour un `8va`).
fn start_type(kind: SpannerType) -> Option<&'static str> {
    match kind {
        SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveUp)
        | SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthUp) => Some("down"),
        SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveDown)
        | SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthDown) => Some("up"),
        _ => None,
    }
}

/// Vrai si l'ottava couvre le tick demandé sur la portée.
fn covers(spanner: &Spanner, staff: StaffId, tick: Tick) -> bool {
    spanner.start.track.staff() == staff && spanner.start.tick <= tick && tick < spanner.end.tick
}

/// Décalage d'octave écrit des notes d'une portée à un tick.
///
/// Plusieurs ottavas peuvent se chevaucher : les décalages s'additionnent,
/// comme `ppitch - pitch` les cumule dans MuseScore.
pub(super) fn shift(score: &Score, staff: StaffId, tick: Tick) -> i32 {
    score
        .spanners
        .iter()
        // Le décalage écrit suit `play` : une ottava de MuseScore (`play`)
        // décale la hauteur écrite même invisible — 4.7.4 omet alors la
        // seule direction (`v4/testExcludeInvisibleElements.ref.musicxml`,
        // mesure 10) ; celle de LilyPond ne change pas la hauteur jouée et,
        // invisible, ne doit ni décaler ni produire de `<octave-shift>`
        // (`\once \override OttavaBracket.transparent`, même fichier côté
        // `.ly`).
        .filter(|spanner| covers(spanner, staff, tick) && (spanner.play || spanner.visible))
        .map(|spanner| written_shift(spanner.kind.clone()))
        .sum()
}

/// Numéro de slot d'une ottava, pour l'attribut `number`.
///
/// MuseScore réutilise les slots libres (`m_ottavas`) et attribue donc un
/// numéro croissant aux ottavas qui se chevauchent sur une même portée.
fn number(score: &Score, spanner: &Spanner) -> usize {
    let staff = spanner.start.track.staff();
    let mut overlapping: Vec<&Spanner> = score
        .spanners
        .iter()
        .filter(|other| {
            matches!(other.kind, SpannerType::Ottava(_))
                && other.start.track.staff() == staff
                && other.start.tick <= spanner.end.tick
                && spanner.start.tick <= other.end.tick
        })
        .collect();
    overlapping.sort_by_key(|other| (other.start.tick, other.end.tick));
    overlapping
        .iter()
        .position(|other| std::ptr::eq(*other, spanner))
        .map_or(1, |index| index + 1)
}

/// Écrit le début d'une ottava précise, dans l'ordre des spanners.
pub(super) fn write_start(
    score: &Score,
    out: &mut super::exportmusicxml::Out,
    index: usize,
    xml_staff: usize,
    written: &mut [bool],
) {
    let Some(spanner) = score.spanners.get(index) else {
        return;
    };
    if !matches!(spanner.kind, SpannerType::Ottava(_))
        || !spanner.visible
        || written.get(index).copied().unwrap_or(true)
    {
        return;
    }
    if let Some(entry) = written.get_mut(index) {
        *entry = true;
    }
    let (Some(kind), Some(size)) = (start_type(spanner.kind.clone()), size(spanner.kind.clone()))
    else {
        return;
    };
    out.attr(false);
    out.xml
        .start_element("direction", &[("placement", "above")]);
    out.xml.start_element("direction-type", &[]);
    out.xml.tag_empty(
        "octave-shift",
        &[
            ("type", kind),
            ("size", size),
            ("number", &number(score, spanner).to_string()),
        ],
    );
    out.xml.end_element();
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

/// Écrit les `<octave-shift type="stop">` qui finissent à ce tick.
pub(super) fn write_stops_at(
    score: &Score,
    out: &mut super::exportmusicxml::Out,
    tick: Tick,
    staff: StaffId,
    xml_staff: usize,
    written: &mut [bool],
) {
    for (index, spanner) in score.spanners.iter().enumerate() {
        if !matches!(spanner.kind, SpannerType::Ottava(_))
            || !spanner.visible
            || spanner.start.track.staff() != staff
            || spanner.end.tick != tick
            || written.get(index).copied().unwrap_or(true)
        {
            continue;
        }
        if let Some(entry) = written.get_mut(index) {
            *entry = true;
        }
        let Some(size) = size(spanner.kind.clone()) else {
            continue;
        };
        out.attr(false);
        out.xml
            .start_element("direction", &[("placement", "above")]);
        out.xml.start_element("direction-type", &[]);
        out.xml.tag_empty(
            "octave-shift",
            &[
                ("type", "stop"),
                ("size", size),
                ("number", &number(score, spanner).to_string()),
            ],
        );
        out.xml.end_element();
        if xml_staff > 0 {
            out.xml.tag("staff", &xml_staff.to_string());
        }
        out.xml.end_element();
    }
}
