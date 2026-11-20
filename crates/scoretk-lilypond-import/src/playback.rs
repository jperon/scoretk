// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier ne porte pas de code C++ : il détecte, dans un document LilyPond,
// le second `\score` qui ne produit que du MIDI et dont la musique est la même
// que la partition gravée, enveloppée dans `\transpose`. D'après la Notation
// Reference de LilyPond 2.26 : §1.2.2 (« \transpose frompitch topitch musicexpr
// » transposes the interval), §24.3 (« a \score block that contains music and a
// \midi block but no \layout block produces a MIDI output file only ») et
// §20.2 (plusieurs `\score` dans un même fichier). Réécrit en Rust en 2026.
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

//! Détection structurelle de la transposition d'une partition MIDI séparée.
//!
//! Le premier `\score` gravé fournit le modèle ; un second `\score` portant
//! `\midi` sans `\layout` ne sert qu'à renseigner la transposition de lecture.
//! On ne retient un intervalle que si, après avoir retiré les enveloppes
//! `\transpose` de tête, les voix des deux partitions sont **structurellement
//! égales** (mêmes variables, mêmes notes, mêmes durées). Toute autre
//! différente — une note changée, `\unfoldRepeats` non transparent, un nombre
//! de portées différent — fait renoncer à l'enregistrement.

use crate::ast::{Context, GroupKind, Music, PitchSpec, ScoreAst};

/// Écart chromatique en demi-tons d'un `\transpose de vers`.
///
/// Même calcul que `events::transpose_delta` : octave `* 12`, degré selon
/// `[0, 2, 4, 5, 7, 9, 11]`, altération en demi-tons (Notation Reference
/// §1.2.2 : les hauteurs d'entrée sont absolues et les enharmonies comptent).
fn delta(from: &PitchSpec, to: &PitchSpec) -> i32 {
    let semitones = |pitch: &PitchSpec| {
        [0, 2, 4, 5, 7, 9, 11][usize::from(pitch.step.min(6))] + i32::from(pitch.alter)
    };
    semitones(to) - semitones(from) + 12 * (i32::from(to.octave) - i32::from(from.octave))
}

/// Forme des portées : nombre et nature des groupes, nombre de voix par portée.
fn shape(score: &ScoreAst) -> Vec<(GroupKind, Vec<usize>)> {
    score
        .groups
        .iter()
        .map(|group| {
            (
                group.kind.clone(),
                group
                    .staves
                    .iter()
                    .map(|staff| staff.voices.len())
                    .collect(),
            )
        })
        .collect()
}

/// Musique de toutes les voix, dans l'ordre des groupes, portées et voix.
fn voices(score: &ScoreAst) -> Vec<&Music> {
    score
        .groups
        .iter()
        .flat_map(|group| group.staves.iter())
        .flat_map(|staff| staff.voices.iter())
        .map(|voice| &voice.music)
        .collect()
}

/// Retire les `\transpose` de tête d'une musique de voix et renvoie leur écart
/// cumulé. Les enveloppes `\fixed` et `\relative` sont conservées à leur place
/// : ce sont les seules que `parser::score_groups_in` empile avec `\transpose`
/// autour d'une voix de `\score`.
fn strip_transposes(music: &Music) -> (i32, Music) {
    match music {
        Music::Context(Context::Transpose {
            from, to, music, ..
        }) => {
            let (reste, interieur) = strip_transposes(music);
            (delta(from, to) + reste, interieur)
        }
        Music::Context(Context::Fixed { reference, music }) => {
            let (ecart, interieur) = strip_transposes(music);
            (
                ecart,
                Music::Context(Context::Fixed {
                    reference: *reference,
                    music: Box::new(interieur),
                }),
            )
        }
        Music::Context(Context::Relative { reference, music }) => {
            let (ecart, interieur) = strip_transposes(music);
            (
                ecart,
                Music::Context(Context::Relative {
                    reference: *reference,
                    music: Box::new(interieur),
                }),
            )
        }
        _ => (0, music.clone()),
    }
}

/// Intervalle de lecture d'un couple « partition gravée / partition MIDI ».
///
/// Renvoie `None` sauf si les deux partitions ont la même forme, le même nombre
/// de voix, et la même musique une fois les `\transpose` retirés. L'écart
/// renvoyé est celui de la partition MIDI **moins** celui de la partition
/// gravée : les deux peuvent porter un `\transpose` (l'identité `\transpose c c`
/// est courante dans les fichiers de `musicxml2ly`).
pub(crate) fn detect(engraved: &ScoreAst, midi: &ScoreAst) -> Option<i32> {
    if engraved.groups.is_empty() || shape(engraved) != shape(midi) {
        return None;
    }
    let graves = voices(engraved);
    let joues = voices(midi);
    if graves.is_empty() || graves.len() != joues.len() {
        return None;
    }
    let mut intervalle: Option<i32> = None;
    let mut transpose_trouve = false;
    for (grave, joue) in graves.iter().zip(joues.iter()) {
        let (ecart_grave, nu_grave) = strip_transposes(grave);
        let (ecart_joue, nu_joue) = strip_transposes(joue);
        if ecart_grave != 0 || ecart_joue != 0 {
            transpose_trouve = true;
        }
        if nu_grave != nu_joue {
            // Une différence ailleurs que dans l'enveloppe `\transpose` : la
            // partition MIDI n'est pas la même musique, on ne retient rien.
            return None;
        }
        let ecart = ecart_joue - ecart_grave;
        match intervalle {
            None => intervalle = Some(ecart),
            Some(precedent) if precedent != ecart => return None,
            Some(_) => {}
        }
    }
    if transpose_trouve {
        intervalle
    } else {
        None
    }
}
