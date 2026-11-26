// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la syntaxe GABC de Gregorio (GPL-3.0-or-later) : `doc/Gabc.tex`,
// `src/struct.{c,h}`, modifié (réécrit en Rust) en 2026.
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

//! Builds a Gregorian payload from a score that has none (MusicXML, MuseScore,
//! MIDI, …), so a monophonic chant can still be written as GABC.
//!
//! GABC pitch letters are **typographical staff positions**, not concert
//! pitches (`Gabc.tex:408-443`). Using Gregorio's key rule
//! (`src/struct.c:1273-1288`, `src/struct.h:809`), a chosen clef is inverted:
//! `height = diatonic_step + clef_key + 5`, then height → letter. The pivot
//! holds concert pitch and a tonal pitch class, so the diatonic step is
//! recovered from both: the class gives the letter, the MIDI pitch its octave
//! and the accidental. Everything this projection cannot carry is documented
//! in `docs/agents/gabc.md` §8/§10.

use scoretk_core::model::{
    ArticulationType, BarKind, BarLineType, ChordRestKind, ClefType, Duration, Gregorian,
    GregorianHeader, GregorianNote, LyricsSyllabic, NeumeSign, OrnamentType, Score, SegmentItem,
    SignKind, Syllable, Track,
};
use scoretk_gabc_common::syntax;

const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Preferred clefs, in tie-break order. `c3` keeps most Gregorian melodies
/// inside `a`–`m` on a four-line staff (`Gabc.tex:434,847-870`).
const CLEFS: [&str; 8] = ["c3", "c4", "f3", "f4", "c2", "c1", "f2", "f1"];

#[derive(Clone, Debug)]
struct Event {
    /// Diatonic step (C = 0) and the semitone difference from its natural.
    note: Option<(i32, i32)>,
    /// Durée du `ChordRest`, réécrite en signes GABC par `signs_for_duration`.
    duration: Duration,
    text: String,
    space_after: bool,
    bar: Option<BarKind>,
    /// `<ornaments><inverted-mordent/></ornaments>` sur une note de chant :
    /// convention gabctk du quilisma (`gabctk.py:1051-1095`, `w` dans GABC).
    quilisma: bool,
    /// `<articulations><tenuto/></articulations>` : l'épisème horizontal
    /// (`Gabc.tex:769-797`) est noté comme un tenuto et joué ×1,5 dans la vue
    /// moderne (`docs/agents/gabc.md` §7). Il doit revenir à `_`, jamais à un
    /// point ou à un autre glyphe.
    episema: bool,
}

