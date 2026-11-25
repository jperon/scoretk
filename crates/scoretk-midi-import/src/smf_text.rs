// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Lecture des méta-textes SMF (`ImportOptions::smf_text`).
//!
//! `[SMF §Meta-Events]` : `FF 01` est un « Text event », `FF 06` un repère
//! (rehearsal mark) et `FF 07` un cue point. MuseScore 4.7.4 lit tout `FF 01`
//! comme une parole et ignore `FF 06` ; ce module est donc une extension
//! additive, désactivée par défaut. Les préfixes `@ST:`/`@SY:`/`@TK:` sont
//! écrits par `scoretk-midi-export::smf_text` dans la charge utile standard
//! de `FF 01` (le même procédé que les directives privées `%%scoretk` du ABC).

use scoretk_core::model::{Annotation, Fraction, RehearsalMark, StaffText, SystemText, Track};

use super::chord::MTrack;
use super::midifile::{decode_meta_text, meta, EventKind, MidiFile};
use super::{get_or_create_chord_rest_segment, measure_index_for_tick};

/// Préfixe d'un texte de portée dans `FF 01`.
const STAFF_PREFIX: &str = "@ST:";
/// Préfixe d'un texte système dans `FF 01`.
const SYSTEM_PREFIX: &str = "@SY:";
/// Préfixe d'une métadonnée dans `FF 01`.
const TITLE_PREFIX: &str = "@TK:";

/// Pose les textes et repères d'un SMF sur les portées importées.
pub(crate) fn process_text_meta(
    score: &mut scoretk_core::model::Score,
    tracks: &mut [MTrack],
    midi: &MidiFile,
) {
    // `FF 03` est le nom de séquence de la première piste (`[SMF §Meta-Events]`) :
    // l'écrivain `smf_text` y place le titre avant le nom de partie. Le lecteur
    // par défaut garde le premier comme nom de piste ; l'option additive sépare
    // les deux sans introduire de nouveau type de méta.
    for track in tracks.iter_mut() {
        let Some(staff) = track.staff else {
            continue;
        };
        let names: Vec<String> = midi.tracks[track.index]
            .events
            .iter()
            .filter_map(|(tick, raw)| match (&raw.kind, tick) {
                (EventKind::Meta { kind, data }, 0) if *kind == meta::TRACK_NAME => {
                    Some(decode_meta_text(data))
                }
                _ => None,
            })
            .collect();
        if staff.index() == 0 && names.len() >= 2 {
            score.metadata.work_title = names[0].clone();
            if track.name == names[0] {
                track.name = names[1].clone();
            }
        }
    }

    for track in tracks.iter() {
        let Some(staff) = track.staff else {
            continue;
        };
        for (raw_tick, raw) in &midi.tracks[track.index].events {
            let EventKind::Meta { kind, data } = &raw.kind else {
                continue;
            };
            let text = decode_meta_text(data);
            let tick = super::midifile::to_muse_score_ticks(
                *raw_tick,
                track.division,
                track.is_division_in_tps,
            );
            match *kind {
                meta::TEXT => {
                    if let Some(field) = text.strip_prefix(TITLE_PREFIX) {
                        let Some((name, value)) = field.split_once(':') else {
                            continue;
                        };
                        set_metadata(score, name, value);
                        continue;
                    }
                    let annotation = if let Some(value) = text.strip_prefix(STAFF_PREFIX) {
                        Annotation::StaffText(StaffText::new(value))
                    } else if let Some(value) = text.strip_prefix(SYSTEM_PREFIX) {
                        Annotation::SystemText(SystemText::new(value))
                    } else {
                        Annotation::StaffText(StaffText::new(text.clone()))
                    };
                    add_annotation(score, staff, tick, annotation);
                }
                meta::MARKER | 0x07 => {
                    add_annotation(
                        score,
                        staff,
                        tick,
                        Annotation::RehearsalMark(RehearsalMark::new(text)),
                    );
                }
                _ => {}
            }
        }
    }
}

/// Écrit une métadonnée connue, sans toucher aux champs inconnus.
fn set_metadata(score: &mut scoretk_core::model::Score, name: &str, value: &str) {
    let meta = &mut score.metadata;
    match name {
        "work_title" => meta.work_title = value.to_owned(),
        "subtitle" => meta.subtitle = value.to_owned(),
        "composer" => meta.composer = value.to_owned(),
        "lyricist" => meta.lyricist = value.to_owned(),
        "poet" => meta.poet = value.to_owned(),
        "translator" => meta.translator = value.to_owned(),
        "arranger" => meta.arranger = value.to_owned(),
        "source" => meta.source = value.to_owned(),
        "movement_title" => meta.movement_title = value.to_owned(),
        "movement_number" => meta.movement_number = value.to_owned(),
        "work_number" => meta.work_number = value.to_owned(),
        "copyright" => meta.copyright = value.to_owned(),
        _ => {}
    }
}

/// Attache une annotation au segment `ChordRest` du tick, comme le font les
/// autres lecteurs (`musicxml-import/src/direction.rs::add_annotation`).
fn add_annotation(
    score: &mut scoretk_core::model::Score,
    staff: scoretk_core::model::StaffId,
    tick: Fraction,
    annotation: Annotation,
) {
    let Some(index) = measure_index_for_tick(score, tick) else {
        return;
    };
    let segment = get_or_create_chord_rest_segment(score, index, tick);
    segment.add_annotation(Track::new(staff, 0), annotation);
}
