// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `ly/drumpitch-init.ly` (`midiDrumPitches`, `drumPitchNames`) et de
// `scm/chord-entry.scm` (`construct-chord-elements`) de LilyPond 2.26.0,
// Copyright (C) 1998--2026 Han-Wen Nienhuys et al., GPL-3.0-or-later ;
// réécriture en Rust en 2026. Les hauteurs MIDI ont été relevées sur le MIDI
// produit par LilyPond 2.27 pour chaque nom, afin de suivre le comportement
// audible plutôt que la seule table Scheme.
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

//! Tables des modes `\drummode` et `\chordmode`.
//!
//! Dans ces modes, les noms (`bd`, `sn`, `c:m7`…) ne sont pas des hauteurs de
//! la langue courante mais des événements : LilyPond les transforme en notes
//! jouées (MIDI). Le lecteur en sort donc des notes à hauteur MIDI explicite,
//! sans passer par la notation diatonique ni par `\relative`.

use crate::pitch::{midi_of, Language};

/// Nom court et nom long d'une percussion, avec sa hauteur MIDI General MIDI.
///
/// Les deux colonnes de `drumPitchNames` (alias et nom canonique) sont
/// aplaties : `bd` et `bassdrum` valent tous deux 36, comme dans le MIDI de
/// LilyPond.
const DRUMS: &[(&str, u8)] = &[
    ("acousticbassdrum", 35),
    ("bassdrum", 36),
    ("hisidestick", 37),
    ("sidestick", 37),
    ("losidestick", 37),
    ("acousticsnare", 38),
    ("snare", 38),
    ("handclap", 39),
    ("electricsnare", 40),
    ("lowfloortom", 41),
    ("closedhihat", 42),
    ("hihat", 42),
    ("highfloortom", 43),
    ("pedalhihat", 44),
    ("splashhihat", 44),
    ("lowtom", 45),
    ("openhihat", 46),
    ("halfopenhihat", 46),
    ("lowmidtom", 47),
    ("himidtom", 48),
    ("crashcymbala", 49),
    ("crashcymbal", 49),
    ("hightom", 50),
    ("ridecymbala", 51),
    ("ridecymbal", 51),
    ("chinesecymbal", 52),
    ("ridebell", 53),
    ("tambourine", 54),
    ("splashcymbal", 55),
    ("cowbell", 56),
    ("crashcymbalb", 57),
    ("vibraslap", 58),
    ("ridecymbalb", 59),
    ("mutehibongo", 60),
    ("hibongo", 60),
    ("openhibongo", 60),
    ("mutelobongo", 61),
    ("lobongo", 61),
    ("openlobongo", 61),
    ("mutehiconga", 62),
    ("muteloconga", 62),
    ("openhiconga", 63),
    ("hiconga", 63),
    ("openloconga", 64),
    ("loconga", 64),
    ("hitimbale", 65),
    ("lotimbale", 66),
    ("hiagogo", 67),
    ("loagogo", 68),
    ("cabasa", 69),
    ("maracas", 70),
    ("shortwhistle", 71),
    ("longwhistle", 72),
    ("shortguiro", 73),
    ("longguiro", 74),
    ("guiro", 74),
    ("claves", 75),
    ("hiwoodblock", 76),
    ("lowoodblock", 77),
    ("mutecuica", 78),
    ("opencuica", 79),
    ("mutetriangle", 80),
    ("triangle", 81),
    ("opentriangle", 81),
    // Alias de `drumPitchNames`.
    ("bda", 35),
    ("bd", 36),
    ("ssh", 37),
    ("ss", 37),
    ("ssl", 37),
    ("sna", 38),
    ("sn", 38),
    ("hc", 39),
    ("sne", 40),
    ("tomfl", 41),
    ("hhc", 42),
    ("hh", 42),
    ("tomfh", 43),
    ("hhp", 44),
    ("hhs", 44),
    ("toml", 45),
    ("hho", 46),
    ("hhho", 46),
    ("tomml", 47),
    ("tommh", 48),
    ("cymca", 49),
    ("cymc", 49),
    ("tomh", 50),
    ("cymra", 51),
    ("cymr", 51),
    ("cymch", 52),
    ("rb", 53),
    ("tamb", 54),
    ("cyms", 55),
    ("cb", 56),
    ("cymcb", 57),
    ("vibs", 58),
    ("cymrb", 59),
    ("bohm", 60),
    ("boh", 60),
    ("boho", 60),
    ("bolm", 61),
    ("bol", 61),
    ("bolo", 61),
    ("cghm", 62),
    ("cglm", 62),
    ("cgho", 63),
    ("cgh", 63),
    ("cglo", 64),
    ("cgl", 64),
    ("timh", 65),
    ("timl", 66),
    ("agh", 67),
    ("agl", 68),
    ("cab", 69),
    ("mar", 70),
    ("whs", 71),
    ("whl", 72),
    ("guis", 73),
    ("guil", 74),
    ("gui", 74),
    ("cl", 75),
    ("wbh", 76),
    ("wbl", 77),
    ("cuim", 78),
    ("cuio", 79),
    ("trim", 80),
    ("tri", 81),
    ("trio", 81),
];

