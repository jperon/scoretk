// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Reconnaissance des instruments pour la lecture MuseScore.
//!
//! Les données du catalogue vivent dans `scoretk_core::instruments` : elles
//! servent aussi à l'import MIDI (choix du gabarit, transposition). Ici ne
//! subsiste que la logique de reconnaissance propre au lecteur 3.x.

use scoretk_core::instruments::{template_for_id, Template, TEMPLATES};

/// Reconnaît l'identifiant MusicXML et le programme d'un instrument
/// (`Instrument::updateInstrumentId`).
pub(crate) fn recognize(
    music_xml_id: &str,
    names: [&str; 3],
    program: Option<i16>,
) -> (String, Option<i16>) {
    let by_name = template_for_names(&names);
    let music_xml_id = if !music_xml_id.is_empty() {
        music_xml_id
    } else if let Some(template) = by_name {
        template.music_xml_id
    } else if let Some(program) = program {
        template_for_program(program).map_or(DEFAULT_MUSIC_XML_ID, |template| template.music_xml_id)
    } else if names
        .iter()
        .any(|name| name.to_lowercase().contains("piano"))
    {
        "keyboard.piano"
    } else {
        DEFAULT_MUSIC_XML_ID
    };

    let program = program.or_else(|| {
        by_name
            .filter(|template| template.music_xml_id == music_xml_id)
            .and_then(|template| template.programs.first().copied())
            .or_else(|| {
                template_for_music_xml_id(music_xml_id)
                    .and_then(|template| template.programs.first().copied())
            })
    });

    (music_xml_id.to_owned(), program)
}

const DEFAULT_MUSIC_XML_ID: &str = "keyboard.piano";

/// `searchTemplateForInstrNameList` : poids 4/2/1, premier à égalité.
fn template_for_names(names: &[&str; 3]) -> Option<&'static Template> {
    let mut best = None;
    let mut best_strength = 0;
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        for template in TEMPLATES {
            let strength = 4 * u8::from(template.track_name.eq_ignore_ascii_case(name))
                + 2 * u8::from(template.long_name.eq_ignore_ascii_case(name))
                + u8::from(template.short_name.eq_ignore_ascii_case(name));
            if strength > best_strength {
                best = Some(template);
                best_strength = strength;
            }
        }
    }
    if best.is_none()
        && names
            .iter()
            .any(|name| name.to_lowercase().contains("piano"))
    {
        return template_for_id("piano");
    }
    best.filter(|template| !template.music_xml_id.is_empty())
}

/// Premier gabarit dont un canal porte le programme donné.
fn template_for_program(program: i16) -> Option<&'static Template> {
    TEMPLATES
        .iter()
        .find(|template| template.programs.contains(&program) && !template.music_xml_id.is_empty())
}

/// Instrument le plus générique portant un identifiant MusicXML donné.
fn template_for_music_xml_id(music_xml_id: &str) -> Option<&'static Template> {
    TEMPLATES
        .iter()
        .filter(|template| template.music_xml_id == music_xml_id)
        .min_by_key(|template| template.id.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recognized(
        music_xml_id: &str,
        names: [&str; 3],
        program: Option<i16>,
    ) -> (String, Option<i16>) {
        recognize(music_xml_id, names, program)
    }

    #[test]
    fn reconnait_par_le_nom_de_piste() {
        assert_eq!(
            recognized("", ["Flute", "", ""], Some(73)),
            ("wind.flutes.flute".to_owned(), Some(73))
        );
        assert_eq!(
            recognized("", ["Piano", "", ""], Some(0)),
            ("keyboard.piano".to_owned(), Some(0))
        );
        assert_eq!(
            recognized("", ["Violin", "", ""], Some(40)),
            ("strings.violin".to_owned(), Some(40))
        );
    }

    #[test]
    fn reconnait_par_le_nom_long_et_le_nom_court() {
        assert_eq!(
            recognized("", ["", "Bass Flute", ""], None).0,
            "wind.flutes.flute.bass"
        );
        assert_eq!(recognized("", ["", "", "Tbn."], None).0, "brass.trombone");
    }

    #[test]
    fn reconnait_le_programme_manquant() {
        assert_eq!(
            recognized("", ["Voice", "", ""], None),
            ("voice.vocals".to_owned(), Some(52))
        );
        assert_eq!(
            recognized("", ["Flute", "", ""], None),
            ("wind.flutes.flute".to_owned(), Some(73))
        );
    }

    #[test]
    fn garde_l_identifiant_ecrit() {
        assert_eq!(
            recognized("wind.reed.clarinet.a", ["A Clarinet", "", ""], Some(71)),
            ("wind.reed.clarinet.a".to_owned(), Some(71))
        );
    }

    #[test]
    fn retombe_sur_le_piano() {
        assert_eq!(
            recognized("", ["Instrument inconnu", "", ""], None),
            ("keyboard.piano".to_owned(), Some(0))
        );
    }
}
