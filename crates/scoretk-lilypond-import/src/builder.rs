// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de l'assemblage de `Score` et des objets d'`engraving/dom/` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026, à partir des événements datés d'une lecture
// LilyPond.
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

//! Construction de la partition à partir des voix interprétées.
//!
//! LilyPond ne connaît pas les mesures : elles se déduisent des indications de
//! mesure et de la musique. Cette passe établit donc une trame de mesures
//! (indications `\time`, `\set Timing.measureLength`, `\partial`, silences
//! multi-mesures), puis range les événements datés dans les segments de chaque
//! mesure, comme le fait le lecteur `.mscx` de MuseScore.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{
    Accidental, AccidentalType, Annotation, BarLineType, Chord, ChordRest, ChordRestKind, Clef,
    Credit, CreditStyle, Duration, Dynamic, Fingering, Fraction, Glissando, GlissandoType,
    GraceChord, Hairpin, HairpinType, Instrument, LineStyle, Lyrics, LyricsSyllabic, Measure, Note,
    NoteId, Part, PedalInfo, Placement, RehearsalMark, Rest, RichText, Score, ScoreMetadata,
    Segment, SegmentItem, SegmentType, Spanner, SpannerAnchor, SpannerNotation, SpannerType, Staff,
    StaffBracket, StaffId, Tempo, Track, Trill, TrillType, Tuplet, TupletId, VOICES,
};

use crate::ast::{Document, GroupKind, LyricsBlock, StaffBlock};
use crate::events::{
    arpeggio_object, articulation_objects, breath_object, ornament_objects, ottava_type,
    AnnotationEvent, ChordContent, ChordEvent, EventKind, GraceEvent, InterpretedVoice, MetaEvent,
    RepeatSection, StaffTarget, VoiceEvent,
};
use crate::warnings::Warnings;
use crate::Interpreter;

/// Intervalle pendant lequel les barres automatiques sont suspendues.
///
/// Correspond à `Timing.timing = ##f` de LilyPond : la mesure en cours
/// s'allonge jusqu'à la fermeture de la cadence (`\cadenzaOff`) ou à la
/// première barre explicite. Les intervalles de toutes les voix sont
/// collectés tels quels : leurs bornes servent à la fois de coupures et de
/// test d'appartenance, sans fusion (deux cadences adjacentes séparées par un
/// `\bar` doivent rester deux mesures).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CadenzaSpan {
    /// Tick du `\cadenzaOn`.
    start: Fraction,
    /// Tick du `\cadenzaOff`, d'une barre explicite ou de la fin de la musique.
    end: Fraction,
}

/// Trame d'une mesure.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MeasureSlot {
    /// Tick de début.
    start: Fraction,
    /// Durée réelle.
    len: Fraction,
    /// Durée nominale de l'indication de mesure.
    nominal: Fraction,
    /// Nombre de mesures regroupées par un silence multi-mesures.
    multi_rest: Option<u16>,
}

/// Voix interprétée et rattachée à une portée.
struct VoiceBuild {
    /// Nom du contexte (`\new Voice = "…"`), pour `\lyricsto`.
    name: Option<String>,
    /// Piste du modèle.
    track: Track,
    /// Événements.
    interpreted: InterpretedVoice,
    /// N-olets globaux, dans l'ordre local.
    tuplets: Vec<TupletId>,
}

/// Paroles interprétées.
struct LyricsBuild {
    /// Nom de la voix cible.
    target: String,
    /// Numéro de couplet.
    verse: usize,
    /// Syllabes.
    syllables: Vec<crate::ast::Syllable>,
}

/// Portée construite.
struct StaffBuild {
    /// Identifiant de la portée.
    staff: StaffId,
    /// Nom du contexte (`\new Staff = "…"`), pour résoudre `\change`.
    name: Option<String>,
    /// Voix.
    voices: Vec<VoiceBuild>,
    /// Paroles attachées à cette portée.
    lyrics: Vec<LyricsBuild>,
}

/// Construit la partition d'un document LilyPond.
pub(crate) fn build(document: &Document, warnings: &mut Warnings) -> Score {
    let mut score = Score::new();
    write_metadata(&mut score, document);

    let mut staves: Vec<StaffBuild> = Vec::new();
    let Some(score_ast) = &document.score else {
        return score;
    };
    for group in &score_ast.groups {
        if group.staves.is_empty() {
            continue;
        }
        let first = &group.staves[0];
        let long_name = group
            .instrument_name
            .clone()
            .or_else(|| first.instrument_name.clone())
            .unwrap_or_default();
        let short_name = group
            .short_instrument_name
            .clone()
            .or_else(|| first.short_instrument_name.clone())
            .unwrap_or_default();
        let mut part = Part::new(long_name.clone(), short_name.clone());
        part.name = long_name.clone();
        part.instrument = instrument_for(&long_name, &short_name);
        let part_id = score.add_part(part);

        let first_staff_index = staves.len();
        for (position, staff_block) in group.staves.iter().enumerate() {
            let mut staff = Staff::new(part_id);
            if let Some(bracket) = group_bracket(group.kind.clone(), position, group.staves.len()) {
                staff.brackets.push(bracket);
            }
            let staff_id = score.add_staff_with(part_id, staff);
            let voices = build_voices(&mut score, staff_id, staff_block, warnings);
            let lyrics = staff_block.lyrics.iter().map(lyrics_build).collect();
            staves.push(StaffBuild {
                staff: staff_id,
                name: staff_block.name.clone(),
                voices,
                lyrics,
            });
        }
        // Paroles du groupe (hors portée) : rattachées à sa première portée.
        if !group.lyrics.is_empty() {
            if let Some(target) = staves.get_mut(first_staff_index) {
                for block in &group.lyrics {
                    target.lyrics.push(lyrics_build(block));
                }
            }
        }
    }

    if staves.is_empty() {
        return score;
    }

    apply_initial_clefs(&mut score, &staves);
    let slots = plan_measures(&staves, warnings);
    create_measures(&mut score, &slots);
    place_measures(&mut score, &staves, &slots, warnings);
    apply_accidentals(&mut score);
    place_spanners(&mut score, &staves);
    place_repeats(&mut score, &staves);
    place_lyrics(&mut score, &staves);
    link_ties(&mut score, &staves);
    score
}

