// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances d'articulations et d'ornements reprennent la table de
// `musicxml2ly` de LilyPond, Copyright (C) 2016--2026 John Gourlay et Werner
// Lemberg, GPL-3.0-or-later ; elles sont réécrites en Rust.
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

//! Accords, silences, ornements et articulations.
//!
//! Portage du rendu de `ChordRest` (`engraving/dom/chordrest.h`), `Chord`
//! (`engraving/dom/chord.h`), `Rest` (`engraving/dom/rest.h`) et
//! `Articulation` (`engraving/dom/articulation.h`) vers la syntaxe LilyPond.
//!
//! Rien n'est décidé ici pour les objets extérieurs à l'accord : nuances,
//! soufflets, liaisons, doigtés de portée et changements de contexte sont des
//! [`Event`](crate::event::Event) posés par [`crate::voice`]. Le jeton produit
//! ici ne contient que ce qui est solidaire de l'accord : hauteurs, durée,
//! articulations, ornements, arpège, trémolo et accords d'agrément.

use mscz2xml::model::{
    AccidentalType, Articulation, ArticulationType, Chord, ChordLineType, ChordRest, Duration,
    Fraction, GraceChord, GraceType, Note, Ornament, OrnamentType, Score, StemDirection, Tremolo,
    TremoloKind, Tuplet,
};

use crate::duration::{duration_token, length_token, with_multiplier};
use crate::pitch;
use crate::text;

/// Rend un accord ou un silence en jeton LilyPond.
///
/// `measure_len` est la longueur réelle de la mesure (un silence de mesure
/// entière s'écrit `R` suivi de cette longueur, ce qui couvre aussi les
/// silences multi-mesures). `transpose_chromatic` est la transposition de
/// l'instrument en vigueur, au sens de `pitch2xml`.
pub(crate) fn render(
    score: &Score,
    chord_rest: &ChordRest,
    measure_len: Fraction,
    transpose_chromatic: i32,
) -> String {
    if let Some(rest) = chord_rest.rest_ref() {
        if !rest.visible {
            return format!("s{}", rest_duration(chord_rest));
        }
        if chord_rest.is_full_measure_rest() {
            return format!(
                "R{}",
                length_token(if measure_len.is_positive() {
                    measure_len
                } else {
                    chord_rest.ticks()
                })
            );
        }
        return format!("r{}", rest_duration(chord_rest));
    }

    let Some(chord) = chord_rest.chord_ref() else {
        return String::new();
    };

    let mut parts: Vec<String> = Vec::new();
    let mut token = String::new();
    if let Some(stem) = stem_override(chord.stem) {
        token.push_str(stem);
        token.push(' ');
    }
    if let Some(arpeggio) = arpeggio_prefix(chord) {
        token.push_str(&arpeggio);
        token.push(' ');
    }

    // Accords d'agrément qui précèdent l'accord.
    let mut after_graces: Vec<&GraceChord> = Vec::new();
    for grace in &chord.grace_chords {
        if is_after_grace(grace.chord.grace) {
            after_graces.push(grace);
            continue;
        }
        let command = match grace.chord.grace {
            Some(GraceType::Acciaccatura) => "\\acciaccatura",
            Some(GraceType::Appoggiatura) => "\\appoggiatura",
            _ => "\\grace",
        };
        parts.push(format!(
            "{command} {{ {} }}",
            chord_notes(score, &grace.chord, &grace.duration, transpose_chromatic)
        ));
    }

    let core = chord_token(score, chord, chord_rest, transpose_chromatic);
    if after_graces.is_empty() {
        parts.push(core);
    } else {
        let mut graces = String::new();
        for grace in after_graces {
            graces.push_str(&chord_notes(
                score,
                &grace.chord,
                &grace.duration,
                transpose_chromatic,
            ));
            graces.push(' ');
        }
        parts.push(format!(
            "\\afterGrace {core} {{ {} }}",
            graces.trim_end()
        ));
    }

    token.push_str(&parts.join(" "));
    token
}

/// Durée d'un silence, multiplicateur compris.
fn rest_duration(chord_rest: &ChordRest) -> String {
    let notated = chord_rest.duration.ticks();
    let actual = chord_rest.ticks.unwrap_or(notated);
    with_multiplier(duration_token(chord_rest.duration), actual, notated)
}

