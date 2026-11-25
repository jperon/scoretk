// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Construction du modèle pivot depuis l'état de lecture ABC. Les types
// employés sont ceux de `engraving/dom/` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others ; ce fichier est une réécriture Rust originale.
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

//! Construction du modèle pivot à partir des voix lues.
//!
//! Les voix ABC deviennent des portées, groupées en parties par `%%score`.
//! Les mesures sont alignées sur la plus longue des voix ; les changements
//! d'armure, de mesure et de clef deviennent des segments au début de la
//! mesure concernée. Les slurs, soufflets et volatas deviennent des
//! [`Spanner`].

use scoretk_core::model::{
    Annotation, BarLine, BarLineType, Clef, ClefType, Credit, CreditStyle, Dynamic, Fraction,
    Harmony, KeySig, Measure, Part, Placement, RichText, Score, Segment, SegmentItem, SegmentType,
    Slur, Spanner, SpannerAnchor, SpannerNotation, SpannerType, Staff, StaffId, TboxText, TimeSig,
    Track, Transpose, Tuplet, TupletId, VoltaInfo,
};

use crate::reader::{AnnotationSpec, Position, Reader, Voice};
use scoretk_abc_common::Result;

/// Construit la partition.
pub(crate) fn build(reader: &mut Reader) -> Result<Score> {
    let voices = std::mem::take(&mut reader.voices);
    let mut score = std::mem::replace(&mut reader.score, Score::new());
    score.metadata.work_title = reader.title.clone();
    score.metadata.composer = reader.composer.clone();
    score.metadata.source = reader.source.clone();
    for (name, value) in &reader.meta {
        let field = match name.as_str() {
            "work_title" => &mut score.metadata.work_title,
            "movement_title" => &mut score.metadata.movement_title,
            "movement_number" => &mut score.metadata.movement_number,
            "work_number" => &mut score.metadata.work_number,
            "composer" => &mut score.metadata.composer,
            "lyricist" => &mut score.metadata.lyricist,
            "poet" => &mut score.metadata.poet,
            "translator" => &mut score.metadata.translator,
            "arranger" => &mut score.metadata.arranger,
            "copyright" => &mut score.metadata.copyright,
            "source" => &mut score.metadata.source,
            "subtitle" => &mut score.metadata.subtitle,
            _ => continue,
        };
        *field = value.clone();
    }
    // ABC 2.1 §3.1 : `W:` porte un texte imprimé, pas une syllable de `w:`.
    // Il devient un `<credit>`-like du modèle (l'ordre est significatif).
    for text in &reader.credits {
        score.add_credit(Credit::new(RichText::new(text.clone()), CreditStyle::Title));
    }
    let mut tbox_texts = std::mem::take(&mut reader.tbox_texts);
    tbox_texts.sort_by_key(|(index, _)| *index);
    for (index, text) in tbox_texts {
        score.add_tbox_text(TboxText::new(RichText::new(text), index));
    }

    let groups = groups(reader, &voices);
    let voice_tracks = create_parts(&mut score, &voices, &groups);
    for part in &mut score.parts {
        part.instrument.midi_program = reader.midi_program;
        part.instrument.midi_channel = reader.midi_channel;
    }
    let staff_count = score.staves.len().max(1);

    let measure_count = voices
        .iter()
        .map(|voice| voice.measures.len())
        .max()
        .unwrap_or(0)
        .max(1);

    let mut starts = Vec::with_capacity(measure_count);
    let mut cursor = Fraction::ZERO;
    for index in 0..measure_count {
        starts.push(cursor);
        cursor += measure_len(&voices, index);
    }

    let tuplet_ids = create_tuplets(&mut score, &voices, &starts);

    for (index, &start) in starts.iter().enumerate() {
        let nominal = nominal_len(&voices, index);
        let len = measure_len(&voices, index);
        let mut measure = Measure::new(index, start, nominal).with_len(len);
        measure.start_bar_line = measure_start_bar(&voices, index);
        measure.end_bar_line = measure_end_bar(&voices, index);
        measure.repeat_count = 0;

        // Segments de contexte, sur chaque portée.
        for (voice_index, voice) in voices.iter().enumerate() {
            let (staff, model_voice) = voice_tracks[voice_index];
            let track = Track::new(staff, model_voice);
            // `abc2xml.py::mkMeasure` writes a `<time>` for every M: field,
            // even several at the same tick (`y` spacers do not advance it).
            // Emit one segment per recorded change at the bar start rather
            // than collapsing them into the last value (ABC 2.1 §3.2).
            let inline_times: Vec<TimeSig> = voice
                .time_changes
                .iter()
                .filter(|(measure, tick, _)| *measure == index && tick.is_zero())
                .filter_map(|(_, _, value)| value.clone())
                .collect();
            if index == 0
                || time_at(voice, index) != time_at(voice, index - 1)
                || !inline_times.is_empty()
            {
                if inline_times.is_empty() {
                    let mut segment = Segment::new(SegmentType::TimeSig, start, staff_count);
                    segment.set_element(track, SegmentItem::TimeSig(time_at(voice, index)));
                    measure.add_segment(segment);
                } else {
                    for time in inline_times {
                        let mut segment = Segment::new(SegmentType::TimeSig, start, staff_count);
                        segment.set_element(track, SegmentItem::TimeSig(time));
                        measure.add_segment(segment);
                    }
                }
            }
            // `abc2xml.py` writes a `<key>` for every K: field, even when the
            // key is unchanged (ABC 2.1 §3.2: a repeated field is a
            // replacement). A K: at a measure start lands in `key_changes` at
            // tick zero; keep its segment even if `key_at` did not change.
            let key_field_here = index > 0
                && voice
                    .key_changes
                    .iter()
                    .any(|(measure, tick, _)| *measure == index && tick.is_zero());
            if index == 0 || key_field_here || key_at(voice, index) != key_at(voice, index - 1) {
                let mut segment = Segment::new(SegmentType::KeySig, start, staff_count);
                segment.set_element(track, SegmentItem::KeySig(key_at(voice, index)));
                measure.add_segment(segment);
            }
            // A K:/V: clef change after the first measure is a mid-tune
            // clef (`abc2xml.py:1734-1744` writes one `<clef>` per field).
            // Use `SegmentType::Clef` so the exporter's `writeElement` path
            // emits it; `HeaderClef` is only read for the first measure
            // (`exportmusicxml.rs::clef_at_start`). ABC 2.1 §4.6.
            if index == 0 || clef_at(voice, index) != clef_at(voice, index - 1) {
                if let Some(clef) = clef_at(voice, index) {
                    let kind = if index == 0 {
                        SegmentType::HeaderClef
                    } else {
                        SegmentType::Clef
                    };
                    let mut segment = Segment::new(kind, start, staff_count);
                    segment.set_element(track, SegmentItem::Clef(Clef::new(clef)));
                    measure.add_segment(segment);
                }
            }
            // Changements en cours de mesure.
            for (change_measure, tick, value) in &voice.time_changes {
                if *change_measure == index && tick.is_positive() {
                    if let Some(time) = value {
                        let mut segment =
                            Segment::new(SegmentType::TimeSig, start + *tick, staff_count);
                        segment.set_element(track, SegmentItem::TimeSig(time.clone()));
                        measure.add_segment(segment);
                    }
                }
            }
            for (change_measure, tick, value) in &voice.key_changes {
                if *change_measure == index && tick.is_positive() {
                    if let Some(key) = value {
                        let mut segment =
                            Segment::new(SegmentType::KeySig, start + *tick, staff_count);
                        segment.set_element(track, SegmentItem::KeySig(key.clone()));
                        measure.add_segment(segment);
                    }
                }
            }
            for (change_measure, tick, value) in &voice.clef_changes {
                if *change_measure == index && tick.is_positive() {
                    if let Some(clef) = value {
                        let mut segment =
                            Segment::new(SegmentType::Clef, start + *tick, staff_count);
                        segment.set_element(track, SegmentItem::Clef(Clef::new(*clef)));
                        measure.add_segment(segment);
                    }
                }
            }
        }

        // Événements de chaque voix.
        for (voice_index, voice) in voices.iter().enumerate() {
            let (staff, model_voice) = voice_tracks[voice_index];
            let track = Track::new(staff, model_voice);
            let Some(data) = voice.measures.get(index) else {
                continue;
            };
            for event in &data.events {
                let tick = start + event.tick;
                let mut chord_rest = event.chord_rest.clone();
                if let Some(tuplet) = event.tuplet {
                    chord_rest.tuplet = tuplet_ids
                        .get(voice_index)
                        .and_then(|ids| ids.get(tuplet))
                        .copied()
                        .flatten();
                }
                let segment = segment_at(&mut measure, SegmentType::ChordRest, tick, staff_count);
                segment.set_element(track, SegmentItem::ChordRest(chord_rest));
                for annotation in &event.annotations {
                    let annotation = annotation_object(annotation);
                    segment.add_annotation(track, annotation);
                }
            }
        }

        // Comme MuseScore, le modèle porte une barre de fin sur chaque mesure
        // même quand sa valeur est `Normal` (aucun symbole ABC n'est écrit).
        let end = measure.end_tick();
        let staff_count_read = score.staves.len();
        let end_bar_line = measure.end_bar_line;
        {
            let segment = segment_at(&mut measure, SegmentType::EndBarLine, end, staff_count);
            for index in 0..staff_count_read {
                let track = Track::new(StaffId::from_index(index), 0);
                if segment.element(track).is_none() {
                    // MuseScore porte la vraie barre de fin sur le segment, en
                    // plus du champ `Measure::end_bar_line`. Écrire `Normal`
                    // ici faisait perdre `End`, `Double` et `Dashed` à
                    // l'aller-retour ABC (ABC 2.1 §4.8).
                    segment.set_element(track, SegmentItem::BarLine(BarLine::new(end_bar_line)));
                }
            }
        }
        measure
            .segments
            .sort_by_key(|segment| (segment.tick, segment.kind));
        score.add_measure(measure);
    }

    // Tempo global sur la première mesure. `abc2xml.py::doTempo`
    // (lignes 1884-1906) écrit la direction sur la mesure qui porte le
    // champ Q: ; l'exporteur ne parcourt les annotations que des segments
    // `ChordRest` (`annotation_track`). Poser le tempo sur un `TimeTick`
    // le rendait invisible : attacher au premier accord de la mesure, où
    // `annotateTracks` le trouve.
    if let Some((bps, text)) = reader.header_tempo.as_ref().or(reader.tempo.as_ref()) {
        if let Some(measure) = score.measures.first_mut() {
            let mut tempo = scoretk_core::model::Tempo::new(text.clone());
            tempo.beats_per_second = Some(*bps);
            let track = Track::new(scoretk_core::model::StaffId::from_index(0), 0);
            let index = measure
                .segments
                .iter()
                .position(|segment| {
                    segment.tick.is_zero()
                        && segment.kind == SegmentType::ChordRest
                        && segment.element(track).is_some()
                })
                .unwrap_or_else(|| {
                    measure
                        .segments
                        .iter()
                        .position(|segment| {
                            segment.tick.is_zero() && segment.kind == SegmentType::ChordRest
                        })
                        .unwrap_or(0)
                });
            measure.segments[index].add_annotation(track, Annotation::Tempo(tempo));
        }
    }

    // Slurs, soufflets et volatas.
    for (voice_index, voice) in voices.iter().enumerate() {
        let (staff, model_voice) = voice_tracks[voice_index];
        let track = Track::new(staff, model_voice);
        for (start, end) in &voice.slurs {
            add_spanner(&mut score, SpannerType::Slur, *start, *end, track, &starts);
        }
        for (start, end, kind) in &voice.hairpins {
            add_spanner(
                &mut score,
                SpannerType::Hairpin(*kind),
                *start,
                *end,
                track,
                &starts,
            );
        }
        if voice_index == 0 {
            for (first, last, endings) in &voice.voltas {
                let end_tick = starts.get(*last).copied().unwrap_or(Fraction::ZERO)
                    + measure_len(&voices, *last);
                let mut spanner = Spanner::new(
                    SpannerType::Volta,
                    SpannerAnchor::new(
                        starts.get(*first).copied().unwrap_or(Fraction::ZERO),
                        track,
                    ),
                    SpannerAnchor::new(end_tick, track),
                );
                spanner.volta = Some(VoltaInfo {
                    endings: endings.clone(),
                    closed: true,
                });
                score.add_spanner(spanner);
            }
        }
    }

    Ok(score)
}

