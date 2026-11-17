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
    Annotation, BarLineType, Clef, ClefType, Dynamic, Fraction, Harmony, KeySig, Measure, Part,
    Placement, RichText, Score, Segment, SegmentItem, SegmentType, Spanner, SpannerAnchor,
    SpannerType, Staff, TimeSig, Track, Tuplet, TupletId, VoltaInfo,
};

use crate::error::Result;
use crate::reader::{AnnotationSpec, Position, Reader, Voice};

/// Construit la partition.
pub(crate) fn build(reader: &mut Reader) -> Result<Score> {
    let voices = std::mem::take(&mut reader.voices);
    let mut score = std::mem::replace(&mut reader.score, Score::new());
    score.metadata.work_title = reader.title.clone();
    score.metadata.composer = reader.composer.clone();
    score.metadata.source = reader.source.clone();

    let groups = groups(reader, &voices);
    let voice_tracks = create_parts(&mut score, &voices, &groups);
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
            if index == 0 || time_at(voice, index) != time_at(voice, index - 1) {
                let mut segment = Segment::new(SegmentType::TimeSig, start, staff_count);
                segment.set_element(track, SegmentItem::TimeSig(time_at(voice, index)));
                measure.add_segment(segment);
            }
            if index == 0 || key_at(voice, index) != key_at(voice, index - 1) {
                let mut segment = Segment::new(SegmentType::KeySig, start, staff_count);
                segment.set_element(track, SegmentItem::KeySig(key_at(voice, index)));
                measure.add_segment(segment);
            }
            if index == 0 || clef_at(voice, index) != clef_at(voice, index - 1) {
                if let Some(clef) = clef_at(voice, index) {
                    let mut segment = Segment::new(SegmentType::HeaderClef, start, staff_count);
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

        measure
            .segments
            .sort_by_key(|segment| (segment.tick, segment.kind));
        score.add_measure(measure);
    }

    // Tempo global sur la première mesure.
    if let Some((bps, text)) = &reader.tempo {
        if let Some(measure) = score.measures.first_mut() {
            let mut tempo = scoretk_core::model::Tempo::new(text.clone());
            tempo.beats_per_second = Some(*bps);
            let segment = segment_at(measure, SegmentType::TimeTick, Fraction::ZERO, staff_count);
            let track = Track::new(scoretk_core::model::StaffId::from_index(0), 0);
            segment.add_annotation(track, Annotation::Tempo(tempo));
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
fn parse_score_spec(spec: &str) -> Vec<PartSpec> {
    let mut parser = ScoreSpecParser {
        chars: spec.chars().collect(),
        pos: 0,
    };
    parser.sequence(None)
}

struct ScoreSpecParser {
    chars: Vec<char>,
    pos: usize,
}

impl ScoreSpecParser {
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
                        if value.is_ascii_digit() {
                            let mut number = String::new();
                            while self
                                .chars
                                .get(self.pos)
                                .is_some_and(|digit| digit.is_ascii_digit())
                            {
                                number.push(self.chars[self.pos]);
                                self.pos += 1;
                            }
                            if let Ok(number) = number.parse() {
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
                value if value.is_ascii_digit() => {
                    let mut number = String::new();
                    while self
                        .chars
                        .get(self.pos)
                        .is_some_and(|digit| digit.is_ascii_digit())
                    {
                        number.push(self.chars[self.pos]);
                        self.pos += 1;
                    }
                    if let Ok(number) = number.parse() {
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
        parse_score_spec(spec)
    } else {
        Vec::new()
    };
    if groups.is_empty() {
        groups = voices
            .iter()
            .map(|voice| PartSpec {
                staves: vec![vec![voice.number]],
            })
            .collect();
    }
    // Ajoute les voix absentes de `%%score` comme portées isolées.
    let mut present = std::collections::BTreeSet::new();
    for part in &groups {
        for staff in &part.staves {
            present.extend(staff.iter().copied());
        }
    }
    for voice in voices {
        if !present.contains(&voice.number) {
            groups.push(PartSpec {
                staves: vec![vec![voice.number]],
            });
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
        let part = score.add_part(Part::new(name.clone(), name));
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
        AnnotationSpec::Dynamic(text) => Annotation::Dynamic(Dynamic::new(text.clone())),
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
    let spanner = Spanner::new(
        kind,
        SpannerAnchor::new(start_tick, track),
        SpannerAnchor::new(end_tick, track),
    );
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
            clef: None,
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
            clef: None,
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