/// Vrai pour un accord d'agrément qui suit l'accord principal.
fn is_after_grace(grace: Option<GraceType>) -> bool {
    matches!(
        grace,
        Some(GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After)
    )
}

/// Correction de hampe explicite.
fn stem_override(stem: StemDirection) -> Option<&'static str> {
    match stem {
        StemDirection::Up => Some("\\once \\override Stem.direction = #UP"),
        StemDirection::Down => Some("\\once \\override Stem.direction = #DOWN"),
        StemDirection::Auto => None,
    }
}

/// Préfixe d'arpège (`\arpeggioArrowUp`, `\arpeggioBracket`…).
fn arpeggio_prefix(chord: &Chord) -> Option<String> {
    let arpeggio = chord.arpeggio.as_ref()?;
    if !arpeggio.visible {
        return None;
    }
    match arpeggio.kind {
        mscz2xml::model::ArpeggioType::Up => Some("\\arpeggioArrowUp".to_owned()),
        mscz2xml::model::ArpeggioType::Down => Some("\\arpeggioArrowDown".to_owned()),
        mscz2xml::model::ArpeggioType::Bracket => Some("\\arpeggioBracket".to_owned()),
        mscz2xml::model::ArpeggioType::Normal => None,
    }
}

/// Jeton d'un accord : hauteurs, durée, articulations, ornements, trémolo.
fn chord_token(
    score: &Score,
    chord: &Chord,
    chord_rest: &ChordRest,
    transpose_chromatic: i32,
) -> String {
    let notes: Vec<&Note> = chord
        .notes
        .iter()
        .map(|id| score.note(*id))
        .filter(|note| note.visible)
        .collect();
    let notes: Vec<&Note> = if notes.is_empty() {
        chord.notes.iter().map(|id| score.note(*id)).collect()
    } else {
        notes
    };

    let mut head = String::new();
    if notes.len() > 1 {
        head.push('<');
    }
    for (index, note) in notes.iter().enumerate() {
        if index > 0 {
            head.push(' ');
        }
        if let Some(style) = pitch::note_head_style(&note.head_group) {
            head.push_str(&format!("\\tweak style #'{style} "));
        }
        head.push_str(&pitch::pitch_name(note, transpose_chromatic));
        if cautionary(note) {
            head.push('?');
        }
        if notes.len() > 1 && note.tie_forward.is_some() {
            head.push('~');
        }
    }
    if notes.len() > 1 {
        head.push('>');
    }
    if notes.is_empty() {
        // Accord sans note (fichier incomplet) : un silence vaut mieux qu'un
        // jeton vide, qui décalerait la mesure.
        head.push('r');
    }

    let notated = chord_rest.duration.ticks();
    let actual = chord_rest.ticks.unwrap_or(notated);
    let duration = if chord_rest.tuplet.is_some() {
        duration_token(chord_rest.duration)
    } else {
        with_multiplier(duration_token(chord_rest.duration), actual, notated)
    };
    let mut token = format!("{head}{duration}");

    if notes.len() == 1 && notes[0].tie_forward.is_some() {
        token.push('~');
    }
    if let Some(tremolo) = &chord.tremolo {
        token.push_str(&tremolo_token(tremolo));
    }
    token.push_str(&articulations(&chord.articulations));
    token.push_str(&ornaments(&chord.ornaments));
    token.push_str(&fingering(&notes));
    if let Some(line) = chord_line(chord.chord_line) {
        token.push_str(line);
    }
    token
}

/// Vrai si l'altération doit être écrite comme de courtoisie (`?`).
fn cautionary(note: &Note) -> bool {
    note.accidental.is_some_and(|accidental| {
        accidental.visible
            && (accidental.user
                || accidental.parentheses
                || accidental.bracket
                || matches!(
                    accidental.kind,
                    AccidentalType::NaturalFlat
                        | AccidentalType::NaturalSharp
                        | AccidentalType::Other(_)
                ))
    })
}

/// Jeton de trémolo (`:8`, `:16`…).
fn tremolo_token(tremolo: &Tremolo) -> String {
    let value = match tremolo.kind {
        TremoloKind::Single | TremoloKind::Start | TremoloKind::Stop => {
            // `count` est un nombre de barres MusicXML : 1 = croche (8),
            // 2 = double croche (16), 3 = triple (32), 4 = quadruple (64).
            8u32 << u32::from(tremolo.count.clamp(1, 4) - 1)
        }
        TremoloKind::Unmeasured => 32,
    };
    format!(":{value}")
}