/// Partie ABC : une ou plusieurs portées, chacune avec ses voix.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PartSpec {
    /// Portées, chacune donnant les numéros des voix qui la partagent.
    pub staves: Vec<Vec<usize>>,
}

/// Analyse `%%score` selon la grammaire d'`abc2xml` : un numéro seul est une
/// portée isolée, `(1 2)` regroupe des voix sur une portée, `{…}` et `[…]`
/// encadrent une partie à accolade ou à crochet.
fn parse_score_spec(spec: &str, voices: &[Voice]) -> Vec<PartSpec> {
    let mut parser = ScoreSpecParser {
        chars: spec.chars().collect(),
        pos: 0,
        voices,
    };
    parser.sequence(None)
}

struct ScoreSpecParser<'a> {
    chars: Vec<char>,
    pos: usize,
    voices: &'a [Voice],
}

impl ScoreSpecParser<'_> {
    /// Lit un numéro ou un nom de voix et le résout en numéro.
    ///
    /// Port de `abc2xml.py::voiceNamesAndMaps` (ligne 2253) : un identifiant
    /// de `%%score` peut être un nom déclaré par `V:S`, pas seulement un
    /// chiffre. La lecture s'arrête sur le premier caractère non alphanumérique.
    fn read_voice(&mut self) -> Option<usize> {
        let start = self.pos;
        while self
            .chars
            .get(self.pos)
            .is_some_and(|character| character.is_ascii_alphanumeric())
        {
            self.pos += 1;
        }
        if start == self.pos {
            return None;
        }
        let token: String = self.chars[start..self.pos].iter().collect();
        if let Ok(number) = token.parse() {
            return Some(number);
        }
        self.voices
            .iter()
            .find(|voice| voice.name == token)
            .map(|voice| voice.number)
    }

    fn sequence(&mut self, terminator: Option<char>) -> Vec<PartSpec> {
        let mut parts = Vec::new();
        loop {
            while self
                .chars
                .get(self.pos)
                .is_some_and(|character| character.is_whitespace() || *character == '|')
            {
                self.pos += 1;
            }
            let Some(character) = self.chars.get(self.pos).copied() else {
                break;
            };
            if Some(character) == terminator {
                self.pos += 1;
                break;
            }
            match character {
                '(' => {
                    self.pos += 1;
                    let mut voices = Vec::new();
                    while let Some(value) = self.chars.get(self.pos).copied() {
                        if value == ')' {
                            self.pos += 1;
                            break;
                        }
                        if value.is_ascii_alphanumeric() {
                            if let Some(number) = self.read_voice() {
                                voices.push(number);
                            }
                        } else {
                            self.pos += 1;
                        }
                    }
                    if !voices.is_empty() {
                        parts.push(PartSpec {
                            staves: vec![voices],
                        });
                    }
                }
                '{' | '[' => {
                    let close = if character == '{' { '}' } else { ']' };
                    self.pos += 1;
                    let inner = self.sequence(Some(close));
                    let mut staves = Vec::new();
                    for part in inner {
                        staves.extend(part.staves);
                    }
                    if !staves.is_empty() {
                        parts.push(PartSpec { staves });
                    }
                }
                value if value.is_ascii_alphanumeric() => {
                    if let Some(number) = self.read_voice() {
                        parts.push(PartSpec {
                            staves: vec![vec![number]],
                        });
                    }
                }
                _ => self.pos += 1,
            }
        }
        parts
    }
}

