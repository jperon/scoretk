// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read206/read206.cpp` de MuseScore Studio (les
// fonctions `readVolta206`, `readSlur206`, `readHairpin206`, `readOttava` et le
// couplage `<endSpanner>` de `readMeasure206`), Copyright (C) MuseScore Limited
// and others, modifié (réécrit en Rust) en 2026.
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

//! Reconnection des spanners identifiés par `id` dans les fichiers 2.x.
//!
//! En 2.x, un spanner est décrit par un élément porteur de son `id`
//! (`<Volta id="N">`, …) et refermé par un `<endSpanner id="N"/>`; les deux
//! moitiés peuvent être très éloignées dans le fichier. Le lecteur C++ les
//! apparie via `ReadContext::findSpanner` et `addSpannerValues`. Le portage
//! collecte les deux extrémités lors de l'adaptation de l'arbre, puis construit
//! les `Spanner` du modèle après la lecture 3.x, comme `fix_ties` le fait pour
//! les liaisons.

use scoretk_core::model::{
    Fraction, Hairpin, HairpinType, OttavaType, Placement, Score, SegmentType, Slur, Spanner,
    SpannerAnchor, SpannerNotation, SpannerType, Track, VoltaInfo,
};

/// Ancrage d'une extrémité de spanner dans le fichier 2.x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanKey {
    /// Élément au niveau mesure : le tick est celui du début de la mesure.
    Measure { measure: usize, track: u32 },
    /// Accord ou silence : le tick est celui du segment qui le porte.
    Chord {
        measure: usize,
        track: u32,
        /// Rang de l'accord ou silence dans la voix de la mesure, 0-basé.
        ordinal: usize,
    },
}

/// Famille de spanner 2.x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanFamily {
    Volta,
    Slur,
    Hairpin,
    Ottava,
}

/// Moitié d'un spanner 2.x ; les deux moitiés sont appariées par `id`.
#[derive(Clone, Debug)]
pub(super) struct SpannerLink {
    pub(super) id: i32,
    pub(super) family: SpanFamily,
    pub(super) start: Option<SpanKey>,
    pub(super) end: Option<SpanKey>,
    pub(super) endings: Vec<u8>,
    pub(super) closed: bool,
    pub(super) text: Option<String>,
    /// Direction explicite d'une liaison d'expression (`<up>` 2.x).
    pub(super) slur_up: Option<String>,
    /// Sens d'un soufflet 2.x (`<subtype>` entier, `readHairpin206`).
    pub(super) hairpin_kind: Option<HairpinType>,
    pub(super) hairpin_line: bool,
    pub(super) hairpin_niente: bool,
    pub(super) hairpin_direction: Option<String>,
    /// Type d'ottava 2.x (`<subtype>`, `readOttava`).
    pub(super) ottava_type: Option<OttavaType>,
}

impl SpannerLink {
    pub(super) fn new(id: i32, family: SpanFamily) -> Self {
        Self {
            id,
            family,
            start: None,
            end: None,
            endings: Vec::new(),
            closed: false,
            text: None,
            slur_up: None,
            hairpin_kind: None,
            hairpin_line: false,
            hairpin_niente: false,
            hairpin_direction: None,
            ottava_type: None,
        }
    }
}

