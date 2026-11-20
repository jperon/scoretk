// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/twrite.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026 :
// écriture des éléments (accords, silences, notes, notations, annotations).
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

//! Écriture des objets d'un segment et de leurs notations.
//!
//! Les noms d'éléments et de propriétés sont ceux du `.mscx` lus par
//! `scoretk_musescore_import::read` : `TWrite::writeItem(BarLine*)`,
//! `TWrite::writeItem(Chord*)`, `TWrite::writeItem(Note*)`,
//! `TWrite::writeProperties` pour les annotations.

use scoretk_core::model::tpc;
use scoretk_core::model::{
    Annotation, Arpeggio, Articulation, BarLine, BarLineType, BeamMode, Breath, Chord, ChordRest,
    Clef, Duration, DurationType, Fermata, FiguredBass, Fingering, Fraction, GraceChord, Hairpin,
    HairpinType, Harmony, Instrument, Key, KeyMode, KeySig, Lyrics, Note, NoteHeadGroup,
    NoteHeadType, Ornament, PedalInfo, Placement, RehearsalFrame, SegmentItem, SpannerNotation,
    StaffText, StemDirection, SwingUnit, Tempo, TempoKind, TextLineInfo, TimeSig, TimeSigType,
    Tremolo, TremoloBar, TremoloKind, Tuplet, TupletNumberType, VoltaInfo,
};

use crate::names;
use crate::text::{fraction, markup, number};
use crate::write::context::WriteContext;
use crate::xmlwriter::XmlWriter;

use scoretk_core::model::{NoteId, Tick, Track};

/// Écrit le contenu d'un élément de segment de système.
///
/// Les accords et silences passent par [`write_chord_rest`], qui a besoin de la
/// position de leurs notes pour les liaisons.
pub(crate) fn write_item(xml: &mut XmlWriter, item: &SegmentItem, track: Track) {
    match item {
        SegmentItem::ChordRest(_) => {}
        SegmentItem::KeySig(key_sig) => write_key_sig(xml, key_sig),
        SegmentItem::TimeSig(time_sig) => write_time_sig(xml, time_sig),
        SegmentItem::Clef(clef) => write_clef(xml, clef, false),
        SegmentItem::BarLine(bar_line) => write_bar_line(xml, bar_line),
        SegmentItem::Breath(breath) => write_breath(xml, breath, track),
    }
}

/// Écrit un accord ou un silence, avec ses accords d'agrément.
///
/// Les accords d'agrément sont des enfants du même `<voice>` dans le `.mscx`
/// (`MeasureRead::readVoice` les rattache à l'accord suivant) : ils sont donc
/// écrits avant lui, sans avancer le tick.
pub(crate) fn write_chord_rest(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    chord_rest: &ChordRest,
    tick: Tick,
    track: Track,
) {
    if let Some(chord) = chord_rest.chord_ref() {
        for grace in &chord.grace_chords {
            write_grace_chord(xml, ctx, chord_rest, grace, tick, track);
        }
        xml.start_element("Chord", &[]);
        write_duration(xml, &chord_rest.duration);
        if let Some(ticks) = chord_rest.ticks {
            xml.tag("duration", &fraction(ticks));
        }
        if chord_rest.beam_mode != BeamMode::Auto {
            xml.tag("BeamMode", names::beam_mode(chord_rest.beam_mode));
        }
        if chord.stem != StemDirection::Auto {
            xml.tag("StemDirection", stem_name(chord.stem));
        }
        if chord.staff_move != 0 {
            xml.tag("staffMove", &chord.staff_move.to_string());
        }
        if chord.small {
            xml.tag("small", "1");
        }
        write_chord_contents(xml, ctx, chord_rest, chord, tick, track);
        xml.end_element();
        return;
    }

    let rest = chord_rest.rest_ref().cloned().unwrap_or_default();
    xml.start_element("Rest", &[]);
    write_duration(xml, &chord_rest.duration);
    if let Some(ticks) = chord_rest.ticks {
        xml.tag("duration", &fraction(ticks));
    }
    if rest.gap {
        xml.tag("gap", "1");
    }
    if !rest.visible {
        xml.tag("visible", "0");
    }
    write_lyrics(xml, &chord_rest.lyrics);
    xml.end_element();
}

/// Écrit un accord d'agrément (`<Chord><appoggiatura/>…`).
fn write_grace_chord(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    parent: &ChordRest,
    grace: &GraceChord,
    tick: Tick,
    track: Track,
) {
    xml.start_element("Chord", &[]);
    write_duration(xml, &grace.duration);
    if let Some(kind) = grace.chord.grace {
        xml.tag_empty_plain(names::grace_tag(kind));
    }
    if grace.chord.grace_slash {
        xml.tag("graceSlash", "1");
    }
    if grace.chord.small {
        xml.tag("small", "1");
    }
    if grace.chord.stem != StemDirection::Auto {
        xml.tag("StemDirection", stem_name(grace.chord.stem));
    }
    write_chord_contents(xml, ctx, parent, &grace.chord, tick, track);
    xml.end_element();
}

/// Nom MuseScore d'une direction de hampe.
fn stem_name(stem: StemDirection) -> &'static str {
    match stem {
        StemDirection::Up => "up",
        StemDirection::Down => "down",
        StemDirection::Auto => "auto",
    }
}

/// Écrit `<dots>` et `<durationType>` d'une durée.
fn write_duration(xml: &mut XmlWriter, duration: &Duration) {
    if duration.dots > 0 {
        xml.tag("dots", &duration.dots.to_string());
    }
    xml.tag("durationType", names::duration_type(duration.kind));
}