/// Groupes de voix : un groupe par partie.
fn groups(reader: &Reader, voices: &[Voice]) -> Vec<PartSpec> {
    let mut groups = if let Some(spec) = &reader.score_spec {
        parse_score_spec(spec, voices)
    } else {
        Vec::new()
    };
    if groups.is_empty() {
        groups = voices
            .iter()
            .filter(|voice| voice.overlay_of.is_none())
            .map(|voice| PartSpec {
                staves: vec![vec![voice.number]],
            })
            .collect();
    }
    // Ajoute les voix absentes de `%%score` comme portées isolées ; une voix
    // de superposition `&` rejoint la portée de sa racine (port de
    // `abc2xml.py::mergePartMeasure`, ligne 951).
    let mut present = std::collections::BTreeSet::new();
    for part in &groups {
        for staff in &part.staves {
            present.extend(staff.iter().copied());
        }
    }
    for voice in voices {
        if let Some(root) = voice.overlay_of {
            let root_number = voices[root].number;
            if let Some(group) = groups.iter_mut().find(|group| {
                group
                    .staves
                    .iter()
                    .any(|staff| staff.contains(&root_number))
            }) {
                let staff = group
                    .staves
                    .iter_mut()
                    .find(|staff| staff.contains(&root_number))
                    .expect("groupe trouvé ci-dessus");
                if !staff.contains(&voice.number) {
                    staff.push(voice.number);
                }
                present.insert(voice.number);
                continue;
            }
        }
        if !present.contains(&voice.number) {
            groups.push(PartSpec {
                staves: vec![vec![voice.number]],
            });
            present.insert(voice.number);
        }
    }
    if groups.is_empty() {
        groups.push(PartSpec {
            staves: vec![vec![1]],
        });
    }
    groups
}

