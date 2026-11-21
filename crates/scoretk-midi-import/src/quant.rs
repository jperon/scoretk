// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_quant.cpp` et `importmidi_quant.h` de MuseScore
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

//! Quantification des attaques et des fins de notes.
//!
//! Portage de `Quantize` (`importmidi_quant.cpp`) : la quantification part de
//! la plus petite valeur choisie dans les préférences (1/16 par défaut), se
//! réduit pour les notes courtes, tient compte des n-olets et préserve un
//! léger legato. La variante complète de MuseScore choisit en plus l'attaque
//! sur une grille métrique par programmation dynamique ; la variante portée
//! ici applique directement `findQuantizedChordOnTime`, ce qui donne le même
//! résultat sur une performance déjà quantifiée et un résultat plus proche de
//! la grille binaire sur une performance humaine (écart documenté).

use scoretk_core::model::Fraction;

use super::chord::{min_allowed_duration, ChordMap, MTrack};
use super::midifile::{DIVISION, UNITS_PER_WHOLE};
use super::operations::QuantValue;
use super::tuplet::{tuplet_limits, TupletData};

/// Valeur de quantification en fraction de ronde (`quantValueToFraction`).
pub fn quant_value_to_fraction(quant_value: QuantValue) -> Fraction {
    let division = Fraction::from_ticks(DIVISION, UNITS_PER_WHOLE as u32);
    match quant_value {
        QuantValue::Q4 => division,
        QuantValue::Q8 => division * Fraction::HALF,
        QuantValue::Q16 => division * Fraction::new(1, 4),
        QuantValue::Q32 => division * Fraction::new(1, 8),
        QuantValue::Q64 => division * Fraction::new(1, 16),
        QuantValue::Q128 => division * Fraction::new(1, 32),
        QuantValue::Q256 => division * Fraction::new(1, 64),
        QuantValue::Q512 => division * Fraction::new(1, 128),
        QuantValue::Q1024 => division * Fraction::new(1, 256),
    }
}

/// Quantifie une valeur sur une grille (`quantizeValue`).
///
/// Reproduit l'arithmétique entière du C++ : `((v + q/2) / q) * q`, en
/// ramenant les fractions au même dénominateur.
pub fn quantize_value(value: Fraction, quant: Fraction) -> Fraction {
    if quant.is_zero() {
        return value;
    }
    let val_num = i128::from(value.numerator()) * i128::from(quant.denominator());
    let rast_num = i128::from(quant.numerator()) * i128::from(value.denominator());
    let common_den = i128::from(value.denominator()) * i128::from(quant.denominator());
    if rast_num == 0 {
        return value;
    }
    let rounded = ((val_num + rast_num / 2) / rast_num) * rast_num;
    Fraction::new(
        i64::try_from(rounded).unwrap_or(value.numerator()),
        i64::try_from(common_den).unwrap_or(value.denominator()),
    )
}

/// Réduit la grille pour une note pointée (`reduceQuantIfDottedNote`).
fn reduce_quant_if_dotted(note_len: Fraction, raster: Fraction) -> Fraction {
    let ratio = (note_len / raster).as_f64();
    if (1.45..1.55).contains(&ratio) {
        raster * Fraction::HALF
    } else {
        raster
    }
}

/// Grille adaptée à la longueur d'une note (`quantForLen`).
pub fn quant_for_len(note_len: Fraction, basic_quant: Fraction) -> Fraction {
    let mut quant = basic_quant;
    let minimum = min_allowed_duration() * Fraction::from_integer(2);
    while quant > note_len && quant >= minimum {
        quant *= Fraction::HALF;
    }
    if quant >= minimum {
        quant = reduce_quant_if_dotted(note_len, quant);
    }
    quant
}

/// Grille adaptée à un n-olet (`quantForTuplet`).
pub fn quant_for_tuplet(tuplet_len: Fraction, tuplet_ratio: Fraction) -> Fraction {
    let mut quant = tuplet_len / Fraction::from_integer(tuplet_ratio.numerator().max(1));
    let minimum = min_allowed_duration() * Fraction::from_integer(2);
    if quant >= minimum {
        quant *= Fraction::HALF;
    }
    quant.max(min_allowed_duration())
}