/// Builds the payload the writer will serialise. Missing L4/L5 data is not
/// invented: no tempo, dynamics, page geometry or neume shape is produced.
pub(super) fn from_score(score: &Score) -> Gregorian {
    let staff = choose_staff(score);
    let staff_lines = score
        .staves
        .get(staff)
        .map_or(4, |item| item.lines)
        .clamp(2, 5);
    let max_height = match staff_lines {
        2 => 11,
        3 => 13,
        4 => 15,
        _ => 19,
    };
    let events = collect_events(score, staff);
    // Quand la partition vient de la vue dérivée grégorienne, sa clé C/F sur
    // portée de quatre lignes est une donnée de la source (`Gabc.tex:847-870`)
    // transportée par `<clef>` en MusicXML standard. La reprendre évite de
    // recalculer une clé par tessiture, ce qui décalerait toutes les lettres.
    let clef = score_gabc_clef(score, staff).unwrap_or_else(|| choose_clef(&events, max_height));
    let key = syntax::clef_key(clef);

    // Le lecteur GABC traite `x`/`#`/`y` comme des glyphes d'alteration non
    // chantés (`Gabc.tex:481-489,512-540`) : pour projeter une hauteur
    // alterée venant d'un autre format, on écrit d'abord le glyphe puis la
    // note chantée de même lettre. L'alteration s'applique ensuite à toutes
    // les notes de cette lettre jusqu'à la fin du mot ou à la barre.
    // Le glyphe est déduit de `pitch`/`tpc` seul : `Note::accidental` n'est
    // jamais lu. Les signes d'avertissement ajoutés par la projection moderne
    // des L6 (`cautionary`, sans crochets) sont donc ignorés ici et ne peuvent
    // pas réapparaître en glyphe `y` (`docs/agents/gabc.md` §7).
    let mut effect = [0i8; 16];
    let mut syllables: Vec<Syllable> = events
        .into_iter()
        .map(|event| {
            let mut notes = Vec::new();
            if let Some((step, diff)) = event.note {
                let height = step + key + 5;
                let letter = letter_for_height(height.clamp(3, max_height));
                let wanted = if diff < 0 {
                    -1
                } else if diff > 0 {
                    1
                } else {
                    0
                };
                let slot = (letter as u8).wrapping_sub(b'a') as usize % 16;
                if effect[slot] != wanted {
                    let raw = match wanted {
                        -1 => "x",
                        1 => "#",
                        _ => "y",
                    };
                    notes.push(GregorianNote {
                        letter,
                        glyph: raw.to_owned(),
                        signs: vec![NeumeSign::new(SignKind::Alteration, raw)],
                        ..GregorianNote::default()
                    });
                    effect[slot] = wanted;
                }
                let (mut glyph, mut signs) = signs_for_duration(event.duration, event.episema);
                if event.quilisma {
                    glyph.push('w');
                    signs.push(NeumeSign::new(SignKind::Quilisma, "w"));
                }
                notes.push(GregorianNote {
                    letter,
                    glyph,
                    signs,
                    ..GregorianNote::default()
                });
            }
            if event.space_after || event.bar.is_some() {
                effect = [0; 16];
            }
            Syllable {
                text: event.text,
                clef: None,
                rubric: None,
                notes,
                signs: Vec::new(),
                bars: event.bar.into_iter().collect(),
                space_after: event.space_after,
                ..Syllable::default()
            }
        })
        .collect();
    if let Some(first) = syllables.first_mut() {
        first.clef = Some(clef.to_owned());
    }

    let name = if score.metadata.work_title.is_empty() {
        if score.metadata.movement_title.is_empty() {
            "Cantus".to_owned()
        } else {
            score.metadata.movement_title.clone()
        }
    } else {
        score.metadata.work_title.clone()
    };
    let mut headers = vec![
        GregorianHeader {
            name: "name".to_owned(),
            value: name,
        },
        GregorianHeader {
            name: "language".to_owned(),
            value: "latin".to_owned(),
        },
        GregorianHeader {
            name: "staff-lines".to_owned(),
            value: staff_lines.to_string(),
        },
    ];
    if !score.metadata.composer.is_empty() {
        headers.push(GregorianHeader {
            name: "author".to_owned(),
            value: score.metadata.composer.clone(),
        });
    }
    Gregorian {
        headers,
        staff_lines,
        clefs: vec![clef.to_owned()],
        syllables,
        opening_separator: None,
        derived_modern_view: false,
    }
}

/// Clé grégorienne déjà portée par la partition (vue dérivée d'un L6), sous
/// la forme du jeton GABC que le lecteur sait relire.
fn score_gabc_clef(score: &Score, staff: usize) -> Option<&'static str> {
    let staff_id = score.staves.get(staff)?.id;
    if score.staves.get(staff)?.lines != 4 {
        return None;
    }
    let track = Track::new(staff_id, 0);
    let clef = score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .find_map(|segment| match segment.element(track) {
            Some(SegmentItem::Clef(clef)) => Some(clef.concert),
            _ => None,
        })?;
    match clef {
        ClefType::C1 => Some("c1"),
        ClefType::C2 => Some("c2"),
        ClefType::C3 => Some("c3"),
        ClefType::C4 => Some("c4"),
        ClefType::C5 => Some("c5"),
        ClefType::F => Some("f4"),
        ClefType::FBaritone => Some("f3"),
        ClefType::FSubbass => Some("f5"),
        _ => None,
    }
}

/// Staff with most notes; ties go to the lowest index. Voices other than 0
/// and chords with several notes are dropped (documented loss).
fn choose_staff(score: &Score) -> usize {
    let mut counts = vec![0usize; score.staves.len()];
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                if track.voice() == 0 {
                    let index = track.staff().index();
                    if let Some(count) = counts.get_mut(index) {
                        *count += chord.notes.len();
                    }
                }
            }
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map_or(0, |(index, _)| index)
}