/// Crée les parties et les portées ; renvoie `(portée, voix modèle)` par voix.
fn create_parts(
    score: &mut Score,
    voices: &[Voice],
    groups: &[PartSpec],
) -> Vec<(scoretk_core::model::StaffId, u8)> {
    let mut tracks = vec![(scoretk_core::model::StaffId::from_index(0), 0u8); voices.len()];
    for group in groups {
        let first_voice = group
            .staves
            .first()
            .and_then(|staff| staff.first())
            .copied();
        let name = first_voice
            .and_then(|number| voices.iter().position(|voice| voice.number == number))
            .and_then(|index| voices.get(index))
            .map(|voice| voice.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| {
                let number = first_voice.unwrap_or(1);
                format!("Voix {number}")
            });
        let short_name = first_voice
            .and_then(|number| voices.iter().position(|voice| voice.number == number))
            .and_then(|index| voices.get(index))
            .map(|voice| voice.short_name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| name.clone());
        let part = score.add_part(Part::new(name.clone(), short_name));
        // `%%scoretk instrument` (ABC 2.1 §11) complète `%%MIDI program` et
        // `nm=`/`snm=` : il restaure l'identifiant d'instrument du modèle,
        // que le numéro GM et les noms affichés ne portent pas.
        if let Some(id) = first_voice
            .and_then(|number| voices.iter().position(|voice| voice.number == number))
            .and_then(|index| voices.get(index))
            .and_then(|voice| voice.instrument_id.clone())
        {
            score.part_mut(part).instrument.id = id;
        }
        // Une transposition de voix devient l'instrument transpositeur de la
        // partie : l'écriture MusicXML garde la hauteur écrite et émet
        // `<transpose>`, exactement comme `abc2xml.py`.
        let transpose = first_voice
            .and_then(|number| voices.iter().find(|voice| voice.number == number))
            .filter(|voice| voice.instrument_transpose != 0)
            .map(|voice| Transpose::new(0, voice.instrument_transpose));
        if let Some(transpose) = transpose {
            score.part_mut(part).instrument.transpose = Some(transpose);
        }
        for staff_voices in &group.staves {
            let mut staff = Staff::new(part);
            if let Some(number) = staff_voices.first() {
                if let Some(voice) = voices.iter().find(|voice| voice.number == *number) {
                    if let Some(clef) = voice.initial_clef {
                        staff.default_clef = Clef::new(clef);
                    }
                }
            }
            let id = score.add_staff_with(part, staff);
            for (voice_index, number) in staff_voices.iter().enumerate() {
                if let Some(index) = voices.iter().position(|voice| voice.number == *number) {
                    tracks[index] = (id, voice_index.min(3) as u8);
                }
            }
        }
    }
    tracks
}

