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

use mscz2xml::model::{
    Annotation, BarLineType, Clef, ClefType, Dynamic, Fraction, Harmony,
    KeySig, Measure, Part, Placement, RichText, Score, Segment, SegmentItem, SegmentType, Spanner,
    SpannerAnchor, SpannerType, Staff, TimeSig, Track, Tuplet, TupletId, VoltaInfo,
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
    let staff_of_voice = create_parts(&mut score, &voices, &groups);
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

    for index in 0..measure_count {
        let nominal = nominal_len(&voices, index);
        let len = measure_len(&voices, index);
        let mut measure = Measure::new(index, starts[index], nominal).with_len(len);
        measure.start_bar_line = measure_start_bar(&voices, index);
        measure.end_bar_line = measure_end_bar(&voices, index);
        measure.repeat_count = 0;

        // Segments de contexte, sur chaque portée.
        for (voice_index, voice) in voices.iter().enumerate() {
            let staff = staff_of_voice[voice_index];
            let track = Track::new(staff, 0);
            if index == 0 || time_at(voice, index) != time_at(voice, index - 1) {
                let mut segment = Segment::new(SegmentType::TimeSig, starts[index], staff_count);
                segment.set_element(track, SegmentItem::TimeSig(time_at(voice, index)));
                measure.add_segment(segment);
            }
            if index == 0 || key_at(voice, index) != key_at(voice, index - 1) {
                let mut segment = Segment::new(SegmentType::KeySig, starts[index], staff_count);
                segment.set_element(track, SegmentItem::KeySig(key_at(voice, index)));
                measure.add_segment(segment);
            }
            if index == 0 || clef_at(voice, index) != clef_at(voice, index - 1) {
                if let Some(clef) = clef_at(voice, index) {
                    let mut segment =
                        Segment::new(SegmentType::HeaderClef, starts[index], staff_count);
                    segment.set_element(
                        track,
                        SegmentItem::Clef(Clef::new(clef)),
                    );
                    measure.add_segment(segment);
                }
            }
        }

        // Événements de chaque voix.
        for (voice_index, voice) in voices.iter().enumerate() {
            let staff = staff_of_voice[voice_index];
            let track = Track::new(staff, 0);
            let Some(data) = voice.measures.get(index) else {
                add_full_measure_rest(&mut measure, starts[index], track, staff_count);
                continue;
            };
            if data.events.is_empty() {
                add_full_measure_rest(&mut measure, starts[index], track, staff_count);
                continue;
            }
            for event in &data.events {
                let tick = starts[index] + event.tick;
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

        measure.segments.sort_by_key(|segment| (segment.tick, segment.kind));
        score.add_measure(measure);
    }

    // Tempo global sur la première mesure.
    if let Some((bps, text)) = &reader.tempo {
        if let Some(measure) = score.measures.first_mut() {
            let mut tempo = mscz2xml::model::Tempo::new(text.clone());
            tempo.beats_per_second = Some(*bps);
            let segment = segment_at(measure, SegmentType::TimeTick, Fraction::ZERO, staff_count);
            let track = Track::new(mscz2xml::model::StaffId::from_index(0), 0);
            segment.add_annotation(track, Annotation::Tempo(tempo));
        }
    }

    // Slurs, soufflets et volatas.
    for (voice_index, voice) in voices.iter().enumerate() {
        let staff = staff_of_voice[voice_index];
        let track = Track::new(staff, 0);
        for (start, end) in &voice.slurs {
            add_spanner(
                &mut score,
                SpannerType::Slur,
                *start,
                *end,
                track,
                &starts,
            );
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

/// Groupes de voix : un groupe par partie.
fn groups(reader: &Reader, voices: &[Voice]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut used = vec![false; voices.len()];
    if let Some(spec) = &reader.score_spec {
        let mut current: Vec<usize> = Vec::new();
        let mut open = false;
        for token in spec.split_whitespace() {
            let token = token.trim();
            let starts_group = token.starts_with('(') || token.starts_with('[') || token.starts_with('{');
            let ends_group = token.ends_with(')') || token.ends_with(']') || token.ends_with('}');
            let mut number_text = token
                .trim_start_matches(['(', '[', '{'])
                .trim_end_matches([')', ']', '}'])
                .to_owned();
            if starts_group {
                open = true;
                current.clear();
            }
            if number_text.contains('+') {
                number_text = number_text.replace('+', " ");
            }
            for part in number_text.split([' ', '+']) {
                if let Ok(number) = part.parse::<usize>() {
                    if let Some(voice_index) =
                        voices.iter().position(|voice| voice.number == number)
                    {
                        used[voice_index] = true;
                        current.push(voice_index);
                    }
                }
            }
            if ends_group || !open {
                if !current.is_empty() {
                    groups.push(std::mem::take(&mut current));
                }
                open = false;
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }
    }
    for (index, used) in used.iter().enumerate() {
        if !used {
            groups.push(vec![index]);
        }
    }
    if groups.is_empty() {
        groups.push((0..voices.len()).collect());
    }
    groups
}

/// Crée les parties et les portées, et renvoie la portée de chaque voix.
fn create_parts(
    score: &mut Score,
    voices: &[Voice],
    groups: &[Vec<usize>],
) -> Vec<mscz2xml::model::StaffId> {
    let mut staff_of_voice = Vec::with_capacity(voices.len());
    staff_of_voice.resize(voices.len(), mscz2xml::model::StaffId::from_index(0));
    let mut staff_index = 0usize;
    for group in groups {
        let name = group
            .first()
            .and_then(|index| voices.get(*index))
            .map(|voice| voice.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Voix {}", staff_index + 1));
        let part = score.add_part(Part::new(name.clone(), name));
        for voice_index in group {
            let mut staff = Staff::new(part);
            if let Some(voice) = voices.get(*voice_index) {
                if let Some(clef) = voice.initial_clef {
                    staff.default_clef = Clef::new(clef);
                }
            }
            let id = score.add_staff_with(part, staff);
            if let Some(slot) = staff_of_voice.get_mut(*voice_index) {
                *slot = id;
            }
            staff_index += 1;
        }
    }
    staff_of_voice
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
        .unwrap_or_else(|| Fraction::ONE)
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
        .find(|(measure, value)| *measure <= index && value.is_some())
        .and_then(|(_, value)| value.clone())
        .unwrap_or_else(|| voice.initial_time.clone())
}

/// Armure en vigueur à une mesure.
fn key_at(voice: &Voice, index: usize) -> KeySig {
    voice
        .key_changes
        .iter()
        .rev()
        .find(|(measure, value)| *measure <= index && value.is_some())
        .and_then(|(_, value)| value.clone())
        .unwrap_or_else(|| voice.initial_key.clone())
}

/// Clef en vigueur à une mesure.
fn clef_at(voice: &Voice, index: usize) -> Option<ClefType> {
    voice
        .clef_changes
        .iter()
        .rev()
        .find(|(measure, value)| *measure <= index && value.is_some())
        .and_then(|(_, value)| *value)
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
fn segment_at<'a>(
    measure: &'a mut Measure,
    kind: SegmentType,
    tick: Fraction,
    staff_count: usize,
) -> &'a mut Segment {
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

/// Silence de mesure entière sur une portée.
fn add_full_measure_rest(
    measure: &mut Measure,
    tick: Fraction,
    track: Track,
    staff_count: usize,
) {
    let segment = segment_at(measure, SegmentType::ChordRest, tick, staff_count);
    segment.set_element(
        track,
        SegmentItem::ChordRest(mscz2xml::model::ChordRest::full_measure_rest()),
    );
}

/// Convertit une annotation lue.
fn annotation_object(spec: &AnnotationSpec) -> Annotation {
    match spec {
        AnnotationSpec::Dynamic(text) => {
            Annotation::Dynamic(Dynamic::new(text.clone()))
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
                let mut staff_text = mscz2xml::model::StaffText::new(text.clone());
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
    for index in 0..voices.iter().map(|voice| voice.measures.len()).max().unwrap_or(0) {
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
        let mut reader = Reader::default();
        reader.score_spec = Some("(1 2) 3".to_owned());
        let fields = |number| crate::reader::VoiceFields {
            number,
            name: String::new(),
            clef: None,
        };
        let voices = vec![
            Voice::new(&fields(1), Fraction::new(1, 8), TimeSig::new(4, 4), KeySig::new(mscz2xml::model::Key::C)),
            Voice::new(&fields(2), Fraction::new(1, 8), TimeSig::new(4, 4), KeySig::new(mscz2xml::model::Key::C)),
            Voice::new(&fields(3), Fraction::new(1, 8), TimeSig::new(4, 4), KeySig::new(mscz2xml::model::Key::C)),
        ];
        let groups = groups(&reader, &voices);
        assert_eq!(groups, vec![vec![0, 1], vec![2]]);
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
            KeySig::new(mscz2xml::model::Key::C),
        );
        voice.finish_measure(BarLineType::Normal);
        let voices = vec![voice];
        assert_eq!(measure_len(&voices, 0), Fraction::ONE);
        assert_eq!(starts_of(&voices).get(&0), Some(&Fraction::ZERO));
    }
}
