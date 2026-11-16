//! Accords, silences, notes, paroles et n-olets.
//!
//! Portage de `TRead::read(Chord*)`, `readProperties(ChordRest*)`,
//! `read(Rest*)`, `read(Note*)`, `read(Lyrics*)` et `read(Tuplet*)`
//! (`engraving/rw/read400/tread.cpp`).

use super::spanner;
use super::tables;
use super::xml::Node;
use super::{Reader, VoiceCtx};
use crate::model::{
    tpc, Accidental, Articulation, BeamMode, Chord, ChordRest, ChordRestKind, Duration,
    DurationType, Fraction, GraceType, Lyrics, LyricsSyllabic, Note, NoteHeadType, NoteId,
    Placement, Rest, Score, StemDirection, Transpose, Tuplet, TupletId, TupletNumberType,
};

/// Lit un `<Chord>` ou un `<Rest>`.
///
/// `grace_index` est la position de l'accord dans la liste des ornements en
/// attente ; elle ne sert qu'aux liaisons et spanners ancrés sur une note.
pub(crate) fn read_chord_rest(
    reader: &mut Reader,
    node: &Node,
    ctx: &VoiceCtx,
    grace_index: usize,
) -> ChordRest {
    let is_rest = node.name() == "Rest";
    let mut duration_kind = DurationType::Quarter;
    let mut dots = 0u8;
    let mut explicit: Option<Fraction> = None;
    let mut grace: Option<GraceType> = None;
    let mut slash = false;
    let mut stem = StemDirection::Auto;
    let mut small = false;
    let mut beam = BeamMode::Auto;
    let mut lyrics = Vec::new();
    let mut articulations = Vec::new();

    for child in node.children() {
        match child.name() {
            "durationType" => {
                duration_kind = tables::duration_type(&child.trimmed_text())
                    .unwrap_or(DurationType::Quarter);
            }
            "dots" => dots = child.int_text().unwrap_or(0).clamp(0, 4) as u8,
            "duration" => explicit = child.fraction(),
            "appoggiatura" => grace = Some(GraceType::Appoggiatura),
            "acciaccatura" => {
                grace = Some(GraceType::Acciaccatura);
                slash = true;
            }
            "grace4" => grace = Some(GraceType::Grace4),
            "grace8after" => grace = Some(GraceType::Grace8After),
            "grace16" => grace = Some(GraceType::Grace16),
            "grace16after" => grace = Some(GraceType::Grace16After),
            "grace32" => grace = Some(GraceType::Grace32),
            "grace32after" => grace = Some(GraceType::Grace32After),
            "StemSlash" => slash = true,
            "StemDirection" => stem = tables::stem_direction(&child.trimmed_text()),
            "small" => small = child.int_text().unwrap_or(0) != 0,
            "BeamMode" => beam = tables::beam_mode(&child.trimmed_text()),
            "Lyrics" => lyrics.push(read_lyrics(child)),
            "Articulation" => articulations.push(read_articulation(child)),
            _ => {}
        }
    }

    let mut duration = Duration::with_dots(duration_kind, dots);
    if let Some(ticks) = explicit {
        if !duration.is_measure() && ticks.is_positive() && ticks != duration.ticks() {
            // TODO(parité): le modèle ne stocke pas de ticks distincts de la
            // figure. MuseScore garde la figure lue et écrase seulement les
            // ticks ; on préfère ici une figure dont la durée est juste.
            duration = tables::duration_from_ticks(ticks);
        }
    }

    let kind = if is_rest {
        ChordRestKind::Rest(Rest {
            full_measure: duration.is_measure(),
            gap: false,
        })
    } else {
        let mut chord = Chord {
            stem,
            small,
            grace,
            grace_slash: slash,
            articulations,
            ..Chord::default()
        };
        let note_grace = if grace.is_some() {
            Some(grace_index)
        } else {
            None
        };
        for child in node.children_named("Note") {
            let index = chord.notes.len();
            let note = read_note(reader, child, ctx, note_grace, index);
            chord.notes.push(note);
        }
        for child in node.children_named("Spanner") {
            spanner::read_chord_spanner(reader, child, ctx);
        }
        ChordRestKind::Chord(chord)
    };

    ChordRest {
        duration,
        tuplet: None,
        lyrics,
        beam_mode: beam,
        kind,
    }
}