/// Écrit le contenu d'un accord : notes, notations et paroles.
fn write_chord_contents(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    chord_rest: &ChordRest,
    chord: &Chord,
    tick: Tick,
    track: Track,
) {
    for note in &chord.notes {
        write_note(xml, ctx, ctx.score.note(*note), tick, track);
    }
    if let Some(tremolo) = chord.tremolo {
        write_tremolo(xml, tremolo);
    }
    for articulation in &chord.articulations {
        write_articulation(xml, articulation);
    }
    for ornament in &chord.ornaments {
        write_ornament(xml, ornament);
    }
    if let Some(arpeggio) = &chord.arpeggio {
        write_arpeggio(xml, arpeggio);
    }
    if let Some(chord_line) = chord.chord_line {
        xml.start_element("ChordLine", &[]);
        xml.tag("subtype", names::chord_line_type(chord_line));
        xml.end_element();
    }
    write_lyrics(xml, &chord_rest.lyrics);
}

/// Écrit une note et ses notations.
fn write_note(xml: &mut XmlWriter, ctx: &WriteContext<'_>, note: &Note, tick: Tick, track: Track) {
    xml.start_element("Note", &[]);
    if let Some(accidental) = &note.accidental {
        xml.start_element("Accidental", &[]);
        if let Some(subtype) = names::accidental_type(accidental.kind) {
            xml.tag("subtype", &subtype);
        }
        if accidental.bracket {
            xml.tag("bracket", "1");
        }
        if accidental.parentheses {
            xml.tag("parens", "1");
        }
        if accidental.user {
            // `AccidentalRole::USER` : l'export écrit `cautionary="yes"`.
            xml.tag("role", "1");
        }
        if !accidental.visible {
            xml.tag("visible", "0");
        }
        xml.end_element();
    }
    if note.head_group != NoteHeadGroup::Normal {
        if let Some(head) = names::note_head(&note.head_group) {
            xml.tag("head", &head);
        }
    }
    if note.head != NoteHeadType::Auto {
        if let Some(head) = names::head_type(note.head) {
            xml.tag("headType", head);
        }
    }
    xml.tag("pitch", &note.pitch.to_string());
    xml.tag("tpc", &note.tpc.to_string());
    if note.tpc_transposed != note.tpc {
        xml.tag("tpc2", &note.tpc_transposed.to_string());
    }
    if !note.visible {
        xml.tag("visible", "0");
    }
    if note.parentheses {
        // Port de `TWrite::writeProperties(const Note*)`
        // (`write/twrite.cpp:690-697`) : `Pid::HAS_PARENTHESES` s'écrit
        // `<parentheses>both</parentheses>` (table `typesconv.cpp:952-958`).
        // Sans lui, l'export MusicXML de MuseScore 4.6+
        // (`importmusicxml.cpp:3883-3902`) ignore le seul
        // `<Symbol name="noteheadParenthesis">` et n'écrit pas
        // `<notehead parentheses="yes">`. Le Symbol reste pour le lecteur
        // de scoretk, qui s'en sert pour positionner `Note::parentheses`.
        xml.tag("parentheses", "both");
        xml.start_element("Symbol", &[]);
        xml.tag("name", "noteheadParenthesis");
        xml.end_element();
    }
    if note.ghost {
        // Port de `TWrite::write(const Note*)` (`write/twrite.cpp:2500-2505`) :
        // MuseScore écrit `Pid::GHOST` et `Pid::DEAD` séparément. Le modèle
        // pivot confond les deux en `Note::ghost` ; l'export MusicXML de
        // scoretk ne met pas de parenthèses sur ces têtes, alors que `ghost()`
        // (`engravingitem.cpp:1688`) en fait mettre une par MuseScore. Écrire
        // `<dead>` conserve la parité avec l'export direct.
        xml.tag("dead", "1");
    }
    if !note.play {
        xml.tag("play", "0");
    }
    for fingering in &note.fingerings {
        write_fingering(xml, fingering);
    }
    if let Some(start) = note.tie_back {
        write_tie_half(
            xml,
            ctx,
            start,
            tick,
            track,
            (false, ctx.note_grace_index(start)),
            (None, true),
        );
    }
    if let Some(end) = note.tie_forward {
        write_tie_half(
            xml,
            ctx,
            end,
            tick,
            track,
            (true, ctx.note_grace_index(end)),
            (note.tie_up, note.tie_forward_visible),
        );
    }
    xml.end_element();
}

/// Écrit une moitié de liaison de prolongation (`<Spanner type="Tie">`).
///
/// La moitié de départ porte `<Tie>` (direction et visibilité) et `<next>`, la
/// moitié d'arrivée `<prev>` seulement ; les deux portent la position relative
/// de l'autre extrémité (`ConnectorInfoReader`).
fn write_tie_half(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    other: NoteId,
    tick: Tick,
    track: Track,
    end: (bool, Option<i64>),
    (tie_up, visible): (Option<bool>, bool),
) {
    let Some((other_tick, other_track)) = ctx.note_position(other) else {
        return;
    };
    xml.start_element("Spanner", &[("type", "Tie")]);
    if end.0 {
        xml.start_element("Tie", &[]);
        if let Some(up) = tie_up {
            xml.tag("up", if up { "up" } else { "down" });
        }
        if !visible {
            xml.tag("visible", "0");
        }
        xml.end_element();
    }
    write_location_element(xml, ctx.score, other_tick, tick, other_track, track, end);
    xml.end_element();
}

/// Indice de mesure et position dans la mesure d'un tick.
fn measure_location(score: &scoretk_core::model::Score, tick: Tick) -> (i64, Fraction) {
    let last = score.measures.len().saturating_sub(1);
    for (index, measure) in score.measures.iter().enumerate() {
        if tick < measure.start_tick + measure.len || index == last {
            return (index as i64, tick - measure.start_tick);
        }
    }
    (0, tick)
}

