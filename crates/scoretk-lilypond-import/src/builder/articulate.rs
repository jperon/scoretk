// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `ly/articulate.ly` (`ac:trill`, `ac:articulate-chord`, actions
// `prall`, `mordent`, `turn`) du projet LilyPond, Copyright (C) 2008--2026
// NICTA / Peter Chubb, modifié (réécrit en Rust) en 2026.
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

//! Excursions jouées de `\articulate` (`ly/articulate.ly`).
//!
//! `\articulate` remplace une note ornée par une séquence de notes courtes
//! dont la somme des durées est celle de la note écrite. Le MIDI de LilyPond
//! les joue toutes ; le MusicXML doit donc les porter comme de vraies notes
//! (pas comme des grâces : elles tombent *dans* la durée de la note, et
//! l'oracle les quantifie sur la même grille Q16).

use scoretk_core::model::{Fraction, OrnamentType};

use super::{ChordContent, EventKind, MetaEvent, StaffBuild};

/// Index diatonique d'un TPC dans l'ordre do, ré, mi, fa, sol, la, si.
fn diatonic_step(tpc: i8) -> usize {
    const FIFTHS_TO_DIATONIC: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    let fifths = (i32::from(tpc) - 13).rem_euclid(7) as usize;
    FIFTHS_TO_DIATONIC[fifths]
}

/// TPC naturel du degré diatonique.
fn natural_tpc(step: usize) -> i8 {
    const NATURAL: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];
    NATURAL[step]
}

/// Altération d'un degré dans l'armure (`ac:current-key`).
fn key_alteration(fifths: i8, step: usize) -> i8 {
    const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    const FLAT_ORDER: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];
    if fifths > 0 {
        if SHARP_ORDER[..(fifths as usize).min(7)].contains(&step) {
            1
        } else {
            0
        }
    } else if FLAT_ORDER[..(-fifths as usize).min(7)].contains(&step) {
        -1
    } else {
        0
    }
}

/// Note à `steps` degrés diatoniques de `pitch` (`ac:up`/`ac:down`).
///
/// L'altération du degré visé vient de l'armure courante, pas de la note
/// écrite : c'est exactement le comportement des helpers Scheme de
/// `ly/articulate.ly:283-306`.
fn step_note(pitch: u8, tpc: i8, steps: i32, fifths: i8) -> (u8, i8) {
    const NATURAL_PITCH_CLASS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let source_step = diatonic_step(tpc);
    let target = (source_step as i32 + steps).rem_euclid(7) as usize;
    let alteration = key_alteration(fifths, target);
    let target_pc = (NATURAL_PITCH_CLASS[target] + i32::from(alteration)).rem_euclid(12);
    let source_pc = i32::from(pitch).rem_euclid(12);
    let up = (target_pc - source_pc).rem_euclid(12);
    let down = (source_pc - target_pc).rem_euclid(12);
    let new_pitch = if steps < 0 {
        i32::from(pitch) - if down == 0 { 12 } else { down }
    } else {
        i32::from(pitch) + if up == 0 { 12 } else { up }
    };
    (
        new_pitch.clamp(0, 127) as u8,
        natural_tpc(target) + 7 * alteration,
    )
}

/// Arrondi au pair d'une fraction positive (`round` Scheme).
fn round_fraction(value: Fraction) -> i64 {
    let (n, d) = (value.numerator(), value.denominator());
    (2 * n + d) / (2 * d)
}

/// Quantifie sur la grille de 768e de ronde : `ly:music-compress` arrondit
/// les durées résultantes à cette grille dans le MIDI (vérifié sur des
/// `\mordent` de noire et de blonde compilés avec LilyPond 2.27).
fn quantize(value: Fraction) -> Fraction {
    let grid = Fraction::new(1, 768);
    let steps = round_fraction(value / grid);
    Fraction::new(steps * grid.numerator(), grid.denominator())
}

