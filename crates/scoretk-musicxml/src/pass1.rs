// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass1.cpp`,
// `musicxmlpart.cpp`, `musicxmlvoicedesc.cpp` et
// `importmusicxmlnoteduration.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Première passe : structure globale du document.
//!
//! Portage de `MusicXmlParserPass1` (`importmusicxmlpass1.cpp`). Cette passe ne
//! construit aucun objet du modèle : elle mesure les mesures de chaque partie,
//! répartit les voix MusicXML sur les voix MuseScore (`allocateVoices`), et
//! relève les métadonnées de `part-list`. La deuxième passe
//! ([`crate::pass2`]) a besoin des longueurs globales de mesures (maximum sur
//! toutes les parties) et de la table des voix avant de placer le moindre
//! accord.

use std::collections::BTreeMap;

use mscz2xml::model::{Fraction, TimeSigType, Transpose};

use crate::error::{Error, Result};
use crate::noteduration::{self, NoteTiming};
use crate::xml::Element;

/// Nombre maximal de portées suivi dans une partie, comme `MAX_STAVES`.
const MAX_STAVES: usize = 32;

/// Instrument déclaré dans `part-list`.
#[derive(Clone, Debug, Default)]
pub struct InstrumentData {
    /// Identifiant MusicXML (`P1-I1`).
    pub id: String,
    /// `<instrument-name>` (`Instrument::trackName`).
    pub name: String,
    /// `<instrument-sound>` (`Instrument::id`).
    pub sound: String,
    /// `<midi-channel>` moins un, si présent.
    pub channel: Option<u8>,
    /// `<midi-program>` moins un, si présent.
    pub program: Option<u8>,
    /// Volume du canal (0-127), si présent.
    pub volume: Option<u8>,
    /// Panoramique du canal (0-127), si présent.
    pub pan: Option<u8>,
}

/// Description d'une voix MusicXML dans une partie (`VoiceDesc`).
#[derive(Clone, Debug, Default)]
pub struct VoiceDesc {
    /// Nombre d'accords ou silences par portée de la partie.
    pub counts: Vec<usize>,
    /// Portée préférée (`preferredStaff`).
    pub staff: usize,
    /// Voix MuseScore attribuée (0-3).
    pub voice: usize,
}

impl VoiceDesc {
    fn incr(&mut self, staff: usize) {
        if staff < MAX_STAVES {
            if self.counts.len() <= staff {
                self.counts.resize(staff + 1, 0);
            }
            self.counts[staff] += 1;
        }
    }

    fn preferred_staff(&self) -> usize {
        let mut best = 0;
        let mut best_count = 0;
        for (staff, count) in self.counts.iter().enumerate() {
            if *count > best_count {
                best_count = *count;
                best = staff;
            }
        }
        best
    }
}

/// Indication de mesure relevée dans une mesure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSigData {
    /// Numérateur.
    pub numerator: i32,
    /// Dénominateur.
    pub denominator: i32,
    /// Type d'affichage.
    pub kind: TimeSigType,
}

impl TimeSigData {
    /// Longueur de la mesure en fractions de ronde.
    pub fn measure_len(self) -> Fraction {
        Fraction::try_new(i64::from(self.numerator), i64::from(self.denominator))
            .unwrap_or(Fraction::ONE)
    }
}

/// Données d'une mesure relevées en première passe.
#[derive(Clone, Debug)]
pub struct MeasureData {
    /// Durée de la mesure (`determineMeasureLength`), maximum sur les parties.
    pub len: Fraction,
    /// Indication de mesure déclarée dans cette mesure, si elle en change.
    pub time_sig: Option<TimeSigData>,
    /// Indication de mesure en vigueur à la fin de la mesure.
    pub time_sig_in_effect: TimeSigData,
    /// Numéro écrit dans l'attribut `number`, s'il est numérique.
    pub number: Option<usize>,
    /// Mesure d'anacrouse ou non numérotée (`implicit="yes"`).
    pub implicit: bool,
}