/// Paroles d'un bloc.
fn lyrics_build(block: &LyricsBlock) -> LyricsBuild {
    LyricsBuild {
        target: block.target.clone(),
        verse: block.verse,
        syllables: block.syllables.clone(),
    }
}

/// Construit les voix d'une portée et leurs n-olets.
fn build_voices(
    score: &mut Score,
    staff: StaffId,
    staff_block: &StaffBlock,
    warnings: &mut Warnings,
) -> Vec<VoiceBuild> {
    let mut voices = Vec::new();
    for (index, block) in staff_block.voices.iter().enumerate() {
        if index >= VOICES {
            warnings.push(
                0,
                0,
                "plus de quatre voix sur une portée : voix ignorée".to_owned(),
            );
            continue;
        }
        let interpreted = Interpreter::run(&block.music, warnings);
        let mut tuplets = Vec::new();
        for tuplet in &interpreted.tuplets {
            let mut value = Tuplet::new(tuplet.ratio, tuplet.base_len, tuplet.tick, tuplet.len);
            value.actual_notes = tuplet.actual_notes;
            value.normal_notes = tuplet.normal_notes;
            value.number_type = tuplet.number_type;
            // `Tuplet::calcHasBracket` (lecteur `.mscx`) : un n-olet dont la
            // durée de base tient dans une croche se passe de crochet ; sinon
            // MuseScore dessine le crochet.
            value.bracket = !(tuplet.base_len < Fraction::new(1, 8)
                || (tuplet.base_len == Fraction::new(1, 8) && tuplet.ratio != Fraction::ONE));
            if let Some(parent) = tuplet.parent {
                value.parent = tuplets.get(parent).copied();
            }
            tuplets.push(score.add_tuplet(value));
        }
        voices.push(VoiceBuild {
            name: block.name.clone(),
            track: Track::new(staff, u8::try_from(index).unwrap_or(0)),
            interpreted,
            tuplets,
        });
    }
    voices
}

/// Accolade d'un groupe de portées.
fn group_bracket(kind: GroupKind, position: usize, count: usize) -> Option<StaffBracket> {
    if position != 0 || count < 2 {
        return None;
    }
    let kind = match kind {
        GroupKind::PianoStaff => 1,
        GroupKind::ChoirStaff => 0,
        _ => return None,
    };
    Some(StaffBracket {
        kind,
        span: count,
        visible: true,
    })
}

/// Instrument approché d'après son nom.
///
/// LilyPond ne porte ni identifiant MusicXML ni programme MIDI dans le
/// sous-ensemble lu : les noms d'instrument sont le seul indice. La table
/// couvre les instruments courants du corpus ; à défaut, l'instrument garde
/// son nom et aucun programme.
fn instrument_for(long_name: &str, short_name: &str) -> Instrument {
    let mut instrument = Instrument::new(long_name, long_name, short_name);
    instrument.track_name = long_name.to_owned();
    let name = long_name.to_lowercase();
    // (motif, identifiant MusicXML, programme MIDI base zéro, nom de piste).
    let table: [(&str, &str, u8, &str); 31] = [
        ("piccolo", "wind.flutes.piccolo", 72, "Piccolo"),
        ("flute", "wind.flutes.flute", 73, "Flute"),
        ("oboe", "wind.reed.oboe", 68, "Oboe"),
        ("english horn", "wind.reed.english-horn", 69, "English Horn"),
        ("bassoon", "wind.reed.bassoon", 70, "Bassoon"),
        ("clarinet", "wind.reed.clarinet.bflat", 71, "Clarinet"),
        ("sax", "wind.reed.saxophone", 65, "Saxophone"),
        ("horn", "brass.french-horn", 60, "Horn"),
        ("trumpet", "brass.trumpet", 56, "Trumpet"),
        ("trombone", "brass.trombone", 57, "Trombone"),
        ("tuba", "brass.tuba", 58, "Tuba"),
        ("timpani", "percussion.timpani", 47, "Timpani"),
        ("violin", "strings.violin", 40, "Violin"),
        ("viola", "strings.viola", 41, "Viola"),
        ("violoncello", "strings.cello", 42, "Violoncello"),
        ("cello", "strings.cello", 42, "Violoncello"),
        ("contrabass", "strings.contrabass", 43, "Contrabass"),
        ("double bass", "strings.contrabass", 43, "Contrabass"),
        ("guitar", "pluck.guitar", 24, "Guitar"),
        ("harp", "pluck.harp", 46, "Harp"),
        ("harpsichord", "keyboard.harpsichord", 6, "Harpsichord"),
        ("organ", "keyboard.organ", 19, "Organ"),
        ("piano", "keyboard.piano", 0, "Piano"),
        ("voice", "voice.vocals", 52, "Voice"),
        ("choir", "voice.vocals", 52, "Choir"),
        ("soprano", "voice.vocals", 52, "Soprano"),
        ("alto", "voice.vocals", 52, "Alto"),
        ("tenor", "voice.vocals", 52, "Tenor"),
        ("bass", "voice.vocals", 52, "Bass"),
        ("drum", "percussion.drumset", 0, "Drumset"),
        ("", "", 0, ""),
    ];
    if let Some((_, id, program, track_name)) = table
        .iter()
        .take(table.len() - 1)
        .find(|(needle, _, _, _)| name.contains(needle))
    {
        instrument.id = (*id).to_owned();
        instrument.midi_program = Some(*program);
        instrument.track_name = (*track_name).to_owned();
    }
    instrument
}

