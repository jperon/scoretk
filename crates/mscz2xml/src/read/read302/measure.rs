// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/staffrw.cpp` et
// `engraving/rw/read400/measurerw.cpp` de MuseScore Studio, Copyright (C)
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

//! Mesures, voix, segments et éléments verticaux.
//!
//! Portage de `StaffRead::readStaff` (`engraving/rw/read400/staffrw.cpp`) et
//! `MeasureRead::readMeasure` / `readVoice`
//! (`engraving/rw/read400/measurerw.cpp`).

use super::tables;
use super::xml::{parse_fraction, Node};
use super::{annotations, chord, repeats, spanner, Reader, VoiceCtx};
use crate::model::{
    Annotation, BarLine, BarLineType, Breath, BreathType, ChordRestKind, Clef, ClefType, Fraction,
    GraceChord, Key, KeyMode, KeySig, Measure, Segment, SegmentItem, SegmentType, StemDirection,
    TimeSig, Track, VOICES,
};

/// Lit une mesure et renvoie son tick de fin.
pub(crate) fn read_measure(
    reader: &mut Reader,
    index: usize,
    node: &Node,
    staff: usize,
) -> Fraction {
    let mut irregular = false;
    if let Some(len) = node.attr("len").and_then(parse_fraction) {
        if len >= Fraction::new(1, 128) {
            reader.score.measures[index].len = len;
            irregular = true;
        }
        // TODO(parité): une longueur invalide est une erreur MSCX dans le C++ ;
        // on garde ici la longueur héritée.
    }

    let mut start_repeat = false;
    let mut end_repeat = false;
    let mut repeat_count = 0u8;
    let mut voice = 0u8;

    for child in node.children() {
        match child.name() {
            "voice" => {
                read_voice(reader, index, child, staff, voice, irregular);
                voice = voice.saturating_add(1);
            }
            "startRepeat" => start_repeat = true,
            "endRepeat" => {
                end_repeat = true;
                // `MeasureRead::readMeasure` lit la valeur de `<endRepeat>`
                // (`readInt` vaut zéro quand la balise est vide).
                repeat_count = child
                    .int_text()
                    .and_then(|value| u8::try_from(value.clamp(0, 255)).ok())
                    .unwrap_or(0);
            }
            "Marker" => {
                let marker = repeats::read_marker(child, staff);
                reader.score.measures[index]
                    .repeat_marks
                    .push(crate::model::RepeatMark::Marker(marker));
            }
            "Jump" => {
                let jump = repeats::read_jump(child, staff);
                reader.score.measures[index]
                    .repeat_marks
                    .push(crate::model::RepeatMark::Jump(jump));
            }
            // `<LayoutBreak><subtype>line|page</subtype>` : la mesure finit un
            // système. Un saut de section n'en est pas un pour l'export (pas de
            // barre de courtoisie).
            "LayoutBreak" => {
                if matches!(
                    child.child("subtype").map(|node| node.trimmed_text()).as_deref(),
                    Some("line" | "page")
                ) {
                    reader.score.measures[index].line_break = true;
                }
            }
            // TODO(parité): `<stretch>` (ignoré avant 4.0),
            // `<noOffset>`, `<irregular>`, `<measureRepeatCount>`, `<vspacer*>`,
            // `<visible>`, `<SystemDivider>`, `<MeasureNumber>` et
            // `<multiMeasureRest>` ne changent pas le noyau musical lu ici.
            _ => {}
        }
    }

    if start_repeat {
        reader.score.measures[index].start_bar_line = BarLineType::StartRepeat;
    }
    if end_repeat {
        reader.score.measures[index].end_bar_line = BarLineType::EndRepeat;
        reader.score.measures[index].repeat_count = repeat_count;
    }
    reader.score.measures[index].end_tick()
}

