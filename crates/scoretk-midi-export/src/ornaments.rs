// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirender.cpp`
// (`CompatMidiRender::renderNoteArticulation`, 668-810) et de la table
// `excursions` de `engraving/compat/midi/compatmidirenderinternal.h:80-100`
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
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

//! Excursions MIDI des ornements.
//!
//! MuseScore ne rend pas un trille comme une note tenue : il le décompose en
//! notes courtes alternant le degré écrit et le degré supérieur. La table
//! `excursions` donne, pour `ornamentTrill` et le style par défaut, un pas de
//! 32e (`_32nd = Constants::DIVISION / 8 = 60` ticks à 480/noire), un préfixe
//! `{0, 1}`, un corps `{0, 1}` répété et la dernière note prolongeant le reste
//! de la durée (`compatmidirenderinternal.h:89-92`).
//!
//! `renderNoteArticulation` (`compatmidirender.cpp:668-810`) calcule le nombre
//! de répétitions en millièmes de la durée totale, puis convertit chaque
//! évènement en ticks. Les bornes de fréquence (64 Hz au plus rapide, 8 Hz au
//! plus lent) sont appliquées quand le tempo le demande ; pour les tempos
//! courants le pas reste 60 ticks.

use scoretk_core::model::{ChordRestKind, Note, Score, SegmentItem, Staff};

use crate::repeat_list::{DIVISION, UNITS_PER_WHOLE};

/// Pas d'une excursion de 32e, en ticks (`compatmidirenderinternal.h:87-88`).
const THIRTY_SECOND: i64 = (DIVISION / 8) as i64;
/// Fréquences limites du trille (`renderNoteArticulation`, 64 et 8 Hz).
const FASTEST_HZ: i64 = 64;
const SLOWEST_HZ: i64 = 8;

/// Degré diatonique d'un TPC dans l'ordre do, ré, mi, fa, sol, la, si.
///
/// Les TPC naturels vont de 13 (fa) à 19 (si) dans l'ordre des quintes ; un
/// dièse ajoute 7, un bémol retire 7. Le tableau convertit l'index des quintes
/// en index diatonique.
fn diatonic_step(tpc: i8) -> usize {
    const FIFTHS_TO_DIATONIC: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    let fifths = (i32::from(tpc) - 13).rem_euclid(7) as usize;
    FIFTHS_TO_DIATONIC[fifths]
}

/// Hauteurs naturelles des degrés dans l'ordre do, ré, mi, fa, sol, la, si.
const NATURAL_PITCH_CLASS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Ordre des dièses et des bémols dans une armure (index de degré).
const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
const FLAT_ORDER: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];

/// Altérations de l'armure par degré (do..si) pour `fifths` dièses (+)
/// ou bémols (−), comme `KeySig::concertKey().fifths()` du modèle.
fn key_accidentals(fifths: i8) -> [i32; 7] {
    let mut result = [0i32; 7];
    if fifths > 0 {
        for step in SHARP_ORDER.iter().take(fifths as usize) {
            result[*step] = 1;
        }
    } else if fifths < 0 {
        for step in FLAT_ORDER.iter().take((-fifths) as usize) {
            result[*step] = -1;
        }
    }
    result
}

/// Hauteur du degré `steps` au-dessus de la note écrite dans l'armure courante
/// (`chromaticPitchSteps`/`tpcInterval` de `compatmidirender.cpp`).
fn step_pitch(note: &Note, steps: i32, fifths: i8) -> u8 {
    if steps == 0 {
        return note.pitch;
    }
    let accidentals = key_accidentals(fifths);
    let step = diatonic_step(note.tpc);
    let target = (step as i32 + steps).rem_euclid(7) as usize;
    let target_pc = (NATURAL_PITCH_CLASS[target] + accidentals[target]).rem_euclid(12);
    let note_pc = i32::from(note.pitch).rem_euclid(12);
    let delta = (target_pc - note_pc).rem_euclid(12);
    (i32::from(note.pitch) + if delta == 0 { 12 } else { delta }).clamp(0, 127) as u8
}