/// Écrit `<next>`/`<prev>` avec la `<location>` relative de l'autre extrémité.
pub(crate) fn write_location_element(
    xml: &mut XmlWriter,
    score: &scoretk_core::model::Score,
    other_tick: Tick,
    tick: Tick,
    other_track: Track,
    track: Track,
    end: (bool, Option<i64>),
) {
    xml.start_element(if end.0 { "next" } else { "prev" }, &[]);
    xml.start_element("location", &[]);
    // Port de `SpannerWriter::fillSpannerPosition`
    // (`write/connectorinfowriter.cpp:82-96`) et de `TWrite::write(Location*)`
    // (`write/twrite.cpp:2355-2368`) : `measures` et `fractions` sont deux
    // propriétés distinctes. Un décalage d'une mesure entière doit rester
    // `<measures>1</measures>` ; MuseScore ne peut pas le reconstituer depuis
    // `<fractions>1/1</fractions>` et raccourcit alors l'ottava d'une mesure.
    let (measure, frac) = measure_location(score, tick);
    let (other_measure, other_frac) = measure_location(score, other_tick);
    let measures = other_measure - measure;
    let delta = other_frac - frac;
    if measures != 0 {
        xml.tag("measures", &measures.to_string());
    }
    if !delta.is_zero() {
        xml.tag("fractions", &fraction(delta));
    }
    // `TWrite::write(const Location*)` (`write/twrite.cpp:2365`) écrit
    // `Pid::LOCATION_GRACE` (`<grace>`) ; sans lui, une liaison dont une
    // extrémité est une note d'agrément perd son ancre à la réouverture
    // (`ConnectorInfoReader`, `read460/connectorinforeader.cpp:346-360`).
    if let Some(grace) = end.1 {
        xml.tag("grace", &grace.to_string());
    }
    let delta_track = i64::from(other_track.raw()) - i64::from(track.raw());
    let voices = delta_track.rem_euclid(4);
    let staves = (delta_track - voices) / 4;
    if staves != 0 {
        xml.tag("staves", &staves.to_string());
    }
    if voices != 0 {
        xml.tag("voices", &voices.to_string());
    }
    xml.end_element();
    xml.end_element();
}