/// Métadonnées et crédits de `\header`.
fn write_metadata(score: &mut Score, document: &Document) {
    let header = &document.header;
    let field = |name: &str| header.get(name).map(str::trim).unwrap_or_default();
    // `workTitle` n'est écrit par l'écrivain que si le modèle a un titre
    // d'œuvre ; un simple crédit de titre ne doit pas créer `<work>`.
    let metadata = ScoreMetadata {
        work_title: field("workTitle").to_owned(),
        movement_title: field("movementTitle").to_owned(),
        movement_number: field("movementNumber").to_owned(),
        work_number: field("workNumber").to_owned(),
        composer: field("composer").to_owned(),
        lyricist: field("poet").to_owned(),
        arranger: field("arranger").to_owned(),
        copyright: field("copyright").to_owned(),
        source: field("source").to_owned(),
        subtitle: field("subtitle").to_owned(),
        ..ScoreMetadata::default()
    };
    score.metadata = metadata;

    // Les crédits reprennent les champs de titre : l'export les écrit dans
    // `<credit>`, comme MuseScore pour les cadres de titre.
    let credits = [
        (CreditStyle::Title, field("title")),
        (CreditStyle::Subtitle, field("subtitle")),
        (CreditStyle::Composer, field("composer")),
        (CreditStyle::Lyricist, field("poet")),
    ];
    for (style, text) in credits {
        if text.is_empty() {
            continue;
        }
        score.add_credit(Credit::new(RichText::new(text), style));
    }
}

/// Clefs d'en-tête : la première clef de chaque portée devient sa clef par
/// défaut.
fn apply_initial_clefs(score: &mut Score, staves: &[StaffBuild]) {
    for build in staves {
        let clef = build.voices.iter().find_map(|voice| {
            voice.interpreted.events.iter().find_map(|event| {
                if event.tick.is_zero() {
                    if let EventKind::Meta(MetaEvent::Clef(clef)) = event.kind {
                        return Some(clef);
                    }
                }
                None
            })
        });
        if let Some(clef) = clef {
            score.staff_mut(build.staff).default_clef = Clef::new(clef);
        }
    }
}

// ----------------------------------------------------------------------
// Trame des mesures
// ----------------------------------------------------------------------

/// Indications de mesure d'un tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MeterAt {
    nominal: Option<Fraction>,
    length: Option<Fraction>,
    partial: Option<Fraction>,
}