/// Plus petite grille des notes d'un accord (`findMinQuant`).
fn find_min_quant(
    on_time: Fraction,
    chord: &super::chord::MidiChord,
    basic_quant: Fraction,
) -> Fraction {
    let mut min_quant = Fraction::from_integer(-1);
    for note in &chord.notes {
        let quant = quant_for_len(note.off_time - on_time, basic_quant);
        if min_quant.is_negative() || quant < min_quant {
            min_quant = quant;
        }
    }
    min_quant
}

/// Attaque quantifiée d'un accord (`findQuantizedChordOnTime`).
pub fn find_quantized_chord_on_time(
    on_time: Fraction,
    chord: &super::chord::MidiChord,
    basic_quant: Fraction,
) -> Fraction {
    let quant = if chord.notes.is_empty() {
        basic_quant
    } else {
        find_min_quant(on_time, chord, basic_quant)
    };
    quantize_value(on_time, quant)
}

/// Attaque quantifiée d'un accord situé dans un n-olet
/// (`findQuantizedTupletChordOnTime`).
pub fn find_quantized_tuplet_chord_on_time(
    on_time: Fraction,
    tuplet: &TupletData,
    tuplet_ratio: Fraction,
) -> Fraction {
    if on_time <= tuplet.on_time {
        return tuplet.on_time;
    }
    let quant = quant_for_tuplet(tuplet.len, tuplet_ratio);
    tuplet.on_time + quantize_value(on_time - tuplet.on_time, quant)
}

/// Fin de note quantifiée (`findQuantizedNoteOffTime`).
///
/// Renvoie `(fin quantifiée, grille retenue)`.
pub fn find_quantized_note_off_time(
    on_time: Fraction,
    off_time: Fraction,
    basic_quant: Fraction,
) -> (Fraction, Fraction) {
    let mut quant = quant_for_len(off_time - on_time, basic_quant);
    loop {
        let mut q_off = quantize_value(off_time, quant);
        if q_off <= on_time {
            if quant >= min_allowed_duration() * Fraction::from_integer(2) {
                quant *= Fraction::HALF;
                continue;
            }
            q_off = on_time + quant;
        }
        return (q_off, quant);
    }
}

/// Fin de note quantifiée à l'intérieur d'un n-olet
/// (`findQuantizedTupletNoteOffTime`).
pub fn find_quantized_tuplet_note_off_time(
    on_time: Fraction,
    off_time: Fraction,
    tuplet: &TupletData,
    tuplet_ratio: Fraction,
) -> (Fraction, Fraction) {
    if off_time <= tuplet.on_time {
        return (tuplet.on_time, tuplet.len);
    }
    let mut quant = quant_for_tuplet(tuplet.len, tuplet_ratio);
    loop {
        let mut q_off = tuplet.on_time + quantize_value(off_time - tuplet.on_time, quant);
        if q_off <= on_time {
            if quant >= min_allowed_duration() * Fraction::from_integer(2) {
                quant *= Fraction::HALF;
                continue;
            }
            q_off = on_time + quant;
        }
        return (q_off, quant);
    }
}

/// Plus petite attaque quantifiée des notes d'un accord
/// (`findMinQuantizedOnTime`).
pub fn find_min_quantized_on_time(
    chord: &super::chord::MidiChord,
    on_time: Fraction,
    basic_quant: Fraction,
) -> Fraction {
    let mut min_on_time = Fraction::from_integer(-1);
    for note in &chord.notes {
        let quant = quant_for_len(note.off_time - on_time, basic_quant);
        let quantized = quantize_value(on_time, quant);
        if min_on_time.is_negative() || quantized < min_on_time {
            min_on_time = quantized;
        }
    }
    if min_on_time.is_negative() {
        find_quantized_chord_on_time(on_time, chord, basic_quant)
    } else {
        min_on_time
    }
}

/// Plus grande fin quantifiée des notes d'un accord (`findMaxQuantizedOffTime`).
pub fn find_max_quantized_off_time(
    chord: &super::chord::MidiChord,
    on_time: Fraction,
    basic_quant: Fraction,
) -> Fraction {
    let mut max_off = Fraction::ZERO;
    for note in &chord.notes {
        let off = find_quantized_note_off_time(on_time, note.off_time, basic_quant).0;
        if off > max_off {
            max_off = off;
        }
    }
    max_off
}

