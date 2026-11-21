// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirenderinternal.cpp`
// (`renderHarmony`, 1021-1078) et de `engraving/dom/realizedharmony.cpp`
// (`generateNotes` 92-180, `getIntervals` 292+, `getActualDuration` 242+)
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

//! Réalisation et lecture MIDI des accords chiffrés.
//!
//! `RealizedHarmony::generateNotes` place la basse à l'octave 3
//! (`DEFAULT_OCTAVE - 2`) et le voicing proche `{fondamentale, degrés}` à
//! l'octave 5 (`DEFAULT_OCTAVE`) pour le style par défaut `Voicing::AUTO`.
//! `renderHarmony` joue chacune de ces hauteurs de l'attaque du symbole
//! jusqu'au symbole suivant (`HDuration::UNTIL_NEXT_CHORD_SYMBOL`, valeur
//! par défaut de `harmonyDuration`, `styledef.cpp:420`).

use scoretk_core::model::{Annotation, Harmony, Score, StaffId};

/// Classe de hauteur d'un TPC.
///
/// `tpc2pitch` (`engraving/dom/pitchspelling.cpp:202-222`) lit une table
/// indexée par `tpc - Tpc::TPC_MIN` : chaque pas sur le cercle des quintes
/// ajoute 7 demi-tons. Sur les classes, cela vaut `(7 * tpc + 10) mod 12`,
/// formule qui couvre aussi les tons bémols (`tpc` < 13, p. ex. si bémol
/// `TPC_B_B` = 12), là où une lecture par naturels + altération les
/// confondait avec fa dièse et suivants.
fn pitch_class(tpc: i8) -> i32 {
    (7 * i32::from(tpc) + 10).rem_euclid(12)
}

/// Intervalles en demi-tons du voicing proche pour un nom d'accord.
/// Les noms non reconnus retombent sur majeure/mineure, comme
/// `ParsedChord::understandable()` de MuseScore pour les cas simples.
fn intervals(name: &str) -> &'static [i32] {
    match name {
        "" | "maj" => &[4, 7],
        "m" | "min" => &[3, 7],
        "5" => &[7],
        "6" => &[4, 7, 9],
        "m6" => &[3, 7, 9],
        "7" | "dom" => &[4, 7, 10],
        "m7" => &[3, 7, 10],
        "maj7" => &[4, 7, 11],
        "dim" => &[3, 6],
        "dim7" => &[3, 6, 9],
        "aug" => &[4, 8],
        "sus2" => &[2, 7],
        "sus4" => &[5, 7],
        "9" => &[4, 7, 10, 14],
        "m9" => &[3, 7, 10, 14],
        _ if name.starts_with('m') => &[3, 7],
        _ => &[4, 7],
    }
}

/// Hauteurs réalisées pour un symbole (`generateNotes`, style `AUTO`).
pub(crate) fn realize(harmony: &Harmony) -> Vec<u8> {
    if harmony.root_tpc == scoretk_core::model::tpc::INVALID {
        return Vec::new();
    }
    let root = pitch_class(harmony.root_tpc);
    let bass = harmony.bass_tpc.map_or(root, pitch_class);
    let mut pitches = vec![(bass + 36) as u8];
    pitches.push((root + 60) as u8);
    for interval in intervals(&harmony.name) {
        pitches.push(((root + interval).rem_euclid(12) + 60) as u8);
    }
    pitches
}

/// `(tick source, durée en ticks source, hauteurs)` pour une portée.
///
/// La durée est celle du symbole courant jusqu'au suivant (ou la fin du
/// dernier mesure), comme `getActualDuration` avec
/// `HDuration::UNTIL_NEXT_CHORD_SYMBOL`.
pub(crate) fn events(score: &Score, staff: StaffId) -> Vec<(i64, i64, Vec<u8>)> {
    let mut found: Vec<(i64, Vec<u8>)> = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if entry.track.staff() != staff {
                    continue;
                }
                if let Annotation::Harmony(harmony) = &entry.annotation {
                    // `renderHarmony` n'est appelé que si `h->play()`
                    // (`compatmidirenderinternal.cpp:1196-1208`) : un symbole
                    // désactivé reste affiché mais ne sonne pas.
                    if !harmony.play {
                        continue;
                    }
                    let pitches = realize(harmony);
                    if !pitches.is_empty() {
                        found.push((
                            segment.tick.ticks(crate::repeat_list::UNITS_PER_WHOLE),
                            pitches,
                        ));
                    }
                }
            }
        }
    }
    found.sort_by_key(|(tick, _)| *tick);
    let end = score.measures.last().map_or(0, |measure| {
        measure
            .end_tick()
            .ticks(crate::repeat_list::UNITS_PER_WHOLE)
    });
    let mut result = Vec::new();
    for index in 0..found.len() {
        let (tick, pitches) = found[index].clone();
        let next = found.get(index + 1).map_or(end, |(tick, _)| *tick);
        result.push((tick, (next - tick).max(1), pitches));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::tpc;

    #[test]
    fn voicing_proche_majeur_mineur_et_septime() {
        let mut dm = Harmony::new("Dm");
        dm.name = "m".to_owned();
        dm.root_tpc = tpc::D;
        assert_eq!(realize(&dm), vec![38, 62, 65, 69]);
        let mut a7 = Harmony::new("A7");
        a7.name = "7".to_owned();
        a7.root_tpc = tpc::A;
        assert_eq!(realize(&a7), vec![45, 69, 61, 64, 67]);
    }
}