/// Intervalles de cadence de toutes les voix.
///
/// Toutes les voix partageant les mesures, une cadence dans une seule voix
/// suffit à suspendre les barres automatiques pour la partition entière
/// pendant cet intervalle (voir § 7 de `docs/agents/lilypond.md`).
fn cadenza_spans(staves: &[StaffBuild], end_tick: Fraction) -> Vec<CadenzaSpan> {
    let mut spans = Vec::new();
    for staff in staves {
        for voice in &staff.voices {
            let mut events: Vec<&VoiceEvent> = voice.interpreted.events.iter().collect();
            events.sort_by_key(|event| (event.tick, event.order));
            let mut open: Option<Fraction> = None;
            for event in events {
                match &event.kind {
                    EventKind::Meta(MetaEvent::Cadenza(true)) => {
                        if open.is_none() {
                            open = Some(event.tick);
                        }
                    }
                    EventKind::Meta(MetaEvent::Cadenza(false)) => {
                        if let Some(start) = open.take() {
                            if event.tick > start {
                                spans.push(CadenzaSpan {
                                    start,
                                    end: event.tick,
                                });
                            }
                        }
                    }
                    // `\bar "|"` ou `|` pendant la cadence : la mesure se
                    // ferme là, mais la cadence continue ensuite.
                    EventKind::Meta(MetaEvent::Bar(_)) => {
                        if let Some(start) = open {
                            if event.tick > start {
                                spans.push(CadenzaSpan {
                                    start,
                                    end: event.tick,
                                });
                                open = Some(event.tick);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(start) = open {
                if end_tick > start {
                    spans.push(CadenzaSpan {
                        start,
                        end: end_tick,
                    });
                }
            }
        }
    }
    spans
}

/// Construit la trame des mesures.
fn plan_measures(staves: &[StaffBuild], warnings: &mut Warnings) -> Vec<MeasureSlot> {
    let mut meter: BTreeMap<Fraction, MeterAt> = BTreeMap::new();
    for (staff_index, staff) in staves.iter().enumerate() {
        for voice in &staff.voices {
            for event in &voice.interpreted.events {
                let EventKind::Meta(meta) = &event.kind else {
                    continue;
                };
                let entry = meter.entry(event.tick).or_default();
                match meta {
                    MetaEvent::Time {
                        numerator,
                        denominator,
                    } => {
                        if staff_index == 0 || entry.nominal.is_none() {
                            entry.nominal = Some(Fraction::new(
                                i64::from(*numerator),
                                i64::from(*denominator),
                            ));
                        }
                    }
                    MetaEvent::MeasureLength(length) => {
                        if staff_index == 0 || entry.length.is_none() {
                            entry.length = Some(*length);
                        }
                    }
                    MetaEvent::Partial(length) => {
                        entry.partial = Some(*length);
                    }
                    _ => {}
                }
            }
        }
    }

    let end_tick = staves
        .iter()
        .flat_map(|staff| staff.voices.iter())
        .map(|voice| voice.interpreted.end)
        .max()
        .unwrap_or(Fraction::ZERO);
    let spans = cadenza_spans(staves, end_tick);
    let mut slots = Vec::new();
    let mut start = Fraction::ZERO;
    let mut nominal = Fraction::ONE;

    while start < end_tick && slots.len() < 100_000 {
        if let Some(at) = meter.get(&start) {
            if let Some(value) = at.nominal {
                nominal = value;
            }
        }
        let at = meter.get(&start).copied().unwrap_or_default();
        let mut len = at.length.or(at.partial).unwrap_or(nominal);
        if !len.is_positive() {
            warnings.push(0, 0, "mesure de longueur nulle ignorée".to_owned());
            break;
        }
        // Une indication de mesure postérieure coupe la mesure.
        if let Some((tick, _)) = meter
            .range(start..)
            .find(|(tick, value)| **tick > start && value.nominal.is_some())
        {
            if *tick < start + len {
                len = *tick - start;
            }
        }
        // Pendant une cadence, aucune barre automatique : la mesure en cours
        // s'allonge jusqu'à la fermeture de la cadence ou à sa barre
        // explicite la plus proche. La durée nominale reste celle du `\time`,
        // ce qui donne une mesure irrégulière (`len != nominal_len`) comme
        // MuseScore pour une mesure de durée réelle différente de la signature.
        let end = start + len;
        if spans
            .iter()
            .any(|span| span.start < end && start < span.end)
        {
            if let Some(cut) = spans
                .iter()
                .map(|span| span.end)
                .filter(|tick| *tick > start)
                .min()
            {
                len = cut - start;
            }
        }
        slots.push(MeasureSlot {
            start,
            len,
            nominal,
            multi_rest: None,
        });
        start += len;
    }

    merge_multi_rests(&mut slots, staves);
    slots
}

/// Regroupe les mesures couvertes par un silence multi-mesures.
fn merge_multi_rests(slots: &mut Vec<MeasureSlot>, staves: &[StaffBuild]) {
    let mut merged = Vec::new();
    let mut index = 0usize;
    while index < slots.len() {
        let start = slots[index].start;
        let mut count = 1usize;
        if let Some(length) = multi_rest_length(staves, start).filter(|length| length.is_positive())
        {
            let mut covered = Fraction::ZERO;
            let mut candidate = 0usize;
            while index + candidate < slots.len() {
                covered += slots[index + candidate].len;
                candidate += 1;
                if covered == length {
                    count = candidate;
                    break;
                }
                if covered > length {
                    break;
                }
            }
        }
        if count > 1 {
            let mut slot = slots[index].clone();
            let mut total = Fraction::ZERO;
            for offset in 0..count {
                total += slots[index + offset].len;
            }
            slot.len = total;
            slot.multi_rest = Some(u16::try_from(count).unwrap_or(u16::MAX));
            merged.push(slot);
        } else {
            merged.push(slots[index].clone());
        }
        index += count;
    }
    *slots = merged;
}

/// Longueur d'un silence multi-mesures commençant à un tick.
fn multi_rest_length(staves: &[StaffBuild], start: Fraction) -> Option<Fraction> {
    for staff in staves {
        for voice in &staff.voices {
            for event in &voice.interpreted.events {
                if event.tick != start {
                    continue;
                }
                if let EventKind::Chord(chord) = &event.kind {
                    if matches!(chord.content, ChordContent::MultiRest) {
                        return Some(chord.actual);
                    }
                }
            }
        }
    }
    None
}

/// Crée les mesures de la trame.
fn create_measures(score: &mut Score, slots: &[MeasureSlot]) {
    for (index, slot) in slots.iter().enumerate() {
        let mut measure = Measure::new(index, slot.start, slot.nominal);
        measure.len = slot.len;
        measure.multi_measure_rest = slot.multi_rest;
        score.add_measure(measure);
    }
}

/// Indice de la mesure contenant un tick.
fn measure_at(slots: &[MeasureSlot], tick: Fraction) -> Option<usize> {
    slots
        .partition_point(|slot| slot.start <= tick)
        .checked_sub(1)
        .filter(|index| *index < slots.len())
}

/// Segment de la mesure, créé au besoin.
fn ensure_segment(
    score: &mut Score,
    measure_index: usize,
    kind: SegmentType,
    tick: Fraction,
) -> usize {
    let staves = score.staves.len();
    let measure = &mut score.measures[measure_index];
    if let Some(position) = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
    {
        return position;
    }
    measure.add_segment(Segment::new(kind, tick, staves));
    measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
        .unwrap_or(0)
}

// ----------------------------------------------------------------------
// Placement des événements
// ----------------------------------------------------------------------

/// Range les événements de toutes les voix dans les mesures.
fn place_measures(
    score: &mut Score,
    staves: &[StaffBuild],
    slots: &[MeasureSlot],
    warnings: &mut Warnings,
) {
    for (staff_index, staff) in staves.iter().enumerate() {
        // `Chord::staffMove` ne traverse que les portées d'une même partie :
        // un `\change` ne peut atteindre que les portées nommées de la partie
        // courante, pas celles d'un autre instrument.
        let part = score.staff(staff.staff).part;
        let local_names: BTreeMap<&str, usize> = staves
            .iter()
            .enumerate()
            .filter(|(_, candidate)| score.staff(candidate.staff).part == part)
            .filter_map(|(index, candidate)| candidate.name.as_deref().map(|name| (name, index)))
            .collect();
        for voice_index in 0..staff.voices.len() {
            // Un seul avertissement par nom inconnu et par voix, daté du
            // `\change` qui l'a introduit plutôt que de chaque accord.
            let mut warned: BTreeSet<&str> = BTreeSet::new();
            let mut events: Vec<VoiceEvent> = staff.voices[voice_index].interpreted.events.clone();
            events.sort_by_key(|event| (event.tick, event.order));
            for event in &events {
                match &event.kind {
                    EventKind::Chord(chord) => {
                        let staff_move = chord.staff.as_ref().map_or(0, |target| {
                            staff_move_of(&local_names, staff_index, target, &mut warned, warnings)
                        });
                        place_chord(
                            score,
                            slots,
                            staff,
                            voice_index,
                            event.tick,
                            chord,
                            staff_move,
                            warnings,
                        );
                    }
                    EventKind::Meta(meta) => {
                        place_meta(
                            score,
                            slots,
                            staff.voices[voice_index].track,
                            event.tick,
                            meta,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Décalage de portée d'un `\change` vers le contexte nommé, ou 0 si la portée
/// est inconnue ou hors de la partie (les notes restent alors sur leur portée
/// d'origine). L'avertissement n'est émis qu'une fois par nom et par voix, à la
/// position de la commande `\change`.
fn staff_move_of<'a>(
    names: &BTreeMap<&str, usize>,
    from: usize,
    target: &'a StaffTarget,
    warned: &mut BTreeSet<&'a str>,
    warnings: &mut Warnings,
) -> i8 {
    match names.get(target.name.as_str()) {
        Some(&index) => {
            let from = i8::try_from(from).unwrap_or(i8::MAX);
            let to = i8::try_from(index).unwrap_or(i8::MAX);
            to.saturating_sub(from)
        }
        None => {
            if warned.insert(target.name.as_str()) {
                warnings.push(
                    target.line,
                    target.column,
                    format!("portée inconnue : {}", target.name),
                );
            }
            0
        }
    }
}

/// Place un accord ou un silence.
#[allow(clippy::too_many_arguments)]
fn place_chord(
    score: &mut Score,
    slots: &[MeasureSlot],
    staff: &StaffBuild,
    voice_index: usize,
    tick: Fraction,
    event: &ChordEvent,
    staff_move: i8,
    warnings: &mut Warnings,
) {
    let voice_track = staff.voices[voice_index].track;
    let tuplets = &staff.voices[voice_index].tuplets;
    // L'écrivain comble les trous d'une voix secondaire par des silences de
    // mesure entière : ces silences ne sont pas dans le modèle d'origine.
    // Les relire créerait des `<rest measure="yes"/>` que la référence
    // n'a pas. On les ignore pour les voix autres que la première.
    if matches!(event.content, ChordContent::MultiRest) && voice_index > 0 {
        return;
    }
    let Some(measure_index) = measure_at(slots, tick) else {
        warnings.push(0, 0, "événement hors mesure ignoré".to_owned());
        return;
    };
    let segment_index = ensure_segment(score, measure_index, SegmentType::ChordRest, tick);
    let mut chord_rest = match &event.content {
        ChordContent::Notes(notes) => {
            let mut chord = Chord {
                stem: event.stem,
                staff_move,
                articulations: articulation_objects(&event.articulations),
                ornaments: ornament_objects(&event.ornaments),
                arpeggio: event.arpeggio.map(arpeggio_object),
                tremolo: event.tremolo,
                chord_line: event.chord_line,
                ..Chord::default()
            };
            for note in notes {
                let id = score.add_note(build_note(note, &event.fingerings));
                chord.notes.push(id);
            }
            for grace in &event.grace_before {
                chord.grace_chords.push(build_grace(score, grace));
            }
            for grace in &event.grace_after {
                chord.grace_chords.push(build_grace(score, grace));
            }
            ChordRest {
                duration: event.written,
                ticks: event.explicit_ticks,
                tuplet: event.tuplet.and_then(|index| tuplets.get(index).copied()),
                lyrics: Vec::new(),
                beam_mode: Default::default(),
                beam_direction: None,
                kind: ChordRestKind::Chord(chord),
            }
        }
        ChordContent::Rest { visible } => ChordRest {
            duration: event.written,
            ticks: event.explicit_ticks,
            tuplet: event.tuplet.and_then(|index| tuplets.get(index).copied()),
            lyrics: Vec::new(),
            beam_mode: Default::default(),
            beam_direction: None,
            kind: ChordRestKind::Rest(Rest {
                full_measure: false,
                gap: false,
                visible: *visible,
            }),
        },
        ChordContent::Skip => return,
        ChordContent::MultiRest => ChordRest {
            duration: Duration::measure(),
            ticks: None,
            tuplet: None,
            lyrics: Vec::new(),
            beam_mode: Default::default(),
            beam_direction: None,
            kind: ChordRestKind::Rest(Rest {
                full_measure: true,
                gap: false,
                visible: true,
            }),
        },
    };
    chord_rest.ticks = chord_rest
        .ticks
        .filter(|ticks| *ticks != chord_rest.duration.ticks());
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.set_element(voice_track, SegmentItem::ChordRest(chord_rest));
    }
    if let Some(breath) = event.breath {
        place_breath(score, slots, voice_track, tick, breath);
    }
}

/// Construit une note du modèle.
fn build_note(note: &crate::events::NoteEvent, fingerings: &[String]) -> Note {
    let mut value = Note::new(note.pitch, note.tpc);
    value.tpc_transposed = note.tpc_transposed;
    value.head_group = note.head_group.clone();
    if note.cautionary {
        value.accidental = Some(Accidental {
            kind: accidental_for(note.tpc),
            bracket: false,
            parentheses: false,
            visible: true,
            user: true,
        });
    }
    value.fingerings = fingerings
        .iter()
        .map(|text| Fingering::new(text.clone()))
        .collect();
    value
}

/// Altération affichée d'après une classe de hauteur tonale.
fn accidental_for(tpc: i8) -> AccidentalType {
    let alter = (i32::from(tpc) - 13).div_euclid(7);
    match alter {
        -2 => AccidentalType::DoubleFlat,
        -1 => AccidentalType::Flat,
        0 => AccidentalType::Natural,
        1 => AccidentalType::Sharp,
        2 => AccidentalType::DoubleSharp,
        _ => AccidentalType::None,
    }
}

/// Construit un accord d'agrément.
fn build_grace(score: &mut Score, grace: &GraceEvent) -> GraceChord {
    let mut value = GraceChord::new(grace.kind, grace.duration);
    value.chord.grace_slash = grace.slash;
    for note in &grace.notes {
        let id = score.add_note(build_note(note, &[]));
        value.chord.notes.push(id);
    }
    value
}

/// Place une respiration.
fn place_breath(
    score: &mut Score,
    slots: &[MeasureSlot],
    track: Track,
    tick: Fraction,
    kind: scoretk_core::model::BreathType,
) {
    let Some(measure_index) = measure_at(slots, tick) else {
        return;
    };
    let segment_index = ensure_segment(score, measure_index, SegmentType::Breath, tick);
    let placed = breath_object(kind, Placement::Above);
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.set_element(track, SegmentItem::Breath(placed));
    }
}

/// Place un changement de contexte.
fn place_meta(
    score: &mut Score,
    slots: &[MeasureSlot],
    track: Track,
    tick: Fraction,
    meta: &MetaEvent,
) {
    let Some(measure_index) = measure_at(slots, tick) else {
        return;
    };
    match meta {
        MetaEvent::Clef(clef) => {
            let (target_measure, kind) = if tick.is_zero() {
                (0, SegmentType::HeaderClef)
            } else if slots
                .get(measure_index)
                .is_some_and(|slot| slot.start == tick)
            {
                (measure_index.saturating_sub(1), SegmentType::Clef)
            } else {
                (measure_index, SegmentType::Clef)
            };
            let segment_index = ensure_segment(score, target_measure, kind, tick);
            if let Some(segment) = score.measures[target_measure]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(track, SegmentItem::Clef(Clef::new(*clef)));
            }
        }
        MetaEvent::Key(key) => {
            let segment_index = ensure_segment(score, measure_index, SegmentType::KeySig, tick);
            if let Some(segment) = score.measures[measure_index]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(track, SegmentItem::KeySig(key.clone()));
            }
        }
        MetaEvent::Time {
            numerator,
            denominator,
        } => {
            let segment_index = ensure_segment(score, measure_index, SegmentType::TimeSig, tick);
            if let Some(segment) = score.measures[measure_index]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(
                    track,
                    SegmentItem::TimeSig(scoretk_core::model::TimeSig::new(
                        *numerator,
                        *denominator,
                    )),
                );
            }
        }
        MetaEvent::Bar(BarLineType::Normal) => {
            // `|` est la barre par défaut du modèle : la poser écraserait une
            // barre plus forte (`\bar "|."`, fin de reprise) au même tick.
            // La trame des mesures s'en sert seule pour couper les cadences.
        }
        MetaEvent::Bar(kind) => {
            // Une barre à la fin d'une mesure ferme cette mesure ; au début,
            // elle ouvre la suivante.
            let target = slots
                .iter()
                .position(|slot| slot.start + slot.len == tick)
                .map(|index| (index, false))
                .or_else(|| {
                    slots
                        .iter()
                        .position(|slot| slot.start == tick)
                        .map(|index| (index, true))
                });
            if let Some((index, start)) = target {
                let measure = &mut score.measures[index];
                if start {
                    measure.start_bar_line = *kind;
                } else {
                    measure.end_bar_line = *kind;
                }
            }
        }
        MetaEvent::Section => {
            if let Some(measure) = score.measures.get_mut(measure_index) {
                measure.section_break_before = true;
            }
        }
        MetaEvent::Break => {
            let index = slots
                .iter()
                .position(|slot| slot.start + slot.len == tick)
                .unwrap_or(measure_index);
            if let Some(measure) = score.measures.get_mut(index) {
                measure.line_break = true;
            }
        }
        MetaEvent::Partial(_) | MetaEvent::MeasureLength(_) | MetaEvent::Cadenza(_) => {}
    }
}

// ----------------------------------------------------------------------
// Spanners, reprises, paroles, liaisons
// ----------------------------------------------------------------------

/// Place les spanners et annotations.
fn place_spanners(score: &mut Score, staves: &[StaffBuild]) {
    for staff in staves {
        for voice in &staff.voices {
            let mut open_slurs: BTreeMap<u8, Fraction> = BTreeMap::new();
            let mut hairpin: Option<(Fraction, HairpinType)> = None;
            let mut trill: Option<Fraction> = None;
            let mut ottava: Option<(Fraction, i8)> = None;
            let mut pedal: Option<Fraction> = None;
            let mut pending_glissando: Option<Fraction> = None;

            let mut events: Vec<&VoiceEvent> = voice.interpreted.events.iter().collect();
            events.sort_by_key(|event| (event.tick, event.order));
            for event in events {
                let tick = event.tick;
                match &event.kind {
                    EventKind::Chord(_) => {
                        if let Some(start) = pending_glissando.take() {
                            add_spanner(
                                score,
                                SpannerType::Glissando,
                                start,
                                tick,
                                voice.track,
                                |spanner| {
                                    spanner.notation = Some(SpannerNotation::Glissando(
                                        Glissando::new(GlissandoType::Straight(LineStyle::Solid)),
                                    ));
                                },
                            );
                        }
                    }
                    EventKind::SlurStart(number) => {
                        open_slurs.insert(*number, tick);
                    }
                    EventKind::SlurStop(number) => {
                        if let Some(start) = open_slurs.remove(number) {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Slur,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| {
                                        spanner.notation = Some(SpannerNotation::Slur(
                                            scoretk_core::model::Slur::default(),
                                        ));
                                    },
                                );
                            }
                        }
                    }
                    EventKind::HairpinStart(kind) => {
                        hairpin.get_or_insert((tick, *kind));
                    }
                    EventKind::HairpinStop => {
                        if let Some((start, kind)) = hairpin.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Hairpin(kind),
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| spanner.hairpin = Some(Hairpin::new()),
                                );
                            }
                        }
                    }
                    EventKind::TrillStart => trill = Some(tick),
                    EventKind::TrillStop => {
                        if let Some(start) = trill.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Trill,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| {
                                        spanner.notation = Some(SpannerNotation::Trill(
                                            Trill::new(TrillType::TrillLine),
                                        ));
                                    },
                                );
                            }
                        }
                    }
                    EventKind::Ottava(shift) => {
                        if *shift == 0 {
                            if let Some((start, shift)) = ottava.take() {
                                if start < tick {
                                    if let Some(kind) = ottava_type(shift) {
                                        add_spanner(
                                            score,
                                            SpannerType::Ottava(kind),
                                            start,
                                            tick,
                                            voice.track,
                                            |_| {},
                                        );
                                    }
                                }
                            }
                        } else {
                            ottava = Some((tick, *shift));
                        }
                    }
                    EventKind::Pedal(on) => {
                        if *on {
                            pedal = Some(tick);
                        } else if let Some(start) = pedal.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Pedal,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| spanner.pedal = Some(PedalInfo::new()),
                                );
                            }
                        }
                    }
                    EventKind::Glissando => pending_glissando = Some(tick),
                    EventKind::Annotation(annotation) => {
                        place_annotation(score, tick, voice.track, annotation);
                    }
                    EventKind::Breath(_) | EventKind::Meta(_) => {}
                }
            }
        }
    }
}

