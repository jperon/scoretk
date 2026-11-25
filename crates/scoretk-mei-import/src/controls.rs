// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` de MuseScore Studio
// 4.7.4 (`readControlEvents`, `addAnnotation`, `addSpanner`, `findStart`,
// `findEnd`, `readVerses`, `readSyl`, `addSpannerEnds`), Copyright (C)
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

//! Control events (slurs, ties, dynamics, directions, …) and lyrics.

use scoretk_core::model::{
    Annotation, Arpeggio, ArpeggioType, Articulation, ArticulationType, Breath, BreathType,
    ChordRestKind, Dynamic, Fermata, FermataType, Fingering, Fraction, HairpinType, Harmony,
    Layers, Lyrics, LyricsSyllabic, Marker, MarkerType, Ornament, OrnamentType, Placement,
    RehearsalMark, Segment, SegmentItem, SegmentType, Slur, Spanner, SpannerAnchor,
    SpannerNotation, SpannerType, StaffId, StaffText, SystemText, Tempo, Track,
};
use scoretk_mei_common::convert;
use scoretk_xml_common::Element;

use crate::reader::{PendingSpanner, Reader};

/// Control events with no pivot representation are counted as losses.
const IGNORED_CONTROLS: [&str; 8] = [
    "annot",
    "harpPedal",
    "lv",
    "copyof",
    "sameas",
    "facs",
    "timeline",
    "expansion",
];

impl<'a> Reader<'a> {
    /// `MeiImporter::readControlEvents` (`meiimporter.cpp:2358`).
    pub(crate) fn read_control_events(&mut self, parent: &Element, measure: usize) {
        for node in &parent.children {
            if IGNORED_CONTROLS.contains(&node.name.as_str()) {
                self.score
                    .report
                    .record(Layers::EXTENSIONS, "control-event", None, None);
                continue;
            }
            match node.name.as_str() {
                "tie" => self.read_tie(node, measure),
                "slur" | "phrase" => self.read_slur(node, measure, false),
                "trill" => {
                    self.read_slur(node, measure, true);
                    self.add_ornament(node, measure, OrnamentType::TrillMark);
                }
                "dynam" => self.add_dynamic(node, measure),
                "dir" => self.add_direction(node, measure),
                "tempo" => self.add_tempo(node, measure),
                "reh" => self.add_rehearsal(node, measure),
                "hairpin" => self.add_hairpin(node, measure),
                "pedal" => self.add_span(node, measure, SpannerType::Pedal),
                "octave" => self.add_octave(node, measure),
                "fermata" => self.add_fermata(node, measure),
                "artic" => self.add_articulation(node, parent, measure),
                "arpeg" => self.add_arpeggio(node, measure),
                "fing" => self.add_fingering(node, measure),
                "harm" => self.add_harmony(node, measure),
                "breath" | "caesura" => self.add_breath(node, measure),
                "trill-spanner" => self.read_slur(node, measure, true),
                "mordent" => {
                    let kind = if node.attribute("form") == Some("lower") {
                        OrnamentType::InvertedMordent
                    } else {
                        OrnamentType::Mordent
                    };
                    self.add_ornament(node, measure, kind);
                }
                "turn" => {
                    let kind = if node.attribute("form") == Some("lower") {
                        OrnamentType::InvertedTurn
                    } else {
                        OrnamentType::Turn
                    };
                    self.add_ornament(node, measure, kind);
                }
                "ornam" => self.add_ornament(node, measure, OrnamentType::Other(String::new())),
                "gliss" => self.add_span(node, measure, SpannerType::Glissando),
                "repeatMark" => self.add_repeat_mark(node, measure),
                "verse" => {
                    let mut scratch = Fraction::ZERO;
                    self.read_verses(parent, measure, Track::from_raw(0), &mut scratch);
                }
                _ => {}
            }
        }
    }