/// Degré diatonique **descendant** (`steps < 0`) : `step_pitch` cherche
/// toujours vers le haut, ce qui donnerait l'octave supérieure pour
/// l'auxiliaire inférieur d'un gruppetto (`{1, 0, -1, 0}`).
fn step_pitch_below(note: &Note, steps: i32, fifths: i8) -> u8 {
    let accidentals = key_accidentals(fifths);
    let step = diatonic_step(note.tpc);
    let target = (step as i32 + steps).rem_euclid(7) as usize;
    let target_pc = (NATURAL_PITCH_CLASS[target] + accidentals[target]).rem_euclid(12);
    let note_pc = i32::from(note.pitch).rem_euclid(12);
    let delta = (note_pc - target_pc).rem_euclid(12);
    (i32::from(note.pitch) - if delta == 0 { 12 } else { delta }).clamp(0, 127) as u8
}

/// Hauteur du degré supérieur quand un accidentique a déjà été écrit sur la
/// même ligne dans la mesure (`chromaticPitchSteps`, `dom/utils.cpp:807`).
///
/// `chromaticPitchSteps` cherche en arrière, dans la même mesure et sur la
/// même portée, la note la plus récente de la ligne visée, toutes octaves
/// confondues, et retient son accident : la mesure de Minuit chrétien porte un
/// mi bémol avant le trille de ré, dont l'auxiliaire est donc mi bémol (75) et
/// non mi naturel (76).
pub(crate) fn previous_accidental_auxiliary(
    score: &Score,
    staff: &Staff,
    tick: i64,
    note: &Note,
) -> Option<u8> {
    let target = (diatonic_step(note.tpc) + 1) % 7;
    let mut found: Option<(i64, i32)> = None;
    for measure in &score.measures {
        let start = measure.start_tick.ticks(UNITS_PER_WHOLE);
        let end = (measure.start_tick + measure.len).ticks(UNITS_PER_WHOLE);
        if tick < start || tick >= end {
            continue;
        }
        for segment in &measure.segments {
            let segment_tick = segment.tick.ticks(UNITS_PER_WHOLE);
            if segment_tick >= tick {
                break;
            }
            for (track, item) in segment.iter_elements() {
                if track.staff() != staff.id {
                    continue;
                }
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    let candidate = &score.notes[id.index()];
                    if diatonic_step(candidate.tpc) != target {
                        continue;
                    }
                    if found.is_none_or(|(found_tick, _)| segment_tick >= found_tick) {
                        found = Some((segment_tick, i32::from(candidate.pitch).rem_euclid(12)));
                    }
                }
            }
        }
    }
    let (_, pitch_class) = found?;
    let base = i32::from(note.pitch);
    let delta = (pitch_class - base.rem_euclid(12)).rem_euclid(12);
    Some((base + if delta == 0 { 12 } else { delta }).clamp(0, 127) as u8)
}