/// Ajoute un spanner au modèle.
fn add_spanner(
    score: &mut Score,
    kind: SpannerType,
    start: Fraction,
    end: Fraction,
    track: Track,
    configure: impl FnOnce(&mut Spanner),
) {
    let mut spanner = Spanner::new(
        kind,
        SpannerAnchor::new(start, track),
        SpannerAnchor::new(end, track),
    );
    configure(&mut spanner);
    score.add_spanner(spanner);
}

/// Place une annotation sur le segment du tick.
fn place_annotation(score: &mut Score, tick: Fraction, track: Track, annotation: &AnnotationEvent) {
    let Some(measure_index) = score
        .measures
        .partition_point(|measure| measure.start_tick <= tick)
        .checked_sub(1)
    else {
        return;
    };
    let segment_index = ensure_segment(score, measure_index, SegmentType::ChordRest, tick);
    let value = match annotation {
        AnnotationEvent::Dynamic {
            text,
            placement,
            custom,
        } => Annotation::Dynamic(Dynamic {
            text: text.clone(),
            velocity: None,
            placement: *placement,
            custom: *custom,
            visible: true,
        }),
        AnnotationEvent::Tempo {
            text,
            beats_per_minute,
        } => Annotation::Tempo(Tempo {
            text: RichText::new(text.clone()),
            beats_per_second: beats_per_minute.map(|value| value / 60.0),
            follow_text: false,
            placement: Placement::Above,
            visible: true,
            kind: Default::default(),
        }),
        AnnotationEvent::Mark(text) => Annotation::RehearsalMark(RehearsalMark::new(text.clone())),
        AnnotationEvent::Text { text, placement } => {
            let mut staff_text = scoretk_core::model::StaffText::new(text.clone());
            staff_text.placement = *placement;
            Annotation::StaffText(staff_text)
        }
    };
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.add_annotation(track, value);
    }
}