/// Quantifie un temps vers le haut (`quantizeToLarge`).
pub fn quantize_to_large(time: Fraction, quant: Fraction) -> Fraction {
    if quant.is_zero() {
        return time;
    }
    let ratio = time / quant;
    let mut quantized = quant * Fraction::from_integer(ratio.numerator() / ratio.denominator());
    if quantized < time {
        quantized += quant;
    }
    quantized
}

/// Quantifie un temps vers le bas (`quantizeToSmall`).
pub fn quantize_to_small(time: Fraction, quant: Fraction) -> Fraction {
    if quant.is_zero() {
        return time;
    }
    let ratio = time / quant;
    let mut quantized = quant * Fraction::from_integer(ratio.numerator() / ratio.denominator());
    if quantized >= time {
        quantized -= quant;
    }
    quantized
}

/// Vrai si une performance semble humaine (`isHumanPerformance`).
///
/// Compte les attaques déjà sur un temps fort : si moins de 60 % d'entre elles
/// sont exactes, la performance est considérée comme humaine.
pub fn is_human_performance(track: &MTrack, sigmap: &super::timesigmap::TimeSigMap) -> bool {
    if track.chords.is_empty() {
        return false;
    }
    let basic_quant = Fraction::from_ticks(DIVISION, UNITS_PER_WHOLE as u32) * Fraction::new(1, 4);
    let mut matches = 0_i32;
    let mut count = 0_i32;
    let mut used_on_times: Vec<Fraction> = Vec::new();

    for (on_time, chord) in track.chords.iter() {
        let quant = basic_quant.min(quant_for_len(
            super::chord::max_note_len(on_time, chord),
            basic_quant,
        ));
        let original_on_time = on_time;
        let on_time = quantize_value(on_time, quant);
        let (bar_index, _, _) = sigmap.tick_values(on_time.ticks(UNITS_PER_WHOLE as u32));
        let bar_start = super::timesigmap::fraction_from_ticks(sigmap.bar2tick(bar_index, 0));
        let ts = sigmap.timesig(bar_start.ticks(UNITS_PER_WHOLE as u32));
        let bar_fraction = ts.measure_len();
        let beat_len = super::meter::beat_length_raw(bar_fraction, ts.numerator);
        if beat_len.is_zero() {
            continue;
        }
        let ratio = (on_time - bar_start) / beat_len;
        if ratio.denominator() == 1 && !used_on_times.contains(&on_time) {
            used_on_times.push(on_time);
            count += 1;
            if (on_time - original_on_time).abs() < min_allowed_duration() {
                matches += 1;
            }
        }
    }
    if count == 0 {
        return false;
    }
    let matched = f64::from(matches) / f64::from(count);
    matched < 0.6
}

/// Préserve un léger legato en coupant la fin de note au prochain accord de
/// la même voix (`preserveLegato`, cas non-n-olet).
fn preserve_legato(
    off_time: &mut Fraction,
    on_time: Fraction,
    voice: usize,
    next: Option<(Fraction, bool)>,
    basic_quant: Fraction,
) {
    if let Some((next_on_time, next_is_in_tuplet)) = next {
        if next_is_in_tuplet {
            return;
        }
        let inter_onset = next_on_time - on_time;
        let cross = *off_time - next_on_time;
        if cross > Fraction::ZERO
            && cross < inter_onset * Fraction::HALF
            && cross < basic_quant * Fraction::HALF
        {
            *off_time = next_on_time;
        }
    }
    let _ = voice;
}