/// Évènements d'un trille sur `note` : `(décalage en ticks, hauteur, durée)`.
///
/// `span` est la durée **totale de la chaîne de liaisons**
/// (`totalTiedNoteTicks`, `compatmidirender.cpp:1196-1204`) : elle détermine le
/// nombre d'évènements, qui traversent donc les mesures liées. `chord_ticks`
/// est la durée effective de l'accord qui porte l'ornement : c'est elle que
/// `makeEvent` divise (`ontime / chord->actualTicks().ticks()`), si bien que
/// chaque évènement tombe à `k × pas` ticks. `beats` est le tempo en
/// battements par seconde au tick source. Le style est celui par défaut de
/// MuseScore ; le style baroque (préfixe `{1,0}`) n'est pas encore modélisé
/// dans le pivot.
#[allow(clippy::too_many_arguments)]
pub(crate) fn trill(
    note: &Note,
    span: i64,
    chord_ticks: i64,
    graces_before: usize,
    graces_after: usize,
    beats_per_second: f64,
    fifths: i8,
    auxiliary: Option<u8>,
) -> Vec<(i64, u8, i64)> {
    if span <= 0 || chord_ticks <= 0 {
        return Vec::new();
    }
    let ticks_per_second = (beats_per_second * f64::from(DIVISION)).round() as i64;
    if ticks_per_second <= 0 {
        return Vec::new();
    }
    let min_ticks = (ticks_per_second / FASTEST_HZ).max(1);
    let max_ticks = ticks_per_second / SLOWEST_HZ;
    let mut step = THIRTY_SECOND;
    if step < min_ticks {
        step = min_ticks;
    }
    if max_ticks > 0 && step > max_ticks {
        step = THIRTY_SECOND;
        while step > max_ticks && step > 1 {
            step /= 2;
        }
        if step < min_ticks {
            step = min_ticks;
        }
    }
    // Préfixe `{0,1}`, corps `{0,1}` et les places des ornements d'agrément
    // fusionnés (`gnb + p + b + s + gna`, `compatmidirender.cpp:691-696`). Si
    // le total ne tient pas dans la durée, `renderNoteArticulation` divise le
    // pas par leur nombre (division entière), ou renonce. `graces_before` et
    // `graces_after` ne produisent aucune note : l'excursion est seulement
    // raccourcie d'autant (les places restent silencieuses en fin d'accord).
    let graces = (graces_before + graces_after) as i64;
    let nombre = 4 + graces;
    if step * nombre > span {
        if step == min_ticks {
            return Vec::new();
        }
        step = span / nombre;
        if step < min_ticks {
            return Vec::new();
        }
    }
    if step <= 0 {
        return Vec::new();
    }

    // Toutes les quantités ci-dessous sont en millièmes de `span`
    // (`millespernote`, `space = 1000 * maxticks`).
    let milles = 1000 * step;
    let repeats = (1000 * span - milles * (graces + 2)) / (milles * 2);
    let sustain = 1000 * span - milles * (graces + 2 + repeats * 2);

    let mut events = Vec::new();
    let mut ontime = 0i64;
    let mut emit = |degre: i32, duration: i64, events: &mut Vec<(i64, u8, i64)>| {
        // `makeEvent` : `e.ontime = ontime / chord->actualTicks()` puis
        // `on = tick1 + ticks * e.ontime / 1000` (`collectNote`).
        let per_mille = ontime / chord_ticks;
        let offset = chord_ticks * per_mille / 1000;
        let length = chord_ticks * (duration / chord_ticks) / 1000 - 1;
        if length > 0 {
            let pitch = if degre == 0 {
                step_pitch(note, degre, fifths)
            } else {
                auxiliary.unwrap_or_else(|| step_pitch(note, degre, fifths))
            };
            events.push((offset, pitch, length));
        }
        ontime += milles;
    };
    emit(0, milles, &mut events);
    emit(1, milles, &mut events);
    // `renderNoteArticulation` (`compatmidirender.cpp:868-881`) : toutes les
    // répétitions du corps sauf la dernière, puis le corps sans sa dernière
    // note, puis cette dernière note prolongée de `sustain`.
    for _ in 0..repeats.saturating_sub(1) {
        emit(0, milles, &mut events);
        emit(1, milles, &mut events);
    }
    emit(0, milles, &mut events);
    emit(1, milles + sustain, &mut events);
    events
}

/// Évènements d'un mordant supérieur court (`ornamentShortTrill`,
/// « prall ») : `(décalage, hauteur, durée)`.
///
/// La table `excursions` (`compatmidirenderinternal.h:95-106`) donne pour
/// `ornamentShortTrill` et le style par défaut un corps `{0, 1, 0}`, sans
/// répétition, avec prolongation de la dernière note. C'est le même calcul
/// de pas et de millièmes que le trille.
#[allow(clippy::too_many_arguments)]
pub(crate) fn short_trill(
    note: &Note,
    span: i64,
    chord_ticks: i64,
    beats_per_second: f64,
    fifths: i8,
    auxiliary: Option<u8>,
) -> Vec<(i64, u8, i64)> {
    if span <= 0 || chord_ticks <= 0 {
        return Vec::new();
    }
    let ticks_per_second = (beats_per_second * f64::from(DIVISION)).round() as i64;
    if ticks_per_second <= 0 {
        return Vec::new();
    }
    let min_ticks = (ticks_per_second / FASTEST_HZ).max(1);
    let mut step = THIRTY_SECOND.max(min_ticks);
    if step * 3 > span {
        step = span / 3;
        if step < min_ticks {
            return Vec::new();
        }
    }
    let milles = 1000 * step;
    let sustain = (1000 * span - milles * 3).max(0);
    let mut events = Vec::new();
    let mut emit = |degree: i32, ontime: i64, duration: i64| {
        let per_mille = ontime / chord_ticks;
        let offset = chord_ticks * per_mille / 1000;
        let length = chord_ticks * (duration / chord_ticks) / 1000 - 1;
        if length > 0 {
            let pitch = if degree == 0 {
                note.pitch
            } else {
                auxiliary.unwrap_or_else(|| step_pitch(note, degree, fifths))
            };
            events.push((offset, pitch, length));
        }
    };
    emit(0, 0, milles);
    emit(1, milles, milles);
    emit(0, 2 * milles, milles + sustain);
    events
}