impl MeasureData {
    fn new() -> Self {
        Self {
            len: Fraction::ZERO,
            time_sig: None,
            time_sig_in_effect: TimeSigData {
                numerator: 4,
                denominator: 4,
                kind: TimeSigType::Normal,
            },
            number: None,
            implicit: false,
        }
    }
}

/// Partie du document, telle que relevée en première passe.
#[derive(Clone, Debug)]
pub struct PartData {
    /// Identifiant MusicXML (`P1`).
    pub id: String,
    /// Nom long (`<part-name>`).
    pub name: String,
    /// Nom court (`<part-abbreviation>`).
    pub abbreviation: String,
    /// Instruments déclarés, le premier étant l'instrument principal.
    pub instruments: Vec<InstrumentData>,
    /// Transposition de l'instrument principal.
    pub transpose: Option<Transpose>,
    /// Nombre de portées.
    pub staves: usize,
    /// Longueur calculée de chaque mesure, dans l'ordre du document.
    pub measures: Vec<Fraction>,
}

/// Résultat de la première passe.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// Parties, dans l'ordre de `part-list`.
    pub parts: Vec<PartData>,
    /// Mesures globales (longueur maximale sur toutes les parties).
    pub measures: Vec<MeasureData>,
    /// Table `voix MusicXML -> description` de chaque partie.
    pub voices: Vec<BTreeMap<i32, VoiceDesc>>,
    /// Divisions de la première mesure, à titre indicatif.
    pub divisions: u32,
}

impl Analysis {
    /// Table `voix MusicXML -> description` d'une partie.
    pub fn voices(&self, part_index: usize) -> &BTreeMap<i32, VoiceDesc> {
        &self.voices[part_index]
    }

    /// Décrit la voix d'un élément.
    pub fn voice(&self, part_index: usize, musicxml_voice: i32) -> VoiceDesc {
        self.voices[part_index]
            .get(&musicxml_voice)
            .cloned()
            .unwrap_or(VoiceDesc {
                counts: vec![0],
                staff: 0,
                voice: 0,
            })
    }

    /// Nombre total de portées.
    pub fn staff_count(&self) -> usize {
        self.parts.iter().map(|part| part.staves).sum()
    }
}

/// Première passe complète.
pub fn analyse(root: &Element) -> Result<Analysis> {
    if root.name != "score-partwise" {
        return Err(Error::UnknownRoot(root.name.clone()));
    }

    let part_list = root.child("part-list").ok_or(Error::MissingPartList)?;
    let mut declared: Vec<PartData> = Vec::new();
    for score_part in part_list.children_named("score-part") {
        let id = score_part.attribute("id").unwrap_or_default().to_owned();
        declared.push(part_data(score_part, &id));
    }
    if declared.is_empty() {
        return Err(Error::NoParts);
    }

    // Partitions `<part>` par identifiant, dans l'ordre de `part-list`.
    let mut parts_elements: BTreeMap<&str, &Element> = BTreeMap::new();
    for part in root.children_named("part") {
        if let Some(id) = part.attribute("id") {
            parts_elements.entry(id).or_insert(part);
        }
    }

    let mut states: Vec<PartState> = Vec::new();
    for part in &mut declared {
        let element = parts_elements.get(part.id.as_str()).copied();
        states.push(measure_part(part, element)?);
    }

    // Longueur globale de chaque mesure : maximum sur les parties.
    let measure_count = states.iter().map(|state| state.lengths.len()).max().unwrap_or(0);
    if measure_count == 0 {
        return Err(Error::NoParts);
    }
    let mut measures = Vec::with_capacity(measure_count);
    let mut time_sig = states
        .iter()
        .find_map(|state| state.time_sigs.first().copied().flatten())
        .unwrap_or(TimeSigData {
            numerator: 4,
            denominator: 4,
            kind: TimeSigType::Normal,
        });
    for index in 0..measure_count {
        let len = states
            .iter()
            .filter_map(|state| state.lengths.get(index).copied())
            .fold(Fraction::ZERO, Fraction::max);
        if let Some(declared) = states
            .iter()
            .filter_map(|state| state.time_sigs.get(index).copied().flatten())
            .next()
        {
            time_sig = declared;
        }
        measures.push(MeasureData {
            len,
            time_sig: states
                .iter()
                .filter_map(|state| state.time_sigs.get(index).copied().flatten())
                .next(),
            time_sig_in_effect: time_sig,
            number: states
                .iter()
                .find_map(|state| state.numbers.get(index).copied().flatten()),
            implicit: states
                .iter()
                .any(|state| state.implicits.get(index).copied().unwrap_or(false)),
        });
    }

    let divisions = states.first().map_or(1, |state| state.divisions);
    Ok(Analysis {
        parts: declared,
        measures,
        voices: states.into_iter().map(|state| state.voices).collect(),
        divisions,
    })
}