    /// Resolve `@startid` / `@tstamp` (`MeiImporter::findStart`, `:620`).
    pub(crate) fn anchor(&self, node: &Element, measure: usize) -> (Fraction, Track) {
        if let Some(id) = node.attribute("startid").or_else(|| node.attribute("id")) {
            let id = id.trim_start_matches('#');
            if let Some((_, track, tick)) = self.id_events.get(id) {
                return (*tick, *track);
            }
        }
        if let Some(stamp) = node
            .attribute("tstamp")
            .and_then(|value| value.parse::<f64>().ok())
        {
            let tick = self.score.measures[measure].start_tick
                + convert::tstamp_to_fraction(
                    stamp,
                    convert::tstamp_denominator(&self.score.measures[measure]),
                );
            let staff = node
                .attribute("staff")
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(1);
            let track = Track::new(StaffId::from_index(staff.saturating_sub(1) as usize), 0);
            return (tick, track);
        }
        // `findStart`: first event at or after the current position.
        for segment in &self.score.measures[measure].segments {
            for (track, item) in segment.iter_elements() {
                if item.as_chord_rest().is_some() {
                    return (segment.tick, track);
                }
            }
        }
        (self.score.measures[measure].start_tick, Track::from_raw(0))
    }

    /// `MeiImporter::findEnd` (`meiimporter.cpp:672`).
    fn end_anchor(&self, node: &Element, measure: usize) -> Option<(Fraction, Track)> {
        if let Some(id) = node.attribute("endid") {
            let id = id.trim_start_matches('#');
            if let Some((_, track, tick)) = self.id_events.get(id) {
                return Some((*tick, *track));
            }
        }
        if let Some(stamp) = node
            .attribute("tstamp2")
            .and_then(|value| value.parse::<f64>().ok())
        {
            return Some((
                self.score.measures[measure].start_tick
                    + convert::tstamp_to_fraction(
                        stamp,
                        convert::tstamp_denominator(&self.score.measures[measure]),
                    ),
                self.anchor(node, measure).1,
            ));
        }
        None
    }

