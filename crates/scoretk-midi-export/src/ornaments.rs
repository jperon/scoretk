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

use scoretk_core::model::Note;

use crate::repeat_list::DIVISION;

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

/// Intervalle en demi-tons vers le degré diatonique suivant.
const STEP_SEMITONES: [i32; 7] = [2, 2, 1, 2, 2, 2, 1];

/// Hauteur du degré `steps` au-dessus de la note écrite
/// (`chromaticPitchSteps` de `compatmidirender.cpp`).
fn step_pitch(note: &Note, steps: i32) -> u8 {
    let mut pitch = i32::from(note.pitch);
    let mut step = diatonic_step(note.tpc);
    let direction = if steps < 0 { -1 } else { 1 };
    for _ in 0..steps.abs() {
        if direction > 0 {
            pitch += STEP_SEMITONES[step];
            step = (step + 1) % 7;
        } else {
            step = (step + 6) % 7;
            pitch -= STEP_SEMITONES[step];
        }
    }
    pitch.clamp(0, 127) as u8
}

/// Évènements d'un trille sur `note` : `(décalage en ticks, hauteur, durée)`.
///
/// `ticks` est la durée totale de l'accord (liaisons comprises), `beats` le
/// tempo en battements par seconde au tick source. Le style est celui par
/// défaut de MuseScore ; le style baroque (préfixe `{1,0}`) n'est pas encore
/// modélisé dans le pivot.
pub(crate) fn trill(note: &Note, ticks: i64, beats_per_second: f64) -> Vec<(i64, u8, i64)> {
    if ticks <= 0 {
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
        step = max_ticks.max(min_ticks);
    }
    if step <= 0 {
        return Vec::new();
    }

    // Préfixe `{0,1}`, corps `{0,1}` (style par défaut) et aucune note de
    // grâce fusionnée : 2+2+n*2 évènements remplissent la durée.
    let milles = 1000 * step / ticks;
    if milles <= 0 || 2 * milles > 1000 {
        return Vec::new();
    }
    let repeats = (1000 - 2 * milles) / (2 * milles);
    let sustain = 1000 - milles * (2 + repeats * 2);

    let mut events = Vec::new();
    let mut ontime = 0i64;
    let mut emit = |degre: i32, duree: i64, events: &mut Vec<(i64, u8, i64)>| {
        events.push((ticks * ontime / 1000, step_pitch(note, degre), duree));
        ontime += milles;
    };
    emit(0, milles * ticks / 1000 - 1, &mut events);
    emit(1, milles * ticks / 1000 - 1, &mut events);
    for _ in 0..repeats {
        emit(0, milles * ticks / 1000 - 1, &mut events);
        emit(1, milles * ticks / 1000 - 1, &mut events);
    }
    if sustain > 0 {
        events.push((
            ticks * ontime / 1000,
            step_pitch(note, 1),
            sustain * ticks / 1000 - 1,
        ));
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
        assert_eq!(step_pitch(&note, 0), 72);
        assert_eq!(step_pitch(&note, 1), 74); // do → ré
        let si = Note::new(71, tpc::B);
        assert_eq!(step_pitch(&si, 1), 72); // si → do
        let events = trill(&note, i64::from(DIVISION), 2.0);
        assert_eq!(events.len(), 8);
        assert_eq!(events[0], (0, 72, 59));
        assert_eq!(events[1], (60, 74, 59));
    }
}