fn read_voice(
    reader: &mut Reader,
    index: usize,
    node: &Node,
    staff: usize,
    voice: u8,
    irregular: bool,
) {
    let measure_start = reader.score.measures[index].start_tick;
    // L'indice de voix est positionnel (ordre des blocs `<voice>`). Au-delà de
    // quatre blocs, le C++ déborde sur la portée suivante ; le modèle, lui, n'a
    // que quatre voix par portée : on garde la dernière plutôt que de déborder.
    let track = Track::from_raw((staff * VOICES + usize::from(voice.min(VOICES as u8 - 1))) as u32);
    let mut ctx = VoiceCtx {
        tick: measure_start,
        measure: index,
        track,
        staff,
        tuplets: Vec::new(),
    };
    let mut grace: Vec<GraceChord> = Vec::new();
    // `<Beam><StemDirection>` s'applique au premier accord ou silence qui
    // suit (`MeasureRead::readVoice` : `startingBeam`).
    let mut pending_beam: Option<StemDirection> = None;

    for child in node.children() {
        match child.name() {
            "location" => apply_location(&mut ctx, child),
            "tick" => {
                if let Some(value) = child.int_text() {
                    ctx.tick = reader.file_tick(value);
                }
            }
            "Chord" | "Rest" => {
                let mut cr = chord::read_chord_rest(reader, child, &ctx, grace.len());
                let grace_type = cr.chord_ref().and_then(|value| value.grace);
                if let Some(grace_type) = grace_type {
                    // Un ornement ne crée pas de segment et n'avance pas le
                    // tick : il est rattaché à l'accord normal suivant.
                    let mut grace_chord = GraceChord::new(grace_type, cr.duration);
                    if let ChordRestKind::Chord(chord) = cr.kind {
                        grace_chord.chord = chord;
                    }
                    grace.push(grace_chord);
                    continue;
                }
                if let ChordRestKind::Chord(chord) = &mut cr.kind {
                    chord.grace_chords = std::mem::take(&mut grace);
                }
                cr.tuplet = ctx.tuplets.last().copied();
                cr.beam_direction = pending_beam.take();
                let measure_len = reader.score.measures[index].len;
                let advance = chord::advance(&cr, measure_len, &ctx.tuplets, &reader.score);
                segment_mut(
                    &mut reader.score.measures[index],
                    SegmentType::ChordRest,
                    ctx.tick,
                )
                .set_element(ctx.track, SegmentItem::ChordRest(cr));
                ctx.tick += advance;
            }
            "Beam" => {
                pending_beam = child
                    .child("StemDirection")
                    .map(|direction| tables::stem_direction(&direction.trimmed_text()));
            }
            "Tuplet" => {
                let parent = ctx.tuplets.last().copied();
                let tuplet = chord::read_tuplet(reader, child, ctx.tick, parent);
                ctx.tuplets.push(tuplet);
            }
            "endTuplet" => {
                // TODO(parité): le C++ supprime un n-olet vide et signale un
                // n-olet non fermé ; on garde ce que le fichier décrit.
                ctx.tuplets.pop();
            }
            "Clef" => read_clef(reader, &ctx, child),
            "KeySig" => read_key_sig(reader, &ctx, child),
            "TimeSig" => read_time_sig(reader, &ctx, child, irregular),
            "BarLine" => read_bar_line(reader, &ctx, child),
            "Breath" => read_breath(reader, &ctx, child),
            "Spanner" => spanner::read_spanner(reader, child, &ctx),
            "MeasureRepeat" | "RepeatMeasure" => {
                // `TRead::read(MeasureRepeat*)` : `<subtype>` porte le nombre
                // de mesures répétées (absent en 3.x).
                let count = child
                    .child("subtype")
                    .and_then(Node::int_text)
                    .and_then(|value| u16::try_from(value.clamp(1, u16::MAX as i32)).ok())
                    .unwrap_or(1);
                reader.measure_repeats.push(
                    super::super::measure_repeat::MeasureRepeat {
                        measure: index,
                        staff,
                        count,
                    },
                );
                ctx.tick += reader.score.measures[index].len;
            }
            "Dynamic" | "Expression" | "Tempo" | "StaffText" | "SystemText" | "Text"
            | "RehearsalMark" | "Harmony" | "Fermata" | "InstrumentChange" | "FiguredBass"
            | "TremoloBar" => {
                if let Some(annotation) = annotations::read(reader, child) {
                    // `Segment::add` enregistre l'instrument d'un changement au
                    // tick du segment (`part->setInstrument`).
                    if let Annotation::InstrumentChange(change) = &annotation {
                        if let Some(instrument) = &change.instrument {
                            let part_id = reader.score.staves[staff].part;
                            reader
                                .score
                                .part_mut(part_id)
                                .set_instrument_at(ctx.tick, instrument.clone());
                        }
                    }
                    segment_mut(
                        &mut reader.score.measures[index],
                        SegmentType::ChordRest,
                        ctx.tick,
                    )
                    .add_annotation(ctx.track, annotation);
                }
            }
            // TODO(parité): `<Symbol>`, `<FretDiagram>`, `<Arpeggio>`,
            // `<StaffState>`, `<Sticking>`, `<PlayTechAnnotation>`, `<Image>`,
            // `<Ambitus>`, `<Beam>`, `<Segment>`, `<LayoutBreak>` : sans effet
            // sur le noyau musical lu ici (le tick n'avance pas).
            _ => {}
        }
    }
}