/// Place les reprises et volatas.
fn place_repeats(score: &mut Score, staves: &[StaffBuild]) {
    let mut sections: Vec<RepeatSection> = Vec::new();
    if let Some(staff) = staves.first() {
        for voice in &staff.voices {
            sections.extend(voice.interpreted.repeats.iter().cloned());
        }
    }
    sections.sort_by_key(|section| section.start);
    sections.dedup_by_key(|section| section.start);

    for section in &sections {
        if section.count < 2 {
            continue;
        }
        let Some(start_index) = score
            .measures
            .iter()
            .position(|measure| measure.start_tick == section.start)
        else {
            continue;
        };
        let end_index = score
            .measures
            .iter()
            .position(|measure| measure.start_tick + measure.len == section.end)
            .or_else(|| {
                score
                    .measures
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, measure)| measure.start_tick < section.end)
                    .map(|(index, _)| index)
            });
        if let Some(measure) = score.measures.get_mut(start_index) {
            if measure.start_bar_line == BarLineType::Normal {
                measure.start_bar_line = BarLineType::StartRepeat;
            }
        }
        if let Some(end_index) = end_index {
            if let Some(measure) = score.measures.get_mut(end_index) {
                measure.end_bar_line = BarLineType::EndRepeat;
                measure.repeat_count = section.count;
            }
        }
        // LilyPond rejoue chaque bloc `\alternative` aux passes qui lui
        // reviennent : avec N lectures et k blocs, le bloc i (à partir de 1) se
        // joue aux passes i, i+k, i+2k… tant qu'elles ne dépassent pas N.
        // C'est la règle que MuseScore stocke dans `<endings>` (par exemple
        // `1, 3` pour le premier bloc d'un `\repeat volta 3` à deux blocs).
        let blocks = u32::try_from(section.alternatives.len())
            .unwrap_or(1)
            .max(1);
        let plays = u32::from(section.count);
        for (index, alternative) in section.alternatives.iter().enumerate() {
            let mut endings = Vec::new();
            let mut pass = u32::try_from(index).unwrap_or(0) + 1;
            while pass <= plays {
                endings.push(u8::try_from(pass).unwrap_or(u8::MAX));
                pass += blocks;
            }
            if endings.is_empty() {
                endings.push(alternative.number);
            }
            let mut volta = Spanner::new(
                SpannerType::Volta,
                SpannerAnchor::new(alternative.start, Track::from_raw(0)),
                SpannerAnchor::new(alternative.end, Track::from_raw(0)),
            );
            volta.volta = Some(scoretk_core::model::VoltaInfo {
                endings,
                closed: true,
            });
            score.add_spanner(volta);
        }
    }
}