/// Évènements d'un gruppetto (`ornamentTurn`) : `(décalage, hauteur, durée)`.
///
/// La table `excursions` (`compatmidirenderinternal.h:88`) donne un corps
/// `{ 1, 0, -1, 0 }` en croches doubles, sans répétition, avec prolongation
/// de la dernière note (`sustainp=true`),
/// `compatmidirender.cpp:665-800`. Comme pour `short_trill`, si la durée ne
/// contient pas quatre croches doubles, le pas est divisé par quatre.
pub(crate) fn turn(
    note: &Note,
    span: i64,
    chord_ticks: i64,
    beats_per_second: f64,
    fifths: i8,
) -> Vec<(i64, u8, i64)> {
    if span <= 0 || chord_ticks <= 0 {
        return Vec::new();
    }
    let ticks_per_second = (beats_per_second * f64::from(DIVISION)).round() as i64;
    if ticks_per_second <= 0 {
        return Vec::new();
    }
    let min_ticks = (ticks_per_second / FASTEST_HZ).max(1);
    let mut step = THIRTY_SECOND.max(min_ticks);
    if step * 4 > span {
        step = span / 4;
        if step < min_ticks {
            return Vec::new();
        }
    }
    let milles = 1000 * step;
    let sustain = (1000 * span - milles * 4).max(0);
    let mut events = Vec::new();
    for (index, degree) in [1, 0, -1, 0].into_iter().enumerate() {
        let ontime = (index as i64) * milles;
        let duration = if index == 3 { milles + sustain } else { milles };
        let per_mille = ontime / chord_ticks;
        let offset = chord_ticks * per_mille / 1000;
        let length = chord_ticks * (duration / chord_ticks) / 1000 - 1;
        if length > 0 {
            let pitch = if degree < 0 {
                step_pitch_below(note, degree, fifths)
            } else {
                step_pitch(note, degree, fifths)
            };
            events.push((offset, pitch, length));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::tpc;

    #[test]
    fn pas_du_trille_et_degre_superieur() {
        let note = Note::new(72, tpc::C);
        assert_eq!(step_pitch(&note, 0, 0), 72);
        assert_eq!(step_pitch(&note, 1, 0), 74); // do → ré
        let si = Note::new(71, tpc::B);
        assert_eq!(step_pitch(&si, 1, 0), 72); // si → do
        let fis = Note::new(78, tpc::F_SHARP);
        assert_eq!(step_pitch(&fis, 1, 2), 79); // ré majeur : fa# → sol naturel
        let events = trill(&note, 480, 480, 0, 0, 2.0, 0, None);
        assert_eq!(events.len(), 8);
        assert_eq!(events[0], (0, 72, 59));
        assert_eq!(events[1], (60, 74, 59));
    }

    #[test]
    fn trille_sur_une_chaine_de_liaisons() {
        // Quatre blanches pointées liées (1440 ticks chacune) : `span` vaut
        // 5760 et le trille remplit la chaîne entière, 96 évènements de 60
        // ticks, comme MuseScore 4.7.4 (`totalTiedNoteTicks`).
        let note = Note::new(72, tpc::C);
        let events = trill(&note, 5760, 1440, 0, 0, 2.0, 0, None);
        assert_eq!(events.len(), 96);
        assert_eq!(events[0], (0, 72, 58));
        assert_eq!(events[1], (59, 74, 58));
        assert_eq!(events[2], (119, 72, 58));
        assert_eq!(events[95], (5699, 74, 58));
    }
}