/// État d'une partie pendant la première passe.
struct PartState {
    lengths: Vec<Fraction>,
    time_sigs: Vec<Option<TimeSigData>>,
    numbers: Vec<Option<usize>>,
    implicits: Vec<bool>,
    voices: BTreeMap<i32, VoiceDesc>,
    divisions: u32,
    max_staff: usize,
}

/// Métadonnées d'une `score-part`.
fn part_data(score_part: &Element, id: &str) -> PartData {
    let name = score_part
        .child_text("part-name")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let abbreviation = score_part
        .child_text("part-abbreviation")
        .unwrap_or_default()
        .trim()
        .to_owned();

    let mut instruments: Vec<InstrumentData> = Vec::new();
    for score_instrument in score_part.children_named("score-instrument") {
        instruments.push(InstrumentData {
            id: score_instrument
                .attribute("id")
                .unwrap_or_default()
                .to_owned(),
            name: score_instrument
                .child_text("instrument-name")
                .unwrap_or_default()
                .trim()
                .to_owned(),
            sound: score_instrument
                .child_text("instrument-sound")
                .unwrap_or_default()
                .trim()
                .to_owned(),
            ..InstrumentData::default()
        });
    }
    for midi in score_part.children_named("midi-instrument") {
        let target = midi.attribute("id").unwrap_or_default();
        let index = instruments
            .iter()
            .position(|instrument| instrument.id == target)
            .unwrap_or(0);
        let Some(instrument) = instruments.get_mut(index) else {
            continue;
        };
        instrument.channel = midi
            .child_int("midi-channel")
            .and_then(|value| u8::try_from(value.saturating_sub(1)).ok());
        instrument.program = midi
            .child_int("midi-program")
            .and_then(|value| u8::try_from(value.saturating_sub(1)).ok());
        instrument.volume = midi
            .child_f64("volume")
            .map(|percent| (percent / 100.0 * 127.0).round().clamp(0.0, 127.0) as u8);
        instrument.pan = midi.child_f64("pan").map(|pan| {
            let raw = pan / 90.0 * 63.5 + 63.5;
            raw.round().clamp(0.0, 127.0) as u8
        });
    }
    if instruments.is_empty() {
        instruments.push(InstrumentData {
            id: String::new(),
            name: name.clone(),
            ..InstrumentData::default()
        });
    }

    PartData {
        id: id.to_owned(),
        name,
        abbreviation,
        instruments,
        transpose: None,
        staves: 1,
        measures: Vec::new(),
    }
}

