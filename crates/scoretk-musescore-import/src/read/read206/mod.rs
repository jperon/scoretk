// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read206/read206.cpp` de MuseScore Studio, Copyright
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

//! Lecteur des partitions MuseScore 2.x (versions 1.15 à 2.07).
//!
//! `RWRegister::reader` (`rw/rwregister.cpp:45-49`) aiguille `<= 114` vers
//! `read114` (hors périmètre) et `<= 207` vers `read206`. Ce module porte
//! `Read206::readScoreFile` : `readScoreTag` y lit les parties, les métadonnées
//! et les portées, `readMeasure206` les mesures, et tous les éléments sont
//! délégués au lecteur commun `read400::TRead`, déjà porté dans `read302`.
//!
//! Le portage sépare donc deux étapes, comme le fait le C++ amont :
//!
//! 1. [`transform`] adapte les conventions d'ensemble du format 2.x (voix
//!    portées par `<track>`/`<tick>`, n-olets référencés par id, liaisons
//!    `<Tie id>`/`<endSpanner id>`) à la forme comprise par `read302` ;
//! 2. `read302::read` lit les éléments proprement dits, puis [`fix_ties`]
//!    rebranche les liaisons de prolongation sur les notes du modèle.
//!
//! Le format n'ayant pas de spécification publiée (voir
//! `docs/agents/format-ref/musescore.md`), la syntaxe suit `read206.cpp` et le
//! comportement de référence est MuseScore 4.7.4.

mod spanners;
mod transform;
mod xml;

use std::collections::BTreeMap;

use crate::container::MscVersion;
use crate::Result;
use scoretk_core::model::{NoteId, Score, SegmentType, Track};

use super::read302;

/// Position d'une note dans le fichier 2.x, utilisée pour rebrancher une
/// liaison après la lecture par `read302`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NoteKey {
    /// Index de mesure, 0-basé.
    measure: usize,
    /// Piste `portée * VOICES + voix`.
    track: u32,
    /// Rang de la note dans la voix de la mesure, 0-basé.
    ordinal: usize,
}

/// Une liaison 2.x, dont les deux extrémités sont annoncées séparément.
#[derive(Clone, Copy, Debug)]
pub(super) struct TieLink {
    id: i32,
    start: Option<NoteKey>,
    end: Option<NoteKey>,
}

/// Point d'entrée du lecteur 2.x (portage de `Read206::readScoreFile`).
///
/// `version` est celle de `<museScore version="…">` (par exemple 206) ; elle
/// n'est plus utilisée après l'adaptation, le lecteur 3.x recevant 302 pour
/// reprendre ses règles de compatibilité.
pub fn read(mscx: &[u8], _version: MscVersion) -> Result<Score> {
    let text = String::from_utf8_lossy(mscx);
    let mut root = xml::parse(&text)?;
    let mut ties = Vec::new();
    let mut links = Vec::new();
    transform::adapt(&mut root, &mut ties, &mut links);
    let mut adapted = String::new();
    root.write_xml(&mut adapted);
    let mut score = read302::read_with_origin(
        adapted.as_bytes(),
        MscVersion::from_raw(302),
        MscVersion::from_raw(206),
    )?;
    fix_ties(&mut score, &ties);
    spanners::fix_spanners(&mut score, &links);
    Ok(score)
}

/// Reconnecte les `<Tie id>` / `<endSpanner id>` 2.x (`Score::connectTies`).
///
/// Les deux moitiés sont appariées par identifiant, puis les notes sont
/// retrouvées par leur position (mesure, piste, rang) dans le modèle produit
/// par `read302`. Sans cette étape, les liaisons 2.x disparaîtraient à l'export
/// MusicXML, MIDI, LilyPond et ABC.
fn fix_ties(score: &mut Score, links: &[TieLink]) {
    let mut paired: BTreeMap<i32, (Option<NoteKey>, Option<NoteKey>)> = BTreeMap::new();
    for link in links {
        let entry = paired.entry(link.id).or_default();
        if link.start.is_some() {
            entry.0 = link.start;
        }
        if link.end.is_some() {
            entry.1 = link.end;
        }
    }
    for (start, end) in paired.into_values() {
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let (Some(from), Some(to)) = (find_note(score, &start), find_note(score, &end)) else {
            continue;
        };
        if from == to {
            continue;
        }
        score.note_mut(from).tie_forward = Some(to);
        score.note_mut(to).tie_back = Some(from);
    }
}

/// Retrouve une note par sa position dans le modèle.
fn find_note(score: &Score, key: &NoteKey) -> Option<NoteId> {
    let measure = score.measures.get(key.measure)?;
    let track = Track::from_raw(key.track);
    let mut segments: Vec<_> = measure.segments.iter().collect();
    segments.sort_by_key(|segment| segment.tick);
    let mut ordinal = 0usize;
    for segment in segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        let Some(chord_rest) = segment.element(track).and_then(|item| item.as_chord_rest()) else {
            continue;
        };
        let Some(chord) = chord_rest.chord_ref() else {
            continue;
        };
        for &note in &chord.notes {
            if ordinal == key.ordinal {
                return Some(note);
            }
            ordinal += 1;
        }
    }
    None
}