/// Notes d'un accord d'agrément, avec leur durée.
fn chord_notes(
    score: &Score,
    chord: &Chord,
    duration: &Duration,
    transpose_chromatic: i32,
) -> String {
    let duration = duration_token(*duration);
    let mut rendered = String::new();
    for id in &chord.notes {
        let note = score.note(*id);
        if !note.visible {
            continue;
        }
        rendered.push_str(&pitch::pitch_name(note, transpose_chromatic));
        rendered.push_str(&duration);
        rendered.push(' ');
    }
    rendered.trim_end().to_owned()
}

/// Suffixe des articulations (`-.`, `->`, `\fermata`…).
pub(crate) fn articulations(articulations: &[Articulation]) -> String {
    let mut result = String::new();
    for articulation in articulations {
        if !articulation.visible {
            continue;
        }
        if let Some(token) = articulation_token(&articulation.kind) {
            result.push_str(&token);
        }
    }
    result
}

/// Jeton LilyPond d'une articulation, ou `None` si elle n'a pas d'équivalent.
fn articulation_token(kind: &ArticulationType) -> Option<String> {
    let token = match kind {
        ArticulationType::Staccato => "-.",
        ArticulationType::Staccatissimo | ArticulationType::Spiccato => "-!",
        ArticulationType::Accent => "->",
        ArticulationType::Marcato => "-^",
        ArticulationType::Tenuto => "--",
        ArticulationType::Portato => "-_",
        ArticulationType::SoftAccent
        | ArticulationType::SoftAccentStaccato
        | ArticulationType::SoftAccentTenuto
        | ArticulationType::SoftAccentTenutoStaccato => "\\espressivo",
        ArticulationType::Fermata => "\\fermata",
        ArticulationType::FermataShort | ArticulationType::FermataVeryShort => "\\shortfermata",
        ArticulationType::FermataLong => "\\longfermata",
        ArticulationType::FermataVeryLong => "\\verylongfermata",
        ArticulationType::BreathMark => "\\breathe",
        ArticulationType::Caesura => "\\caesura",
        ArticulationType::UpBow => "\\upbow",
        ArticulationType::DownBow => "\\downbow",
        ArticulationType::Harmonic => "\\flageolet",
        ArticulationType::Open => "\\open",
        ArticulationType::Stopped => "\\stopped",
        ArticulationType::SnapPizzicato => "\\snappizzicato",
        ArticulationType::Stress
        | ArticulationType::Unstress
        | ArticulationType::Other(_) => return None,
    };
    Some(token.to_owned())
}

/// Suffixe des ornements (`\trill`, `\mordent`…).
pub(crate) fn ornaments(ornaments: &[Ornament]) -> String {
    let mut result = String::new();
    for ornament in ornaments {
        if !ornament.visible {
            continue;
        }
        if let Some(token) = ornament_token(&ornament.kind) {
            result.push_str(token);
        }
    }
    result
}

/// Jeton LilyPond d'un ornement, ou `None` s'il n'a pas d'équivalent.
fn ornament_token(kind: &OrnamentType) -> Option<&'static str> {
    let token = match kind {
        OrnamentType::TrillMark => "\\trill",
        OrnamentType::Turn => "\\turn",
        OrnamentType::InvertedTurn => "\\reverseturn",
        OrnamentType::TurnSlash => "\\slashturn",
        OrnamentType::Mordent => "\\mordent",
        OrnamentType::InvertedMordent => "\\prall",
        OrnamentType::InvertedMordentLong
        | OrnamentType::InvertedMordentLongApproachBelow
        | OrnamentType::InvertedMordentLongApproachAbove
        | OrnamentType::InvertedMordentLongDepartureBelow
        | OrnamentType::InvertedMordentLongDepartureAbove => "\\prallprall",
        OrnamentType::MordentLong
        | OrnamentType::MordentLongApproachBelow
        | OrnamentType::MordentLongApproachAbove => "\\mordent",
        OrnamentType::Haydn => "\\haydnturn",
        OrnamentType::Schleifer => "\\upmordent",
        OrnamentType::VerticalTurn
        | OrnamentType::InvertedVerticalTurn
        | OrnamentType::Other(_) => return None,
    };
    Some(token)
}