/// Une partition sans indication de mesure, sur portée à quatre lignes, est
/// traitée comme du chant grégorien : un `inverted-mordent` y est le quilisma
/// de la convention gabctk (`gabctk.py:1051-1095`, choix de l'utilisateur).
/// Ailleurs l'ornement reste un ornement et n'est pas réécrit en `w`.
fn is_chant(score: &Score) -> bool {
    let unmetered = !score.measures.iter().any(|measure| {
        measure.segments.iter().any(|segment| {
            segment
                .iter_elements()
                .any(|(_, item)| matches!(item, SegmentItem::TimeSig(_)))
        })
    });
    unmetered && score.staves.iter().any(|staff| staff.lines == 4)
}

fn collect_events(score: &Score, staff: usize) -> Vec<Event> {
    let chant = is_chant(score);
    let mut events: Vec<Event> = Vec::new();
    for measure in &score.measures {
        let first = events.len();
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                if track.staff().index() != staff || track.voice() != 0 {
                    continue;
                }
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                let Some(&id) = chord.notes.first() else {
                    continue;
                };
                let note = &score.notes[id.index()];
                events.push(Event {
                    note: Some(pitch_step(note.pitch, note.tpc)),
                    duration: chord_rest.duration,
                    text: lyric_text(chord_rest),
                    space_after: chord_rest.lyrics.first().is_some_and(|lyric| {
                        matches!(lyric.syllabic, LyricsSyllabic::Single | LyricsSyllabic::End)
                    }),
                    bar: None,
                    quilisma: chant
                        && chord
                            .ornaments
                            .iter()
                            .any(|ornament| ornament.kind == OrnamentType::InvertedMordent),
                    episema: chord
                        .articulations
                        .iter()
                        .any(|articulation| articulation.kind == ArticulationType::Tenuto),
                });
            }
        }
        if let Some(bar) = bar_kind(measure.end_bar_line) {
            let count = events.len();
            if count > first {
                if let Some(last) = events.last_mut() {
                    last.bar = Some(bar);
                    continue;
                }
            }
            events.push(Event {
                note: None,
                duration: Duration::quarter(),
                text: String::new(),
                space_after: false,
                bar: Some(bar),
                quilisma: false,
                episema: false,
            });
        }
    }
    events
}

/// First verse of a chord: the syllable text and its word-space flag
/// (`Gabc.tex:241-245,314-317`). Melismas keep the text on the first note.
fn lyric_text(chord_rest: &scoretk_core::model::ChordRest) -> String {
    chord_rest
        .lyrics
        .iter()
        .find(|lyric| lyric.verse == 0)
        .map_or(String::new(), |lyric| lyric.text.trim().to_owned())
}

/// One MIDI note + tonal class → a diatonic step (C = 0) and the semitone
/// difference from that step's natural pitch. The tpc gives the letter
/// (`tpc % 7` maps `13..=19` to `F,C,G,D,A,E,B`), the pitch gives the octave.
fn pitch_step(pitch: u8, tpc: i8) -> (i32, i32) {
    let natural_index = i32::from((tpc.rem_euclid(7) * 4) % 7);
    let semitone = SEMITONES[natural_index as usize];
    let target = pitch as i32;
    let guess = (target - 60 - semitone).div_euclid(12);
    let mut octave = guess;
    for candidate in [guess - 1, guess + 1] {
        let current = (60 + semitone + 12 * octave - target).abs();
        if (60 + semitone + 12 * candidate - target).abs() < current {
            octave = candidate;
        }
    }
    let difference = target - (60 + semitone + 12 * octave);
    (7 * octave + natural_index, difference)
}

/// Inverts `diatonic_step` for one clef
/// (`src/struct.c:1273-1288`; `src/struct.h:809-813`).
fn letter_for_height(height: i32) -> char {
    char::from(b'a' + (height.clamp(3, 16) - 3) as u8)
}

