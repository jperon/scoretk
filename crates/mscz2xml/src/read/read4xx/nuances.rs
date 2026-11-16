// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/rw/read410/tread.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Nuances et soufflets du lecteur 4.x.
//!
//! Portage de `TRead::read(Dynamic*)` et du quart de `TRead::read(Hairpin*)`
//! utile à l'export MusicXML (`engraving/rw/read410/tread.cpp`). Les
//! connecteurs `<Spanner type="HairPin">` sont appariés comme
//! `ConnectorInfoReader` : chaque moitié porte une extrémité implicite (sa
//! position) et l'autre dans `<prev>` ou `<next>`.

use super::dom::Element;
use crate::model::{
    Fraction, Hairpin, HairpinType, Placement, Score, Spanner, SpannerAnchor, SpannerType, Tick,
    Track,
};

/// Lit une `<Dynamic>` (`TRead::read(Dynamic*)`).
pub(crate) fn read_dynamic(element: &Element) -> crate::model::Dynamic {
    let subtype = element
        .child("subtype")
        .map(Element::text)
        .unwrap_or_default()
        .to_owned();
    let custom_text = element
        .child("text")
        .map(|node| node.text.clone())
        .filter(|text| !text.trim().is_empty());
    let custom = custom_text.is_some() || subtype == "other-dynamics";

    let mut dynamic = crate::model::Dynamic::new(if let Some(text) = custom_text {
        text.trim().to_owned()
    } else if subtype == "other-dynamics" {
        String::new()
    } else {
        subtype.clone()
    });
    dynamic.custom = custom;
    dynamic.velocity = element
        .child("velocity")
        .and_then(Element::int)
        .and_then(|value| u8::try_from(value.clamp(0, 127)).ok())
        .or_else(|| default_velocity(&dynamic.text));
    dynamic.placement = match element.child("placement").map(Element::text) {
        Some("above") => Placement::Above,
        Some("below") => Placement::Below,
        _ => Placement::Below,
    };
    dynamic.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    dynamic
}

/// Vélocité par défaut de `Dynamic::velocity()`, d'après `DYN_LIST`.
///
/// Utilisée quand le fichier ne porte pas `<velocity>`.
fn default_velocity(text: &str) -> Option<u8> {
    let velocity = match text {
        "pppppp" => 1,
        "ppppp" => 5,
        "pppp" => 10,
        "ppp" => 16,
        "pp" => 33,
        "p" => 49,
        "mp" => 64,
        "mf" => 80,
        "f" => 96,
        "ff" => 112,
        "fff" => 126,
        "ffff" | "fffff" | "ffffff" => 127,
        "fp" => 96,
        "pf" => 49,
        "sf" | "sfz" | "sff" | "sffz" | "sfff" | "sfffz" | "sfp" | "sfpp" | "rfz" | "rf" | "fz" => {
            112
        }
        "m" => 96,
        "r" | "s" | "z" => 112,
        "n" => 49,
        _ => return None,
    };
    Some(velocity)
}

/// Moitié de connecteur `<Spanner type="HairPin">` en attente d'appariement.
#[derive(Clone, Debug)]
pub(crate) struct Half {
    /// Sens du soufflet.
    kind: HairpinType,
    /// Détails (`beginText`, `endText`, niente, placement).
    hairpin: Hairpin,
    /// Mesure de la moitié, pour résoudre les `<location>` en mesures.
    measure: usize,
    /// Tick absolu de la moitié.
    tick: Tick,
    /// Piste de la moitié.
    track: Track,
    /// Extrémité `<prev>`, relative.
    prev: Option<Rel>,
    /// Extrémité `<next>`, relative.
    next: Option<Rel>,
}

/// `<location>` relative à la moitié courante.
#[derive(Clone, Copy, Debug)]
struct Rel {
    measures: i32,
    fractions: Fraction,
    staves: i32,
    voices: i32,
}

impl Rel {
    /// Tick absolu de l'autre extrémité.
    ///
    /// Reproduit `Location::toAbsolute` : les fractions s'ajoutent au tick
    /// courant, les mesures décalent la base.
    fn absolute_tick(self, measure: usize, tick: Tick, measures: &[crate::model::Measure]) -> Tick {
        let current_base = measures
            .get(measure)
            .map_or(Fraction::ZERO, |value| value.start_tick);
        let target = measure as i64 + i64::from(self.measures);
        let base = usize::try_from(target)
            .ok()
            .and_then(|index| measures.get(index))
            .map_or(current_base, |value| value.start_tick);
        base + (tick - current_base) + self.fractions
    }

    /// Piste absolue de l'autre extrémité.
    fn absolute_track(self, track: Track) -> Track {
        let raw = i64::from(track.raw())
            + i64::from(self.voices)
            + i64::from(self.staves) * crate::model::VOICES as i64;
        Track::from_raw(raw.max(0) as u32)
    }
}