/// Segment d'une mesure, créé s'il n'existe pas encore.
///
/// Équivalent de `Measure::getSegment(type, tick)` : deux éléments de même type
/// au même tick partagent le segment.
pub(crate) fn segment_mut(
    measure: &mut Measure,
    kind: SegmentType,
    tick: Fraction,
) -> &mut Segment {
    if let Some(position) = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
    {
        return &mut measure.segments[position];
    }
    measure.add_segment(Segment::new(kind, tick, 1));
    let position = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
        .expect("le segment vient d'être inséré");
    &mut measure.segments[position]
}

/// Applique un `<location>` relatif à la position courante.
fn apply_location(ctx: &mut VoiceCtx, node: &Node) {
    let mut staves = 0i32;
    let mut voices = 0i32;
    for child in node.children() {
        match child.name() {
            "fractions" => {
                if let Some(value) = child.fraction() {
                    ctx.tick += value;
                }
            }
            "staves" => staves = child.int_text().unwrap_or(0),
            "voices" => voices = child.int_text().unwrap_or(0),
            // TODO(parité): `<measures>` est asserté sur la mesure courante
            // par `ReadContext::setLocation` ; il ne peut pas déplacer la voix.
            _ => {}
        }
    }
    let raw = i64::from(ctx.track.raw()) + i64::from(voices) + i64::from(staves) * VOICES as i64;
    ctx.track = Track::from_raw(raw.max(0) as u32);
}

fn read_clef(reader: &mut Reader, ctx: &VoiceCtx, node: &Node) {
    let mut clef = Clef::default();
    for child in node.children() {
        match child.name() {
            "concertClefType" => {
                clef.concert =
                    tables::clef_type(&child.trimmed_text()).unwrap_or(ClefType::Invalid);
            }
            "transposingClefType" => {
                clef.transposed =
                    tables::clef_type(&child.trimmed_text()).unwrap_or(ClefType::Invalid);
            }
            _ => {}
        }
    }
    if clef.concert == ClefType::Invalid {
        clef = Clef::new(ClefType::G);
    } else if clef.transposed == ClefType::Invalid {
        clef.transposed = clef.concert;
    }

    // Avant 4.1, l'en-tête de mesure est deviné (`readVoice`) : au début de la
    // mesure, la première clef d'une voix est une clef d'en-tête même si une
    // autre portée a déjà posé l'armure ou la métrique ; ensuite, seule une
    // clef qui ne suit aucune armure ni métrique en est une.
    let measure = &reader.score.measures[ctx.measure];
    let voice_has_segment = measure
        .segments
        .iter()
        .any(|segment| segment.tick == measure.start_tick && segment.element(ctx.track).is_some());
    let has_key_or_time = measure.segments.iter().any(|segment| {
        segment.tick == measure.start_tick
            && (segment.kind.is_key_sig_type() || segment.kind.is_time_sig_type())
    });
    let header = ctx.tick == measure.start_tick && (!voice_has_segment || !has_key_or_time);
    let kind = if header {
        SegmentType::HeaderClef
    } else {
        SegmentType::Clef
    };
    segment_mut(&mut reader.score.measures[ctx.measure], kind, ctx.tick)
        .set_element(ctx.track, SegmentItem::Clef(clef));
}