/// Durée d'une mesure, toutes voix confondues.
fn measure_len(voices: &[Voice], index: usize) -> Fraction {
    voices
        .iter()
        .filter_map(|voice| voice.measures.get(index))
        .map(|measure| measure.len)
        .fold(Fraction::ZERO, Fraction::max)
}

/// Durée nominale d'une mesure.
fn nominal_len(voices: &[Voice], index: usize) -> Fraction {
    voices
        .iter()
        .filter_map(|voice| voice.measures.get(index))
        .map(|measure| measure.nominal_len)
        .find(|len| len.is_positive())
        .unwrap_or(Fraction::ONE)
}

/// Barre de début commune.
fn measure_start_bar(voices: &[Voice], index: usize) -> BarLineType {
    voices
        .iter()
        .filter_map(|voice| voice.measures.get(index))
        .map(|measure| measure.start_bar)
        .find(|bar| *bar != BarLineType::Normal)
        .unwrap_or(BarLineType::Normal)
}

/// Barre de fin commune.
fn measure_end_bar(voices: &[Voice], index: usize) -> BarLineType {
    voices
        .iter()
        .filter_map(|voice| voice.measures.get(index))
        .map(|measure| measure.end_bar)
        .find(|bar| *bar != BarLineType::Normal)
        .unwrap_or(BarLineType::Normal)
}

