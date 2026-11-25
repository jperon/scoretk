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

use scoretk_core::model::{Fraction, TimeSigType, Transpose};

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
    /// `<midi-device port>` moins un, si présent (`Instrument::midi_port`).
    pub port: Option<u8>,
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
    /// Vrai si la voix chevauche plusieurs portées (`VoiceOverlapDetector`).
    pub overlaps: bool,
    /// Portée attribuée à une voix non chevauchante (`None` si aucune).
    pub staff: Option<usize>,
    /// Portées sur lesquelles une voix chevauchante est présente.
    pub staff_alloc: Vec<bool>,
    /// Voix MuseScore par portée, pour une voix chevauchante.
    pub voices: Vec<usize>,
    /// Voix MuseScore attribuée à une voix non chevauchante (0-3).
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

    /// Nombre total d'accords et silences de la voix.
    fn total(&self) -> usize {
        self.counts.iter().sum()
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
    /// Indication de mesure en vigueur à la fin de la mesure.
    pub time_sig_in_effect: TimeSigData,
    /// Vrai si un saut de section précède la mesure.
    ///
    /// MusicXML n'a pas d'équivalent direct : MuseScore réécrit la
    /// numérotation à 1 et répète `<attributes>` au début d'une section. Un
    /// numéro de mesure qui recule (hors mesure implicite) est donc interprété
    /// comme un saut de section.
    pub section_break: bool,
    /// Vrai si un `<print new-system="yes">` ou `new-page="yes"` ouvre cette
    /// mesure (`MusicXmlParserPass1::print` et `createMeasuresAndVboxes`,
    /// `importmusicxmlpass1.cpp:2916-2929, 1205-1214`) : la rupture est portée
    /// par la mesure précédente dans le modèle MuseScore.
    pub line_break: bool,
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
    /// Accolades et crochets déclarés par `<part-group>` (type, portées
    /// couvertes), posés sur la première portée de la partie.
    pub brackets: Vec<(i8, usize)>,
    /// Vrai si la barre de mesure est commune aux portées du groupe
    /// (`<group-barline>yes</group-barline>`).
    pub bar_line_span: bool,
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
    /// Résout la portée, la voix et le décalage de portée d'un élément
    /// (`determineStaffMoveVoice`).
    ///
    /// Renvoie `(portée, voix, décalage)` : la portée et la voix MuseScore, et
    /// `staffMove` (portée MusicXML moins portée attribuée).
    pub fn resolve_voice(
        &self,
        part_index: usize,
        musicxml_voice: i32,
        musicxml_staff: usize,
    ) -> (usize, usize, i8) {
        let Some(desc) = self.voices[part_index].get(&musicxml_voice) else {
            return (musicxml_staff, 0, 0);
        };
        let (staff, voice) = if desc.overlaps {
            // Une voix chevauchante reste sur la portée écrite ; sa voix
            // dépend de la portée (`VoiceDesc::voice(staff)`).
            (
                musicxml_staff,
                desc.voices.get(musicxml_staff).copied().unwrap_or(0),
            )
        } else {
            (
                desc.staff.unwrap_or_else(|| desc.preferred_staff()),
                desc.voice,
            )
        };
        let staff_move = musicxml_staff as i64 - staff as i64;
        (
            staff,
            voice,
            i8::try_from(staff_move.clamp(-128, 127)).unwrap_or(0),
        )
    }
}