/// Écrit une articulation.
fn write_articulation(xml: &mut XmlWriter, articulation: &Articulation) {
    xml.start_element("Articulation", &[]);
    xml.tag(
        "subtype",
        &names::articulation(&articulation.kind, articulation.up),
    );
    let anchor = names::articulation_anchor(articulation.anchor);
    if anchor != 0 {
        xml.tag("anchor", &anchor.to_string());
    }
    if !articulation.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit un ornement.
fn write_ornament(xml: &mut XmlWriter, ornament: &Ornament) {
    xml.start_element("Ornament", &[]);
    xml.tag("subtype", &names::ornament_type(&ornament.kind));
    // `TWrite::writeProperties(Ornament*)` (`write/twrite.cpp`) écrit les
    // propriétés de style, dont `INTERVAL_ABOVE`/`INTERVAL_BELOW`.
    if let Some(interval) = &ornament.interval_above {
        xml.tag("intervalAbove", interval);
    }
    if let Some(interval) = &ornament.interval_below {
        xml.tag("intervalBelow", interval);
    }
    if let Some(placement) = ornament.placement {
        xml.tag("placement", placement_name(placement));
    }
    for accidental in &ornament.accidentals {
        xml.start_element("Accidental", &[]);
        if let Some(subtype) = names::accidental_type(accidental.accidental.kind) {
            xml.tag("subtype", &subtype);
        }
        if accidental.accidental.bracket {
            xml.tag("bracket", "1");
        }
        if accidental.accidental.parentheses {
            xml.tag("parentheses", "1");
        }
        xml.tag("placement", placement_name(accidental.placement));
        xml.end_element();
    }
    if !ornament.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit un trémolo.
///
/// Un trémolo à deux notes s'écrit avec le préfixe `c` (`c8`) sur ses deux
/// accords, un trémolo simple avec `r` (`r8`), exactement comme MuseScore 4.x.
fn write_tremolo(xml: &mut XmlWriter, tremolo: Tremolo) {
    let Some(figure) = names::tremolo_figure(tremolo.count) else {
        return;
    };
    let double = matches!(tremolo.kind, TremoloKind::Start | TremoloKind::Stop);
    let subtype = if double {
        format!("c{figure}")
    } else {
        format!("r{figure}")
    };
    xml.start_element("Tremolo", &[]);
    xml.tag("subtype", &subtype);
    xml.end_element();
}

/// Écrit un arpège.
fn write_arpeggio(xml: &mut XmlWriter, arpeggio: &Arpeggio) {
    xml.start_element("Arpeggio", &[]);
    xml.tag("subtype", names::arpeggio_type(arpeggio.kind));
    if !arpeggio.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit un doigté.
fn write_fingering(xml: &mut XmlWriter, fingering: &Fingering) {
    xml.start_element("Fingering", &[]);
    xml.tag("text", &fingering.text);
    if let Some(placement) = fingering.placement {
        xml.tag("placement", placement_name(placement));
    }
    if !fingering.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit les paroles d'un accord.
fn write_lyrics(xml: &mut XmlWriter, lyrics: &[Lyrics]) {
    for verse in lyrics {
        xml.start_element("Lyrics", &[]);
        if verse.verse != 0 {
            xml.tag("no", &verse.verse.to_string());
        }
        if let Some(syllabic) = names::syllabic(verse.syllabic) {
            xml.tag("syllabic", syllabic);
        }
        xml.tag("text", &verse.text);
        if let Some(melisma) = verse.melisma {
            xml.tag("ticks_f", &fraction(melisma));
        }
        xml.tag("placement", placement_name(verse.placement));
        xml.end_element();
    }
}

/// Écrit une armure.
fn write_key_sig(xml: &mut XmlWriter, key_sig: &KeySig) {
    xml.start_element("KeySig", &[]);
    xml.tag("accidental", &key_sig.concert_key.fifths().to_string());
    xml.tag("concertKey", &key_sig.concert_key.fifths().to_string());
    xml.tag("actualKey", &key_sig.transposed_key.fifths().to_string());
    if key_sig.mode != KeyMode::Unknown {
        xml.tag("mode", names::key_mode(key_sig.mode));
    }
    if key_sig.courtesy {
        xml.tag("isCourtesy", "1");
    }
    if !key_sig.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit une indication de mesure.
fn write_time_sig(xml: &mut XmlWriter, time_sig: &TimeSig) {
    xml.start_element("TimeSig", &[]);
    if time_sig.kind != TimeSigType::Normal {
        xml.tag(
            "subtype",
            &names::time_sig_subtype(time_sig.kind).to_string(),
        );
    }
    xml.tag("sigN", &time_sig.numerator.to_string());
    xml.tag("sigD", &time_sig.denominator.to_string());
    if time_sig.stretch != Fraction::ONE {
        xml.tag("stretchN", &time_sig.stretch.numerator().to_string());
        xml.tag("stretchD", &time_sig.stretch.denominator().to_string());
    }
    if time_sig.courtesy {
        xml.tag("isCourtesy", "1");
    }
    if !time_sig.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit une clef.
///
/// Port de `TWrite::write(const Clef*)` (`write/twrite.cpp:1281-1299`) : les
/// deux propriétés `CLEF_TYPE_CONCERT` et `CLEF_TYPE_TRANSPOSING` sont écrites.
/// Le lecteur (`read400/tread.cpp:2551-2553`) force la clé en G quand le type
/// transposant manque, même si la clé concert est F : l'omettre ferait rouvrir
/// une portée de basse en clé de sol. `isHeader` fixe le type de segment.
pub(crate) fn write_clef(xml: &mut XmlWriter, clef: &Clef, header: bool) {
    xml.start_element("Clef", &[]);
    if header {
        xml.tag("isHeader", "1");
    }
    xml.tag("concertClefType", names::clef_type(clef.concert));
    xml.tag("transposingClefType", names::clef_type(clef.transposed));
    xml.end_element();
}

/// Écrit une barre de mesure.
pub(crate) fn write_bar_line(xml: &mut XmlWriter, bar_line: &BarLine) {
    xml.start_element("BarLine", &[]);
    if bar_line.kind != BarLineType::Normal {
        xml.tag("subtype", names::bar_line_type(bar_line.kind));
    }
    if bar_line.span_staff {
        xml.tag("span", "1");
    }
    if bar_line.span_from != 0 {
        xml.tag("spanFromOffset", &bar_line.span_from.to_string());
    }
    if bar_line.span_to != 0 {
        xml.tag("spanToOffset", &bar_line.span_to.to_string());
    }
    xml.end_element();
}

/// Écrit une respiration (`TWrite::write(const Breath*)`,
/// `rw/write/twrite.cpp:1043-1052`).
///
/// La propriété `Pid::SYMBOL` et la propriété `Pid::PAUSE` sont écrites si
/// elles diffèrent de leur défaut ; le modèle ne porte pas la pause, qui n'a
/// aucun effet dans `engraving` (seul le module *playback*, absent du
/// snapshot, la consomme).
fn write_breath(xml: &mut XmlWriter, breath: &Breath, track: Track) {
    xml.start_element("Breath", &[]);
    if let Some(symbol) = names::breath_symbol(breath.kind) {
        xml.tag("symbol", symbol);
    }
    // `Breath::propertyDefault(Pid::PLACEMENT)` (breath.cpp:135-136) :
    // piste paire → ABOVE, piste impaire → BELOW. On n'écrit `<placement>`
    // que s'il diffère de ce défaut.
    let default_placement = if track.raw() & 1 == 1 {
        Placement::Below
    } else {
        Placement::Above
    };
    if breath.placement != default_placement {
        xml.tag("placement", placement_name(breath.placement));
    }
    xml.end_element();
}

/// Nom MuseScore d'un placement.
pub(crate) fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

/// Écrit une annotation de segment.
pub(crate) fn write_annotation(
    xml: &mut XmlWriter,
    annotation: &Annotation,
    track: Track,
    score: &scoretk_core::model::Score,
    tick: Tick,
) {
    match annotation {
        Annotation::Dynamic(dynamic) => {
            xml.start_element("Dynamic", &[]);
            if dynamic.custom {
                xml.tag("subtype", "other-dynamics");
                if !dynamic.text.is_empty() {
                    xml.tag("text", &dynamic.text);
                }
            } else {
                xml.tag("subtype", &dynamic.text);
            }
            if let Some(velocity) = dynamic.velocity {
                xml.tag("velocity", &velocity.to_string());
            }
            xml.tag("placement", placement_name(dynamic.placement));
            if !dynamic.visible {
                xml.tag("visible", "0");
            }
            xml.end_element();
        }
        Annotation::StaffText(text) => write_text_base(xml, "StaffText", text),
        Annotation::Expression(text) => write_text_base(xml, "Expression", text),
        Annotation::SystemText(text) => {
            xml.start_element("SystemText", &[]);
            xml.tag("placement", placement_name(text.placement));
            if !text.visible {
                xml.tag("visible", "0");
            }
            if let Some(swing) = text.swing {
                let unit = match swing.unit {
                    SwingUnit::Straight => "straight",
                    SwingUnit::Eighth => "eighth",
                    SwingUnit::Sixteenth => "16th",
                };
                xml.tag_empty(
                    "swing",
                    &[("unit", unit), ("ratio", &swing.ratio.to_string())],
                );
            }
            xml.tag_raw("text", &markup(&text.text));
            xml.end_element();
        }
        Annotation::Tempo(tempo) => write_tempo(xml, tempo),
        Annotation::RehearsalMark(mark) => {
            xml.start_element("RehearsalMark", &[]);
            let frame = match mark.frame {
                RehearsalFrame::NoFrame => 0,
                RehearsalFrame::Rectangle => 1,
                RehearsalFrame::Circle => 2,
            };
            xml.tag("frameType", &frame.to_string());
            xml.tag("placement", placement_name(mark.placement));
            if !mark.visible {
                xml.tag("visible", "0");
            }
            xml.tag_raw("text", &markup(&mark.text));
            xml.end_element();
        }
        Annotation::InstrumentChange(change) => {
            xml.start_element("InstrumentChange", &[]);
            xml.tag("placement", placement_name(change.placement));
            if !change.visible {
                xml.tag("visible", "0");
            }
            if let Some(instrument) = &change.instrument {
                write_instrument(xml, instrument);
            }
            xml.tag_raw("text", &markup(&change.text));
            xml.end_element();
        }
        Annotation::Harmony(harmony) => {
            write_harmony(xml, harmony, key_at(score, tick, track));
        }
        Annotation::Fermata(fermata) => write_fermata(xml, fermata, track),
        Annotation::FiguredBass(bass) => write_figured_bass(xml, bass),
        Annotation::TremoloBar(bar) => write_tremolo_bar(xml, bar),
        Annotation::Other { .. } => {}
    }
}

/// Écrit un texte de portée (`StaffText`, `Expression`).
fn write_text_base(xml: &mut XmlWriter, name: &str, text: &StaffText) {
    xml.start_element(name, &[]);
    xml.tag("placement", placement_name(text.placement));
    if !text.visible {
        xml.tag("visible", "0");
    }
    xml.tag_raw("text", &markup(&text.text));
    xml.end_element();
}

/// Écrit une indication de tempo.
fn write_tempo(xml: &mut XmlWriter, tempo: &Tempo) {
    xml.start_element("Tempo", &[]);
    if let Some(beats_per_second) = tempo.beats_per_second {
        xml.tag("tempo", &number(beats_per_second));
    }
    if tempo.follow_text {
        xml.tag_empty_plain("followText");
    }
    let kind = match tempo.kind {
        TempoKind::Normal => None,
        TempoKind::ATempo => Some("aTempo"),
        TempoKind::TempoPrimo => Some("tempoPrimo"),
    };
    if let Some(kind) = kind {
        xml.tag("type", kind);
    }
    xml.tag("placement", placement_name(tempo.placement));
    if !tempo.visible {
        xml.tag("visible", "0");
    }
    xml.tag_raw("text", &markup(&tempo.text));
    xml.end_element();
}

/// Armure en vigueur pour une annotation, sur sa portée.
fn key_at(score: &scoretk_core::model::Score, tick: Tick, track: Track) -> Key {
    let mut key = Key::C;
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.tick > tick {
                return key;
            }
            if let Some(SegmentItem::KeySig(sig)) = segment.element(track) {
                key = sig.concert_key;
            }
        }
    }
    key
}

/// Racine d'un chiffrage de Nashville écrit par `<function>`.
///
/// Port de `function2Tpc` (`engraving/dom/pitchspelling.cpp:726-754`) et de
/// `tpcInterval` (`:658-669`) : `keyTpc = key + 14`, les degrés diatoniques
/// valent `0, 2, 4, -1, 1, 3, 5` et un altération vaut 7 tpcs. Le résultat est
/// ramené dans l'ambiguïté enharmonique par `clampEnharmonic` (`:929-937`).
fn root_from_function(function: &str, key: Key) -> Option<i8> {
    let chars: Vec<char> = function.chars().collect();
    let (alter, digit) = if chars.first().is_some_and(char::is_ascii_digit) {
        (0, chars[0])
    } else {
        let altered = chars.len() > 1;
        let digit = *chars.last()?;
        let prefix: String = if altered {
            chars[..chars.len() - 1].iter().collect()
        } else {
            String::new()
        };
        let alter = if prefix.starts_with("bb") {
            -2
        } else if prefix.starts_with("##") {
            2
        } else if prefix.starts_with('b') {
            -1
        } else if prefix.starts_with('#') {
            1
        } else {
            return None;
        };
        (alter, digit)
    };
    if !digit.is_ascii_digit() {
        return None;
    }
    let step = digit.to_digit(10)? as usize;
    if step == 0 {
        return None;
    }
    const INTERVALS: [i8; 7] = [0, 2, 4, -1, 1, 3, 5];
    let mut tpc = key.fifths() + 14 + INTERVALS[(step - 1) % 7] + alter * 7;
    while tpc > 27 {
        tpc -= 12;
    }
    while tpc < 1 {
        tpc += 12;
    }
    Some(tpc)
}

/// Écrit un symbole d'accord.
fn write_harmony(xml: &mut XmlWriter, harmony: &Harmony, key: Key) {
    xml.start_element("Harmony", &[]);
    // Port de `writeHarmonyInfo` (write/twrite.cpp:1786-1826) : depuis 4.6, le
    // lecteur (`read460/tread.cpp:2868-2879`) n'accepte les accords chiffrés que
    // dans un enfant `<harmonyInfo>`. Les balises directes sont conservées en
    // plus pour le lecteur de scoretk (`read4xx/mod.rs:1205`), qui n'a pas été
    // porté à `read460` : un fichier 4.70 doit rester lisible des deux côtés.
    // `writeHarmonyInfo` écrit `<root>` depuis `rootTpc`. Le pivot 3.x stocke
    // parfois seulement `<function>` (Nashville) : le lecteur de MuseScore
    // (`read410/tread.cpp:2839`) le convertit en `rootTpc` ; on fait de même
    // ici pour que le lecteur 4.6+ retrouve l'accord.
    let root = if harmony.root_tpc == tpc::INVALID {
        harmony
            .function
            .as_deref()
            .and_then(|function| root_from_function(function, key))
            .unwrap_or(tpc::INVALID)
    } else {
        harmony.root_tpc
    };
    let info: Vec<(&str, String)> = {
        let mut champs = Vec::new();
        champs.push(("name", harmony.name.clone()));
        if root != tpc::INVALID {
            champs.push(("root", root.to_string()));
        }
        if let Some(bass) = harmony.bass_tpc {
            champs.push(("bass", bass.to_string()));
        }
        champs
    };
    xml.start_element("harmonyInfo", &[]);
    for (tag, value) in &info {
        xml.tag(tag, value);
    }
    xml.end_element();
    for (tag, value) in &info {
        xml.tag(tag, value);
    }
    if harmony.kind != "normal" {
        let kind = match harmony.kind.as_str() {
            "roman" => "1",
            "nashville" => "2",
            _ => "0",
        };
        xml.tag("harmonyType", kind);
    }
    if let Some(function) = &harmony.function {
        xml.tag("function", function);
    }
    for degree in &harmony.degrees {
        xml.start_element("degree", &[]);
        xml.tag("degree-value", &degree.value.to_string());
        xml.tag("degree-alter", &degree.alter.to_string());
        xml.tag(
            "degree-type",
            match degree.kind {
                scoretk_core::model::HarmonyDegreeKind::Add => "add",
                scoretk_core::model::HarmonyDegreeKind::Alter => "alter",
                scoretk_core::model::HarmonyDegreeKind::Subtract => "subtract",
            },
        );
        xml.end_element();
    }
    xml.tag("placement", placement_name(harmony.placement));
    if !harmony.text.is_empty() {
        xml.tag("text", &harmony.text);
    }
    xml.end_element();
}

/// Écrit un point d'orgue.
fn write_fermata(xml: &mut XmlWriter, fermata: &Fermata, track: Track) {
    xml.start_element("Fermata", &[]);
    xml.tag("subtype", names::fermata_type(fermata.kind));
    // `Fermata::propertyDefault(Pid::PLACEMENT)` (fermata.cpp:187-190) :
    // piste paire → ABOVE, piste impaire → BELOW. On n'écrit `<placement>`
    // que s'il diffère de ce défaut.
    let default_placement = if track.raw() & 1 == 1 {
        Placement::Below
    } else {
        Placement::Above
    };
    if fermata.placement != default_placement {
        xml.tag("placement", placement_name(fermata.placement));
    }
    if !fermata.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit un chiffrage figuré.
fn write_figured_bass(xml: &mut XmlWriter, bass: &FiguredBass) {
    xml.start_element("FiguredBass", &[]);
    xml.tag("ticks", &fraction(bass.ticks));
    for item in &bass.items {
        xml.start_element("FiguredBassItem", &[]);
        if let Some(prefix) = item.prefix {
            xml.tag("prefix", &names::figured_bass_modifier(prefix).to_string());
        }
        if let Some(digit) = item.digit {
            xml.tag("digit", &digit.to_string());
        }
        if let Some(suffix) = item.suffix {
            xml.tag("suffix", &names::figured_bass_modifier(suffix).to_string());
        }
        xml.tag(
            "continuationLine",
            &names::figured_bass_cont_line(item.cont_line).to_string(),
        );
        xml.end_element();
    }
    if bass.place_above {
        xml.tag("placement", "above");
    }
    if bass.parentheses {
        xml.tag("parentheses", "1");
    }
    if !bass.visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit une barre de vibrato.
fn write_tremolo_bar(xml: &mut XmlWriter, bar: &TremoloBar) {
    xml.start_element("TremoloBar", &[]);
    let kind = names::tremolo_bar_type(bar.kind);
    if kind != 0 {
        xml.tag("type", &kind.to_string());
    }
    for point in &bar.points {
        xml.tag_empty(
            "point",
            &[
                ("time", &point.time.to_string()),
                ("pitch", &point.pitch.to_string()),
            ],
        );
    }
    xml.end_element();
}

/// Écrit un instrument (`TWrite::writeProperties(Instrument*)`).
/// Vrai si l'instrument du modèle n'a aucune propriété à écrire.
fn instrument_est_vide(instrument: &Instrument) -> bool {
    instrument.id.is_empty()
        && instrument.long_name.is_empty()
        && instrument.short_name.is_empty()
        && instrument.track_name.is_empty()
        && instrument.transpose.is_none()
        && instrument.midi_program.is_none()
        && instrument.midi_channel.is_none()
        && instrument.midi_port.is_none()
        && instrument.volume.is_none()
        && instrument.pan.is_none()
        && instrument.midi_articulations.is_empty()
}

pub(crate) fn write_instrument(xml: &mut XmlWriter, instrument: &Instrument) {
    // Aucune norme écrite (`docs/agents/format-ref/musescore.md`) : le
    // comportement de MuseScore 4.7.4 fait foi. Son écrivain
    // (`TWrite::write(const Instrument*)`, write/twrite.cpp:1947) produit
    // toujours un instrument valide ; un `<Instrument/>` vide est refusé au
    // chargement (code 1, sans message). Les sources LilyPond/ABC ne fixent pas
    // d'instrument dans le modèle pivot : on écrit le canal minimal par défaut.
    let repli = Instrument {
        midi_program: Some(0),
        ..Instrument::new("", "", "")
    };
    let instrument = if instrument_est_vide(instrument) {
        &repli
    } else {
        instrument
    };
    let attrs: Vec<(&str, &str)> = if instrument.id.is_empty() {
        Vec::new()
    } else {
        vec![("id", instrument.id.as_str())]
    };
    xml.start_element("Instrument", &attrs);
    if !instrument.id.is_empty() {
        xml.tag("instrumentId", &instrument.id);
    }
    if !instrument.long_name.is_empty() {
        xml.tag("longName", &instrument.long_name);
    }
    if !instrument.short_name.is_empty() {
        xml.tag("shortName", &instrument.short_name);
    }
    if !instrument.track_name.is_empty() {
        xml.tag("trackName", &instrument.track_name);
    }
    if let Some(transpose) = instrument.transpose {
        xml.tag("transposeDiatonic", &transpose.diatonic.to_string());
        xml.tag("transposeChromatic", &transpose.chromatic.to_string());
    }
    // Port de `TWrite::write(const Instrument*)` (`write/twrite.cpp:1947`) : un
    // instrument MuseScore possède toujours au moins un canal. Le laisser vide
    // fait échouer `PlaybackSetupDataResolver` et plantent MuseScore 4.7.4 à
    // l'ouverture d'une partition issue de LilyPond/ABC.
    {
        xml.start_element("Channel", &[]);
        if let Some(program) = instrument.midi_program {
            xml.tag_empty("program", &[("value", &program.to_string())]);
        }
        // Port de `TWrite::write(const InstrChannel*)` (`write/twrite.cpp:2111-2114`) :
        // quand l'attribution MIDI est exportée, le port et le canal sont écrits
        // ensemble. Sans `<midiPort>`, MuseScore recalcule le port au chargement
        // et l'export MusicXML change `<midi-device port>`.
        if let Some(port) = instrument.midi_port {
            xml.tag("midiPort", &port.to_string());
        }
        if let Some(channel) = instrument.midi_channel {
            // `Instrument::midiChannel` (base zéro) : l'export en dérive
            // `<midi-channel>` (base un) et l'attribution des canaux libres.
            xml.tag("midiChannel", &channel.to_string());
        }
        if let Some(volume) = instrument.volume {
            xml.tag_empty(
                "controller",
                &[("ctrl", "7"), ("value", &volume.to_string())],
            );
        }
        if let Some(pan) = instrument.pan {
            xml.tag_empty("controller", &[("ctrl", "10"), ("value", &pan.to_string())]);
        }
        xml.end_element();
    }
    for articulation in &instrument.midi_articulations {
        let attrs: Vec<(&str, &str)> = if articulation.name.is_empty() {
            Vec::new()
        } else {
            vec![("name", articulation.name.as_str())]
        };
        xml.start_element("Articulation", &attrs);
        xml.tag("velocity", &articulation.velocity.to_string());
        xml.tag("gateTime", &articulation.gate_time.to_string());
        xml.end_element();
    }
    xml.end_element();
}

/// Écrit un n-olet (`TWrite::writeItem(Tuplet*)`).
pub(crate) fn write_tuplet(xml: &mut XmlWriter, tuplet: &Tuplet) {
    xml.start_element("Tuplet", &[]);
    xml.tag("normalNotes", &tuplet.normal_notes.to_string());
    xml.tag("actualNotes", &tuplet.actual_notes.to_string());
    let (kind, dots) = duration_of_fraction(tuplet.base_len);
    if let Some(kind) = kind {
        xml.tag("baseNote", names::duration_type(kind));
        if dots > 0 {
            xml.tag("baseDots", &dots.to_string());
        }
    }
    if let Some(placement) = tuplet.placement {
        xml.tag(
            "direction",
            match placement {
                Placement::Above => "up",
                Placement::Below => "down",
            },
        );
    }
    match tuplet.number_type {
        TupletNumberType::Number => {}
        TupletNumberType::Ratio => {
            xml.start_element("Number", &[]);
            xml.tag(
                "text",
                &format!(
                    "{}:{}",
                    tuplet.normal_notes.max(1),
                    tuplet.ratio.numerator().max(1)
                ),
            );
            xml.end_element();
        }
        TupletNumberType::None => {
            xml.start_element("Number", &[]);
            xml.tag("text", "");
            xml.end_element();
        }
    }
    // `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) appelle
    // `writeStyledProperties` (`:644-647`) : `Pid::NUMBER_TYPE` est une
    // propriété de style (`dom/tuplet.cpp:50`) nommée `numberType`
    // (`dom/property.cpp:119`), 1 = SHOW_RELATION et 2 = NO_TEXT
    // (`dom/types.h:1600`). Le lecteur 4.6+ (`read460/tread.cpp:1530`) ne lit
    // que cette propriété ; sans elle il retombe sur SHOW_NUMBER et l'export
    // MusicXML de MuseScore perd `show-number="none"`/`"both"`.
    match tuplet.number_type {
        TupletNumberType::Number => {}
        TupletNumberType::Ratio => xml.tag("numberType", "1"),
        TupletNumberType::None => xml.tag("numberType", "2"),
    }
    // `calcHasBracket` n'est qu'une mise en page, mais MuseScore 3 écrivait le
    // choix dans `<bracket>` et le lecteur le relit : on le conserve.
    xml.tag("bracket", if tuplet.bracket { "1" } else { "0" });
    // `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) écrit aussi
    // `Pid::BRACKET_TYPE` via `writeStyledProperties` : `SHOW_BRACKET = 1`,
    // `SHOW_NO_BRACKET = 2` (`types.h:1604-1607`). Le lecteur 4.6+ ignore
    // `<bracket>` et ne consulte que cette propriété dans `calcHasBracket`
    // (`dom/tuplet.cpp:204-208`).
    xml.tag("bracketType", if tuplet.bracket { "1" } else { "2" });
    xml.end_element();
}

/// Retrouve une figure (et ses points) à partir d'une durée en fractions.
pub(crate) fn duration_of_fraction(value: Fraction) -> (Option<DurationType>, u8) {
    const BASE: [DurationType; 13] = [
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
    for kind in BASE {
        if kind.ticks() == value {
            return (Some(kind), 0);
        }
        let mut total = kind.ticks();
        let mut increment = kind.ticks() * Fraction::HALF;
        for dots in 1..=4u8 {
            total += increment;
            increment *= Fraction::HALF;
            if total == value {
                return (Some(kind), dots);
            }
        }
    }
    (None, 0)
}

/// Écrit la charge d'une ligne de texte (`TextLineBase`).
pub(crate) fn write_text_line(xml: &mut XmlWriter, info: &TextLineInfo) {
    let attrs: Vec<(&str, &str)> = if info.system {
        vec![("system", "1")]
    } else {
        Vec::new()
    };
    xml.start_element("TextLine", &attrs);
    xml.tag("lineVisible", if info.line_visible { "1" } else { "0" });
    xml.tag(
        "lineStyle",
        &names::text_line_style(info.line_style).to_string(),
    );
    xml.tag(
        "beginHookType",
        &names::hook_type(info.begin_hook).to_string(),
    );
    xml.tag("endHookType", &names::hook_type(info.end_hook).to_string());
    xml.tag("beginHookHeight", &number(info.begin_hook_height));
    xml.tag("endHookHeight", &number(info.end_hook_height));
    if let Some(text) = &info.begin_text {
        xml.tag_raw("beginText", &markup(text));
    }
    if let Some(text) = &info.end_text {
        xml.tag_raw("endText", &markup(text));
    }
    xml.tag("beginFontFace", &info.begin_font.family);
    xml.tag("beginFontSize", &number(info.begin_font.size));
    xml.tag("beginFontStyle", &font_style(&info.begin_font).to_string());
    xml.tag("endFontFace", &info.end_font.family);
    xml.tag("endFontSize", &number(info.end_font.size));
    xml.tag("endFontStyle", &font_style(&info.end_font).to_string());
    xml.tag("placement", placement_name(info.placement));
    xml.end_element();
}

/// Bits d'un `FontStyle` MuseScore (`Bold` 1, `Italic` 2, `Underline` 4,
/// `Strike` 8).
fn font_style(font: &scoretk_core::model::LineFont) -> i32 {
    i32::from(font.bold)
        + 2 * i32::from(font.italic)
        + 4 * i32::from(font.underline)
        + 8 * i32::from(font.strike)
}

/// Écrit la charge d'un soufflet (`TWrite::writeProperties(Hairpin*)`).
pub(crate) fn write_hairpin(
    xml: &mut XmlWriter,
    kind: HairpinType,
    hairpin: &Hairpin,
    visible: bool,
) {
    xml.start_element("HairPin", &[]);
    xml.tag(
        "subtype",
        &names::hairpin_subtype(kind, hairpin.line).to_string(),
    );
    if hairpin.niente {
        xml.tag("hairpinCircledTip", "1");
    }
    xml.tag(
        "direction",
        match hairpin.placement {
            Placement::Above => "up",
            Placement::Below => "down",
        },
    );
    if !hairpin.begin_text.is_empty() {
        xml.tag("beginText", &hairpin.begin_text);
    }
    if !hairpin.end_text.is_empty() {
        xml.tag("endText", &hairpin.end_text);
    }
    xml.tag("beginFontFace", &hairpin.begin_font.family);
    xml.tag("beginFontSize", &number(hairpin.begin_font.size));
    xml.tag(
        "beginFontStyle",
        word_font_style(hairpin.begin_font.bold, hairpin.begin_font.italic),
    );
    xml.tag("endFontFace", &hairpin.end_font.family);
    xml.tag("endFontSize", &number(hairpin.end_font.size));
    xml.tag(
        "endFontStyle",
        word_font_style(hairpin.end_font.bold, hairpin.end_font.italic),
    );
    if !visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Style de police tel que `TRead::read(Hairpin*)` le relit : le lecteur teste
/// la présence des mots `italic` et `bold` dans le texte.
fn word_font_style(bold: bool, italic: bool) -> &'static str {
    match (bold, italic) {
        (true, true) => "bold italic",
        (true, false) => "bold",
        (false, true) => "italic",
        (false, false) => "regular",
    }
}

/// Écrit la charge d'une pédale.
pub(crate) fn write_pedal(xml: &mut XmlWriter, pedal: &PedalInfo, visible: bool) {
    xml.start_element("Pedal", &[]);
    if !pedal.begin_text.is_empty() {
        xml.tag("beginText", &pedal.begin_text);
    }
    if !pedal.end_text.is_empty() {
        xml.tag("endText", &pedal.end_text);
    }
    xml.tag("lineVisible", if pedal.line_visible { "1" } else { "0" });
    xml.tag(
        "beginHookType",
        &names::hook_type(pedal.begin_hook).to_string(),
    );
    xml.tag("endHookType", &names::hook_type(pedal.end_hook).to_string());
    xml.tag("direction", placement_name(pedal.placement));
    if !visible {
        xml.tag("visible", "0");
    }
    xml.end_element();
}

/// Écrit la charge d'une volta.
pub(crate) fn write_volta(xml: &mut XmlWriter, volta: &VoltaInfo, text: Option<&str>) {
    xml.start_element("Volta", &[]);
    let endings = volta
        .endings
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    xml.tag("endings", &endings);
    xml.tag("endHookType", if volta.closed { "1" } else { "0" });
    if let Some(text) = text {
        xml.tag("beginText", text);
    }
    xml.end_element();
}

/// Écrit la charge d'une liaison d'expression, d'un trille ou d'un glissando.
pub(crate) fn write_notation(xml: &mut XmlWriter, notation: &SpannerNotation) {
    match notation {
        SpannerNotation::Slur(slur) => {
            xml.start_element("Slur", &[]);
            if let Some(placement) = slur.placement {
                xml.tag(
                    "up",
                    match placement {
                        Placement::Above => "up",
                        Placement::Below => "down",
                    },
                );
            }
            if !slur.visible {
                xml.tag("visible", "0");
            }
            xml.end_element();
        }
        SpannerNotation::Trill(trill) => {
            xml.start_element("Trill", &[]);
            xml.tag("subtype", names::trill_type(trill.kind));
            if let Some(accidental) = trill.accidental {
                xml.start_element("Accidental", &[]);
                if let Some(subtype) = names::accidental_type(accidental.kind) {
                    xml.tag("subtype", &subtype);
                }
                xml.end_element();
            }
            xml.end_element();
        }
        SpannerNotation::Glissando(glissando) => {
            xml.start_element("Glissando", &[]);
            xml.tag("subtype", names::glissando_type(glissando.kind));
            let style = match glissando.kind {
                scoretk_core::model::GlissandoType::Straight(style) => style,
                scoretk_core::model::GlissandoType::Wavy => scoretk_core::model::LineStyle::Solid,
            };
            xml.tag("lineStyle", names::line_style(style));
            xml.tag("text", &glissando.text);
            if !glissando.show_text {
                xml.tag("showText", "0");
            }
            if !glissando.visible {
                xml.tag("visible", "0");
            }
            xml.end_element();
        }
    }
}

/// Nom MuseScore du type de spanner (`<Spanner type="…">`).
pub(crate) fn spanner_type_name(spanner: &scoretk_core::model::Spanner) -> Option<&'static str> {
    use scoretk_core::model::SpannerType;
    Some(match &spanner.kind {
        SpannerType::Slur => "Slur",
        SpannerType::Hairpin(_) => "HairPin",
        SpannerType::Ottava(_) => "Ottava",
        SpannerType::Pedal => "Pedal",
        SpannerType::Volta => "Volta",
        SpannerType::TextLine => "TextLine",
        SpannerType::Trill => "Trill",
        SpannerType::Glissando => "Glissando",
        SpannerType::Other(_) => return None,
    })
}
