// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`,
// `importmusicxmlnotepitch.cpp`, `musicxmltupletstate.cpp` et du passage
// `part`/`measure`/`note` de `importmusicxmlpass1.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Deuxième passe : construction du modèle.
//!
//! Portage de `MusicXmlParserPass2` (`importmusicxmlpass2.cpp`) : parcourt de
//! nouveau les mesures de chaque partie, place les accords, silences, clefs,
//! armures et indications de mesure dans les segments, puis complète les
//! silences manquants de la première voix (`fillGapsInFirstVoices`) et les
//! liaisons de prolongation.
//!
//! Les notations (`<notations>`) ne sont lues que pour ce que le modèle sait
//! représenter : liaisons de prolongation et n-olets. Les articulations, les
//! ornements, les spanners, les directions et les harmonies restent hors de
//! cette première version (voir `docs/agents/musicxml-import.md`).

use std::collections::BTreeMap;

use scoretk_core::model::{
    Accidental, AccidentalType, Chord, ChordRest, ChordRestKind, Clef, ClefType, Duration,
    DurationType, Fraction, GraceChord, GraceType, Instrument, Key, KeyMode, KeySig, Lyrics,
    LyricsSyllabic, Measure, MeasureRepeatMark, Note, NoteHeadGroup, NoteHeadType, NoteId, Part,
    Placement, Score, Segment, SegmentItem, SegmentType, StaffId, StemDirection, TimeSig, Track,
    Tuplet, TupletId, TupletNumberType, VOICES,
};

use crate::direction;
use crate::error::Result;
use crate::harmony;
use crate::notations;
use crate::noteduration::{self, duration_type};
use crate::pass1::{self, Analysis, InstrumentData, TimeSigData};
use crate::xml::Element;

/// Description d'un `<tuplet>` dans les notations d'une note.
#[derive(Clone, Debug, Default)]
struct TupletNotation {
    start: bool,
    stop: bool,
    number: usize,
    bracket: bool,
    bracket_explicit: bool,
    placement: Option<Placement>,
    number_type: TupletNumberType,
    actual: Option<i64>,
    normal: Option<i64>,
    base_len: Option<Fraction>,
}

/// N-olet ouvert dans une voix.
struct OpenTuplet {
    id: TupletId,
    number: usize,
    ratio: Fraction,
    /// Nombre de notes normales (affiché, non réduit).
    normal_notes: i64,
    /// Vrai quand le `<tuplet-type>` du fichier fixe déjà la durée de base.
    explicit_base: bool,
    /// Somme des durées notées des enfants, pour `determineTupletBaseLen`.
    notated: Fraction,
    start: Fraction,
    /// Fin réelle du dernier enfant (n-olets imbriqués compris).
    max_end: Fraction,
}

/// Accord d'agrément en attente de rattachement.
struct PendingGrace {
    grace: GraceType,
    duration: Duration,
    slash: bool,
    notes: Vec<NoteId>,
    staff_move: i8,
}

/// Construit le modèle à partir du document et de l'analyse de première passe.
pub fn build(root: &Element, analysis: &Analysis) -> Result<Score> {
    let mut score = Score::with_division(analysis.divisions.max(1));
    score.metadata = read_metadata(root);
    score.credits = crate::credits::read_credits(root);
    // Le nombre de pages conditionne la répétition du copyright à l'export
    // (`credits`) : il se déduit des crédits, seule information lisible.
    score.style.page_count = root
        .children_named("credit")
        .filter_map(|credit| credit.attribute("page"))
        .filter_map(|page| page.parse::<u32>().ok())
        .max()
        .unwrap_or(1)
        .max(1);

    // Parties et portées.
    for part_data in &analysis.parts {
        let mut part = Part::new(part_data.name.clone(), part_data.abbreviation.clone());
        part.instrument = instrument(&part_data.instruments[0]);
        part.instrument.transpose = part_data.transpose;
        part.long_name = part_data.name.clone();
        part.short_name = part_data.abbreviation.clone();
        let id = score.add_part(part);
        for _ in 0..part_data.staves {
            score.add_staff(id);
        }
        let first = score.parts[id.index()].staves[0];
        let staff = score.staff_mut(first);
        staff.brackets = part_data
            .brackets
            .iter()
            .map(|(kind, span)| scoretk_core::model::StaffBracket {
                kind: *kind,
                span: *span,
                visible: true,
            })
            .collect();
        staff.bar_line_span = part_data.bar_line_span;
    }

    // Mesures.
    let mut start = Fraction::ZERO;
    for (index, data) in analysis.measures.iter().enumerate() {
        let mut measure = Measure::new(index, start, data.len);
        measure.nominal_len = data.time_sig_in_effect.measure_len();
        measure.len = data.len;
        measure.section_break_before = data.section_break;
        score.add_measure(measure);
        start += data.len;
    }

    let elements: BTreeMap<&str, &Element> = root
        .children_named("part")
        .filter_map(|part| part.attribute("id").map(|id| (id, part)))
        .collect();

    let mut ties = TieMap::default();
    let mut direction_state = direction::DirectionState::default();
    for (part_index, part_data) in analysis.parts.iter().enumerate() {
        let Some(part_element) = elements.get(part_data.id.as_str()).copied() else {
            continue;
        };
        direction_state.start_part();
        read_part(
            &mut score,
            analysis,
            part_index,
            part_element,
            &mut ties,
            &mut direction_state,
        );
    }

    Ok(score)
}

/// Instrument du modèle à partir de `part-list`.
fn instrument(data: &InstrumentData) -> Instrument {
    Instrument {
        id: data.sound.clone(),
        long_name: data.name.clone(),
        short_name: String::new(),
        track_name: data.name.clone(),
        transpose: None,
        midi_program: data.program,
        midi_channel: data.channel,
        channel_count: 1,
        channels: data.channel.map_or_else(Vec::new, |channel| {
            vec![scoretk_core::model::InstrumentChannel {
                program: data.program,
                midi_port: data.port,
                midi_channel: Some(channel),
                ..scoretk_core::model::InstrumentChannel::default()
            }]
        }),
        midi_port: data.port,
        volume: data.volume,
        pan: data.pan,
        midi_articulations: Vec::new(),
    }
}

/// Métadonnées d'œuvre (`<work>`, `<movement-*>`, `<identification>`).
fn read_metadata(root: &Element) -> scoretk_core::model::ScoreMetadata {
    let mut metadata = scoretk_core::model::ScoreMetadata::default();
    if let Some(work) = root.child("work") {
        metadata.work_title = work.child_text("work-title").unwrap_or_default().to_owned();
        metadata.work_number = work
            .child_text("work-number")
            .unwrap_or_default()
            .to_owned();
    }
    metadata.movement_title = root
        .child_text("movement-title")
        .unwrap_or_default()
        .to_owned();
    metadata.movement_number = root
        .child_text("movement-number")
        .unwrap_or_default()
        .to_owned();
    // Le modèle suppose `Linux` par défaut ; un fichier sans
    // `<miscellaneous-field name="platform">` n'en écrit pas.
    metadata.platform = String::new();
    if let Some(identification) = root.child("identification") {
        for creator in identification.children_named("creator") {
            let value = creator.text_trimmed().to_owned();
            match creator.attribute("type") {
                Some("composer") => metadata.composer = value,
                // `MusicXmlParserPass1::identification` appelle
                // `setMetaTag(type, texte)` pour chaque créateur
                // (`importmusicxmlpass1.cpp:1527-1528`) : les trois types
                // restent distincts dans le modèle.
                Some("lyricist") => metadata.lyricist = value,
                Some("poet") => metadata.poet = value,
                Some("translator") => metadata.translator = value,
                Some("arranger") => metadata.arranger = value,
                _ => {}
            }
        }
        if let Some(rights) = identification.child_text("rights") {
            metadata.copyright = rights.to_owned();
        }
        if let Some(source) = identification.child_text("source") {
            metadata.source = source.to_owned();
        }
        if let Some(miscellaneous) = identification.child("miscellaneous") {
            for field in miscellaneous.children_named("miscellaneous-field") {
                let value = field.text_trimmed().to_owned();
                match field.attribute("name") {
                    Some("creationDate") => metadata.creation_date = value,
                    Some("platform") => metadata.platform = value,
                    Some("subtitle") => metadata.subtitle = value,
                    Some("mscVersion") => metadata.msc_version = value,
                    _ => {}
                }
            }
        }
    }
    if metadata.copyright.is_empty() {
        for credit in root.children_named("credit") {
            if credit.child_text("credit-type") != Some("rights") {
                continue;
            }
            if let Some(words) = credit.child_text("credit-words") {
                metadata.copyright = words.to_owned();
            }
        }
    }
    metadata
}