/// Durée réelle consommée par un accord ou un silence.
///
/// `DurationElement::actualTicks` : durée notée divisée par le ratio de chaque
/// n-olet englobant. Un silence de mesure entière vaut la longueur de la
/// mesure.
pub(crate) fn advance(
    cr: &ChordRest,
    measure_len: Fraction,
    tuplets: &[TupletId],
    score: &Score,
) -> Fraction {
    let mut ticks = cr.ticks_in_measure(measure_len);
    for &id in tuplets {
        ticks /= score.tuplet(id).ratio;
    }
    ticks
}

/// Lit un n-olet (`<Tuplet>`).
///
/// `tick` est le tick absolu où il commence, `parent` le n-olet englobant
/// (n-olets imbriqués). La longueur suit MuseScore : `baseNote ×
/// dénominateur`, sans division par le parent.
pub(crate) fn read_tuplet(
    reader: &mut Reader,
    node: &Node,
    tick: Fraction,
    parent: Option<TupletId>,
) -> TupletId {
    let mut actual = 1i64;
    let mut normal = 1i64;
    let mut base_kind = DurationType::Eighth;
    let mut base_dots = 0u8;
    let mut number_type = TupletNumberType::Number;
    let mut bracket = false;

    for child in node.children() {
        match child.name() {
            "actualNotes" => actual = i64::from(child.int_text().unwrap_or(1).max(1)),
            "normalNotes" => normal = i64::from(child.int_text().unwrap_or(1).max(1)),
            "baseNote" => {
                base_kind =
                    tables::duration_type(&child.trimmed_text()).unwrap_or(DurationType::Eighth);
            }
            "baseDots" => base_dots = child.int_text().unwrap_or(0).clamp(0, 4) as u8,
            "numberType" => number_type = tables::tuplet_number_type(child.int_text()),
            "bracket" => bracket = child.int_text().unwrap_or(0) != 0,
            _ => {}
        }
    }

    let ratio = Fraction::new(actual, normal);
    let base_len = Duration::with_dots(base_kind, base_dots).ticks();
    let len = base_len * Fraction::from_integer(ratio.denominator());
    let mut tuplet = Tuplet::new(ratio, base_len, tick, len);
    tuplet.parent = parent;
    tuplet.number_type = number_type;
    tuplet.bracket = bracket;
    reader.score.add_tuplet(tuplet)
}