/// Séquence `(hauteur, TPC, durée)` de l'ornement, d'après
/// `ly/articulate.ly:733-780`. Les durées sont en fractions de ronde,
/// comme dans le modèle.
fn excursion(
    pitch: u8,
    tpc: i8,
    original: Fraction,
    fifths: i8,
    kind: &OrnamentType,
) -> Vec<(u8, i8, Fraction)> {
    let above = step_note(pitch, tpc, 1, fifths);
    let below = step_note(pitch, tpc, -1, fifths);
    match kind {
        // `newmusic (list above music below music)`, chaque durée divisée
        // par quatre : quatre notes égales.
        OrnamentType::Turn => vec![
            (above.0, above.1, original / Fraction::from_integer(4)),
            (pitch, tpc, original / Fraction::from_integer(4)),
            (below.0, below.1, original / Fraction::from_integer(4)),
            (pitch, tpc, original / Fraction::from_integer(4)),
        ],
        // `prallMusic (list abovenote gracenote abovenoteTwo mainnote)` :
        // trois 32es et la note principale, comprimés dans `original`.
        OrnamentType::InvertedMordent => {
            let g32 = Fraction::new(1, 32);
            let total = original + g32 + g32;
            let short = quantize(g32 * original / total);
            let long = original - short - short;
            vec![
                (above.0, above.1, short),
                (pitch, tpc, short),
                (above.0, above.1, short),
                (pitch, tpc, long),
            ]
        }
        // `mordentMusic (list gracenote belownote music)` : la note, le
        // degré inférieur, la note, comprimés dans `original`.
        OrnamentType::Mordent => {
            let g32 = Fraction::new(1, 32);
            let total = original + g32 + g32;
            let short = quantize(g32 * original / total);
            let long = original - short - short;
            vec![
                (pitch, tpc, short),
                (below.0, below.1, short),
                (pitch, tpc, long),
            ]
        }
        // `ac:trill` : `count = round((orig / 1/64) / 2)` paires
        // `(above, note)`, toutes de même durée.
        OrnamentType::TrillMark => {
            let g64 = Fraction::new(1, 64);
            let count = round_fraction(original / g64 / Fraction::from_integer(2)).max(2);
            let duration = original / Fraction::from_integer(2 * count);
            let mut result = Vec::with_capacity(2 * count as usize);
            for _ in 0..count {
                result.push((above.0, above.1, duration));
                result.push((pitch, tpc, duration));
            }
            result
        }
        _ => Vec::new(),
    }
}

/// Développe les ornements joués par `\articulate` dans les événements des
/// voix. Sans `\articulate`, LilyPond ne joue pas l'ornement : la gravure
/// seule reste dans le modèle.
pub(crate) fn expand_articulate(staves: &mut [StaffBuild], enabled: bool) {
    if !enabled {
        return;
    }
    // `ac:current-key` est une variable Scheme **globale** à toute la musique
    // articulée (`ly/articulate.ly:193-202`), initialisée en do majeur et
    // remplacée par chaque `KeyChangeEvent` (l. 818-820). `ac:up`/`ac:down`
    // (l. 283-306) en tire l'altération du degré voisin. Une armure posée
    // dans un `\global` parallèle vaut donc pour les ornements de *toutes*
    // les voix, même si celles-ci ne portent aucun événement `\key`.
    let mut keys: Vec<(Fraction, usize, i8)> = staves
        .iter()
        .flat_map(|staff| staff.voices.iter())
        .flat_map(|voice| voice.interpreted.events.iter())
        .filter_map(|event| match &event.kind {
            EventKind::Meta(MetaEvent::Key(key)) => {
                Some((event.tick, event.order, key.concert_key.fifths()))
            }
            _ => None,
        })
        .collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    for staff in staves.iter_mut() {
        for voice in &mut staff.voices {
            let mut events = Vec::with_capacity(voice.interpreted.events.len());
            for event in voice.interpreted.events.drain(..) {
                let position = keys.partition_point(|(tick, _, _)| *tick <= event.tick);
                let fifths = position.checked_sub(1).map_or(0, |index| keys[index].2);
                let EventKind::Chord(chord) = &event.kind else {
                    events.push(event);
                    continue;
                };
                let ChordContent::Notes(notes) = &chord.content else {
                    events.push(event);
                    continue;
                };
                let Some(kind) = chord.ornaments.first().cloned() else {
                    events.push(event);
                    continue;
                };
                if notes.len() != 1 || !chord.actual.is_positive() {
                    events.push(event);
                    continue;
                }
                let sequence = excursion(notes[0].pitch, notes[0].tpc, chord.actual, fifths, &kind);
                let count = sequence.len();
                if count < 2 {
                    events.push(event);
                    continue;
                }
                let mut tick = event.tick;
                for (index, (pitch, tpc, duration)) in sequence.into_iter().enumerate() {
                    let mut expanded = event.clone();
                    if let EventKind::Chord(chord) = &mut expanded.kind {
                        chord.ornaments.clear();
                        chord.explicit_ticks = Some(duration);
                        chord.actual = duration;
                        chord.grace_before.clear();
                        chord.grace_after.clear();
                        chord.breath = None;
                        if let ChordContent::Notes(notes) = &mut chord.content {
                            notes[0].pitch = pitch;
                            notes[0].tpc = tpc;
                            notes[0].tpc_transposed = tpc;
                            if index + 1 < count {
                                notes[0].tie = false;
                            }
                        }
                    }
                    expanded.tick = tick;
                    expanded.order = events.len();
                    tick += duration;
                    events.push(expanded);
                }
            }
            voice.interpreted.events = events;
        }
    }
}