/// Attache les paroles aux accords des voix ciblées.
fn place_lyrics(score: &mut Score, staves: &[StaffBuild]) {
    for staff in staves {
        for lyrics in &staff.lyrics {
            let voice = staff
                .voices
                .iter()
                .find(|voice| {
                    !lyrics.target.is_empty()
                        && voice
                            .name
                            .as_deref()
                            .is_some_and(|name| name == lyrics.target)
                })
                .or_else(|| staff.voices.first());
            let Some(voice) = voice else {
                continue;
            };
            attach_lyrics(score, voice, lyrics.verse, &lyrics.syllables);
        }
    }
}

/// Attache les syllabes aux accords d'une voix.
fn attach_lyrics(
    score: &mut Score,
    voice: &VoiceBuild,
    verse: usize,
    syllables: &[crate::ast::Syllable],
) {
    let mut chords = Vec::new();
    for event in &voice.interpreted.events {
        if let EventKind::Chord(chord) = &event.kind {
            if matches!(chord.content, ChordContent::Notes(_)) {
                chords.push(event.tick);
            }
        }
    }
    let mut previous_hyphen = false;
    for (tick, syllable) in chords.iter().zip(syllables) {
        if syllable.skip || syllable.text.is_empty() {
            previous_hyphen = false;
            continue;
        }
        let syllabic = if syllable.hyphen {
            if previous_hyphen {
                LyricsSyllabic::Middle
            } else {
                LyricsSyllabic::Begin
            }
        } else if previous_hyphen {
            LyricsSyllabic::End
        } else {
            LyricsSyllabic::Single
        };
        previous_hyphen = syllable.hyphen;
        let value = Lyrics {
            verse,
            syllabic,
            text: syllable.text.clone(),
            melisma: syllable.extender.then_some(Fraction::ONE),
            placement: Placement::Below,
        };
        let Some(measure_index) = score
            .measures
            .partition_point(|measure| measure.start_tick <= *tick)
            .checked_sub(1)
        else {
            continue;
        };
        for segment in &mut score.measures[measure_index].segments {
            if segment.tick != *tick || segment.kind != SegmentType::ChordRest {
                continue;
            }
            if let Some(chord_rest) = segment.chord_rest_mut(voice.track) {
                chord_rest.lyrics.push(value);
                break;
            }
        }
    }
}