/// Hauteur MIDI d'un nom de percussion, ou `None` pour un mot inconnu.
pub(crate) fn drum_pitch(name: &str) -> Option<u8> {
    DRUMS
        .iter()
        .find(|(candidat, _)| *candidat == name)
        .map(|(_, pitch)| *pitch)
}

/// Intervalles d'une qualité d'accord, en demi-tons depuis la fondamentale.
///
/// Les qualités couvertes sont celles de `scm/chord-entry.scm` les plus
/// courantes ; les modificateurs composés (`:m7.5-`, `:7sus4`) sont acceptés.
/// `m`/`min` ne figurent pas ici en entrées figées par degré : ce sont des
/// modificateurs leading appliqués à la qualité qui suit (voir
/// [`chord_pitches`]).
const QUALITIES: &[(&str, &[i8])] = &[
    ("maj", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("min", &[0, 3, 7]),
    ("7", &[0, 4, 7, 10]),
    ("m7", &[0, 3, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
    ("9", &[0, 4, 7, 10, 14]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("dim7", &[0, 3, 6, 9]),
    ("m7.5-", &[0, 3, 6, 10]),
    // `the-canonical-chord` et `stack-thirds` (`scm/chord-entry.scm:270-278`)
    // empilent les tierces jusqu'au degré demandé. Le 11 naturel est retiré
    // plus bas quand la tierce majeure est présente sans 11 explicite
    // (`scm/chord-entry.scm:154-159`) : `c:13` n'a donc pas de fa, mais
    // `c:min11`/`c:min13` gardent le leur (vérifié sur le MIDI de LilyPond).
    ("11", &[0, 4, 7, 10, 14, 17]),
    ("13", &[0, 4, 7, 10, 14, 17, 21]),
    ("7.9", &[0, 4, 7, 10, 14]),
    ("5", &[0, 7]),
    ("1", &[0]),
];

/// Hauteurs MIDI d'un nom d'accord LilyPond (`c`, `c:m7`, `c/g`, `d:maj7`).
///
/// La fondamentale est toujours au do central (60 pour `c`), comme dans le
/// MIDI de `\chordmode` de LilyPond. La basse après `/` est placée sous la
/// fondamentale, comme `process-inversion` de `chord-entry.scm`.
pub(crate) fn chord_pitches(language: Language, name: &str) -> Option<(Vec<u8>, Option<u8>)> {
    let (head, basse) = name
        .split_once('/')
        .map_or((name, None), |(h, b)| (h, Some(b)));
    // `/+d` : basse explicite (`scm/chord-entry.scm` l. 46-53,
    // `interpret-bass`). Le `+` est un marqueur de basse, pas une altération.
    // `/+d` ajoute la basse à l'accord fondamental ; `/d` est un
    // renversement (`process-inversion`).
    let (basse, explicite) = match basse {
        Some(b) => match b.strip_prefix('+') {
            Some(rest) => (Some(rest), true),
            None => (Some(b), false),
        },
        None => (None, false),
    };
    let (racine, qualite) = head.split_once(':').map_or((head, "maj"), |(r, q)| (r, q));
    let (step, alter) = language.parse(racine)?;
    let root = midi_of(step, alter, 4)?;
    // `scm/chord-entry.scm:125-145` : un modificateur leading (`m`/`min`,
    // `default-chord-modifier-list` l. 252) est extrait AVANT le degré, puis
    // appliqué à l'accord de base construit depuis ce degré. On ne fige donc
    // pas `min7`/`min9`/`min6` dans la table : `min11`, `min13` et `min7.9`
    // fonctionnent de la même manière.
    let mut base = qualite;
    let mut mineur = false;
    for prefixe in ["min", "m"] {
        if let Some(reste) = base.strip_prefix(prefixe) {
            let reste_connu =
                reste.is_empty() || QUALITIES.iter().any(|(candidat, _)| *candidat == reste);
            if reste_connu {
                base = reste;
                mineur = true;
                break;
            }
        }
    }
    let intervalles = QUALITIES
        .iter()
        .find(|(candidat, _)| *candidat == base)
        .map(|(_, values)| *values)
        .unwrap_or(&[0, 4, 7]);
    // `scm/chord-entry.scm:154-159` : un 11 naturel avec une tierce majeure
    // non explicitée est retiré. Seul `:13` porte un 11 implicite dans notre
    // table ; `:11` l'explicite et une tierce mineure n'est pas « naturelle ».
    let retire_onzieme = base == "13" && !mineur;
    let notes: Vec<u8> = intervalles
        .iter()
        .filter(|ecart| !(retire_onzieme && **ecart == 17))
        .map(|ecart| {
            // `minor-modifier` (`scm/chord-entry.scm:235-236`) : la tierce
            // majeure (+4) devient mineure (+3).
            let ecart = if mineur && *ecart == 4 { 3 } else { *ecart };
            i32::from(root).checked_add(i32::from(ecart))
        })
        .filter_map(|p| p.and_then(|p| u8::try_from(p).ok()))
        .collect();
    let basse_midi = basse.and_then(|b| chord_bass_pitch(language, step, b));
    // Renversement : `process-inversion` retire la note de basse de l'accord
    // et `make-chord-elements` (`chord-entry.scm` l. 100-140) ramène les
    // autres notes dans l'octave au-dessus d'elle : `bes/d` = ré4, fa4, sib4,
    // et non l'accord fondamental plus une basse. Une basse explicite
    // (`/+d`) ou étrangère à l'accord garde l'accord fondamental.
    let notes = match basse_midi {
        Some(basse) if !explicite && notes.iter().any(|p| p % 12 == basse % 12) => {
            let mut reste: Vec<u8> = notes
                .iter()
                .copied()
                .filter(|p| p % 12 != basse % 12)
                .collect();
            let pc_basse = basse % 12;
            reste.sort_by_key(|p| (p % 12 + 12 - pc_basse) % 12);
            let mut curseur = basse;
            let mut renverse = Vec::with_capacity(reste.len());
            for note in reste {
                let mut valeur = basse + (note % 12 + 12 - pc_basse) % 12;
                if valeur <= curseur {
                    valeur += 12;
                }
                renverse.push(valeur);
                curseur = valeur;
            }
            renverse
        }
        _ => notes,
    };
    Some((notes, basse_midi))
}

/// Hauteur de la basse d'un accord `/b`, strictement sous la fondamentale
/// (`process-inversion` dans `scm/chord-entry.scm`). Quand le degré de la
/// basse est égal ou supérieur à celui de la racine, la basse descend d'une
/// octave : `b/b` joue si3, pas si4.
pub(crate) fn chord_bass_pitch(language: Language, root_step: u8, bass: &str) -> Option<u8> {
    let (step, alter) = language.parse(bass)?;
    let naturel = i32::from(midi_of(step, alter, 4)?);
    let dessous = if step >= root_step {
        naturel - 12
    } else {
        naturel
    };
    u8::try_from(dessous).ok()
}

#[cfg(test)]
mod tests {
    use super::{chord_pitches, drum_pitch};
    use crate::pitch::Language;

    #[test]
    fn noms_de_percussions() {
        assert_eq!(drum_pitch("bd"), Some(36));
        assert_eq!(drum_pitch("sn"), Some(38));
        assert_eq!(drum_pitch("hh"), Some(42));
        assert_eq!(drum_pitch("cymr"), Some(51));
        assert_eq!(drum_pitch("inconnu"), None);
    }

    #[test]
    fn noms_daccords() {
        let langue = Language::Nederlands;
        assert_eq!(chord_pitches(langue, "c"), Some((vec![60, 64, 67], None)));
        assert_eq!(
            chord_pitches(langue, "c:m7"),
            Some((vec![60, 63, 67, 70], None))
        );
        assert_eq!(
            chord_pitches(langue, "c:dim7"),
            Some((vec![60, 63, 66, 69], None))
        );
        assert_eq!(
            chord_pitches(langue, "d:maj7"),
            Some((vec![62, 66, 69, 73], None))
        );
        // `min` est un modificateur, pas une entrée figée : il s'applique à
        // n'importe quel degré, y compris composé.
        assert_eq!(
            chord_pitches(langue, "c:min11"),
            Some((vec![60, 63, 67, 70, 74, 77], None))
        );
        assert_eq!(
            chord_pitches(langue, "c:min13"),
            Some((vec![60, 63, 67, 70, 74, 77, 81], None))
        );
        assert_eq!(
            chord_pitches(langue, "c:min7.9"),
            Some((vec![60, 63, 67, 70, 74], None))
        );
        assert_eq!(
            chord_pitches(langue, "c:13"),
            Some((vec![60, 64, 67, 70, 74, 81], None)),
            "le 11 implicite est retiré d'une tierce majeure"
        );
        assert_eq!(
            chord_pitches(langue, "c:maj"),
            Some((vec![60, 64, 67], None)),
            "`maj` ne doit pas être pris pour le modificateur `m`"
        );
        // `c/g` est un renversement : sol3, do4, mi4.
        assert_eq!(chord_pitches(langue, "c/g"), Some((vec![60, 64], Some(55))));
        // `/+d` : basse explicite (`scm/chord-entry.scm` l. 46-53,
        // `interpret-bass`), distincte d'un ajout. Le `+` ne fait pas partie
        // du nom de hauteur.
        assert_eq!(
            chord_pitches(langue, "bes/+d"),
            Some((vec![70, 74, 77], Some(62)))
        );
        // Renversement : `bes/d` = ré4, fa4, sib4 (basse + notes ramenées
        // au-dessus), pas l'accord fondamental plus une basse.
        assert_eq!(
            chord_pitches(langue, "bes/d"),
            Some((vec![65, 70], Some(62)))
        );
        assert_eq!(chord_pitches(langue, "inconnu"), None);
    }
}