fn read_note(
    reader: &mut Reader,
    node: &Node,
    ctx: &VoiceCtx,
    grace: Option<usize>,
    index: usize,
) -> NoteId {
    let id = reader.score.add_note(Note::new(60, tpc::C));
    let mut pitch = 60u8;
    let mut tpc1 = tpc::INVALID;
    let mut tpc2 = tpc::INVALID;
    let mut tpc2_from_file = false;
    let mut accidental = None;
    let mut head = NoteHeadType::Auto;
    let mut ghost = false;
    let mut play = true;

    for child in node.children() {
        match child.name() {
            "pitch" => pitch = child.int_text().unwrap_or(60).clamp(0, 127) as u8,
            "tpc" => {
                let value = child.int_text().unwrap_or(i32::from(tpc::INVALID)) as i8;
                tpc1 = value;
                tpc2 = value;
            }
            "tpc2" => {
                tpc2 = child.int_text().unwrap_or(i32::from(tpc::INVALID)) as i8;
                tpc2_from_file = true;
            }
            "Accidental" => accidental = Some(read_accidental(child)),
            "head" => head = tables::note_head(child.int_text()),
            "ghost" | "dead" => ghost = child.int_text().unwrap_or(0) != 0,
            "play" => play = child.int_text().unwrap_or(1) != 0,
            "Spanner" => spanner::read_note_spanner(reader, child, ctx, id, grace, index),
            _ => {}
        }
    }

    // `Note::setupAfterRead` : complète l'orthographe concert (`tpc1`) ou
    // écrite (`tpc2`) manquante, en transposant l'autre.
    let transpose = super::instrument_transpose(&reader.score, ctx.staff);
    if tpc1 == tpc::INVALID && tpc2 == tpc::INVALID {
        // TODO(parité): `pitch2tpc` choisit l'orthographe selon l'armure
        // courante (`staff()->key(tick)`) et la préférence dièse/bémol du
        // fichier ; on se rabat sur une orthographe en dièses.
        tpc1 = tables::pitch_to_tpc(pitch, 0);
    }
    if tpc1 == tpc::INVALID {
        tpc1 = tables::transpose_tpc(tpc2, transpose);
    } else if !tpc2_from_file && tpc2 == tpc::INVALID {
        tpc2 = if transpose.chromatic == 0 && transpose.diatonic == 0 {
            tpc1
        } else {
            tables::transpose_tpc(
                tpc1,
                Transpose::new(-transpose.diatonic, -transpose.chromatic),
            )
        };
    }
    if tpc2 == tpc::INVALID {
        tpc2 = tpc1;
    }

    let note = reader.score.note_mut(id);
    note.pitch = pitch;
    note.tpc = tpc1;
    note.tpc_transposed = tpc2;
    note.accidental = accidental;
    note.head = head;
    note.ghost = ghost;
    note.play = play;
    id
}

fn read_accidental(node: &Node) -> Accidental {
    let mut accidental = Accidental::new(crate::model::AccidentalType::None);
    for child in node.children() {
        match child.name() {
            "subtype" => {
                accidental.kind = tables::accidental_type(&child.trimmed_text());
            }
            "bracket" => accidental.bracket = child.int_text().unwrap_or(0) != 0,
            "parens" | "parentheses" => accidental.parentheses = child.int_text().unwrap_or(0) != 0,
            "visible" => accidental.visible = child.int_text().unwrap_or(1) != 0,
            _ => {}
        }
    }
    accidental
}

fn read_lyrics(node: &Node) -> Lyrics {
    let mut verse = 0usize;
    let mut syllabic = LyricsSyllabic::Single;
    let mut text = String::new();
    let mut melisma = None;
    let mut placement = Placement::Below;

    for child in node.children() {
        match child.name() {
            "no" => verse = child.int_text().unwrap_or(0).max(0) as usize,
            "syllabic" => syllabic = tables::syllabic(&child.trimmed_text()),
            "text" => text = child.text().trim().to_owned(),
            "ticks" | "ticks_f" => melisma = child.fraction(),
            "placement" => placement = tables::placement(&child.trimmed_text()),
            _ => {}
        }
    }

    Lyrics {
        verse,
        syllabic,
        text,
        melisma,
        placement,
    }
}

fn read_articulation(node: &Node) -> Articulation {
    let mut articulation = Articulation::new(crate::model::ArticulationType::Other(String::new()));
    for child in node.children() {
        match child.name() {
            "subtype" => articulation.kind = tables::articulation_type(&child.trimmed_text()),
            "anchor" => articulation.anchor = tables::articulation_anchor(child.int_text()),
            "direction" => {
                articulation.anchor = match child.trimmed_text().as_str() {
                    "up" => crate::model::ArticulationAnchor::Up,
                    "down" => crate::model::ArticulationAnchor::Down,
                    _ => crate::model::ArticulationAnchor::Auto,
                };
            }
            _ => {}
        }
    }
    articulation
}