/// Quantifie tous les accords d'une piste (`Quantize::quantizeChords`).
///
/// Deux passes, comme le C++ : les attaques sont d'abord toutes quantifiées
/// (et fusionnées), puis les fins de notes sont quantifiées en tenant compte
/// des attaques déjà quantifiées (`preserveLegato` lit `quantizedChords`).
pub fn quantize_chords(track: &mut MTrack, basic_quant: Fraction) {
    if track.chords.is_empty() {
        return;
    }
    let keys = track.chords.keys();
    let mut entries: Vec<(crate::chord::ChordKey, Fraction, super::chord::MidiChord)> =
        Vec::with_capacity(keys.len());

    // Première passe : attaques.
    for key in &keys {
        let Some(chord) = track.chords.get(*key) else {
            continue;
        };
        let on_time = key.0;
        let q_on = if chord.is_in_tuplet {
            match chord
                .tuplet
                .and_then(|tuplet_key| track.tuplets.get(&tuplet_key))
            {
                Some(tuplet) => {
                    let ratio = tuplet_limits(tuplet.tuplet_number).ratio;
                    find_quantized_tuplet_chord_on_time(on_time, tuplet, ratio)
                }
                None => find_quantized_chord_on_time(on_time, chord, basic_quant),
            }
        } else {
            find_quantized_chord_on_time(on_time, chord, basic_quant)
        };
        entries.push((*key, q_on, chord.clone()));
    }

    // Seconde passe : fins de notes, sur les attaques quantifiées.
    let mut quantized = ChordMap::new();
    for (position, (_, q_on, chord)) in entries.iter().enumerate() {
        let q_on = *q_on;
        let mut new_chord = chord.clone();
        // Prochain accord de la même voix, au sens de `quantizedChords`.
        let next = entries
            .iter()
            .skip(position + 1)
            .find(|(_, _, other)| other.voice == chord.voice)
            .map(|(_, next_q_on, next_chord)| (*next_q_on, next_chord.is_in_tuplet));
        for note in &mut new_chord.notes {
            let (mut q_off, quant) = if note.is_in_tuplet {
                match note
                    .tuplet
                    .and_then(|tuplet_key| track.tuplets.get(&tuplet_key))
                {
                    Some(tuplet) => {
                        let ratio = tuplet_limits(tuplet.tuplet_number).ratio;
                        find_quantized_tuplet_note_off_time(q_on, note.off_time, tuplet, ratio)
                    }
                    None => find_quantized_note_off_time(q_on, note.off_time, basic_quant),
                }
            } else {
                find_quantized_note_off_time(q_on, note.off_time, basic_quant)
            };
            if !note.is_in_tuplet {
                preserve_legato(&mut q_off, q_on, new_chord.voice, next, basic_quant);
            }
            if q_off - q_on < min_allowed_duration() {
                q_off = q_on + min_allowed_duration();
            }
            note.orig_on_time = q_on;
            note.off_time = q_off;
            note.off_time_quant = Some(quant);
        }
        quantized.insert(q_on, new_chord);
    }

    remove_useless_tuplet_references(&mut quantized, &track.tuplets);
    track.chords = quantized;
    track.chords = merge_same_on_time_and_voice(std::mem::take(&mut track.chords));
}

/// Fusionne les accords de même attaque et de même voix après quantification.
fn merge_same_on_time_and_voice(mut chords: ChordMap) -> ChordMap {
    let mut result = ChordMap::new();
    let keys = chords.keys();
    for key in keys {
        let Some(chord) = chords.remove(key) else {
            continue;
        };
        let existing = result
            .iter()
            .find(|(on_time, other)| *on_time == key.0 && other.voice == chord.voice)
            .map(|(on_time, _)| on_time);
        match existing {
            Some(existing_on_time) => {
                if let Some(target) = result.iter_mut().find_map(|(on_time, other)| {
                    (on_time == existing_on_time && other.voice == chord.voice).then_some(other)
                }) {
                    target.notes.extend(chord.notes);
                }
            }
            None => {
                result.insert(key.0, chord);
            }
        }
    }
    result
}

/// Retire les références de n-olet devenues inutiles
/// (`removeUselessTupletReferences`).
fn remove_useless_tuplet_references(
    chords: &mut ChordMap,
    tuplets: &std::collections::BTreeMap<super::tuplet::TupletKey, TupletData>,
) {
    for (on_time, chord) in chords.iter_mut() {
        if chord.is_in_tuplet {
            match chord.tuplet.and_then(|key| tuplets.get(&key)) {
                Some(tuplet) if on_time >= tuplet.on_time + tuplet.len => {
                    chord.is_in_tuplet = false;
                    chord.tuplet = None;
                }
                Some(_) => {}
                None => {
                    chord.is_in_tuplet = false;
                    chord.tuplet = None;
                }
            }
        }
        for note in &mut chord.notes {
            if !note.is_in_tuplet {
                continue;
            }
            match note.tuplet.and_then(|key| tuplets.get(&key)) {
                Some(tuplet) if note.off_time <= tuplet.on_time => {
                    note.is_in_tuplet = false;
                    note.tuplet = None;
                }
                Some(_) => {}
                None => {
                    note.is_in_tuplet = false;
                    note.tuplet = None;
                }
            }
        }
    }
}