/// Parcourt les mesures d'une partie (`MusicXmlParserPass1::part`).
fn measure_part(part: &mut PartData, element: Option<&Element>) -> Result<PartState> {
    let mut state = PartState {
        lengths: Vec::new(),
        time_sigs: Vec::new(),
        numbers: Vec::new(),
        implicits: Vec::new(),
        voices: BTreeMap::new(),
        divisions: 1,
        max_staff: 0,
    };

    let Some(element) = element else {
        return Ok(state);
    };

    let mut time_sig: Option<TimeSigData> = None;

    for measure in element.children_named("measure") {
        let mut measure_time = Fraction::ZERO;
        let mut duration = Fraction::ZERO;

        for child in &measure.children {
            match child.name.as_str() {
                "attributes" => {
                    read_attributes_part(
                        child,
                        &mut state.divisions,
                        &mut time_sig,
                        part,
                        &mut state.max_staff,
                    );
                }
                "note" => {
                    let timing = read_note_timing(child, state.divisions);
                    let (voice, staff, chord_or_grace) = read_note_identity(child);
                    if !chord_or_grace && timing.dura.is_positive() {
                        let desc = state.voices.entry(voice).or_default();
                        desc.incr(staff);
                    }
                    measure_time += timing.dura;
                    duration = duration.max(measure_time);
                }
                "forward" => {
                    measure_time += read_divisions_duration(child, state.divisions);
                    duration = duration.max(measure_time);
                }
                "backup" => {
                    let delta = read_divisions_duration(child, state.divisions);
                    measure_time = if delta <= measure_time {
                        measure_time - delta
                    } else {
                        Fraction::ZERO
                    };
                }
                _ => {}
            }
            duration = duration.max(measure_time);
        }

        // Corrections de durée (`MusicXmlParserPass1::measure`).
        if duration.is_zero() {
            duration = time_sig.map_or_else(|| Fraction::ONE, TimeSigData::measure_len);
        }
        duration = round_measure_length(duration);
        if let Some(signature) = time_sig {
            duration = measure_duration_as_fraction(duration, signature.denominator);
        }

        state.lengths.push(duration);
        state.time_sigs.push(time_sig);
        state.numbers.push(
            measure
                .attribute("number")
                .and_then(|number| number.parse().ok()),
        );
        state.implicits.push(measure.attribute("implicit") == Some("yes"));
    }

    part.staves = state.max_staff.max(1);
    part.measures = state.lengths.clone();
    allocate_voices(&mut state.voices);
    Ok(state)
}

/// Répartit les voix MusicXML sur les voix MuseScore (`allocateVoices`).
fn allocate_voices(voices: &mut BTreeMap<i32, VoiceDesc>) {
    for desc in voices.values_mut() {
        desc.staff = desc.preferred_staff();
    }
    let mut next_voice = [0usize; MAX_STAVES];
    for desc in voices.values_mut() {
        let staff = desc.staff.min(MAX_STAVES - 1);
        desc.voice = next_voice[staff];
        next_voice[staff] += 1;
    }
}

/// Lit les attributs utiles à la première passe.
fn read_attributes_part(
    attributes: &Element,
    divisions: &mut u32,
    time_sig: &mut Option<TimeSigData>,
    part: &mut PartData,
    max_staff: &mut usize,
) {
    for child in &attributes.children {
        match child.name.as_str() {
            "divisions" => {
                if let Ok(value) = child.text_trimmed().parse::<u32>() {
                    if value > 0 {
                        *divisions = value;
                    }
                }
            }
            "time" => {
                if let Some(signature) = read_time(child) {
                    *time_sig = Some(signature);
                }
            }
            "staves" => {
                if let Ok(count) = child.text_trimmed().parse::<usize>() {
                    *max_staff = (*max_staff).max(count);
                }
            }
            "transpose" => part.transpose = read_transpose(child),
            _ => {}
        }
    }
}

/// Indication de mesure d'un `<time>` (`determineTimeSig`).
pub fn read_time(element: &Element) -> Option<TimeSigData> {
    let mut beats = String::new();
    let mut beat_type = String::new();
    for child in &element.children {
        match child.name.as_str() {
            "beats" => beats.push_str(child.text_trimmed()),
            "beat-type" => beat_type.push_str(child.text_trimmed()),
            _ => {}
        }
    }
    if beats.is_empty() || beat_type.is_empty() {
        return None;
    }
    let numerator: i32 = beats
        .split('+')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .sum();
    let denominator: i32 = beat_type.trim().parse().ok()?;
    if numerator <= 0 || denominator <= 0 {
        return None;
    }
    let kind = match element.attribute("symbol") {
        Some("common") => TimeSigType::FourFour,
        Some("cut") => TimeSigType::AllaBreve,
        _ => TimeSigType::Normal,
    };
    Some(TimeSigData {
        numerator,
        denominator,
        kind,
    })
}