/// Traduit une durée du pivot en signes GABC (`Gabc.tex:742-797`) : le
/// punctum mora `.` double la note, l'épisème `_` est le tenuto (joué ×1,5
/// dans la vue moderne). C'est la réciproque de `derived_duration` dans le
/// noyau ; sans elle, un aller-retour par du MusicXML standard perdrait tous
/// les écarts de durée. La figure pointée sans tenuto reste acceptée comme
/// entrée tiers : elle devient aussi un épisème, jamais un point GABC.
fn signs_for_duration(duration: Duration, episema: bool) -> (String, Vec<NeumeSign>) {
    let has_mora = duration.ticks() >= Duration::half().ticks();
    let has_episema = episema || duration.is_dotted();
    let mut glyph = String::new();
    let mut signs = Vec::new();
    if has_mora {
        glyph.push('.');
        signs.push(NeumeSign::new(SignKind::PunctumMora, "."));
    }
    if has_episema {
        glyph.push('_');
        signs.push(NeumeSign::new(SignKind::HorizontalEpisema, "_"));
    }
    (glyph, signs)
}

/// Picks the clef that keeps most notes inside the staff's letter range;
/// ties go to the order of [`CLEFS`].
fn choose_clef(events: &[Event], max_height: i32) -> &'static str {
    CLEFS
        .iter()
        .enumerate()
        .min_by_key(|(rank, clef)| {
            let key = syntax::clef_key(clef);
            // Une seule passe : éviter un `Vec` par clé réduit le code
            // compilé pour wasm autant que les allocations.
            let mut outside = 0;
            let mut lowest = i32::MAX;
            let mut highest = i32::MIN;
            for event in events {
                if let Some((step, _)) = event.note {
                    let height = step + key + 5;
                    if !(3..=max_height).contains(&height) {
                        outside += 1;
                    }
                    lowest = lowest.min(height);
                    highest = highest.max(height);
                }
            }
            let span = if lowest <= highest {
                highest - lowest
            } else {
                0
            };
            (outside, span, *rank)
        })
        .map_or("c3", |(_, clef)| *clef)
}