/// Lit une moitié de soufflet, ou `None` pour un autre type de spanner.
pub(crate) fn read_half(
    element: &Element,
    measure: usize,
    tick: Tick,
    track: Track,
) -> Option<Half> {
    if element.attr("type") != Some("HairPin") {
        return None;
    }
    // La moitié de fin ne porte que `<prev>`, sans `<HairPin>` : les détails
    // viennent toujours de la moitié de début.
    let payload = element.child("HairPin");
    let subtype = payload
        .and_then(|node| node.child("subtype"))
        .and_then(Element::int)
        .unwrap_or(0);
    let kind = if subtype == 1 || subtype == 3 {
        HairpinType::Diminuendo
    } else {
        HairpinType::Crescendo
    };
    let mut hairpin = Hairpin::new();
    hairpin.line = subtype >= 2;
    hairpin.niente = payload
        .and_then(|node| node.child("hairpinCircledTip"))
        .and_then(Element::int)
        .is_some_and(|value| value != 0);
    hairpin.placement = match payload
        .and_then(|node| node.child("direction"))
        .map(Element::text)
    {
        Some("up") => Placement::Above,
        _ => Placement::Below,
    };
    hairpin.begin_text = payload
        .and_then(|node| node.child("beginText"))
        .map(|node| node.text.clone())
        .unwrap_or_default();
    hairpin.end_text = payload
        .and_then(|node| node.child("endText"))
        .map(|node| node.text.clone())
        .unwrap_or_default();

    let mut half = Half {
        kind,
        hairpin,
        measure,
        tick,
        track,
        prev: None,
        next: None,
    };
    for child in &element.children {
        match child.name.as_str() {
            "prev" => half.prev = child.child("location").map(read_rel),
            "next" => half.next = child.child("location").map(read_rel),
            _ => {}
        }
    }
    Some(half)
}

fn read_rel(node: &Element) -> Rel {
    Rel {
        measures: node.child("measures").and_then(Element::int).unwrap_or(0) as i32,
        fractions: node
            .child("fractions")
            .and_then(Element::fraction)
            .unwrap_or(Fraction::ZERO),
        staves: node.child("staves").and_then(Element::int).unwrap_or(0) as i32,
        voices: node.child("voices").and_then(Element::int).unwrap_or(0) as i32,
    }
}

/// Apparie les moitiés de soufflets et ajoute les spanners à la partition.
///
/// L'appariement se fait d'abord par positions relatives exactes (comme
/// `ConnectorInfoReader::connect`), puis, pour les fichiers dont les
/// `<location>` sont approximatives, par proximité : chaque début restant est
/// associé à la première fin restante de même sens. Le couple
/// (`start`, `end`) est celui des moitiés **réelles**, comme
/// `readAddConnector` qui prend `item->tick()`, pas la valeur de `<location>`.
pub(crate) fn resolve(score: &mut Score, halves: &[Half]) {
    let mut used = vec![false; halves.len()];
    let mut pairs: Vec<(usize, usize)> = Vec::new();

    for (start, half) in halves.iter().enumerate() {
        if half.next.is_none() || half.prev.is_some() || used[start] {
            continue;
        }
        let wanted = half
            .next
            .map(|rel| rel.absolute_tick(half.measure, half.tick, &score.measures));
        let wanted_track = half.next.map(|rel| rel.absolute_track(half.track));
        let start_track = half.next.map(|rel| rel.absolute_track(half.track));
        let partner = halves.iter().enumerate().find(|(index, candidate)| {
            *index != start
                && !used[*index]
                && candidate.prev.is_some()
                && candidate.next.is_none()
                && wanted_track == Some(candidate.track)
                && start_track
                    == candidate
                        .prev
                        .map(|rel| rel.absolute_track(candidate.track))
                && candidate.prev.map(|rel| {
                    rel.absolute_tick(candidate.measure, candidate.tick, &score.measures)
                }) == Some(half.tick)
                && wanted == Some(candidate.tick)
                && half.prev.is_none()
        });
        if let Some((end, _)) = partner {
            used[start] = true;
            used[end] = true;
            pairs.push((start, end));
        }
    }

    // Repli : appariement dans l'ordre du document, par sens et par piste.
    for (start, half) in halves.iter().enumerate() {
        if used[start] || half.next.is_none() || half.prev.is_some() {
            continue;
        }
        let partner = halves.iter().enumerate().position(|(index, candidate)| {
            !used[index]
                && index > start
                && candidate.prev.is_some()
                && candidate.next.is_none()
                && candidate.track == half.track
        });
        if let Some(end) = partner {
            used[start] = true;
            used[end] = true;
            pairs.push((start, end));
        }
    }

    for (start, end) in pairs {
        let first = &halves[start];
        let last = &halves[end];
        let mut spanner = Spanner::new(
            SpannerType::Hairpin(first.kind),
            SpannerAnchor::new(first.tick, first.track),
            SpannerAnchor::new(last.tick, last.track),
        );
        spanner.hairpin = Some(first.hairpin.clone());
        spanner.text = Some(first.hairpin.begin_text.clone());
        score.add_spanner(spanner);
    }
}