/// Transposition d'un `<transpose>` (diatonique, chromatique, octaves cumulées).
pub fn read_transpose(element: &Element) -> Option<Transpose> {
    let mut diatonic = element.child_int("diatonic").unwrap_or(0) as i32;
    let mut chromatic = element.child_int("chromatic").unwrap_or(0) as i32;
    let octave_change = element.child_int("octave-change").unwrap_or(0) as i32;
    diatonic += octave_change * 7;
    chromatic += octave_change * 12;
    if diatonic == 0 && chromatic == 0 {
        return None;
    }
    Some(Transpose::new(diatonic, chromatic))
}

/// Voix MusicXML, portée (0-based) et indicateur accord/ornement d'une note.
pub fn read_note_identity(note: &Element) -> (i32, usize, bool) {
    let voice = note
        .child_text("voice")
        .and_then(|text| text.trim().parse::<i32>().ok())
        .or_else(|| {
            note.child_text("voice").map(|text| {
                text.trim()
                    .chars()
                    .next()
                    .map_or(1, |character| character as i32)
            })
        })
        .unwrap_or(1);
    let staff = note
        .child_text("staff")
        .and_then(|text| text.trim().parse::<usize>().ok())
        .map_or(0, |number| number.saturating_sub(1));
    let chord_or_grace = note.has_child("chord") || note.has_child("grace");
    (voice, staff, chord_or_grace)
}

/// Durée d'un `<forward>` ou `<backup>` en divisions.
fn read_divisions_duration(element: &Element, divisions: u32) -> Fraction {
    if divisions == 0 {
        return Fraction::ZERO;
    }
    element
        .child_text("duration")
        .and_then(|text| text.trim().parse::<i64>().ok())
        .map(|ticks| Fraction::from_ticks(ticks, divisions * 4))
        .unwrap_or(Fraction::ZERO)
}

/// Calcule le timing d'une note comme `MusicXmlNoteDuration`.
pub fn read_note_timing(note: &Element, divisions: u32) -> NoteTiming {
    noteduration::read(note, divisions)
}

/// Arrondit une durée de mesure à un multiple de 1/128.
///
/// Reproduit la correction de `MusicXmlParserPass1::measure` : MuseScore impose
/// que la longueur d'une mesure soit un multiple entier de 1/128 de ronde.
fn round_measure_length(duration: Fraction) -> Fraction {
    let scaled = duration * Fraction::from_integer(128);
    if scaled.denominator() == 1 {
        return duration;
    }
    let down = Fraction::new(scaled.numerator() / scaled.denominator(), 128);
    let up = Fraction::new(scaled.numerator() / scaled.denominator() + 1, 128);
    // Écart à la borne inférieure inférieur à 1/128 : erreur d'arrondi.
    if duration - down < up - duration {
        down
    } else {
        up
    }
}

/// `measureDurationAsFraction` : dénominateur au moins égal à celui de
/// l'indication de mesure.
///
/// Le C++ réécrit `4/4` en `8/8` … pour que la longueur de mesure s'exprime
/// avec un dénominateur au moins égal à `beat-type`. La `Fraction` du modèle
/// est toujours réduite (`8/8` y vaut `1/1`), donc la valeur ne change pas :
/// seule la représentation interne du C++ diffère. La fonction conserve donc
/// la valeur telle quelle.
fn measure_duration_as_fraction(length: Fraction, _beat_type: i32) -> Fraction {
    length
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_de_voix() {
        let mut voices = BTreeMap::new();
        voices.entry(1).or_default().incr(0);
        voices.entry(2).or_default().incr(0);
        voices.entry(3).or_default().incr(0);
        allocate_voices(&mut voices);
        assert_eq!(voices[&1].voice, 0);
        assert_eq!(voices[&2].voice, 1);
        assert_eq!(voices[&3].voice, 2);
    }
}