/// Doigtés (`-1`, `-\markup { "T" }`).
fn fingering(notes: &[&Note]) -> String {
    let mut result = String::new();
    for note in notes {
        for fingering in &note.fingerings {
            if !fingering.visible {
                continue;
            }
            let text_value = fingering.text.trim();
            if text_value.is_empty() {
                continue;
            }
            if text_value.chars().all(|character| character.is_ascii_digit()) {
                result.push('-');
                result.push_str(text_value);
            } else {
                result.push('-');
                result.push_str(&text::markup(&text::escape(text_value)));
            }
        }
    }
    result
}

/// Ligne d'accord, rendue par `\bendAfter` (seule commande standard de
/// LilyPond pour une inflexion de hauteur) : MuseScore distingue chute,
/// montée, plop et scoop, LilyPond non.
fn chord_line(chord_line: Option<ChordLineType>) -> Option<&'static str> {
    match chord_line? {
        ChordLineType::Fall => Some("\\bendAfter #-3"),
        ChordLineType::Doit => Some("\\bendAfter #3"),
        ChordLineType::Plop => Some("\\bendAfter #-5"),
        ChordLineType::Scoop => Some("\\bendAfter #3"),
    }
}

/// Ratio d'un n-olet tel qu'il s'écrit dans `\tuplet num/den`.
pub(crate) fn tuplet_ratio(tuplet: &Tuplet) -> (i64, i64) {
    let (actual, normal) = tuplet.displayed_numbers();
    if actual > 0 && normal > 0 {
        (actual, normal)
    } else {
        (tuplet.ratio.numerator(), tuplet.ratio.denominator())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mscz2xml::model::{Measure, Part, Segment, SegmentItem, SegmentType, Track, tpc};

    fn score_avec_accord(chord_rest: ChordRest) -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord_rest));
        measure.add_segment(segment);
        score.add_measure(measure);
        score
    }

    #[test]
    fn accord_simple() {
        let mut chord = ChordRest::chord(Duration::quarter());
        let note = Note::new(60, tpc::C);
        let mut score = score_avec_accord(chord.clone());
        let id = score.add_note(note);
        if let Some(inner) = chord.chord_mut() {
            inner.notes.push(id);
        }
        assert_eq!(render(&score, &chord, Fraction::ONE, 0), "c4");
    }

    #[test]
    fn accord_de_plusieurs_notes() {
        let mut chord = ChordRest::chord(Duration::half());
        let mut score = Score::new();
        let c = score.add_note(Note::new(60, tpc::C));
        let e = score.add_note(Note::new(64, tpc::E));
        if let Some(inner) = chord.chord_mut() {
            inner.notes.extend([c, e]);
        }
        assert_eq!(render(&score, &chord, Fraction::ONE, 0), "<c e>2");
    }

    #[test]
    fn silence_de_mesure() {
        let chord_rest = ChordRest::full_measure_rest();
        let score = score_avec_accord(chord_rest.clone());
        assert_eq!(render(&score, &chord_rest, Fraction::ONE, 0), "R1");
        assert_eq!(
            render(&score, &chord_rest, Fraction::new(3, 4), 0),
            "R2."
        );
    }

    #[test]
    fn articulations_et_ornements() {
        let mut chord = ChordRest::chord(Duration::quarter());
        if let Some(inner) = chord.chord_mut() {
            inner
                .articulations
                .push(Articulation::new(ArticulationType::Staccato));
            inner
                .articulations
                .push(Articulation::new(ArticulationType::Fermata));
            inner.ornaments.push(Ornament::new(OrnamentType::TrillMark));
        }
        let mut score = score_avec_accord(chord.clone());
        let id = score.add_note(Note::new(60, tpc::C));
        if let Some(inner) = chord.chord_mut() {
            inner.notes.push(id);
        }
        assert_eq!(
            render(&score, &chord, Fraction::ONE, 0),
            "c4-.\\fermata\\trill"
        );
    }

    #[test]
    fn liaison_de_prolongation() {
        let mut chord = ChordRest::chord(Duration::quarter());
        let mut score = Score::new();
        let mut note = Note::new(60, tpc::C);
        let next = score.add_note(Note::new(60, tpc::C));
        note.tie_forward = Some(next);
        let id = score.add_note(note);
        if let Some(inner) = chord.chord_mut() {
            inner.notes.push(id);
        }
        assert_eq!(render(&score, &chord, Fraction::ONE, 0), "c4~");
    }
}