    /// Attach an annotation to a segment, creating it if needed.
    fn add_annotation(&mut self, tick: Fraction, track: Track, annotation: Annotation) {
        let measure = self
            .score
            .measures
            .iter()
            .position(|measure| tick >= measure.start_tick && tick < measure.end_tick())
            .unwrap_or(0);
        let staff_count = self.score.staves.len().max(1);
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|segment| segment.tick == tick && segment.kind == SegmentType::ChordRest);
        if let Some(position) = position {
            self.score.measures[measure].segments[position].add_annotation(track, annotation);
        } else {
            let mut segment = Segment::new(SegmentType::ChordRest, tick, staff_count);
            segment.add_annotation(track, annotation);
            self.score.measures[measure].add_segment(segment);
        }
    }

    /// `MeiImporter::readTie` (`meiimporter.cpp:3111`).
    fn read_tie(&mut self, node: &Element, _measure: usize) {
        let start = node
            .attribute("startid")
            .unwrap_or_default()
            .trim_start_matches('#')
            .to_owned();
        let end = node
            .attribute("endid")
            .unwrap_or_default()
            .trim_start_matches('#')
            .to_owned();
        // `Convert::curvedirFromMEI` (meiconverter.cpp:1112) : `@curvedir`
        // porte la direction explicite de la liaison (`Note::tie_up`).
        let up = match node.attribute("curvedir") {
            Some("above") => Some(true),
            Some("below") => Some(false),
            _ => None,
        };
        if !start.is_empty() && !end.is_empty() {
            self.pending_ties.push((start, end, up));
        }
    }

    /// `MeiImporter::readSlur` (`meiimporter.cpp:3058`).
    fn read_slur(&mut self, node: &Element, measure: usize, is_trill: bool) {
        let start = self.anchor(node, measure);
        let kind = if is_trill {
            SpannerType::Trill
        } else {
            SpannerType::Slur
        };
        let end_id = node.attribute("endid").map(str::to_owned);
        let end_tstamp = node
            .attribute("tstamp2")
            .and_then(|value| value.parse::<f64>().ok());
        let end = self.end_anchor(node, measure);
        let number = node
            .attribute("n")
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0);
        let placement = convert::curvedir_from_mei(node.attribute("curvedir"));
        let notation = placement.map(|placement| {
            SpannerNotation::Slur(Slur {
                placement: Some(placement),
                visible: true,
            })
        });
        if let (Some((tick, track)), None, None) = (end, end_id.as_ref(), end_tstamp) {
            let mut spanner = Spanner::new(
                kind,
                SpannerAnchor::new(start.0, start.1),
                SpannerAnchor::new(tick, track),
            );
            spanner.number = number;
            spanner.notation = notation;
            self.score.add_spanner(spanner);
        } else {
            self.pending.push(PendingSpanner {
                kind,
                number,
                start,
                end_id,
                end_tstamp,
                measure,
                placement,
            });
        }
    }

    /// `MeiImporter::addSpanner` (`meiimporter.cpp:516`) for span-based events.
    fn add_span(&mut self, node: &Element, measure: usize, kind: SpannerType) {
        let start = self.anchor(node, measure);
        let end = self.end_anchor(node, measure);
        match end {
            Some((tick, track)) => {
                self.score.add_spanner(Spanner::new(
                    kind,
                    SpannerAnchor::new(start.0, start.1),
                    SpannerAnchor::new(tick, track),
                ));
            }
            None => self.pending.push(PendingSpanner {
                kind,
                number: 0,
                start,
                end_id: node.attribute("endid").map(str::to_owned),
                end_tstamp: node
                    .attribute("tstamp2")
                    .and_then(|value| value.parse::<f64>().ok()),
                measure,
                placement: None,
            }),
        }
    }

    /// `MeiImporter::readHairpin` (`meiimporter.cpp:2769`).
    fn add_hairpin(&mut self, node: &Element, measure: usize) {
        let form = node.attribute("form").unwrap_or("cres");
        let kind = SpannerType::Hairpin(if form == "dim" {
            HairpinType::Diminuendo
        } else {
            HairpinType::Crescendo
        });
        self.add_span(node, measure, kind);
    }

    /// `MeiImporter::readOctave` (`meiimporter.cpp:2922`).
    fn add_octave(&mut self, node: &Element, measure: usize) {
        let dis = node
            .attribute("dis")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(8);
        let below = node.attribute("dis.place") == Some("below");
        let kind = match (dis, below) {
            (8, false) => scoretk_core::model::OttavaType::OctaveUp,
            (8, true) => scoretk_core::model::OttavaType::OctaveDown,
            (15, false) => scoretk_core::model::OttavaType::FifteenthUp,
            _ => scoretk_core::model::OttavaType::FifteenthDown,
        };
        self.add_span(node, measure, SpannerType::Ottava(kind));
    }

    /// `MeiImporter::readDynam` (`meiimporter.cpp:2548`).
    fn add_dynamic(&mut self, node: &Element, measure: usize) {
        let text = node
            .attribute("label")
            .map(str::to_owned)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| crate::lines_of(node));
        // Une nuance « blanche » (texte vide, simple changement de
        // vitesse) est un objet du modèle : `@val` suffit à la porter.
        let velocity = node
            .attribute("val")
            .and_then(|value| value.parse::<u8>().ok());
        if text.is_empty() && velocity.is_none() {
            return;
        }
        let (tick, track) = self.anchor(node, measure);
        let mut dynamic = Dynamic::new(text);
        dynamic.placement = convert::place_from_mei(node.attribute("place"));
        // `att.midiValue/@val` (MEI 5.1 `att.dynam.ges`) : les Guidelines
        // donnent `<dynam val="84">f</dynam>` comme vitesse MIDI de la
        // nuance. MuseScore 4.7.4 n'écrit pas l'attribut ; le lecture le
        // consomme quand un encodeur externe le fournit.
        dynamic.velocity = velocity;
        self.add_annotation(tick, track, Annotation::Dynamic(dynamic));
    }

    /// `MeiImporter::readDir` (`meiimporter.cpp:2511`).
    fn add_direction(&mut self, node: &Element, measure: usize) {
        let text = crate::lines_of(node);
        let typ = node.attribute("type").unwrap_or_default();
        // Porteur additionnel pour un `TBox` de MuseScore : MEI 5.1 n'a pas
        // d'élément de cadre ancré sur une mesure, mais `<dir>` est un
        // événement de contrôle légal et `@label`/`@type` gardent la position.
        if let Some(index) = typ
            .strip_prefix(scoretk_mei_common::ids::TBOX_TYPE)
            .and_then(|value| value.parse::<usize>().ok())
        {
            let mut tbox = scoretk_core::model::TboxText::new(
                scoretk_core::model::RichText::new(text),
                measure,
            );
            if let Some(frame) = node
                .attribute("label")
                .and_then(|label| label.strip_prefix(scoretk_mei_common::ids::TBOX_FRAME_LABEL))
            {
                let mut fields = frame.split(':');
                tbox.measure = fields
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(measure);
                match fields.next() {
                    Some("Below") => tbox.placement = Placement::Below,
                    _ => tbox.placement = Placement::Above,
                }
                tbox.font_size = fields.next().and_then(|value| value.parse::<f64>().ok());
            }
            let texts = &mut self.score.engraving.tbox_texts;
            if texts.len() <= index {
                texts.resize_with(index + 1, || {
                    scoretk_core::model::TboxText::new(
                        scoretk_core::model::RichText::new(String::new()),
                        0,
                    )
                });
            }
            texts[index] = tbox;
            return;
        }
        let (tick, track) = self.anchor(node, measure);
        if typ.contains("mscore-hairpin") {
            let kind = if typ.contains("cresc") {
                HairpinType::Crescendo
            } else {
                HairpinType::Diminuendo
            };
            self.add_annotation(
                tick,
                track,
                Annotation::Other {
                    kind: "hairpin-text".to_owned(),
                    text,
                    placement: convert::place_from_mei(node.attribute("place")),
                },
            );
            let _ = kind;
            return;
        }
        if typ.contains("mscore-system-text") {
            self.add_annotation(tick, track, Annotation::SystemText(SystemText::new(text)));
        } else {
            self.add_annotation(tick, track, Annotation::StaffText(StaffText::new(text)));
        }
    }

    /// `MeiImporter::readTempo` (`meiimporter.cpp:3083`).
    fn add_tempo(&mut self, node: &Element, measure: usize) {
        let text = node.attribute("mm").map(str::to_owned).unwrap_or_else(|| {
            // Mixed content: MuseScore writes a SMuFL `<rend>` glyph before
            // the plain text (`tempoFromMEI`, meiconverter.cpp:3327).
            let mut text = String::new();
            for child in &node.children {
                if child.name == "rend" || child.name == "symbol" {
                    text.push_str(&crate::lines_of(child));
                }
            }
            let plain = node.text_trimmed();
            if !text.is_empty() && !plain.is_empty() {
                // `<rend glyph.auth="smufl"></rend> = 80` : le glyphe et
                // le texte sont séparés par un espace dans le modèle.
                text.push(' ');
            }
            text.push_str(plain);
            text
        });
        let bpm = node
            .attribute("midi.bpm")
            .and_then(|value| value.parse::<f64>().ok());
        let (tick, track) = self.anchor(node, measure);
        let mut tempo = Tempo::new(text);
        // `@midi.bpm` is beats per minute; the pivot stores beats per second.
        tempo.beats_per_second = bpm.map(|value| value / 60.0);
        self.add_annotation(tick, track, Annotation::Tempo(tempo));
    }

    /// `MeiImporter::readReh` (`meiimporter.cpp:2998`).
    fn add_rehearsal(&mut self, node: &Element, measure: usize) {
        let text = crate::lines_of(node);
        if text.is_empty() {
            return;
        }
        let (tick, track) = self.anchor(node, measure);
        self.add_annotation(
            tick,
            track,
            Annotation::RehearsalMark(RehearsalMark::new(text)),
        );
    }

    /// `MeiImporter::readFermata` (`meiimporter.cpp:2649`).
    fn add_fermata(&mut self, node: &Element, measure: usize) {
        let kind = match node.attribute("form").unwrap_or("-normal") {
            "short" | "angled" => FermataType::Short,
            "long" | "square" => FermataType::Long,
            _ => FermataType::Normal,
        };
        let (tick, track) = self.anchor(node, measure);
        self.add_annotation(tick, track, Annotation::Fermata(Fermata::new(kind)));
    }

    /// `MeiImporter::readArtic` (`meiimporter.cpp:1720`).
    fn add_articulation(&mut self, node: &Element, parent: &Element, measure: usize) {
        let name = node.attribute("artic").unwrap_or_default();
        let kind = match name {
            "stacc" => ArticulationType::Staccato,
            "stacciss" | "spicc" => ArticulationType::Spiccato,
            "accent" | "acc" => ArticulationType::Accent,
            "marc" => ArticulationType::Marcato,
            "ten" => ArticulationType::Tenuto,
            "port" => ArticulationType::Portato,
            "stress" => ArticulationType::Stress,
            "unstress" => ArticulationType::Unstress,
            "upbow" => ArticulationType::UpBow,
            "dnbow" => ArticulationType::DownBow,
            "harm" => ArticulationType::Harmonic,
            "open" => ArticulationType::Open,
            "stop" => ArticulationType::Stopped,
            "snap" => ArticulationType::SnapPizzicato,
            _ => ArticulationType::Other(name.to_owned()),
        };
        // `<artic>` enfant d'un événement : MEI 5.1 `att.articulated`
        // s'applique à cet événement, pas au dernier accord de la portée.
        // L'`xml:id` du parent (ou `@startid`) donne l'ancrage exact.
        let id = node
            .attribute("startid")
            .or_else(|| parent.attribute("id"))
            .map(|id| id.trim_start_matches('#'));
        let (tick, track) = id
            .and_then(|id| self.id_events.get(id))
            .map(|(_, track, tick)| (*tick, *track))
            .unwrap_or_else(|| self.anchor(node, measure));
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|segment| segment.tick == tick && segment.kind == SegmentType::ChordRest);
        let cr = position.and_then(|position| {
            self.score.measures[measure].segments[position].element_mut(track)
        });
        if let Some(SegmentItem::ChordRest(cr)) = cr {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.articulations.push(Articulation {
                    kind,
                    ..Articulation::new(ArticulationType::Staccato)
                });
            }
        }
    }

    /// `MeiImporter::readArpeg` (`meiimporter.cpp:2431`).
    fn add_arpeggio(&mut self, node: &Element, measure: usize) {
        let (_, track) = self.anchor(node, measure);
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.arpeggio = Some(Arpeggio::new(ArpeggioType::Normal));
            }
        }
    }

    /// `MeiImporter::readFing` (`meiimporter.cpp:2691`).
    fn add_fingering(&mut self, node: &Element, measure: usize) {
        let text = crate::lines_of(node);
        let (_, track) = self.anchor(node, measure);
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                if let Some(id) = chord.notes.first().copied() {
                    if id.index() < self.score.notes.len() {
                        self.score.notes[id.index()]
                            .fingerings
                            .push(Fingering::new(text));
                    }
                }
            }
        }
    }

    /// `MeiImporter::readHarm` (`meiimporter.cpp:2795`).
    fn add_harmony(&mut self, node: &Element, measure: usize) {
        let text = crate::lines_of(node);
        if text.is_empty() {
            return;
        }
        let (tick, track) = self.anchor(node, measure);
        self.add_annotation(tick, track, Annotation::Harmony(Harmony::new(text)));
    }

    /// `MeiImporter::readBreath`/`readCaesura` (`:2461`, `:2486`).
    fn add_breath(&mut self, node: &Element, measure: usize) {
        let (tick, track) = self.anchor(node, measure);
        let kind = if node.name == "caesura" {
            BreathType::Caesura
        } else {
            BreathType::Comma
        };
        let mut breath = Breath {
            kind,
            placement: Placement::Above,
            pause_ms: None,
        };
        breath.placement = convert::place_from_mei(node.attribute("place"));
        let staff_count = self.score.staves.len().max(1);
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|segment| segment.tick == tick);
        if let Some(position) = position {
            self.score.measures[measure].segments[position]
                .set_element(track, SegmentItem::Breath(breath));
        } else {
            let mut segment = Segment::new(SegmentType::Breath, tick, staff_count);
            segment.set_element(track, SegmentItem::Breath(breath));
            self.score.measures[measure].add_segment(segment);
        }
    }

    /// `MeiImporter::readOrnam` / `readMordent` / `readTurn` / `readTrill`.
    fn add_ornament(&mut self, node: &Element, measure: usize, kind: OrnamentType) {
        let (_, track) = self.anchor(node, measure);
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.ornaments.push(Ornament::new(kind));
            }
        }
    }

    /// `MeiImporter::readRepeatMark` (`meiimporter.cpp:3029`).
    fn add_repeat_mark(&mut self, node: &Element, measure: usize) {
        let func = node.attribute("func").unwrap_or_default();
        let typed = node.attribute("type").unwrap_or_default();
        let marker_suffix =
            convert::get_type_value_with_prefix(typed, scoretk_mei_common::ids::MARKER_TYPE);
        let jump_suffix =
            convert::get_type_value_with_prefix(typed, scoretk_mei_common::ids::JUMP_TYPE);
        let text = crate::lines_of(node);
        let kind = match marker_suffix {
            Some(suffix) => Some(convert::marker_type_from_mei(suffix)),
            None => match func {
                "segno" => Some(MarkerType::Segno),
                "coda" => Some(MarkerType::Coda),
                "fine" => Some(MarkerType::Fine),
                _ => None,
            },
        };
        if let Some(kind) = kind {
            let label = if text.is_empty() {
                marker_suffix
                    .map(|_| convert::marker_type_to_mei(kind))
                    .unwrap_or(func)
                    .to_owned()
            } else {
                // Le texte de `<repeatMark>` est l'étiquette MuseScore
                // (`Marker::label`) ; `@type` ne donne que le genre.
                text.clone()
            };
            let marker = Marker {
                kind,
                label: label.clone(),
                text: label,
                placement: Placement::Above,
                track: Track::from_raw(0),
                visible: true,
            };
            self.score.measures[measure]
                .repeat_marks
                .push(scoretk_core::model::RepeatMark::Marker(marker));
        } else if func.contains("daCapo") || func.contains("dalSegno") {
            let jump_kind = jump_suffix.map(convert::jump_type_from_mei).unwrap_or(
                if func.contains("daCapo") {
                    scoretk_core::model::JumpType::Dc
                } else {
                    scoretk_core::model::JumpType::Ds
                },
            );
            let jump_to = if text.is_empty() {
                if jump_kind.is_da_capo() {
                    "start"
                } else {
                    "segno"
                }
            } else {
                &text
            };
            let jump_to = jump_to.to_owned();
            let mut jump = scoretk_core::model::Jump::new(
                text,
                jump_to.to_owned(),
                String::new(),
                String::new(),
                Placement::Above,
                Track::from_raw(0),
            );
            jump.kind = jump_kind;
            self.score.measures[measure]
                .repeat_marks
                .push(scoretk_core::model::RepeatMark::Jump(jump));
        }
    }

    /// `MeiImporter::addSpannerEnds` (`meiimporter.cpp:3562`).
    pub(crate) fn add_spanner_ends(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for item in pending {
            let end = item
                .end_id
                .as_deref()
                .and_then(|id| self.id_events.get(id.trim_start_matches('#')))
                .copied()
                .or_else(|| {
                    item.end_tstamp.map(|stamp| {
                        let den = convert::tstamp_denominator(&self.score.measures[item.measure]);
                        (
                            item.measure,
                            item.start.1,
                            self.score.measures[item.measure].start_tick
                                + convert::tstamp_to_fraction(stamp, den),
                        )
                    })
                });
            let (tick, track) = end
                .map(|(_, track, tick)| (tick, track))
                .unwrap_or((item.start.0, item.start.1));
            let mut spanner = Spanner::new(
                item.kind,
                SpannerAnchor::new(item.start.0, item.start.1),
                SpannerAnchor::new(tick, track),
            );
            spanner.number = item.number;
            spanner.notation = item.placement.map(|placement| {
                SpannerNotation::Slur(Slur {
                    placement: Some(placement),
                    visible: true,
                })
            });
            self.score.add_spanner(spanner);
        }
        let ties = std::mem::take(&mut self.pending_ties);
        for (start, end, up) in ties {
            let (Some(start), Some(end)) = (
                self.id_notes.get(&start).copied(),
                self.id_notes.get(&end).copied(),
            ) else {
                continue;
            };
            if start.index() >= self.score.notes.len() || end.index() >= self.score.notes.len() {
                continue;
            }
            self.score.notes[start.index()].tie_forward = Some(end);
            self.score.notes[start.index()].tie_up = up;
            self.score.notes[end.index()].tie_back = Some(start);
        }
    }

    /// `MeiImporter::readVerses` (`meiimporter.cpp:2248`).
    pub(crate) fn read_verses(
        &mut self,
        parent: &Element,
        measure: usize,
        track: Track,
        _ticks: &mut Fraction,
    ) {
        for verse in parent.children_named("verse") {
            let number = verse
                .attribute("n")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1);
            for syl in verse.children_named("syl") {
                let text = crate::lines_of(syl);
                let syllabic = match syl.attribute("con").unwrap_or("d") {
                    "u" => LyricsSyllabic::Begin,
                    "m" => LyricsSyllabic::Middle,
                    "t" => LyricsSyllabic::End,
                    _ => LyricsSyllabic::Single,
                };
                if let Some(cr) = self.last_chord_rest_mut(measure, track) {
                    cr.lyrics
                        .push(Lyrics::new(number.saturating_sub(1), syllabic, text));
                }
            }
        }
    }
}