/// Indication de mesure en vigueur à une mesure.
fn time_at(voice: &Voice, index: usize) -> TimeSig {
    voice
        .time_changes
        .iter()
        .rev()
        .find(|(measure, tick, value)| *measure <= index && tick.is_zero() && value.is_some())
        .and_then(|(_, _, value)| value.clone())
        .unwrap_or_else(|| voice.initial_time.clone())
}

/// Armure en vigueur à une mesure.
fn key_at(voice: &Voice, index: usize) -> KeySig {
    voice
        .key_changes
        .iter()
        .rev()
        .find(|(measure, tick, value)| *measure <= index && tick.is_zero() && value.is_some())
        .and_then(|(_, _, value)| value.clone())
        .unwrap_or_else(|| voice.initial_key.clone())
}

/// Clef en vigueur à une mesure.
fn clef_at(voice: &Voice, index: usize) -> Option<ClefType> {
    voice
        .clef_changes
        .iter()
        .rev()
        .find(|(measure, tick, value)| *measure <= index && tick.is_zero() && value.is_some())
        .and_then(|(_, _, value)| *value)
        .or(voice.initial_clef)
}

/// Crée les n-olets et renvoie leur identifiant par spécification.
fn create_tuplets(
    score: &mut Score,
    voices: &[Voice],
    starts: &[Fraction],
) -> Vec<Vec<Option<TupletId>>> {
    let mut ids = Vec::new();
    for voice in voices {
        let mut local: Vec<Option<TupletId>> = Vec::with_capacity(voice.tuplets.len());
        for spec in &voice.tuplets {
            let tick = starts
                .get(spec.position.measure)
                .copied()
                .unwrap_or(Fraction::ZERO)
                + spec.position.tick;
            let mut tuplet = Tuplet::new(spec.ratio, spec.base_len, tick, spec.len);
            if let Some(parent) = spec.parent {
                tuplet.parent = local.get(parent).copied().flatten();
            }
            let id = score.add_tuplet(tuplet);
            local.push(Some(id));
        }
        ids.push(local);
    }
    ids
}

/// Événement de segment à un tick donné, créé au besoin.
fn segment_at(
    measure: &mut Measure,
    kind: SegmentType,
    tick: Fraction,
    staff_count: usize,
) -> &mut Segment {
    let position = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind);
    match position {
        Some(index) => &mut measure.segments[index],
        None => {
            let mut segment = Segment::new(kind, tick, staff_count);
            segment.resize_staves(staff_count);
            measure.add_segment(segment);
            let index = measure
                .segments
                .iter()
                .position(|segment| segment.tick == tick && segment.kind == kind)
                .expect("segment ajouté");
            &mut measure.segments[index]
        }
    }
}

