// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/connectorinforeader.cpp` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Ottavas du lecteur 4.x.
//!
//! Portage de la lecture des `<Spanner type="Ottava">` par `Read410` et
//! `ConnectorInfoReader` (`engraving/rw/read400/connectorinforeader.cpp`) :
//! deux moitiés (`<next>` / `<prev>`) portent une `<location>` relative, et
//! l'enfant `<Ottava>` (sur la moitié de départ) donne le sous-type.

use super::dom::Element;
use scoretk_core::model::{
    Fraction, OttavaType, Score, Spanner, SpannerAnchor, SpannerType, Tick, Track, VOICES,
};

/// `<location>` relative d'une extrémité.
#[derive(Clone, Copy, Debug)]
struct Location {
    measures: i64,
    fractions: Fraction,
    staves: i64,
    voices: i64,
}

impl Location {
    fn absolute(
        self,
        measure: usize,
        tick: Tick,
        track: Track,
        measures: &[scoretk_core::model::Measure],
    ) -> (Tick, Track) {
        let current_base = measures
            .get(measure)
            .map_or(Fraction::ZERO, |value| value.start_tick);
        let target = measure as i64 + self.measures;
        let base = usize::try_from(target)
            .ok()
            .and_then(|index| measures.get(index))
            .map_or(current_base, |value| value.start_tick);
        let tick = base + (tick - current_base) + self.fractions;
        let raw_track = i64::from(track.raw()) + self.voices + self.staves * VOICES as i64;
        (tick, Track::from_raw(raw_track.max(0) as u32))
    }
}

/// Moitié d'ottava en attente d'appariement.
#[derive(Clone, Debug)]
pub(crate) struct Half {
    kind: SpannerType,
    visible: bool,
    measure: usize,
    tick: Tick,
    track: Track,
    prev: Option<Location>,
    next: Option<Location>,
}

/// Lit une moitié de `<Spanner type="Ottava">`, ou `None` pour un autre type.
pub(crate) fn read_half(
    element: &Element,
    measure: usize,
    tick: Tick,
    track: Track,
) -> Option<Half> {
    if element.attr("type") != Some("Ottava") {
        return None;
    }
    let payload = element.child("Ottava");
    let kind = if payload.is_some() {
        SpannerType::Ottava(subtype(payload))
    } else {
        // La moitié de fin ne répète pas `<Ottava>` : le type vient du début.
        SpannerType::Ottava(OttavaType::OctaveUp)
    };
    let visible = payload
        .and_then(|node| node.child("visible"))
        .and_then(Element::int)
        .is_none_or(|value| value != 0);
    let mut half = Half {
        kind,
        visible,
        measure,
        tick,
        track,
        prev: None,
        next: None,
    };
    for child in &element.children {
        match child.name.as_str() {
            "prev" => half.prev = child.child("location").map(read_location),
            "next" => half.next = child.child("location").map(read_location),
            _ => {}
        }
    }
    Some(half)
}

/// Sous-type MuseScore (`<subtype>8va</subtype>`).
fn subtype(payload: Option<&Element>) -> OttavaType {
    match payload
        .and_then(|node| node.child("subtype"))
        .map(Element::text)
    {
        Some("8vb") => OttavaType::OctaveDown,
        Some("15ma") => OttavaType::FifteenthUp,
        Some("15mb") => OttavaType::FifteenthDown,
        _ => OttavaType::OctaveUp,
    }
}

fn read_location(element: &Element) -> Location {
    Location {
        measures: element
            .child("measures")
            .and_then(Element::int)
            .unwrap_or(0),
        fractions: element
            .child("fractions")
            .and_then(Element::fraction)
            .unwrap_or(Fraction::ZERO),
        staves: element.child("staves").and_then(Element::int).unwrap_or(0),
        voices: element.child("voices").and_then(Element::int).unwrap_or(0),
    }
}

/// Apparie les moitiés et ajoute les ottavas à la partition.
pub(crate) fn resolve(score: &mut Score, halves: &[Half]) {
    let mut used = vec![false; halves.len()];
    for (start, half) in halves.iter().enumerate() {
        let Some(next) = half.next else {
            continue;
        };
        if half.prev.is_some() || used[start] {
            continue;
        }
        let (wanted_tick, wanted_track) =
            next.absolute(half.measure, half.tick, half.track, &score.measures);
        let partner = halves.iter().enumerate().find(|(index, candidate)| {
            *index != start
                && !used[*index]
                && candidate.prev.is_some()
                && candidate.next.is_none()
                && candidate.track == wanted_track
                && candidate.tick == wanted_tick
                && candidate.prev.map(|prev| {
                    prev.absolute(
                        candidate.measure,
                        candidate.tick,
                        candidate.track,
                        &score.measures,
                    )
                }) == Some((half.tick, half.track))
        });
        let Some((end, _)) = partner else {
            continue;
        };
        used[start] = true;
        used[end] = true;
        let last = &halves[end];
        let mut spanner = Spanner::new(
            half.kind.clone(),
            SpannerAnchor::new(half.tick, half.track),
            SpannerAnchor::new(last.tick, last.track),
        );
        spanner.visible = half.visible;
        score.add_spanner(spanner);
    }
}