// ----------------------------------------------------------------------
// Parcours d'une partie
// ----------------------------------------------------------------------

/// Parcourt les mesures d'une partie (`MusicXmlParserPass2::part`).
fn read_part(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    element: &Element,
    ties: &mut TieMap,
    directions: &mut direction::DirectionState,
) {
    for (measure_index, measure) in element.children_named("measure").enumerate() {
        if measure_index >= score.measures.len() {
            break;
        }
        read_measure(
            score,
            analysis,
            part_index,
            measure_index,
            measure,
            ties,
            directions,
        );
    }
}

/// État d'une voix pendant la lecture d'une mesure.
#[derive(Default)]
struct VoiceState {
    tuplets: Vec<OpenTuplet>,
}

fn read_measure(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    ties: &mut TieMap,
    directions: &mut direction::DirectionState,
) {
    let measure_start = score.measures[measure_index].start_tick;
    let measure_len = score.measures[measure_index].len;
    let divisions = analysis.divisions.max(1);
    let staves = analysis.parts[part_index].staves;

    let mut m_time = Fraction::ZERO;
    let mut prev_time = Fraction::ZERO;
    let mut voice_states: BTreeMap<i32, VoiceState> = BTreeMap::new();
    let mut grace_chords: Vec<PendingGrace> = Vec::new();
    let mut grace_after: usize = 0;
    let mut prev_chord: Option<(Fraction, Track)> = None;
    let mut multi_rest: Option<u16> = None;
    let mut measure_style_repeat: Option<u16> = None;
    let mut figured_bass: Vec<(Fraction, Track, scoretk_core::model::FiguredBass)> = Vec::new();
    // Décisions de visibilité différées : `<attributes>` précède les notes,
    // donc la mesure n'est complète qu'en fin de passe (voir C++
    // `MusicXmlParserPass2::staffDetails`, importmusicxmlpass2.cpp:3262-3297).
    let mut pending_staff_details: Vec<(StaffId, bool, bool)> = Vec::new();

    for child in &element.children {
        match child.name.as_str() {
            "attributes" => {
                for attribute in &child.children {
                    match attribute.name.as_str() {
                        "key" => read_key(
                            score,
                            analysis,
                            part_index,
                            measure_index,
                            attribute,
                            measure_start + m_time,
                        ),
                        "time" => read_time(
                            score,
                            analysis,
                            part_index,
                            measure_index,
                            attribute,
                            measure_start + m_time,
                        ),
                        "clef" => read_clef(
                            score,
                            analysis,
                            part_index,
                            measure_index,
                            attribute,
                            measure_start + m_time,
                        ),
                        "transpose" => {
                            if let Some(transpose) = pass1::read_transpose(attribute) {
                                // Une transposition qui suit un changement
                                // d'instrument appartient à ce changement : le
                                // C++ la porte dans `<transpose>` de son
                                // `<attributes>`, pas dans l'instrument
                                // principal.
                                if measure_index == 0 && directions.last_transpose.is_none() {
                                    score.parts[part_index].instrument.transpose = Some(transpose);
                                } else {
                                    directions.last_transpose = Some(transpose);
                                }
                            }
                        }
                        "staff-details" => read_staff_details(
                            score,
                            analysis,
                            part_index,
                            attribute,
                            &mut pending_staff_details,
                        ),
                        "measure-style" => {
                            for style in &attribute.children {
                                match style.name.as_str() {
                                    "multiple-rest" => {
                                        if let Ok(count) = style.text_trimmed().parse::<u16>() {
                                            if count >= 1 {
                                                multi_rest = Some(count);
                                            }
                                        }
                                    }
                                    "measure-repeat"
                                        if style.attribute("type") == Some("start") =>
                                    {
                                        if let Ok(count) = style.text_trimmed().parse::<u16>() {
                                            measure_style_repeat = Some(count);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "note" => {
                read_note(
                    score,
                    analysis,
                    part_index,
                    measure_index,
                    child,
                    divisions,
                    measure_start,
                    measure_len,
                    staves,
                    &mut m_time,
                    &mut prev_time,
                    &mut voice_states,
                    &mut grace_chords,
                    &mut grace_after,
                    &mut prev_chord,
                    ties,
                    directions,
                );
            }
            "forward" => {
                if divisions > 0 {
                    if let Some(ticks) = child
                        .child_text("duration")
                        .and_then(|text| text.trim().parse::<i64>().ok())
                    {
                        m_time += Fraction::from_ticks(ticks, divisions * 4);
                    }
                }
            }
            "backup" => {
                if divisions > 0 {
                    if let Some(ticks) = child
                        .child_text("duration")
                        .and_then(|text| text.trim().parse::<i64>().ok())
                    {
                        let delta = Fraction::from_ticks(ticks, divisions * 4);
                        m_time = if delta <= m_time {
                            m_time - delta
                        } else {
                            Fraction::ZERO
                        };
                    }
                }
            }
            "barline" => crate::barline::read_barline(score, part_index, measure_index, child),
            "harmony" => harmony::read_harmony(
                score,
                part_index,
                measure_index,
                child,
                measure_start,
                m_time,
            ),
            "figured-bass" => {
                let bass = harmony::read_figured_bass(child, divisions);
                let Some(staff_id) = score.parts[part_index].staves.first().copied() else {
                    continue;
                };
                figured_bass.push((measure_start + m_time, Track::new(staff_id, 0), bass));
            }
            "direction" => direction::read_direction(
                score,
                analysis,
                part_index,
                measure_index,
                child,
                m_time,
                directions,
            ),
            "sound" => {
                // `<sound tempo>` hors `<direction>` : MuseScore y voit un
                // tempo invisible qui n'exporte que sa valeur sonore.
                if let Some(tempo) = child
                    .attribute("tempo")
                    .and_then(|tempo| tempo.parse::<f64>().ok())
                {
                    let tick = measure_start + m_time;
                    let index = get_segment(score, measure_index, SegmentType::ChordRest, tick);
                    let track = Track::new(score.parts[part_index].staves[0], 0);
                    let mut annotation = scoretk_core::model::Tempo::new("");
                    annotation.beats_per_second = Some(tempo / 60.0);
                    annotation.visible = false;
                    score.measures[measure_index].segments[index]
                        .add_annotation(track, scoretk_core::model::Annotation::Tempo(annotation));
                }
                if let Some(change) = child.child("instrument-change") {
                    direction::read_standalone_instrument_change(
                        score,
                        analysis,
                        part_index,
                        measure_index,
                        change,
                        m_time,
                        directions,
                    );
                }
            }
            _ => {}
        }
    }

    // N-olets restés ouverts faute de `<tuplet type="stop">`.
    for state in voice_states.values_mut() {
        while let Some(open) = state.tuplets.pop() {
            finish_tuplet(score, &open);
            if let Some(parent) = state.tuplets.last_mut() {
                let end = open.max_end;
                if end > parent.max_end {
                    parent.max_end = end;
                }
            }
        }
    }

    // Ornements restants : `gac = gcl.size()` puis `addGraceChordsAfter`,
    // c'est-à-dire rattachés à l'accord précédent comme ornements « après ».
    if !grace_chords.is_empty() {
        if let Some((tick, track)) = prev_chord {
            let count = grace_chords.len();
            attach_grace_after(score, measure_index, tick, track, &mut grace_chords, count);
        }
        if !grace_chords.is_empty() {
            attach_orphan_graces(score, measure_index, &mut grace_chords);
        }
    }

    // Silences de remplissage de la première voix.
    for staff in 0..staves {
        if let Some(staff_id) = score.parts[part_index].staves.get(staff).copied() {
            fill_gaps_in_first_voice(score, measure_index, staff_id);
        }
    }

    if let Some(count) = multi_rest {
        score.measures[measure_index].multi_measure_rest = Some(count);
    }
    if let Some(count) = measure_style_repeat {
        score.measures[measure_index].measure_repeat = Some(MeasureRepeatMark {
            staff: score.parts[part_index].staves[0],
            count,
        });
    }

    // Chiffrage figuré : la durée vaut celle de l'accord qu'il accompagne
    // (`addFiguredBassElements`).
    for (tick, track, mut bass) in figured_bass {
        if !bass.ticks.is_positive() {
            let actual = score.measures[measure_index]
                .segments
                .iter()
                .find(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick)
                .and_then(|segment| segment.chord_rest(track))
                .map_or(Fraction::ZERO, |chord_rest| {
                    actual_ticks(score, chord_rest, measure_index)
                });
            bass.ticks = if actual.is_positive() {
                actual
            } else {
                measure_len
            };
        }
        let index = get_segment(score, measure_index, SegmentType::ChordRest, tick);
        score.measures[measure_index].segments[index]
            .add_annotation(track, scoretk_core::model::Annotation::FiguredBass(bass));
    }

    // La mesure est complète : on peut distinguer une portée entièrement
    // invisible d'une portée vide masquée par le style, ou d'un cutaway.
    for (staff_id, hidden, spacing) in pending_staff_details {
        if hidden {
            if spacing {
                // Visibilité par mesure (cutaway) : non modélisée par le pivot.
            } else if staff_only_rests(score, measure_index, staff_id) {
                score.style.hide_empty_staves = true;
                score.style.dont_hide_staves_in_first_system = false;
            } else {
                score.staff_mut(staff_id).visible = false;
            }
        } else {
            score.staff_mut(staff_id).visible = true;
        }
    }

    direction::flush_pending_instrument(score, part_index, measure_index, directions);
}

/// Lit une note ou un silence (`MusicXmlParserPass2::note`).
#[allow(clippy::too_many_arguments)]
fn read_note(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    divisions: u32,
    measure_start: Fraction,
    measure_len: Fraction,
    staves: usize,
    m_time: &mut Fraction,
    prev_time: &mut Fraction,
    voice_states: &mut BTreeMap<i32, VoiceState>,
    grace_chords: &mut Vec<PendingGrace>,
    grace_after: &mut usize,
    prev_chord: &mut Option<(Fraction, Track)>,
    ties: &mut TieMap,
    directions: &mut direction::DirectionState,
) {
    let timing = noteduration::read(element, divisions);
    let (voice_key, staff_xml, _) = pass1::read_note_identity(element);
    let chord = element.has_child("chord");
    let grace = element.has_child("grace");
    let rest = element.has_child("rest");

    let (staff, voice, staff_move) = analysis.resolve_voice(part_index, voice_key, staff_xml);
    let Some(staff_id) = score.parts[part_index]
        .staves
        .get(staff.min(staves.saturating_sub(1)))
        .copied()
    else {
        return;
    };
    let track = Track::new(staff_id, (voice.min(VOICES - 1)) as u8);
    // Une ottava décale la hauteur écrite : le modèle stocke la hauteur
    // sonore, comme `xmlSetPitch` à l'import.
    let octave_shift = directions.octave_shift(staff);
    let notation_info = notations::parse(element);

    if grace {
        let grace_transpose = score.parts[part_index]
            .instrument_at(measure_start + *m_time)
            .transpose;
        if chord && !grace_chords.is_empty() {
            let note = build_note(element, grace_transpose, octave_shift);
            let id = score.add_note(note);
            if let Some(pending) = grace_chords.last_mut() {
                pending.notes.push(id);
            }
        } else {
            let mut pending = PendingGrace {
                grace: grace_type(element),
                duration: noteduration::figure(element)
                    .unwrap_or(Duration::new(DurationType::Eighth)),
                slash: element
                    .child("grace")
                    .and_then(|grace| grace.attribute("slash"))
                    == Some("yes"),
                notes: Vec::new(),
                staff_move,
            };
            let note = build_note(element, grace_transpose, octave_shift);
            let id = score.add_note(note);
            apply_ties(element, id, track, score, ties);
            pending.notes.push(id);
            grace_chords.push(pending);
            // Un ornement portant un `<slur type="stop">` sans départ marque
            // tous les ornements déjà collectés comme « après l'accord »
            // (`mustStopGraceAfter`, `gac = gcl.size()`).
            if slur_stop_without_start(element) {
                *grace_after = grace_chords.len();
            }
        }
        return;
    }

    let state = voice_states.entry(voice_key).or_default();
    let notations = tuplet_notations(element);

    if chord {
        let tick = measure_start + *prev_time;
        let chord_transpose = score.parts[part_index].instrument_at(tick).transpose;
        let note = build_note(element, chord_transpose, octave_shift);
        let id = score.add_note(note);
        if let Some(chord_rest) = chord_rest_at_mut(score, measure_index, tick, track) {
            if let Some(chord) = chord_rest.chord_mut() {
                chord.notes.push(id);
                chord.staff_move = staff_move;
                apply_chord_attributes(element, chord);
                notations::apply_chord(&notation_info, chord);
            }
        }
        apply_ties(element, id, track, score, ties);
        notations::apply_score(
            score,
            &notation_info,
            Some(id),
            measure_index,
            tick,
            tick,
            track,
            &mut directions.notations,
        );
        return;
    }

    let cr_tick = measure_start + *m_time;
    let duration = chord_rest_duration(element, timing.dots, measure_len, timing.dura);
    let mut chord_rest = make_chord_rest(element, duration, timing.dura, measure_len);
    // Un trémolo à deux notes vaut la moitié de sa figure : la durée spécifiée
    // fait foi (`Chord::tremoloTwoChord`).
    if is_tremolo_two(element) {
        if let Some(spec) = timing.spec.filter(|spec| spec.is_positive()) {
            chord_rest.ticks = Some(spec);
        }
    }
    chord_rest.lyrics = read_lyrics(element);
    if let Some(mode) = read_beam(element) {
        chord_rest.beam_mode = mode;
        if matches!(
            mode,
            scoretk_core::model::BeamMode::Begin
                | scoretk_core::model::BeamMode::Begin16
                | scoretk_core::model::BeamMode::Begin32
        ) {
            chord_rest.beam_direction = read_stem(element);
        }
    }
    if let Some(chord) = chord_rest.chord_mut() {
        chord.staff_move = staff_move;
        if let Some(stem) = read_stem(element) {
            chord.stem = stem;
        }
    }

    let index = get_segment(score, measure_index, SegmentType::ChordRest, cr_tick);
    score.measures[measure_index].segments[index]
        .set_element(track, SegmentItem::ChordRest(chord_rest));

    if rest {
        notations::apply_score(
            score,
            &notation_info,
            None,
            measure_index,
            cr_tick,
            cr_tick + timing.dura,
            track,
            &mut directions.notations,
        );
    }

    if !rest {
        let note_transpose = score.parts[part_index].instrument_at(cr_tick).transpose;
        let note = build_note(element, note_transpose, octave_shift);
        let id = score.add_note(note);
        if let Some(chord) = score.measures[measure_index].segments[index]
            .element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
            .and_then(ChordRest::chord_mut)
        {
            chord.notes.push(id);
            apply_chord_attributes(element, chord);
            notations::apply_chord(&notation_info, chord);
        }
        apply_ties(element, id, track, score, ties);
        notations::apply_score(
            score,
            &notation_info,
            Some(id),
            measure_index,
            cr_tick,
            cr_tick + timing.dura,
            track,
            &mut directions.notations,
        );
    }

    // N-olets : l'accord appartient au n-olet le plus interne.
    let inner = start_tuplets(score, state, &notations, timing.time_mod, cr_tick, duration);
    if let Some(id) = inner.or_else(|| state.tuplets.last().map(|open| open.id)) {
        if let Some(chord_rest) = score.measures[measure_index].segments[index]
            .element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
        {
            chord_rest.tuplet = Some(id);
        }
        if let Some(open) = state.tuplets.last_mut() {
            open.notated += duration.ticks();
            // La durée réelle de la note inclut tous les n-olets parents.
            let end = cr_tick + timing.dura;
            if end > open.max_end {
                open.max_end = end;
            }
        }
    }
    stop_tuplets(score, state, &notations);

    *prev_time = *m_time;
    *m_time += timing.dura;

    // Rattache les ornements : ceux marqués « après » vont à l'accord
    // précédent (`addGraceChordsAfter`), les autres à celui-ci
    // (`addGraceChordsBefore`). Comme le C++, rien n'est rattaché à un
    // silence : les ornements en attente passent à l'accord suivant.
    if !grace_chords.is_empty() && !rest {
        let count = (*grace_after).min(grace_chords.len());
        *grace_after = 0;
        if count > 0 {
            if let Some((previous_tick, previous_track)) = *prev_chord {
                attach_grace_after(
                    score,
                    measure_index,
                    previous_tick,
                    previous_track,
                    grace_chords,
                    count,
                );
            }
        }
        attach_grace_chords(score, measure_index, cr_tick, track, grace_chords, count);
    }
    if !rest {
        *prev_chord = Some((cr_tick, track));
    }
}

/// Écrit un segment à un tick donné, en le créant au besoin.
pub(crate) fn get_segment(
    score: &mut Score,
    measure_index: usize,
    kind: SegmentType,
    tick: Fraction,
) -> usize {
    if let Some(position) = score.measures[measure_index]
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
    {
        return position;
    }
    let segment = Segment::new(kind, tick, score.staves.len().max(1));
    score.measures[measure_index].add_segment(segment);
    score.measures[measure_index]
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
        .expect("segment inséré")
}

/// Accès mutable à l'accord ou silence d'un segment.
fn chord_rest_at_mut(
    score: &mut Score,
    measure_index: usize,
    tick: Fraction,
    track: Track,
) -> Option<&mut ChordRest> {
    score.measures[measure_index]
        .segments
        .iter_mut()
        .find(|segment| segment.tick == tick && segment.kind == SegmentType::ChordRest)
        .and_then(|segment| segment.element_mut(track))
        .and_then(SegmentItem::as_chord_rest_mut)
}

// ----------------------------------------------------------------------
// Attributs de mesure
// ----------------------------------------------------------------------

/// Lignes et visibilité d'une portée (`MusicXmlParserPass2::staffDetails`,
/// `importmusicxmlpass2.cpp:3246-3305`).
///
/// `print-object="no"` ne rend la portée entière invisible (`:3288-3291`) que
/// si le cas n'est ni un cutaway (`print-spacing="yes"`, visibilité par mesure,
/// `:3272-3274`) ni une portée vide masquée par le style `hideEmptyStaves`
/// (`:3275-3286`). L'export de scoretk écrit précisément ce dernier cas pour
/// une portée masquée sur un seul système : la traiter comme invisible pour
/// toute la partition était faux, et un `<staff-details>` postérieur sans
/// l'attribut la rendait de nouveau visible (`:3292-3297`).
fn read_staff_details(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    element: &Element,
    pending: &mut Vec<(StaffId, bool, bool)>,
) {
    let position = element
        .attribute("number")
        .and_then(|number| number.parse::<usize>().ok())
        .map_or(0, |number| number.saturating_sub(1));
    let _ = analysis;
    let Some(staff_id) = score.parts[part_index].staves.get(position).copied() else {
        return;
    };
    pending.push((
        staff_id,
        element.attribute("print-object") == Some("no"),
        element.attribute("print-spacing") == Some("yes"),
    ));
    let Some(lines) = element.child_int("staff-lines") else {
        return;
    };
    if let Ok(lines) = u8::try_from(lines) {
        score.staff_mut(staff_id).lines = lines;
    }
}

/// Vrai si la mesure ne porte, sur cette portée, que des silences en voix 1
/// (`Measure::hasVoices`/`isOnlyRests`, `importmusicxmlpass2.cpp:3275-3286`).
fn staff_only_rests(score: &Score, measure_index: usize, staff: StaffId) -> bool {
    let Some(measure) = score.measures.get(measure_index) else {
        return false;
    };
    for segment in &measure.segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        for (track, item) in segment.iter_elements() {
            if i64::from(track.raw()) / VOICES as i64 != i64::from(staff.raw()) {
                continue;
            }
            if track.raw() % VOICES as u32 != 0 {
                return false;
            }
            if let SegmentItem::ChordRest(chord_rest) = item {
                if matches!(chord_rest.kind, ChordRestKind::Chord(_)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Armure (`MusicXmlParserPass2::key`).
fn read_key(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    tick: Fraction,
) {
    let fifths = element.child_int("fifths").unwrap_or(0) as i8;
    let mode = match element.child_text("mode") {
        Some("major") => KeyMode::Major,
        Some("minor") => KeyMode::Minor,
        Some("none") | Some("atonality") => KeyMode::None,
        _ => KeyMode::Unknown,
    };
    let mut signature = KeySig::new(Key::new(fifths));
    signature.mode = mode;
    signature.visible = element.attribute("print-object") != Some("no");

    for staff_id in staff_ids(score, analysis, part_index, element) {
        let track = Track::new(staff_id, 0);
        let index = get_segment(score, measure_index, SegmentType::KeySig, tick);
        score.measures[measure_index].segments[index]
            .set_element(track, SegmentItem::KeySig(signature.clone()));
    }
}

/// Indication de mesure (`MusicXmlParserPass2::time`).
fn read_time(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    tick: Fraction,
) {
    let Some(signature) = pass1::read_time(element) else {
        return;
    };
    let mut time = time_sig(signature, element);
    // Indication de mesure locale : sa longueur diffère de la longueur
    // globale de la mesure, et l'étirement de portée vaut leur rapport
    // (`TimeSig::stretch`, `Staff::timeStretch`).
    let local_len = analysis
        .parts
        .get(part_index)
        .and_then(|part| part.measures.get(measure_index))
        .copied()
        .unwrap_or(Fraction::ZERO);
    let global_len = analysis
        .measures
        .get(measure_index)
        .map_or(Fraction::ZERO, |measure| measure.len);
    let stretch = if local_len.is_positive() && global_len.is_positive() && local_len != global_len
    {
        local_len / global_len
    } else {
        Fraction::ONE
    };
    time.stretch = stretch;

    for staff_id in staff_ids(score, analysis, part_index, element) {
        let track = Track::new(staff_id, 0);
        let index = get_segment(score, measure_index, SegmentType::TimeSig, tick);
        score.measures[measure_index].segments[index]
            .set_element(track, SegmentItem::TimeSig(time.clone()));
        if stretch != Fraction::ONE {
            score.staff_mut(staff_id).time_stretch = stretch;
        }
    }
}

/// Indication de mesure du modèle.
fn time_sig(signature: TimeSigData, element: &Element) -> TimeSig {
    let mut time = TimeSig::new(signature.numerator, signature.denominator);
    time.kind = signature.kind;
    time.visible = element.attribute("print-object") != Some("no");
    time
}

/// Portées visées par un attribut numéroté (toutes si `number` est absent).
fn staff_ids(
    score: &Score,
    analysis: &Analysis,
    part_index: usize,
    element: &Element,
) -> Vec<StaffId> {
    let all = staff_ids_all(score, analysis, part_index);
    match element.attribute("number") {
        Some(number) => number
            .parse::<usize>()
            .ok()
            .map(|number| number.saturating_sub(1))
            .and_then(|position| all.get(position).copied())
            .into_iter()
            .collect(),
        None => all,
    }
}

fn staff_ids_all(score: &Score, analysis: &Analysis, part_index: usize) -> Vec<StaffId> {
    score.parts[part_index]
        .staves
        .iter()
        .copied()
        .take(analysis.parts[part_index].staves)
        .collect()
}

/// Clef (`MusicXmlParserPass2::clef`).
fn read_clef(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    tick: Fraction,
) {
    let sign = element.child_text("sign").unwrap_or("G").to_owned();
    let octave = element.child_int("clef-octave-change").unwrap_or(0) as i8;
    let line = element
        .child_int("line")
        .map(|line| line as i8)
        .unwrap_or(match sign.as_str() {
            "G" => 2,
            "F" => 4,
            "C" => 3,
            _ => 0,
        });
    let clef = Clef::new(clef_type(&sign, octave, line));
    let after_barline = element.attribute("after-barline") == Some("yes");
    let measure_start = score.measures[measure_index].start_tick;

    // Un changement de clef au début d'une mesure est placé à la fin de la
    // précédente (`ClefToBarlinePosition::AUTO`).
    let (target_measure, kind) = if !after_barline && tick == measure_start && measure_index > 0 {
        (measure_index - 1, SegmentType::Clef)
    } else if tick.is_zero() {
        (measure_index, SegmentType::HeaderClef)
    } else {
        (measure_index, SegmentType::Clef)
    };

    for staff_id in staff_ids(score, analysis, part_index, element) {
        let track = Track::new(staff_id, 0);
        let mut index = get_segment(score, target_measure, kind, tick);
        // Une clef d'en-tête déjà posée au même tick n'est pas écrasée : la
        // seconde devient un changement après la barre, que l'export réécrit
        // séparément (`midMeasureClef`).
        if kind == SegmentType::HeaderClef
            && score.measures[target_measure].segments[index]
                .element(track)
                .is_some()
        {
            index = get_segment(score, target_measure, SegmentType::Clef, tick);
        }
        score.measures[target_measure].segments[index].set_element(track, SegmentItem::Clef(clef));
    }
}

/// Type de clef à partir de signe, octave et ligne.
fn clef_type(sign: &str, octave: i8, line: i8) -> ClefType {
    match (sign, octave, line) {
        ("G", 0, 2) => ClefType::G,
        ("G", 1, 2) => ClefType::G8Va,
        ("G", 2, 2) => ClefType::G15Ma,
        ("G", -1, 2) => ClefType::G8Vb,
        ("G", -2, 2) => ClefType::G15Mb,
        ("G", 0, 1) => ClefType::G1,
        ("F", 0, 3) => ClefType::FBaritone,
        ("F", 0, 4) => ClefType::F,
        ("F", 1, 4) => ClefType::F8Va,
        ("F", 2, 4) => ClefType::F15Ma,
        ("F", -1, 4) => ClefType::F8Vb,
        ("F", -2, 4) => ClefType::F15Mb,
        ("F", 0, 5) => ClefType::FSubbass,
        ("C", _, 5) => ClefType::C5,
        ("C", _, 4) => ClefType::C4,
        ("C", _, 3) => ClefType::C3,
        ("C", _, 2) => ClefType::C2,
        ("C", _, 1) => ClefType::C1,
        ("percussion", _, _) => ClefType::Perc,
        ("TAB", _, _) => ClefType::Tab,
        _ => ClefType::G,
    }
}

// ----------------------------------------------------------------------
// Accords, silences et notes
// ----------------------------------------------------------------------

/// Durée d'un accord ou silence (`determineDuration`).
fn chord_rest_duration(
    note: &Element,
    dots: u8,
    measure_len: Fraction,
    dura: Fraction,
) -> Duration {
    let rest = note.has_child("rest");
    let measure_rest = note
        .child("rest")
        .and_then(|rest| rest.attribute("measure"))
        == Some("yes");
    let kind = note.child_text("type");
    let whole_rest = rest
        && ((kind.is_none() && dura == measure_len)
            || (kind == Some("whole") && dura == measure_len));

    if rest && (measure_rest || whole_rest) {
        return Duration::measure();
    }
    if let Some(name) = kind {
        let kind = duration_type(name);
        if kind != DurationType::Invalid {
            return Duration::with_dots(kind, dots);
        }
    }
    duration_from_ticks(dura)
}

/// Plus grande figure (avec points) dont la durée ne dépasse pas `ticks`.
fn duration_from_ticks(ticks: Fraction) -> Duration {
    if ticks.is_zero() {
        return Duration::measure();
    }
    const TYPES: [DurationType; 13] = [
        DurationType::Long,
        DurationType::Breve,
        DurationType::Whole,
        DurationType::Half,
        DurationType::Quarter,
        DurationType::Eighth,
        DurationType::Sixteenth,
        DurationType::ThirtySecond,
        DurationType::SixtyFourth,
        DurationType::OneHundredTwentyEighth,
        DurationType::TwoHundredFiftySixth,
        DurationType::FiveHundredTwelfth,
        DurationType::OneThousandTwentyFourth,
    ];
    for kind in TYPES {
        let mut dots = 4u8;
        loop {
            let candidate = Duration::with_dots(kind, dots);
            if candidate.ticks() <= ticks {
                return candidate;
            }
            if dots == 0 {
                break;
            }
            dots -= 1;
        }
    }
    Duration::new(DurationType::Invalid)
}

/// Construit un accord ou silence vide (`setChordRestDuration`).
fn make_chord_rest(
    note: &Element,
    duration: Duration,
    dura: Fraction,
    measure_len: Fraction,
) -> ChordRest {
    let rest = note.has_child("rest");
    let mut chord_rest = if rest {
        ChordRest::rest(duration)
    } else {
        ChordRest::chord(duration)
    };
    if duration.is_measure() {
        // Un silence de mesure entière garde la durée de *sa* partie, qui peut
        // différer de la longueur globale (indications de mesure locales).
        chord_rest.ticks = Some(if dura.is_positive() {
            dura
        } else {
            measure_len
        });
    }
    if rest && note.attribute("print-object") == Some("no") {
        if let Some(inner) = chord_rest.rest_mut() {
            inner.visible = false;
        }
    }
    chord_rest
}

/// Construit une note (`xmlSetPitch`).
fn build_note(
    element: &Element,
    transpose: Option<scoretk_core::model::Transpose>,
    octave_shift: i32,
) -> Note {
    let (step, alter, octave) = read_pitch(element);
    let transpose = transpose.map_or(0, |transpose| transpose.chromatic);
    let step_index = step.unwrap_or(0);
    let octave = octave.unwrap_or(4);
    let mut pitch =
        [0, 2, 4, 5, 7, 9, 11][usize::from(step_index).min(6)] + alter + (octave + 1) * 12;
    pitch += transpose;
    pitch -= 12 * octave_shift;
    let pitch = pitch.clamp(0, 127) as u8;
    let tpc = tpc_from_step(step_index, alter);
    let mut note = Note::new(pitch, tpc);
    note.tpc_transposed = tpc;

    if let Some(accidental) = read_accidental(element) {
        note.accidental = Some(accidental);
    }
    if let Some(head) = element.child("notehead") {
        let value = head.text_trimmed();
        let (group, kind) = read_notehead(value, head.attribute("smufl"));
        note.head_group = group;
        note.head = kind;
        if value == "none" {
            note.visible = false;
        }
    }
    if element
        .child("notehead")
        .and_then(|head| head.attribute("parentheses"))
        == Some("yes")
    {
        note.parentheses = true;
    }
    if element.attribute("print-object") == Some("no") {
        note.visible = false;
    }
    if element.has_child("cue") {
        note.play = false;
    }
    note
}

/// Lit `<pitch>` : degré, altération, octave.
fn read_pitch(element: &Element) -> (Option<u8>, i32, Option<i32>) {
    let Some(pitch) = element.child("pitch") else {
        let unpitched = element.child("unpitched");
        let step = unpitched
            .and_then(|value| value.child_text("display-step"))
            .and_then(|step| "CDEFGAB".find(step).map(|index| index as u8));
        let octave = unpitched
            .and_then(|value| value.child_int("display-octave"))
            .map(|octave| octave as i32);
        return (step, 0, octave);
    };
    let step = pitch
        .child_text("step")
        .and_then(|step| "CDEFGAB".find(step).map(|index| index as u8));
    let alter = pitch.child_int("alter").unwrap_or(0) as i32;
    let octave = pitch.child_int("octave").map(|octave| octave as i32);
    (step, alter, octave)
}

/// `Tpc` d'un degré et de son altération (`step2tpc`).
pub(crate) fn tpc_from_step(step: u8, alter: i32) -> i8 {
    // C, D, E, F, G, A, B dans `engraving/dom/pitchspelling.h`.
    const BASE: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];
    let base = BASE[usize::from(step).min(6)];
    let value = i32::from(base) + 7 * alter;
    i8::try_from(value).unwrap_or(scoretk_core::model::tpc::INVALID)
}

/// Altération affichée (`importmusicxmlnotepitch.cpp`).
fn read_accidental(element: &Element) -> Option<Accidental> {
    let accidental = element.child("accidental")?;
    let kind = match accidental.text_trimmed() {
        "flat" => AccidentalType::Flat,
        "natural" => AccidentalType::Natural,
        "sharp" => AccidentalType::Sharp,
        "flat-flat" => AccidentalType::DoubleFlat,
        "double-sharp" | "sharp-sharp" => AccidentalType::DoubleSharp,
        "natural-flat" => AccidentalType::NaturalFlat,
        "natural-sharp" => AccidentalType::NaturalSharp,
        "triple-flat" => AccidentalType::TripleFlat,
        "triple-sharp" => AccidentalType::TripleSharp,
        _ => return None,
    };
    let mut result = Accidental::new(kind);
    let cautionary = accidental.attribute("cautionary") == Some("yes");
    let editorial = accidental.attribute("editorial") == Some("yes");
    result.user = cautionary || editorial;
    result.parentheses = accidental.attribute("parentheses") == Some("yes");
    result.bracket = accidental.attribute("bracket") == Some("yes");
    result.visible = element.attribute("print-object") != Some("no");
    Some(result)
}

/// Groupe et type de tête (`convertNotehead`), y compris les têtes SMuFL.
fn read_notehead(name: &str, smufl: Option<&str>) -> (NoteHeadGroup, NoteHeadType) {
    if let Some(smufl) = smufl {
        let group = match smufl {
            "noteheadVoidWithX"
            | "noteheadWholeWithX"
            | "noteheadHalfWithX"
            | "noteheadDoubleWholeWithX" => Some(NoteHeadGroup::WithX),
            "noteheadDiamondBlackOld"
            | "noteheadDiamondHalfOld"
            | "noteheadDiamondWholeOld"
            | "noteheadDiamondDoubleWholeOld" => Some(NoteHeadGroup::DiamondOld),
            "noteheadCircledBlackLarge"
            | "noteheadCircledHalfLarge"
            | "noteheadCircledWholeLarge"
            | "noteheadCircledDoubleWholeLarge" => Some(NoteHeadGroup::CircledLarge),
            "noteheadLargeArrowUpBlack"
            | "noteheadLargeArrowDownBlack"
            | "noteheadLargeArrowUpHalf"
            | "noteheadLargeArrowDownHalf"
            | "noteheadLargeArrowUpWhole"
            | "noteheadLargeArrowDownWhole" => Some(NoteHeadGroup::LargeArrow),
            "noteheadBlack" | "noteheadHalf" | "noteheadWhole" | "noteheadDoubleWholeSquare" => {
                Some(NoteHeadGroup::BrevisAlt)
            }
            "noteheadSlashWhiteWhole"
            | "noteheadSlashWhiteDoubleWhole"
            | "noteheadSlashDiamondWhite"
            | "noteheadSlashHorizontalEnds" => Some(NoteHeadGroup::LargeDiamond),
            "noteheadHeavyX" => Some(NoteHeadGroup::HeavyCross),
            "noteheadHeavyXHat" => Some(NoteHeadGroup::HeavyCrossHat),
            _ => None,
        };
        if let Some(group) = group {
            return (group, NoteHeadType::Auto);
        }
    }
    match name {
        "normal" => (NoteHeadGroup::Normal, NoteHeadType::Auto),
        "x" => (NoteHeadGroup::Cross, NoteHeadType::Auto),
        "cross" => (NoteHeadGroup::Plus, NoteHeadType::Auto),
        "circle-x" => (NoteHeadGroup::XCircle, NoteHeadType::Auto),
        "triangle" => (NoteHeadGroup::TriangleUp, NoteHeadType::Auto),
        "inverted triangle" => (NoteHeadGroup::TriangleDown, NoteHeadType::Auto),
        "diamond" => (NoteHeadGroup::Diamond, NoteHeadType::Auto),
        "circled" => (NoteHeadGroup::Circled, NoteHeadType::Auto),
        "slashed" => (NoteHeadGroup::Slashed1, NoteHeadType::Auto),
        "back slashed" => (NoteHeadGroup::Slashed2, NoteHeadType::Auto),
        "slash" => (NoteHeadGroup::Slash, NoteHeadType::Auto),
        "do" => (NoteHeadGroup::Do, NoteHeadType::Auto),
        "re" => (NoteHeadGroup::Re, NoteHeadType::Auto),
        "mi" => (NoteHeadGroup::Mi, NoteHeadType::Auto),
        "fa" | "fa up" => (NoteHeadGroup::Fa, NoteHeadType::Auto),
        "so" => (NoteHeadGroup::Sol, NoteHeadType::Auto),
        "la" => (NoteHeadGroup::La, NoteHeadType::Auto),
        "ti" => (NoteHeadGroup::Ti, NoteHeadType::Auto),
        other => (NoteHeadGroup::Other(other.to_owned()), NoteHeadType::Auto),
    }
}

/// Paroles d'une note (`MusicXmlParserLyric::parse`).
fn read_lyrics(element: &Element) -> Vec<Lyrics> {
    let mut result = Vec::new();
    for lyric in element.children_named("lyric") {
        let text = lyric.child_text("text").unwrap_or_default().to_owned();
        if text.is_empty() {
            continue;
        }
        let verse = lyric
            .attribute("number")
            .and_then(|number| number.trim().parse::<usize>().ok())
            .map_or(0, |number| number.saturating_sub(1));
        let syllabic = match lyric.child_text("syllabic") {
            Some("begin") => LyricsSyllabic::Begin,
            Some("middle") => LyricsSyllabic::Middle,
            Some("end") => LyricsSyllabic::End,
            _ => LyricsSyllabic::Single,
        };
        let mut entry = Lyrics::new(verse, syllabic, text);
        if lyric.attribute("placement") == Some("above") {
            entry.placement = Placement::Above;
        }
        if lyric.has_child("extend") {
            entry.melisma = Some(Fraction::ONE);
        }
        result.push(entry);
    }
    result
}

/// Mode de ligature écrit dans `<beam>` (`MusicXmlParserPass2::beam`).
fn read_beam(element: &Element) -> Option<scoretk_core::model::BeamMode> {
    use scoretk_core::model::BeamMode;
    for beam in element.children_named("beam") {
        let number = beam
            .attribute("number")
            .and_then(|number| number.parse::<usize>().ok())
            .unwrap_or(1);
        let begin = match number {
            2 => BeamMode::Begin16,
            3 => BeamMode::Begin32,
            _ => BeamMode::Begin,
        };
        let mode = match beam.text_trimmed() {
            "begin" => begin,
            "continue" => BeamMode::Mid,
            "end" => BeamMode::End,
            "forward hook" => BeamMode::ForwardHook,
            "backward hook" => BeamMode::BackwardHook,
            _ => continue,
        };
        return Some(mode);
    }
    None
}

/// Direction de hampe écrite dans `<stem>`.
fn read_stem(element: &Element) -> Option<StemDirection> {
    match element.child_text("stem") {
        Some("up") => Some(StemDirection::Up),
        Some("down") => Some(StemDirection::Down),
        Some("none") => Some(StemDirection::Auto),
        _ => None,
    }
}

/// Attributs portés par l'accord (hampe, ornement barré, trémolo).
fn apply_chord_attributes(element: &Element, chord: &mut Chord) {
    if let Some(stem) = read_stem(element) {
        chord.stem = stem;
    }
    if let Some(grace) = element.child("grace") {
        chord.grace_slash = grace.attribute("slash") == Some("yes");
    }
    if let Some(tremolo) = element
        .child("notations")
        .and_then(|notations| notations.child("ornaments"))
        .and_then(|ornaments| ornaments.child("tremolo"))
    {
        let kind = match tremolo.attribute("type") {
            Some("single") => scoretk_core::model::TremoloKind::Single,
            Some("start") => scoretk_core::model::TremoloKind::Start,
            Some("stop") => scoretk_core::model::TremoloKind::Stop,
            Some("unmeasured") => scoretk_core::model::TremoloKind::Unmeasured,
            _ => scoretk_core::model::TremoloKind::Single,
        };
        chord.tremolo = Some(scoretk_core::model::Tremolo {
            kind,
            count: tremolo.text_trimmed().parse().unwrap_or(1),
        });
        // Un trémolo à deux notes fait durer chaque accord la moitié du total
        // (`Chord::tremoloTwoChord`).
        chord.tremolo_two_chord = matches!(
            kind,
            scoretk_core::model::TremoloKind::Start | scoretk_core::model::TremoloKind::Stop
        );
    }
}

/// Type d'ornement d'un accord (`graceNoteType`).
fn grace_type(element: &Element) -> GraceType {
    let slash = element
        .child("grace")
        .and_then(|grace| grace.attribute("slash"))
        == Some("yes");
    if slash {
        return GraceType::Acciaccatura;
    }
    match element.child_text("type") {
        Some("quarter") => GraceType::Grace4,
        Some("16th") => GraceType::Grace16,
        Some("32nd") => GraceType::Grace32,
        _ => GraceType::Appoggiatura,
    }
}

/// Rattache les ornements en attente à l'accord qui les suit.
fn attach_grace_chords(
    score: &mut Score,
    measure_index: usize,
    tick: Fraction,
    track: Track,
    grace_chords: &mut Vec<PendingGrace>,
    from: usize,
) {
    if from >= grace_chords.len() {
        return;
    }
    let pending: Vec<PendingGrace> = grace_chords.split_off(from);
    grace_chords.clear();
    let Some(chord_rest) = chord_rest_at_mut(score, measure_index, tick, track) else {
        return;
    };
    let Some(chord) = chord_rest.chord_mut() else {
        return;
    };
    for pending in pending {
        let mut grace = GraceChord::new(pending.grace, pending.duration);
        grace.chord.grace_slash = pending.slash;
        grace.chord.notes = pending.notes;
        grace.chord.staff_move = pending.staff_move;
        chord.grace_chords.push(grace);
    }
}

/// Rattache les ornements « après l'accord » (`addGraceChordsAfter`).
fn attach_grace_after(
    score: &mut Score,
    measure_index: usize,
    tick: Fraction,
    track: Track,
    grace_chords: &mut Vec<PendingGrace>,
    count: usize,
) {
    if count == 0 || count > grace_chords.len() {
        return;
    }
    let after: Vec<PendingGrace> = grace_chords.drain(..count).collect();
    let Some(chord_rest) = chord_rest_at_mut(score, measure_index, tick, track) else {
        return;
    };
    let Some(chord) = chord_rest.chord_mut() else {
        return;
    };
    for pending in after {
        // `Chord::toGraceAfter` : un ornement « après » change de type.
        let kind = match pending.grace {
            GraceType::Appoggiatura => GraceType::Grace8After,
            GraceType::Grace16 => GraceType::Grace16After,
            GraceType::Grace32 => GraceType::Grace32After,
            other => other,
        };
        let mut grace = GraceChord::new(kind, pending.duration);
        grace.chord.grace_slash = pending.slash;
        grace.chord.notes = pending.notes;
        grace.chord.staff_move = pending.staff_move;
        chord.grace_chords.push(grace);
    }
}

/// Vrai si la note porte un trémolo à deux notes (`type="start"` ou `"stop"`).
fn is_tremolo_two(element: &Element) -> bool {
    element
        .child("notations")
        .and_then(|notations| notations.child("ornaments"))
        .and_then(|ornaments| ornaments.child("tremolo"))
        .and_then(|tremolo| tremolo.attribute("type"))
        .is_some_and(|kind| kind == "start" || kind == "stop")
}

/// Vrai si la note porte un `<slur type="stop">` sans départ.
fn slur_stop_without_start(element: &Element) -> bool {
    let Some(notations) = element.child("notations") else {
        return false;
    };
    let mut stop = false;
    let mut start = false;
    for slur in notations.children_named("slur") {
        match slur.attribute("type") {
            Some("stop") => stop = true,
            Some("start") => start = true,
            _ => {}
        }
    }
    stop && !start
}

/// Rattache les ornements restants au dernier accord de la mesure.
fn attach_orphan_graces(
    score: &mut Score,
    measure_index: usize,
    grace_chords: &mut Vec<PendingGrace>,
) {
    let Some(segment) = score.measures[measure_index]
        .segments
        .iter_mut()
        .rev()
        .find(|segment| segment.kind == SegmentType::ChordRest)
    else {
        grace_chords.clear();
        return;
    };
    let Some(track) = segment.iter_elements().find_map(|(track, item)| {
        matches!(item, SegmentItem::ChordRest(chord_rest) if chord_rest.is_chord()).then_some(track)
    }) else {
        grace_chords.clear();
        return;
    };
    let Some(chord) = segment
        .element_mut(track)
        .and_then(SegmentItem::as_chord_rest_mut)
        .and_then(ChordRest::chord_mut)
    else {
        grace_chords.clear();
        return;
    };
    for pending in grace_chords.drain(..) {
        let mut grace = GraceChord::new(pending.grace, pending.duration);
        grace.chord.grace_slash = pending.slash;
        grace.chord.notes = pending.notes;
        grace.chord.staff_move = pending.staff_move;
        chord.grace_chords.push(grace);
    }
}

// ----------------------------------------------------------------------
// N-olets
// ----------------------------------------------------------------------

/// Notations `<tuplet>` d'une note.
fn tuplet_notations(element: &Element) -> Vec<TupletNotation> {
    let mut result = Vec::new();
    let Some(notations) = element.child("notations") else {
        return result;
    };
    for tuplet in notations.children_named("tuplet") {
        let mut notation = TupletNotation {
            number: tuplet
                .attribute("number")
                .and_then(|number| number.parse().ok())
                .unwrap_or(1),
            bracket: tuplet.attribute("bracket") == Some("yes"),
            bracket_explicit: tuplet.attribute("bracket").is_some(),
            placement: match tuplet.attribute("placement") {
                Some("above") => Some(Placement::Above),
                Some("below") => Some(Placement::Below),
                _ => None,
            },
            number_type: match tuplet.attribute("show-number") {
                Some("both") => TupletNumberType::Ratio,
                Some("none") => TupletNumberType::None,
                _ => TupletNumberType::Number,
            },
            ..TupletNotation::default()
        };
        match tuplet.attribute("type") {
            Some("start") => notation.start = true,
            Some("stop") => notation.stop = true,
            _ => {}
        }
        if let Some(actual) = tuplet.child("tuplet-actual") {
            notation.actual = actual.child_int("tuplet-number");
            notation.base_len = actual.child_text("tuplet-type").and_then(|name| {
                let mut figure = noteduration::type_fraction(name)?;
                let dots = actual
                    .children
                    .iter()
                    .filter(|child| child.name == "tuplet-dot")
                    .count() as u32;
                let mut increment = figure * Fraction::HALF;
                for _ in 0..dots {
                    figure += increment;
                    increment *= Fraction::HALF;
                }
                Some(figure)
            });
        }
        if let Some(normal) = tuplet.child("tuplet-normal") {
            notation.normal = normal.child_int("tuplet-number");
        }
        result.push(notation);
    }
    result
}

/// Ouverture des n-olets (`handleTupletStart`), renvoie le n-olet le plus
/// interne après ouverture.
fn start_tuplets(
    score: &mut Score,
    state: &mut VoiceState,
    notations: &[TupletNotation],
    time_mod: Option<Fraction>,
    tick: Fraction,
    duration: Duration,
) -> Option<TupletId> {
    let combined = time_mod
        .map(|modification| Fraction::new(modification.denominator(), modification.numerator()))
        .filter(|ratio| *ratio != Fraction::ONE && *ratio != Fraction::new(1, 2));

    for notation in notations {
        if !notation.start {
            continue;
        }
        let mut outer = Fraction::ONE;
        for open in &state.tuplets {
            outer *= open.ratio;
        }
        let ratio = match combined {
            Some(combined) => combined / outer,
            None => Fraction::ONE,
        };
        let actual_notes = notation.actual.unwrap_or(ratio.numerator());
        let normal_notes = notation.normal.unwrap_or(ratio.denominator());
        let base_len = notation.base_len.unwrap_or_else(|| duration.ticks());
        let mut tuplet = Tuplet::new(ratio, base_len, tick, Fraction::ZERO);
        tuplet.parent = state.tuplets.last().map(|open| open.id);
        tuplet.bracket = notation.bracket;
        tuplet.bracket_explicit = notation.bracket_explicit;
        tuplet.number_type = notation.number_type;
        tuplet.placement = notation.placement;
        tuplet.actual_notes = actual_notes;
        tuplet.normal_notes = normal_notes;
        let id = TupletId::from_index(score.tuplets.len());
        score.tuplets.push(tuplet);
        state.tuplets.push(OpenTuplet {
            id,
            number: notation.number.max(1),
            ratio,
            normal_notes,
            explicit_base: notation.base_len.is_some(),
            notated: Fraction::ZERO,
            start: tick,
            max_end: tick,
        });
    }

    state.tuplets.last().map(|open| open.id)
}

/// Fermeture des n-olets (`handleTupletStop`).
fn stop_tuplets(score: &mut Score, state: &mut VoiceState, notations: &[TupletNotation]) {
    for notation in notations {
        if !notation.stop {
            continue;
        }
        let target = notation.number.max(1);
        while let Some(open) = state.tuplets.pop() {
            finish_tuplet(score, &open);
            if let Some(parent) = state.tuplets.last_mut() {
                let end = open.max_end;
                if end > parent.max_end {
                    parent.max_end = end;
                }
            }
            if open.number == target {
                break;
            }
        }
    }
}

/// Termine un n-olet : base et longueur (`handleTupletStop`).
fn finish_tuplet(score: &mut Score, open: &OpenTuplet) {
    let Some(tuplet) = score.tuplets.get_mut(open.id.index()) else {
        return;
    };
    if open.ratio.is_zero() {
        return;
    }
    let total = open.notated / open.ratio;
    let (_, full) = fraction_and_full_duration(total);
    let computed_base = full / Fraction::from_integer(open.ratio.denominator());
    let span = open.max_end - open.start;
    let normal = Fraction::from_integer(open.normal_notes.max(1));
    // MuseScore calcule la longueur d'un n-olet comme `baseLen * normalNotes`
    // (`TRead::read(Tuplet*)`). Quand le fichier donne le `<tuplet-type>`, il
    // fait foi ; sinon la base est déduite de l'étendue réelle des enfants.
    let (base_len, len) = if open.explicit_base {
        let base = tuplet.base_len;
        (base, base * normal)
    } else if span.is_positive() {
        (span / normal, span)
    } else {
        (computed_base, total)
    };
    tuplet.base_len = base_len;
    tuplet.tick = open.start;
    tuplet.len = if len.is_positive() { len } else { span };
}

/// `determineTupletFractionAndFullDuration`.
fn fraction_and_full_duration(duration: Fraction) -> (Fraction, Fraction) {
    if !duration.is_positive() {
        return (Fraction::ONE, Fraction::ONE);
    }
    let mut fraction = duration;
    let mut full = Fraction::ONE;
    while fraction.denominator() % 2 == 0 {
        fraction *= Fraction::from_integer(2);
        full *= Fraction::HALF;
    }
    while fraction.numerator() % 2 == 0 {
        fraction *= Fraction::HALF;
        full *= Fraction::from_integer(2);
    }
    while fraction <= Fraction::HALF {
        full *= Fraction::HALF;
        fraction *= Fraction::from_integer(2);
    }
    if fraction > Fraction::ONE && fraction.denominator() == 1 {
        full *= fraction;
        fraction = Fraction::ONE;
    }
    (fraction, full)
}

// ----------------------------------------------------------------------
// Liaisons de prolongation
// ----------------------------------------------------------------------

/// Départs de liaison en attente d'appariement (`m_unstartedTieNotes`).
#[derive(Default)]
struct TieMap {
    pending: BTreeMap<(Track, u8), NoteId>,
}

/// Applique `<tie>` et `<tied>` à une note.
fn apply_ties(
    element: &Element,
    note_id: NoteId,
    track: Track,
    score: &mut Score,
    ties: &mut TieMap,
) {
    let mut start = false;
    let mut stop = false;
    let mut direction_up = None;
    let mut visible_start = true;
    let mut visible_stop = true;

    for tie in element.children_named("tie") {
        match tie.attribute("type") {
            Some("start") => start = true,
            Some("stop") => stop = true,
            _ => {}
        }
    }
    if let Some(notations) = element.child("notations") {
        let visible = notations.attribute("print-object") != Some("no");
        for tied in notations.children_named("tied") {
            match tied.attribute("type") {
                Some("start") => {
                    start = true;
                    visible_start = visible;
                    direction_up = match tied.attribute("orientation") {
                        Some("over") => Some(true),
                        Some("under") => Some(false),
                        _ => direction_up,
                    };
                }
                Some("stop") => {
                    stop = true;
                    visible_stop = visible;
                }
                _ => {}
            }
        }
    }

    let note = &mut score.notes[note_id.index()];
    if stop {
        note.tie_back_visible = visible_stop;
    }
    if start {
        note.tie_forward_visible = visible_start;
        note.tie_up = direction_up;
    }
    let pitch = note.pitch;

    // `addTie` : l'appariement se fait par piste et hauteur. Un arrêt referme
    // le départ en attente, ce qui donne les deux moitiés de la liaison.
    if stop {
        if let Some(previous) = ties.pending.remove(&(track, pitch)) {
            score.notes[note_id.index()].tie_back = Some(previous);
            score.notes[previous.index()].tie_forward = Some(note_id);
        }
    }
    if start {
        ties.pending.insert((track, pitch), note_id);
    }
}

// ----------------------------------------------------------------------
// Silences de remplissage
// ----------------------------------------------------------------------

/// Remplit les trous de la voix 0 d'une portée (`fillGapsInFirstVoices`).
fn fill_gaps_in_first_voice(score: &mut Score, measure_index: usize, staff_id: StaffId) {
    let measure_start = score.measures[measure_index].start_tick;
    let measure_end = score.measures[measure_index].end_tick();
    let track = Track::new(staff_id, 0);

    let mut events: Vec<(Fraction, Fraction)> = Vec::new();
    for segment in &score.measures[measure_index].segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        let Some(chord_rest) = segment.chord_rest(track) else {
            continue;
        };
        events.push((segment.tick, actual_ticks(score, chord_rest, measure_index)));
    }

    let mut end_of_last = measure_start;
    let mut fills: Vec<(Fraction, Fraction)> = Vec::new();
    for (tick, length) in events {
        if tick > end_of_last {
            fills.push((end_of_last, tick - end_of_last));
        }
        end_of_last = tick + length;
    }
    if measure_end > end_of_last {
        fills.push((end_of_last, measure_end - end_of_last));
    }

    for (tick, length) in fills {
        let mut rest = ChordRest::rest(duration_from_ticks(length));
        rest.ticks = Some(length);
        if let Some(inner) = rest.rest_mut() {
            inner.visible = false;
        }
        let index = get_segment(score, measure_index, SegmentType::ChordRest, tick);
        score.measures[measure_index].segments[index]
            .set_element(track, SegmentItem::ChordRest(rest));
    }
}

/// Durée réelle d'un accord ou silence, n-olets appliqués.
fn actual_ticks(score: &Score, chord_rest: &ChordRest, measure_index: usize) -> Fraction {
    let mut ticks = chord_rest
        .ticks
        .unwrap_or_else(|| chord_rest.duration.ticks());
    if chord_rest.is_full_measure_rest() {
        ticks = score.measures[measure_index].len;
    }
    let mut tuplet = chord_rest.tuplet;
    let mut depth = 0;
    while let Some(id) = tuplet {
        if depth >= 64 {
            break;
        }
        let Some(value) = score.tuplets.get(id.index()) else {
            break;
        };
        if value.ratio.is_zero() {
            break;
        }
        ticks /= value.ratio;
        tuplet = value.parent;
        depth += 1;
    }
    ticks
}
