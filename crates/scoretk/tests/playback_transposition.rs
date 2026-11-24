// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier ne porte pas de code C++ : il vérifie que la transposition de
// lecture trouvée dans un second `\score` MIDI ne touche que l'écriture MIDI,
// d'après la Notation Reference de LilyPond 2.26 (§1.2.2 `\transpose`, §24.3
// « A `\score` block that contains music and a `\midi` block but no `\layout`
// block produces a MIDI output file only »).
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

//! Une source `.ly` avec une partition gravée et une partition MIDI transposée
//! : le modèle reste gravé (MusicXML inchangé), seul le MIDI est transposé.

#![cfg(all(
    feature = "lilypond-import",
    feature = "musicxml-export",
    feature = "midi-export",
    feature = "midi-import"
))]

use scoretk::{read, write, Format, Options};

/// Partition réduite mais ordinaire : deux portées, deux variables, barres de
/// contrôle, accord, silence, liaison, et une partition MIDI séparée qui
/// enveloppe la même musique dans `\transpose c d`.
const SOURCE: &str = r#"
\version "2.24.0"
\language "english"
haut = \fixed c' {
  \key c \major \time 4/4 \clef treble
  c4 d e f | g4 ~ g4 a4 b4 | <c e g>2. r4 |
}
bas = \fixed c {
  \key c \major \time 4/4 \clef bass
  c4 d e f | g4 r4 a4 r4 | c1 |
}
\score {
  <<
    \new Staff = "haut" { \haut }
    \new Staff = "bas" { \bas }
  >>
  \layout {}
}
\score {
  \transpose c d
  <<
    \new Staff = "haut" { \fixed c' {
      \key c \major \time 4/4 \clef treble
      c4 d e f | g4 ~ g4 a4 b4 | <c e g>2. r4 |
    } }
    \new Staff = "bas" { \bas }
  >>
  \midi {}
}
"#;

/// Mêmes notes, sans la seconde partition : référence du MusicXML gravé.
const SOURCE_GRAVE_SEULE: &str = r#"
\version "2.24.0"
\language "english"
haut = \fixed c' {
  \key c \major \time 4/4 \clef treble
  c4 d e f | g4 ~ g4 a4 b4 | <c e g>2. r4 |
}
bas = \fixed c {
  \key c \major \time 4/4 \clef bass
  c4 d e f | g4 r4 a4 r4 | c1 |
}
\score {
  <<
    \new Staff = "haut" { \haut }
    \new Staff = "bas" { \bas }
  >>
  \layout {}
}
"#;

fn hauteurs_triees(source: &str) -> Vec<u8> {
    let score = read(source.as_bytes(), Format::Ly).expect("lecture .ly");
    let mut pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    pitches.sort_unstable();
    pitches
}

#[test]
fn transposition_de_lecture_midi_seulement() {
    let score = read(SOURCE.as_bytes(), Format::Ly).expect("lecture .ly");
    // NR §24.3 : le second `\score`, qui porte `\midi` sans `\layout`, est
    // le seul que LilyPond joue : c'est sa musique transposée qui fournit le
    // modèle, déjà en hauteur de concert.
    assert_eq!(
        score.playback.transposition, None,
        "le modèle est déjà la partition MIDI transposée"
    );

    let gravé = read(SOURCE_GRAVE_SEULE.as_bytes(), Format::Ly).expect("lecture gravée");
    assert_eq!(score.notes.len(), 18, "18 notes écrites");
    assert_eq!(score.measures.len(), 3, "trois mesures");
    // Une octave et un ton au-dessus de la partition gravée (transposition c→d).
    assert_eq!(
        hauteurs_triees(SOURCE),
        hauteurs_triees(SOURCE_GRAVE_SEULE)
            .into_iter()
            .map(|pitch| pitch + 2)
            .collect::<Vec<_>>()
    );
    let durees: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            segment.chord_rest(scoretk_core::model::Track::new(score.staves[0].id, 0))
        })
        .map(|chord_rest| chord_rest.duration.ticks())
        .collect();
    assert_eq!(
        durees.len(),
        10,
        "dix accords/silences sur la première portée"
    );
    assert_eq!(durees[0], scoretk_core::model::Fraction::QUARTER);
    assert_eq!(
        durees[8],
        scoretk_core::model::Fraction::HALF + scoretk_core::model::Fraction::QUARTER,
        "accord pointé en fin de première portée"
    );

    // Le modèle suit maintenant la partition MIDI : le MusicXML est transposé
    // d'un ton par rapport au score gravé, comme la référence MIDI.
    let xml = write(&score, Format::MusicXml, &Options::default()).expect("export MusicXML");
    let xml_grave = write(&gravé, Format::MusicXml, &Options::default()).expect("export gravé");
    assert_ne!(xml, xml_grave, "le MusicXML suit la partition MIDI");

    // MIDI : seules les hauteurs sont décalées d'un ton, pas les clés (`FF 59`).
    let midi = write(&score, Format::Midi, &Options::default()).expect("export MIDI");
    let relu = read(&midi, Format::Midi).expect("relecture MIDI");
    let mut jouees: Vec<u8> = relu.notes.iter().map(|note| note.pitch).collect();
    jouees.sort_unstable();
    // Le lecteur MIDI fusionne les notes liées (`tie_back`) en une seule
    // attaque : on retire donc la seconde `g` de la référence.
    let mut attendues: Vec<u8> = read(SOURCE_GRAVE_SEULE.as_bytes(), Format::Ly)
        .expect("lecture gravée")
        .notes
        .iter()
        .filter(|note| note.tie_back.is_none())
        .map(|note| note.pitch + 2)
        .collect();
    attendues.sort_unstable();
    assert_eq!(
        jouees, attendues,
        "le MIDI joue un ton au-dessus de la notation"
    );
}

#[test]
fn transposition_non_enregistree_si_la_partition_midi_differe() {
    // Une note diffère dans la partition MIDI : ce n'est plus la même musique,
    // on ne doit rien enregistrer (et donc rien transposer).
    let different = SOURCE.replace(
        "\\new Staff = \"haut\" { \\fixed c' {",
        "\\new Staff = \"haut\" { \\fixed c' { c,4",
    );
    let score = read(different.as_bytes(), Format::Ly).expect("lecture .ly");
    assert_eq!(
        score.playback.transposition, None,
        "une partition MIDI différente ne doit pas être prise comme référence"
    );
    let midi = write(&score, Format::Midi, &Options::default()).expect("export MIDI");
    let relu = read(&midi, Format::Midi).expect("relecture MIDI");
    let mut jouees: Vec<u8> = relu.notes.iter().map(|note| note.pitch).collect();
    jouees.sort_unstable();
    let mut attendues = hauteurs_triees(&different);
    for pitch in &mut attendues {
        *pitch += 2;
    }
    assert_ne!(jouees, attendues);

    // Cas limite : la seconde partition porte `\layout` aussi : elle est
    // gravée, il n'y a pas de partition « MIDI only » à détecter.
    let avec_layout = SOURCE.replace("  \\midi {}", "  \\layout {}\n  \\midi {}");
    let score = read(avec_layout.as_bytes(), Format::Ly).expect("lecture .ly");
    assert_eq!(score.playback.transposition, None);

    // Cas non concerné : une seule partition, sans `\midi`.
    let seul = read(SOURCE_GRAVE_SEULE.as_bytes(), Format::Ly).expect("lecture .ly");
    assert_eq!(seul.playback.transposition, None);
}