/// Construit les spanners du modèle à partir des moitiés collectées.
///
/// Portage de l'appariement de `ReadContext::addSpannerValues` et de
/// `readMeasure206` (`read206.cpp:2834-2852`) : la fin d'un spanner de mesure
/// est le tick de la mesure où `<endSpanner>` apparaît, soit le début de la
/// mesure qui suit la dernière mesure couverte.
pub(super) fn fix_spanners(score: &mut Score, links: &[SpannerLink]) {
    let mut paired: Vec<SpannerLink> = Vec::new();
    for link in links {
        // Un `<endSpanner id>` ne porte pas sa famille : on l'apparie au début
        // d'identifiant correspondant, quelle que soit la famille collectée.
        if let Some(existing) = paired
            .iter_mut()
            .find(|other| other.id == link.id && other.start.is_some())
        {
            merge(existing, link);
        } else {
            paired.push(link.clone());
        }
    }
    for link in paired {
        let (Some(start), Some(end)) = (link.start, link.end) else {
            continue;
        };
        let (Some((start_tick, start_track)), Some((end_tick, end_track))) =
            (anchor(score, &start), anchor(score, &end))
        else {
            continue;
        };
        let mut spanner = Spanner::new(
            match link.family {
                SpanFamily::Volta => SpannerType::Volta,
                SpanFamily::Slur => SpannerType::Slur,
                SpanFamily::Hairpin => {
                    SpannerType::Hairpin(link.hairpin_kind.unwrap_or(HairpinType::Crescendo))
                }
                SpanFamily::Ottava => {
                    SpannerType::Ottava(link.ottava_type.unwrap_or(OttavaType::OctaveUp))
                }
            },
            SpannerAnchor::new(start_tick, start_track),
            SpannerAnchor::new(end_tick, end_track),
        );
        spanner.text = link.text.clone();
        match link.family {
            SpanFamily::Volta => {
                spanner.volta = Some(VoltaInfo {
                    endings: link.endings.clone(),
                    closed: link.closed,
                });
            }
            SpanFamily::Hairpin => {
                let mut hairpin = Hairpin::new();
                hairpin.line = link.hairpin_line;
                hairpin.niente = link.hairpin_niente;
                hairpin.placement = match link.hairpin_direction.as_deref() {
                    Some("up") => Placement::Above,
                    _ => Placement::Below,
                };
                hairpin.begin_text = link.text.clone().unwrap_or_default();
                spanner.hairpin = Some(hairpin);
            }
            SpanFamily::Ottava => {
                // Le type est déjà porté par `SpannerType::Ottava` : aucun
                // complément à renseigner dans le modèle.
            }
            SpanFamily::Slur => {
                let mut slur = Slur::new();
                slur.placement = match link.slur_up.as_deref() {
                    Some("up") => Some(Placement::Above),
                    Some("down") => Some(Placement::Below),
                    _ => None,
                };
                spanner.notation = Some(SpannerNotation::Slur(slur));
            }
        }
        score.add_spanner(spanner);
    }
}

fn merge(existing: &mut SpannerLink, link: &SpannerLink) {
    // Une définition `<Slur id>` donne une position de mesure approximative ;
    // le `<Slur type="start" id>` de l'accord est l'ancrage retenu par
    // `readChordRestProperties206` (read206.cpp:1823-1903).
    let start_is_provisional = matches!(existing.start, Some(SpanKey::Measure { .. }));
    if existing.start.is_none()
        || (start_is_provisional && matches!(link.start, Some(SpanKey::Chord { .. })))
    {
        existing.start = link.start;
    }
    if existing.end.is_none() {
        existing.end = link.end;
    }
    if link.slur_up.is_some() {
        existing.slur_up = link.slur_up.clone();
    }
    if link.hairpin_kind.is_some() {
        existing.hairpin_kind = link.hairpin_kind;
    }
    if link.hairpin_line {
        existing.hairpin_line = true;
    }
    if link.hairpin_niente {
        existing.hairpin_niente = true;
    }
    if link.hairpin_direction.is_some() {
        existing.hairpin_direction = link.hairpin_direction.clone();
    }
    if link.ottava_type.is_some() {
        existing.ottava_type = link.ottava_type;
    }
    if !link.endings.is_empty() {
        existing.endings = link.endings.clone();
    }
    if link.closed {
        existing.closed = true;
    }
    if link.text.is_some() {
        existing.text = link.text.clone();
    }
}

/// Résout un ancrage 2.x en tick et piste du modèle.
pub(super) fn anchor(score: &Score, key: &SpanKey) -> Option<(Fraction, Track)> {
    match *key {
        SpanKey::Measure { measure, track } => Some((
            score.measures.get(measure)?.start_tick,
            Track::from_raw(track),
        )),
        SpanKey::Chord {
            measure,
            track,
            ordinal,
        } => {
            let track = Track::from_raw(track);
            let measure = score.measures.get(measure)?;
            let mut segments: Vec<_> = measure.segments.iter().collect();
            segments.sort_by_key(|segment| segment.tick);
            let mut rank = 0usize;
            for segment in segments {
                if segment.kind != SegmentType::ChordRest
                    || segment
                        .element(track)
                        .and_then(|item| item.as_chord_rest())
                        .is_none()
                {
                    continue;
                }
                if rank == ordinal {
                    return Some((segment.tick, track));
                }
                rank += 1;
            }
            // Un `<endSpanner>` placé après le dernier accord de la mesure
            // tombe à `ctx.tick()` en fin de mesure : la fin du spanner est
            // alors le début de la mesure suivante.
            Some((measure.start_tick + measure.len, track))
        }
    }
}