/// Première passe complète.
pub fn analyse(root: &Element) -> Result<Analysis> {
    if root.name != "score-partwise" {
        return Err(Error::UnknownRoot(root.name.clone()));
    }

    let part_list = root.child("part-list").ok_or(Error::MissingPartList)?;
    let mut declared: Vec<PartData> = Vec::new();
    // Groupes de parties ouverts (`<part-group>`) : symbole, barre commune et
    // index de la partie qui les ouvre.
    let mut open_groups: Vec<(i8, bool, usize)> = Vec::new();
    for child in &part_list.children {
        match child.name.as_str() {
            "score-part" => {
                let id = child.attribute("id").unwrap_or_default().to_owned();
                declared.push(part_data(child, &id));
            }
            "part-group" => match child.attribute("type") {
                Some("start") => {
                    let symbol = match child.child_text("group-symbol") {
                        Some("brace") => 1,
                        Some("bracket") => 0,
                        Some("square") => 3,
                        Some("line") => 2,
                        Some("none") | None => -1,
                        Some(_) => -1,
                    };
                    let barline = child.child_text("group-barline") == Some("yes");
                    open_groups.push((symbol, barline, declared.len()));
                }
                Some("stop") => {
                    if let Some((symbol, barline, start)) = open_groups.pop() {
                        let span = declared.len() - start;
                        if let Some(part) = declared.get_mut(start) {
                            if symbol >= 0 {
                                part.brackets.push((symbol, span));
                            }
                            part.bar_line_span = part.bar_line_span || barline;
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
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
        // Une partie à plusieurs portées sans `<part-group>` reçoit une
        // accolade implicite, que l'export ne réécrit pas (`partList`).
        if part.staves > 1 && part.brackets.is_empty() {
            part.brackets.push((1, part.staves));
        }
    }

    // Longueur globale de chaque mesure : maximum sur les parties.
    let measure_count = states
        .iter()
        .map(|state| state.lengths.len())
        .max()
        .unwrap_or(0);
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
        let number = states
            .iter()
            .find_map(|state| state.numbers.get(index).copied().flatten());
        let implicit = states
            .iter()
            .any(|state| state.implicits.get(index).copied().unwrap_or(false));
        let section_break = !implicit && number.is_some() && states.iter().any(|state| {
            let current = state.numbers.get(index).copied().flatten();
            let previous = index
                .checked_sub(1)
                .and_then(|previous| state.numbers.get(previous).copied().flatten());
            matches!((current, previous), (Some(current), Some(previous)) if current <= previous)
        });
        let line_break = states
            .iter()
            .any(|state| state.line_breaks.get(index).copied().unwrap_or(false));
        measures.push(MeasureData {
            len,
            time_sig_in_effect: time_sig,
            section_break,
            line_break,
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
    /// `<print new-system>` / `<print new-page>` de chaque mesure.
    line_breaks: Vec<bool>,
    voices: BTreeMap<i32, VoiceDesc>,
    divisions: u32,
    max_staff: usize,
}

/// Intervalles d'une voix dans la mesure courante (détection de chevauchement).
#[derive(Default)]
struct OverlapDetector {
    /// Par voix, puis par portée : intervalles `(début, fin)`.
    lists: BTreeMap<i32, Vec<Vec<(Fraction, Fraction)>>>,
}

impl OverlapDetector {
    fn new_measure(&mut self) {
        self.lists.clear();
    }

    fn add(&mut self, start: Fraction, end: Fraction, voice: i32, staff: usize) {
        if staff >= MAX_STAVES {
            return;
        }
        let entry = self.lists.entry(voice).or_default();
        if entry.len() <= staff {
            entry.resize(staff + 1, Vec::new());
        }
        entry[staff].push((start, end));
    }

    /// Portées en chevauchement (`NoteList::anyStaffOverlaps`).
    fn overlapping_voices(&self) -> Vec<i32> {
        let mut result = Vec::new();
        for (voice, staves) in &self.lists {
            let mut overlaps = false;
            for first in 0..staves.len() {
                for second in first + 1..staves.len() {
                    for (start1, end1) in &staves[first] {
                        for (start2, end2) in &staves[second] {
                            if !(start1 >= end2 || end1 <= start2) {
                                overlaps = true;
                            }
                        }
                    }
                }
            }
            if overlaps {
                result.push(*voice);
            }
        }
        result
    }
}

/// Métadonnées d'une `score-part`.
fn part_data(score_part: &Element, id: &str) -> PartData {
    // Le nom long peut porter des bémols et dièses typographiques : MusicXML
    // les décompose en `<display-text>`/`<accidental-text>`, et MuseScore les
    // recompose dans `Part::longName` (`writeDisplayName`).
    let name = display_name(score_part, "part-name");
    let abbreviation = display_name(score_part, "part-abbreviation");

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
        // `<midi-device port>` est un frère de `<midi-instrument>` et vaut
        // `Instrument::midi_port + 1` (`exportmusicxml.cpp:7939-7947`).
        instrument.port = score_part
            .child("midi-device")
            .and_then(|device| device.attribute("port"))
            .and_then(|value| value.parse::<u8>().ok())
            .and_then(|value| value.checked_sub(1));
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
        brackets: Vec::new(),
        bar_line_span: false,
        measures: Vec::new(),
    }
}

/// Nom d'une partie, reconstitué depuis sa variante `-display` si elle existe.
fn display_name(score_part: &Element, name: &str) -> String {
    if let Some(display) = score_part.child(&format!("{name}-display")) {
        let mut text = String::new();
        let mut after_accidental = false;
        for child in &display.children {
            match child.name.as_str() {
                "display-text" => {
                    let fragment = child.text_trimmed();
                    // L'export C++ perd l'espace qui suit l'altération ; on le
                    // rétablit pour que `<part-name>` reste « Bb Clarinet ».
                    if after_accidental
                        && !fragment.is_empty()
                        && !fragment.starts_with(' ')
                        && !text.ends_with(' ')
                    {
                        text.push(' ');
                    }
                    text.push_str(fragment);
                    after_accidental = false;
                }
                "accidental-text" => {
                    match child.text_trimmed() {
                        "flat" => text.push('♭'),
                        "sharp" => text.push('♯'),
                        _ => {}
                    }
                    after_accidental = true;
                }
                _ => {}
            }
        }
        if !text.is_empty() {
            return text.trim().to_owned();
        }
    }
    score_part
        .child_text(name)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Parcourt les mesures d'une partie (`MusicXmlParserPass1::part`).
fn measure_part(part: &mut PartData, element: Option<&Element>) -> Result<PartState> {
    let mut state = PartState {
        lengths: Vec::new(),
        time_sigs: Vec::new(),
        numbers: Vec::new(),
        implicits: Vec::new(),
        line_breaks: Vec::new(),
        voices: BTreeMap::new(),
        divisions: 1,
        max_staff: 0,
    };

    let Some(element) = element else {
        return Ok(state);
    };

    let mut time_sig: Option<TimeSigData> = None;
    let mut detector = OverlapDetector::default();

    for measure in element.children_named("measure") {
        detector.new_measure();
        let mut measure_time = Fraction::ZERO;
        let mut duration = Fraction::ZERO;
        let mut line_break = false;
        let measure_start = length_start(&state.lengths);

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
                        detector.add(
                            measure_start + measure_time,
                            measure_start + measure_time + timing.dura,
                            voice,
                            staff,
                        );
                    }
                    measure_time += timing.dura;
                    duration = duration.max(measure_time);
                }
                "print" => {
                    // `print` (`importmusicxmlpass1.cpp:2916-2929`) ne lit
                    // que ces deux attributs et ignore le reste du bloc.
                    if child.attribute("new-page") == Some("yes")
                        || child.attribute("new-system") == Some("yes")
                    {
                        line_break = true;
                    }
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
        state
            .implicits
            .push(measure.attribute("implicit") == Some("yes"));
        state.line_breaks.push(line_break);

        for voice in detector.overlapping_voices() {
            if let Some(desc) = state.voices.get_mut(&voice) {
                desc.overlaps = true;
            }
        }
    }

    part.staves = state.max_staff.max(1);
    part.measures = state.lengths.clone();
    allocate_voices(&mut state.voices);
    Ok(state)
}

/// Somme des longueurs déjà relevées, c'est-à-dire tick de début courant.
fn length_start(lengths: &[Fraction]) -> Fraction {
    lengths
        .iter()
        .fold(Fraction::ZERO, |total, length| total + *length)
}

/// Répartit les voix MusicXML sur les portées puis les voix MuseScore
/// (`allocateStaves` puis `allocateVoices`).
fn allocate_voices(voices: &mut BTreeMap<i32, VoiceDesc>) {
    let mut allocated = [0usize; MAX_STAVES];

    // Voix régulières : de la plus fournie à la moins fournie.
    loop {
        let mut chosen: Option<(i32, usize)> = None;
        for (key, desc) in voices.iter() {
            if desc.overlaps || desc.staff.is_some() {
                continue;
            }
            let total = desc.total();
            if total == 0 {
                continue;
            }
            if chosen.is_none_or(|(_, best)| total > best) {
                chosen = Some((*key, total));
            }
        }
        let Some((key, _)) = chosen else {
            break;
        };
        let preferred = voices[&key].preferred_staff();
        let desc = voices.get_mut(&key).expect("voix présente");
        if preferred < MAX_STAVES && allocated[preferred] < 4 {
            desc.staff = Some(preferred);
            allocated[preferred] += 1;
        } else {
            // Plus de voix libre : la voix n'est pas attribuée.
            desc.staff = None;
        }
    }

    // Voix chevauchantes : présentes sur chaque portée où elles apparaissent,
    // de la plus fournie à la moins fournie.
    for staff in 0..MAX_STAVES {
        loop {
            let mut chosen: Option<(i32, usize)> = None;
            for (key, desc) in voices.iter() {
                if !desc.overlaps {
                    continue;
                }
                let count = desc.counts.get(staff).copied().unwrap_or(0);
                if count == 0 {
                    continue;
                }
                if desc.staff_alloc.get(staff).copied().unwrap_or(false) {
                    continue;
                }
                if chosen.is_none_or(|(_, best)| count > best) {
                    chosen = Some((*key, count));
                }
            }
            let Some((key, _)) = chosen else {
                break;
            };
            let desc = voices.get_mut(&key).expect("voix présente");
            if desc.staff_alloc.len() <= staff {
                desc.staff_alloc.resize(staff + 1, false);
            }
            if allocated[staff] < 4 {
                desc.staff_alloc[staff] = true;
                allocated[staff] += 1;
            } else {
                desc.staff_alloc[staff] = false;
                // Marquer comme traité pour ne pas boucler.
                desc.staff_alloc.resize(staff + 1, false);
                desc.staff_alloc[staff] = false;
                break;
            }
        }
    }

    // Numérotation : voix régulières d'abord, dans l'ordre des numéros
    // MusicXML, puis voix chevauchantes par portée.
    let mut next_voice = [0usize; MAX_STAVES];
    for desc in voices.values_mut() {
        if desc.overlaps {
            continue;
        }
        if let Some(staff) = desc.staff {
            let staff = staff.min(MAX_STAVES - 1);
            desc.voice = next_voice[staff];
            next_voice[staff] += 1;
        }
    }
    for desc in voices.values_mut() {
        if !desc.overlaps {
            continue;
        }
        if desc.voices.len() < MAX_STAVES {
            desc.voices.resize(MAX_STAVES, 0);
        }
        for staff in 0..MAX_STAVES {
            if desc.staff_alloc.get(staff).copied().unwrap_or(false) {
                desc.voices[staff] = next_voice[staff];
                next_voice[staff] += 1;
            }
        }
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
        let mut voices: BTreeMap<i32, VoiceDesc> = BTreeMap::new();
        voices.entry(1).or_default().incr(0);
        voices.entry(2).or_default().incr(0);
        voices.entry(3).or_default().incr(0);
        allocate_voices(&mut voices);
        assert_eq!(voices[&1].voice, 0);
        assert_eq!(voices[&2].voice, 1);
        assert_eq!(voices[&3].voice, 2);
    }
}