/// Ajoute les altérations affichées d'après l'armure et la mesure.
///
/// MuseScore stocke un objet `Accidental` pour chaque altération affichée ;
/// LilyPond, lui, laisse l'éditeur décider. On applique donc la règle de
/// gravure : une note porte une altération quand son `tpc` écrit diffère de
/// l'altération en vigueur pour ce degré et cette octave (armure au début de
/// la mesure, dernière altération affichée ensuite).
fn apply_accidentals(score: &mut Score) {
    let staff_count = score.staves.len();
    for staff_index in 0..staff_count {
        let staff = StaffId::from_index(staff_index);
        let track = Track::new(staff, 0);
        let mut key_fifths = 0i8;
        for measure_index in 0..score.measures.len() {
            // Armure en vigueur au début de la mesure.
            for segment in &score.measures[measure_index].segments {
                if !segment.kind.is_key_sig_type()
                    || segment.tick > score.measures[measure_index].start_tick
                {
                    continue;
                }
                if let Some(SegmentItem::KeySig(key)) = segment.element(track) {
                    key_fifths = key.transposed_key.fifths();
                }
            }
            let mut state: BTreeMap<(u8, i8), i8> = BTreeMap::new();
            let mut positions: Vec<(Fraction, usize)> = score.measures[measure_index]
                .segments
                .iter()
                .enumerate()
                .filter(|(_, segment)| segment.kind == SegmentType::ChordRest)
                .map(|(index, segment)| (segment.tick, index))
                .collect();
            positions.sort();
            for (_, segment_index) in positions {
                for voice in 0..VOICES {
                    let note_ids: Vec<NoteId> = score.measures[measure_index].segments
                        [segment_index]
                        .chord_rest(Track::new(staff, u8::try_from(voice).unwrap_or(0)))
                        .and_then(ChordRest::chord_ref)
                        .map(|chord| chord.notes.clone())
                        .unwrap_or_default();
                    for id in note_ids {
                        let note = score.note(id);
                        let tpc = note.tpc_transposed;
                        if !(0..=40).contains(&tpc) {
                            continue;
                        }
                        // `tpc2step` de MuseScore : la spirale des quintes
                        // parcourt fa, do, sol, ré, la, mi, si.
                        let index =
                            usize::try_from((i32::from(tpc) + 8).rem_euclid(7)).unwrap_or(0);
                        let step = [3u8, 0, 4, 1, 5, 2, 6][index];
                        let alter = ((i32::from(tpc) + 8) / 7 - 3) as i8;
                        let octave = (i32::from(note.pitch) - i32::from(alter)).div_euclid(12) - 1;
                        let octave = i8::try_from(octave).unwrap_or(4);
                        let current = state
                            .get(&(step, octave))
                            .copied()
                            .unwrap_or_else(|| key_alter(key_fifths, step));
                        if alter == current {
                            continue;
                        }
                        state.insert((step, octave), alter);
                        if note.accidental.is_some() {
                            continue;
                        }
                        let kind = match alter {
                            -2 => AccidentalType::DoubleFlat,
                            -1 => AccidentalType::Flat,
                            0 => AccidentalType::Natural,
                            1 => AccidentalType::Sharp,
                            2 => AccidentalType::DoubleSharp,
                            _ => continue,
                        };
                        score.note_mut(id).accidental = Some(Accidental {
                            kind,
                            bracket: false,
                            parentheses: false,
                            visible: true,
                            user: false,
                        });
                    }
                }
            }
        }
    }
}

/// Altération d'un degré naturel donnée par une armure.
fn key_alter(fifths: i8, step: u8) -> i8 {
    let sharps = [3u8, 0, 4, 1, 5, 2, 6];
    let flats = [6u8, 2, 5, 1, 4, 0, 3];
    if fifths > 0 {
        let count = usize::try_from(fifths).unwrap_or(0).min(7);
        if sharps[..count].contains(&step) {
            return 1;
        }
    } else if fifths < 0 {
        let count = usize::try_from(-fifths).unwrap_or(0).min(7);
        if flats[..count].contains(&step) {
            return -1;
        }
    }
    0
}

/// Relie les notes de prolongation.
fn link_ties(score: &mut Score, staves: &[StaffBuild]) {
    for staff in staves {
        for voice in &staff.voices {
            let mut sequence: Vec<(Fraction, NoteId, bool)> = Vec::new();
            for event in &voice.interpreted.events {
                let EventKind::Chord(chord) = &event.kind else {
                    continue;
                };
                let ChordContent::Notes(notes) = &chord.content else {
                    continue;
                };
                let Some(segment) = score
                    .measures
                    .iter()
                    .find(|measure| {
                        measure.start_tick <= event.tick
                            && event.tick < measure.start_tick + measure.len
                    })
                    .and_then(|measure| {
                        measure.segments.iter().find(|segment| {
                            segment.tick == event.tick && segment.kind == SegmentType::ChordRest
                        })
                    })
                else {
                    continue;
                };
                let ids: Vec<NoteId> = segment
                    .chord_rest(voice.track)
                    .and_then(ChordRest::chord_ref)
                    .map(|chord| chord.notes.clone())
                    .unwrap_or_default();
                for (note, id) in notes.iter().zip(ids) {
                    sequence.push((event.tick, id, note.tie));
                }
            }
            sequence.sort_by_key(|(tick, _, _)| *tick);
            let mut open: BTreeMap<u8, NoteId> = BTreeMap::new();
            for (_, id, tie) in sequence {
                let pitch = score.note(id).pitch;
                if let Some(previous) = open.remove(&pitch) {
                    score.note_mut(previous).tie_forward = Some(id);
                    score.note_mut(id).tie_back = Some(previous);
                }
                if tie {
                    open.insert(pitch, id);
                }
            }
        }
    }
}