/// Convertit une annotation lue.
fn annotation_object(spec: &AnnotationSpec) -> Annotation {
    match spec {
        AnnotationSpec::Fermata(fermata) => Annotation::Fermata(*fermata),
        AnnotationSpec::Tempo(bps, text) => {
            let mut tempo = scoretk_core::model::Tempo::new(text.clone());
            tempo.beats_per_second = Some(*bps);
            Annotation::Tempo(tempo)
        }
        AnnotationSpec::Dynamic(text, velocity) => {
            let mut dynamic = Dynamic::new(text.clone());
            dynamic.velocity = *velocity;
            Annotation::Dynamic(dynamic)
        }
        AnnotationSpec::Text {
            text,
            above,
            harmony,
        } => {
            if *harmony {
                let mut harmony = Harmony::new(text.clone());
                harmony.placement = if *above {
                    Placement::Above
                } else {
                    Placement::Below
                };
                Annotation::Harmony(harmony)
            } else {
                let mut staff_text = scoretk_core::model::StaffText::new(text.clone());
                staff_text.text = RichText::new(text.clone());
                staff_text.placement = if *above {
                    Placement::Above
                } else {
                    Placement::Below
                };
                Annotation::StaffText(staff_text)
            }
        }
    }
}

/// Ajoute un spanner entre deux positions.
fn add_spanner(
    score: &mut Score,
    kind: SpannerType,
    start: Position,
    end: Position,
    track: Track,
    starts: &[Fraction],
) {
    let start_tick = starts.get(start.measure).copied().unwrap_or(Fraction::ZERO) + start.tick;
    let end_tick = starts.get(end.measure).copied().unwrap_or(Fraction::ZERO) + end.tick;
    let mut spanner = Spanner::new(
        kind,
        SpannerAnchor::new(start_tick, track),
        SpannerAnchor::new(end_tick, track),
    );
    // `abc2xml.py::doSlurs` (lignes 1566-1580) écrit `<slur type="start"/>`
    // pour chaque `( … )` ABC. L'export MusicXML n'écrit les slurs que si le
    // spanner porte une `SpannerNotation::Slur` (`notations.rs::slurs`).
    if spanner.kind == SpannerType::Slur {
        spanner.notation = Some(SpannerNotation::Slur(Slur::new()));
    }
    score.add_spanner(spanner);
}

/// Table `mesure → début`, utile aux tests.
#[cfg(test)]
fn starts_of(voices: &[Voice]) -> std::collections::BTreeMap<usize, Fraction> {
    let mut starts = std::collections::BTreeMap::new();
    let mut cursor = Fraction::ZERO;
    for index in 0..voices
        .iter()
        .map(|voice| voice.measures.len())
        .max()
        .unwrap_or(0)
    {
        starts.insert(index, cursor);
        cursor += measure_len(voices, index);
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groupes_de_voix() {
        let reader = Reader {
            score_spec: Some("(1 2) 3".to_owned()),
            ..Default::default()
        };
        let fields = |number| crate::reader::VoiceFields {
            number,
            name: String::new(),
            short_name: String::new(),
            clef: None,
            transpose: None,
        };
        let voices = vec![
            Voice::new(
                &fields(1),
                Fraction::new(1, 8),
                TimeSig::new(4, 4),
                KeySig::new(scoretk_core::model::Key::C),
            ),
            Voice::new(
                &fields(2),
                Fraction::new(1, 8),
                TimeSig::new(4, 4),
                KeySig::new(scoretk_core::model::Key::C),
            ),
            Voice::new(
                &fields(3),
                Fraction::new(1, 8),
                TimeSig::new(4, 4),
                KeySig::new(scoretk_core::model::Key::C),
            ),
        ];
        let groups = groups(&reader, &voices);
        assert_eq!(
            groups,
            vec![
                PartSpec {
                    staves: vec![vec![1, 2]]
                },
                PartSpec {
                    staves: vec![vec![3]]
                },
            ]
        );
    }

    #[test]
    fn durees_des_mesures() {
        let fields = crate::reader::VoiceFields {
            number: 1,
            name: String::new(),
            short_name: String::new(),
            clef: None,
            transpose: None,
        };
        let mut voice = Voice::new(
            &fields,
            Fraction::new(1, 8),
            TimeSig::new(4, 4),
            KeySig::new(scoretk_core::model::Key::C),
        );
        voice.finish_measure(BarLineType::Normal);
        let voices = vec![voice];
        assert_eq!(measure_len(&voices, 0), Fraction::ONE);
        assert_eq!(starts_of(&voices).get(&0), Some(&Fraction::ZERO));
    }
}