fn read_key_sig(reader: &mut Reader, ctx: &VoiceCtx, node: &Node) {
    let key_visible = node
        .child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    let mut written = Key::C;
    let mut mode = KeyMode::Unknown;
    let mut valid = false;
    let mut courtesy = false;

    for child in node.children() {
        match child.name() {
            "accidental" => {
                let fifths = child.int_text().unwrap_or(0).clamp(-128, 127) as i8;
                written = Key::new(fifths);
                valid = true;
            }
            "mode" => mode = tables::key_mode(&child.trimmed_text()),
            "showCourtesy" => courtesy = child.int_text().unwrap_or(0) != 0,
            // TODO(parité): `<subtype>` (ancien encodage, `initFromSubtype`),
            // `<custom>` et `<CustDef>` (armures personnalisées) ne sont pas lus.
            _ => {}
        }
    }
    if !valid {
        return;
    }

    let transpose = super::instrument_transpose(&reader.score, ctx.staff);
    let concert = if transpose.chromatic == 0 {
        written
    } else {
        Key::new(tables::transpose_key_fifths(
            written.fifths(),
            transpose.chromatic,
        ))
    };
    let measure = &reader.score.measures[ctx.measure];
    let announce = courtesy || ctx.tick == measure.end_tick();
    let kind = if announce {
        SegmentType::KeySigAnnounce
    } else {
        SegmentType::KeySig
    };
    let key_sig = KeySig {
        concert_key: concert,
        transposed_key: written,
        mode,
        courtesy: announce,
        visible: key_visible,
    };
    segment_mut(&mut reader.score.measures[ctx.measure], kind, ctx.tick)
        .set_element(ctx.track, SegmentItem::KeySig(key_sig));
}

fn read_time_sig(reader: &mut Reader, ctx: &VoiceCtx, node: &Node, irregular: bool) {
    let mut time_sig = TimeSig::new(4, 4);
    let mut subtype = None;
    let mut stretch_numerator = None;
    let mut stretch_denominator = None;

    for child in node.children() {
        match child.name() {
            "sigN" => time_sig.numerator = child.int_text().unwrap_or(4),
            "sigD" => time_sig.denominator = child.int_text().unwrap_or(4),
            "subtype" => subtype = child.int_text(),
            "stretchN" => stretch_numerator = child.int_text(),
            "stretchD" => stretch_denominator = child.int_text(),
            "visible" => time_sig.visible = child.int_text().unwrap_or(1) != 0,
            "isCourtesy" => time_sig.courtesy = child.int_text().unwrap_or(0) != 0,
            // TODO(parité): `<textN>`/`<textD>` et `<Groups>` (affichage) ne
            // sont pas modélisés.
            _ => {}
        }
    }
    time_sig.kind = tables::time_sig_type(subtype);
    if let (Some(numerator), Some(denominator)) = (stretch_numerator, stretch_denominator) {
        if denominator != 0 {
            time_sig.stretch = Fraction::new(i64::from(numerator), i64::from(denominator));
        }
    }

    // Une métrique qui ne commence pas la mesure annonce la suivante : elle ne
    // change ni la durée nominale ni la durée réelle.
    let measure = &reader.score.measures[ctx.measure];
    let announce = time_sig.courtesy || ctx.tick > measure.start_tick;
    if !announce {
        let nominal = time_sig.measure_len();
        reader.score.measures[ctx.measure].nominal_len = nominal;
        if !irregular {
            reader.score.measures[ctx.measure].len = nominal;
        }
    }
    let kind = if announce {
        SegmentType::TimeSigAnnounce
    } else {
        SegmentType::TimeSig
    };
    segment_mut(&mut reader.score.measures[ctx.measure], kind, ctx.tick)
        .set_element(ctx.track, SegmentItem::TimeSig(time_sig));
    // TODO(parité): `<stretchN>`/`<stretchD>` est conservé dans le modèle mais
    // l'étirement local n'est pas appliqué aux durées (`Staff::timeStretch`).
}