fn bar_kind(bar: BarLineType) -> Option<BarKind> {
    use BarLineType as B;
    match bar {
        B::Normal => Some(BarKind::DivisioMinima),
        B::Double | B::DoubleShort | B::DoubleDotted => Some(BarKind::DivisioMaior),
        B::End => Some(BarKind::DivisioFinalis),
        B::StartRepeat | B::EndRepeat | B::EndStartRepeat => Some(BarKind::DivisioMaior),
        B::Dotted | B::Dashed => Some(BarKind::DivisioMaiorDot),
        // `Tick` est le style MusicXML de la divisio minima (`,`), `Short`
        // celui de la divisio minor (`;`) : les deux redeviennent des barres
        // de séparation GABC, pas des mesures métriques.
        B::Tick | B::Broken => Some(BarKind::DivisioMinima),
        B::Short => Some(BarKind::DivisioMinor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        Chord, ChordRest, Duration, Fraction, Lyrics, Measure, Note, Ornament, Part, Segment,
        SegmentType, Track,
    };

    fn score_with_notes(
        pitches: &[(u8, i8)],
        lyrics: &[&str],
        syllabics: &[LyricsSyllabic],
    ) -> Score {
        let mut score = Score::new();
        score.metadata.work_title = "Cantus".to_owned();
        let part = score.add_part(Part::new("Voice", "Vo"));
        let staff = score.add_staff(part);
        score.staves[staff.index()].lines = 4;
        let track = Track::new(staff, 0);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 1));
        for (index, &(pitch, tpc)) in pitches.iter().enumerate() {
            let id = score.add_note(Note::new(pitch, tpc));
            let mut chord_rest = ChordRest::chord(Duration::quarter());
            chord_rest.kind = ChordRestKind::Chord(Chord {
                notes: vec![id],
                ..Chord::default()
            });
            if let Some(text) = lyrics.get(index) {
                chord_rest.lyrics.push(Lyrics::new(
                    0,
                    *syllabics.get(index).unwrap_or(&LyricsSyllabic::Single),
                    (*text).to_owned(),
                ));
            }
            let mut segment =
                Segment::new(SegmentType::ChordRest, Fraction::new(index as i64, 4), 1);
            segment.set_element(track, SegmentItem::ChordRest(chord_rest));
            measure.add_segment(segment);
        }
        measure.end_bar_line = BarLineType::End;
        score.add_measure(measure);
        score
    }

    #[test]
    fn concert_pitches_become_letters_under_a_chosen_clef() {
        let score = score_with_notes(&[(60, 14), (62, 16), (64, 18)], &["Do", "re", "mi"], &[]);
        let gregorian = from_score(&score);
        assert_eq!(gregorian.clefs, vec!["c3"]);
        assert_eq!(gregorian.syllables.len(), 3);
        let letters: Vec<char> = gregorian
            .syllables
            .iter()
            .map(|syllable| syllable.notes[0].letter)
            .collect();
        assert_eq!(letters, ['h', 'i', 'j']);
        assert_eq!(gregorian.syllables[0].text, "Do");
        assert_eq!(gregorian.syllables[1].text, "re");
        assert_eq!(gregorian.syllables[2].text, "mi");
        assert_eq!(gregorian.syllables[2].bars, vec![BarKind::DivisioFinalis]);
        assert!(gregorian.syllables[0].space_after);
        assert_eq!(gregorian.headers[0].value, "Cantus");
    }

    #[test]
    fn accidentals_and_word_spaces_are_kept() {
        let score = score_with_notes(
            &[(70, 12), (71, 19)],
            &["B", "flat"],
            &[LyricsSyllabic::Single, LyricsSyllabic::Middle],
        );
        let gregorian = from_score(&score);
        // B flat is a separate GABC alteration glyph, not another letter.
        assert_eq!(gregorian.syllables[0].notes[0].glyph, "x");
        assert_eq!(gregorian.syllables[1].notes[0].glyph, "");
        assert!(gregorian.syllables[0].space_after);
        assert!(!gregorian.syllables[1].space_after);
    }

    #[test]
    fn an_inverted_mordent_on_a_chant_note_becomes_a_quilisma() {
        // L'aller-retour par du MusicXML standard : `<ornaments>` est le
        // seul transport du quilisma quand les champs privés `gabc:` sont
        // retirés (`docs/agents/gabc.md` §9.1).
        let mut score = score_with_notes(&[(60, 14)], &["Do"], &[]);
        let track = Track::new(score.staves[0].id, 0);
        let measure = &mut score.measures[0];
        let segment = measure
            .segments
            .iter_mut()
            .find(|segment| segment.element(track).is_some())
            .expect("a note segment");
        if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element_mut(track) {
            if let Some(chord) = chord_rest.chord_mut() {
                chord
                    .ornaments
                    .push(Ornament::new(OrnamentType::InvertedMordent));
            }
        }
        let gregorian = from_score(&score);
        let note = &gregorian.syllables[0].notes[0];
        assert_eq!(note.glyph, "w");
        assert_eq!(note.signs, vec![NeumeSign::new(SignKind::Quilisma, "w")]);
    }

    #[test]
    fn a_tenuto_on_a_plain_quarter_becomes_an_episema_not_a_dot() {
        // L'inverse MusicXML → GABC : `<type>quarter</type>` sans `<dot/>` mais
        // avec `<tenuto/>` (durée d'exécution ×1,5) doit réémettre `_`.
        let mut score = score_with_notes(&[(60, 14)], &["Do"], &[]);
        let track = Track::new(score.staves[0].id, 0);
        let segment = score.measures[0]
            .segments
            .iter_mut()
            .find(|segment| segment.element(track).is_some())
            .expect("a note segment");
        if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element_mut(track) {
            if let Some(chord) = chord_rest.chord_mut() {
                chord
                    .articulations
                    .push(scoretk_core::model::Articulation::new(
                        ArticulationType::Tenuto,
                    ));
            }
        }
        let gregorian = from_score(&score);
        let note = &gregorian.syllables[0].notes[0];
        assert_eq!(note.glyph, "_");
        assert_eq!(
            note.signs,
            vec![NeumeSign::new(SignKind::HorizontalEpisema, "_")]
        );
    }

    #[test]
    fn an_empty_score_still_yields_a_valid_header_block() {
        let gregorian = from_score(&Score::new());
        assert_eq!(gregorian.headers[0].value, "Cantus");
        assert!(gregorian.syllables.is_empty());
    }

    #[test]
    fn clef_choice_favours_the_four_line_staff_range() {
        let events = (48..=72)
            .map(|pitch| Event {
                note: Some(pitch_step(pitch, 14)),
                duration: Duration::quarter(),
                text: String::new(),
                space_after: false,
                bar: None,
                quilisma: false,
                episema: false,
            })
            .collect::<Vec<_>>();
        assert!(["c2", "c3", "c4", "f3", "f4"].contains(&choose_clef(&events, 15)));
    }
}