fn read_bar_line(reader: &mut Reader, ctx: &VoiceCtx, node: &Node) {
    let mut bar_line = BarLine::default();
    for child in node.children() {
        match child.name() {
            "subtype" => {
                bar_line.kind =
                    tables::bar_line_type(&child.trimmed_text()).unwrap_or(BarLineType::Normal);
            }
            "span" => bar_line.span_staff = child.int_text().unwrap_or(0) != 0,
            "spanFromOffset" => bar_line.span_from = child.int_text().unwrap_or(0) as i8,
            "spanToOffset" => bar_line.span_to = child.int_text().unwrap_or(0) as i8,
            _ => {}
        }
    }

    let measure = &reader.score.measures[ctx.measure];
    let rtick = ctx.tick - measure.start_tick;
    let measure_len = measure.len;
    let has_segment = measure
        .segments
        .iter()
        .any(|segment| segment.tick == measure.start_tick);

    let kind = if !rtick.is_zero() && rtick != measure_len {
        SegmentType::BarLine
    } else if bar_line.kind.is_repeat_start() && rtick.is_zero() {
        SegmentType::StartRepeatBarLine
    } else if bar_line.kind.is_repeat_start() && rtick == measure_len {
        // Ancienne version : le C++ ignore cette barre.
        return;
    } else if rtick.is_zero() && !has_segment {
        SegmentType::BeginBarLine
    } else {
        SegmentType::EndBarLine
    };

    match kind {
        SegmentType::BeginBarLine => {
            reader.score.measures[ctx.measure].start_bar_line = bar_line.kind;
        }
        SegmentType::EndBarLine => {
            reader.score.measures[ctx.measure].end_bar_line = bar_line.kind;
        }
        _ => {}
    }
    segment_mut(&mut reader.score.measures[ctx.measure], kind, ctx.tick)
        .set_element(ctx.track, SegmentItem::BarLine(bar_line));
}

fn read_breath(reader: &mut Reader, ctx: &VoiceCtx, node: &Node) {
    let mut breath = Breath::default();
    for child in node.children() {
        match child.name() {
            "subtype" => {
                breath.kind = match child.int_text() {
                    Some(2) | Some(3) => BreathType::Caesura,
                    _ => BreathType::Comma,
                };
            }
            "symbol" => {
                breath.kind = match child.trimmed_text().as_str() {
                    "caesura" => BreathType::Caesura,
                    "caesuraCurved" => BreathType::CaesuraCurved,
                    "caesuraShort" => BreathType::CaesuraShort,
                    "caesuraThick" => BreathType::CaesuraThick,
                    "caesuraSingleStroke" | "chantCaesura" => BreathType::CaesuraSingle,
                    "breathMarkTick" => BreathType::Tick,
                    "stringsUpBow" => BreathType::Upbow,
                    _ => BreathType::Comma,
                };
            }
            "placement" => {
                breath.placement = tables::placement(&child.trimmed_text());
            }
            _ => {}
        }
    }
    // `MeasureRead::readVoice` : la voix 0 place la respiration au-dessus,
    // la voix 1 au-dessous.
    if ctx.track.voice() % 2 == 1 {
        breath.placement = crate::model::Placement::Below;
    }
    segment_mut(
        &mut reader.score.measures[ctx.measure],
        SegmentType::Breath,
        ctx.tick,
    )
    .set_element(ctx.track, SegmentItem::Breath(breath));
}
