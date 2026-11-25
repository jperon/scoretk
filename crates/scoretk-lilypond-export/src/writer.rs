// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/part.h`, `engraving/dom/staff.h` et
// `engraving/dom/score.h` de MuseScore Studio, Copyright (C) MuseScore Limited
// and others, modifié (réécrit en Rust) en 2026 : regroupement des portées et
// en-tête du document.
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

//! Assemblage du document LilyPond : en-tête, variables de voix, bloc
//! `\score`.
//!
//! Le regroupement des portées suit les accolades de MuseScore
//! (`StaffBracket` lu dans `<bracket>`) : une accolade donne `PianoStaff`, un
//! crochet `ChoirStaff`, plusieurs portées sans accolade `StaffGroup`.

use std::fmt::Write as _;

use scoretk_core::model::{CreditStyle, Score, Staff};

use crate::structure;
use crate::text;
use crate::voice;
use scoretk_lilypond_common::{Error, Result};

/// Version de LilyPond annoncée dans le document.
///
/// 2.24 est la version stable de référence du projet ; les versions plus
/// récentes compilent le fichier après leurs conversions habituelles.
pub(crate) const LILYPOND_VERSION: &str = "2.24.0";

/// Assembleur de document.
pub(crate) struct Writer<'a> {
    score: &'a Score,
}

impl<'a> Writer<'a> {
    /// Prépare l'écriture d'une partition.
    pub(crate) fn new(score: &'a Score) -> Self {
        Self { score }
    }

    /// Produit le document complet.
    pub(crate) fn render(self) -> Result<String> {
        if self.score.staves.is_empty() {
            return Err(Error::PartitionVide);
        }
        let structure = structure::build(self.score);
        let mut out = String::new();

        let _ = writeln!(out, "\\version \"{LILYPOND_VERSION}\"");
        out.push_str("\\language \"english\"\n\n");
        // Noter le spatium par la taille globale de portée (§27.2) plutôt
        // que par un réglage `\paper` : c'est la seule écriture qui change
        // effectivement la taille de la portée et de toutes les polices.
        let style = &self.score.engraving.style;
        let defaults = scoretk_core::model::StyleSettings::default();
        if (style.spatium_mm - defaults.spatium_mm).abs() > 1e-9 {
            let points = style.spatium_mm * 4.0 * 7227.0 / 2540.0;
            let _ = writeln!(out, "#(set-global-staff-size {points:.8})");
        }
        // `ly/articulate.ly` définit le music-function qui raccourcit les
        // notes jouées. Le lecteur pose `Score::playback.articulate` quand le source
        // utilise `\articulate` (`ly/articulate.ly`) ; l'écrire ici conserve
        // les durées MIDI à la relecture du `.ly`.
        if self.score.playback.articulate {
            out.push_str("\\include \"articulate.ly\"\n");
        }

        self.write_header(&mut out);
        self.write_paper(&mut out);
        self.write_variables(&mut out, &structure);
        self.write_score(&mut out);

        Ok(out)
    }

    /// Bloc `\header` : métadonnées, puis crédits du fichier en repli.
    fn write_header(&self, out: &mut String) {
        let metadata = &self.score.metadata;
        let credit = |style: CreditStyle| {
            self.score
                .engraving
                .credits
                .iter()
                .find(|credit| credit.style == style)
                .map(|credit| credit.text.plain().trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        // Les champs de `\header` couvrent les métadonnées du modèle ; les
        // crédits servent de repli pour les trois textes de titre, comme
        // `write_header` s'appuie sur les cadres de MuseScore. Les champs
        // absents de la norme LilyPond (`workTitle`, `movementTitle`…) sont
        // acceptés sans avertissement et permettent à la lecture de retrouver
        // exactement les métadonnées du `.mscx`.
        let mut entries: Vec<(&str, String)> = vec![
            ("title", metadata.work_title.clone()),
            ("workTitle", metadata.work_title.clone()),
            ("subtitle", metadata.subtitle.clone()),
            ("movementTitle", metadata.movement_title.clone()),
            ("movementNumber", metadata.movement_number.clone()),
            ("workNumber", metadata.work_number.clone()),
            ("composer", metadata.composer.clone()),
            // `\header poet` est le champ du parolier (`titling-init.ly:87`).
            // Le modèle distingue `lyricist` et `poet` depuis MuseScore 4 ;
            // on reprend `poet` quand `lyricist` est vide, comme le fait
            // l'import MusicXML de MuseScore (`importmusicxmlpass1.cpp:1143-1146`).
            (
                "poet",
                if metadata.lyricist.trim().is_empty() {
                    metadata.poet.clone()
                } else {
                    metadata.lyricist.clone()
                },
            ),
            ("arranger", metadata.arranger.clone()),
            ("copyright", metadata.copyright.clone()),
            ("source", metadata.source.clone()),
        ];
        // Les tailles de texte n'ont pas de champ `\paper` normalisé
        // (Notation Reference §26.6) : des champs `\header` privés, acceptés
        // sans avertissement par LilyPond, les transportent sans changer le
        // rendu des titres. Voir `builder.rs::apply_text_styles` à la lecture.
        let style = &self.score.engraving.style;
        let defaults = scoretk_core::model::StyleSettings::default();
        for (name, value, baseline) in [
            (
                "scoretkTitleFontSize",
                style.title_font_size,
                defaults.title_font_size,
            ),
            (
                "scoretkComposerFontSize",
                style.composer_font_size,
                defaults.composer_font_size,
            ),
            (
                "scoretkLyricistFontSize",
                style.lyricist_font_size,
                defaults.lyricist_font_size,
            ),
        ] {
            if (value - baseline).abs() > 1e-9 {
                entries.push((name, format!("{value:.5}")));
            }
        }
        let fallbacks = [
            ("title", CreditStyle::Title),
            ("subtitle", CreditStyle::Subtitle),
            ("composer", CreditStyle::Composer),
            ("poet", CreditStyle::Lyricist),
            ("arranger", CreditStyle::Other("arranger".to_owned())),
            ("copyright", CreditStyle::Other("copyright".to_owned())),
        ];
        for (field, style) in fallbacks {
            if let Some(entry) = entries.iter_mut().find(|(name, _)| *name == field) {
                if entry.1.trim().is_empty() {
                    entry.1 = credit(style).unwrap_or_default();
                }
            }
        }

        let mut lines = Vec::new();
        for (field, value) in entries {
            let value = value.trim();
            if !value.is_empty() {
                lines.push(format!("  {field} = \"{}\"", text::escape(value)));
            }
        }
        // Les crédits du modèle peuvent différer des champs de titre (textes
        // de cadre, auteurs multiples,…). Des champs `\header` privés les
        // portent un par un, dans l'ordre, comme les `<credit>` MusicXML.
        // Les identifiants LilyPond ne peuvent pas se terminer par un chiffre
        // (le lexeur sépare alors mot et nombre) : l'ordre est codé A, B, C…
        let rank = |index: usize| char::from(b'A' + u8::try_from(index).unwrap_or(25));
        for (index, credit) in self.score.engraving.credits.iter().enumerate() {
            let text = credit.text.plain().trim().replace(['\n', '\r'], " ");
            if !text.is_empty() {
                lines.push(format!(
                    "  scoretkCredit{} = \"{}\"",
                    rank(index),
                    text::escape(&text)
                ));
            }
        }
        for (index, tbox) in self.score.engraving.tbox_texts.iter().enumerate() {
            let text = tbox.text.plain().trim().replace(['\n', '\r'], " ");
            if !text.is_empty() {
                lines.push(format!(
                    "  scoretkTbox{} = \"{}|{}\"",
                    rank(index),
                    tbox.measure,
                    text::escape(&text)
                ));
            }
        }
        if lines.is_empty() {
            return;
        }
        out.push_str("\\header {\n");
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }

    /// Bloc `\paper` : la géométrie de page que le format porte réellement
    /// (Notation Reference, `\paper` ; `format-mapping/lilypond.md` ligne 39).
    /// LilyPond exprime ces valeurs en millimètres ; le modèle les stocke en
    /// pouces comme MuseScore.
    fn write_paper(&self, out: &mut String) {
        let style = &self.score.engraving.style;
        out.push_str("\\paper {\n");
        for (name, inches) in [
            ("paper-width", style.page_width_in),
            ("paper-height", style.page_height_in),
            ("left-margin", style.page_odd_left_margin_in),
            ("right-margin", style.page_odd_right_margin_in),
            ("top-margin", style.page_odd_top_margin_in),
            ("bottom-margin", style.page_odd_bottom_margin_in),
        ] {
            let _ = writeln!(out, "  {name} = {:.3}\\mm", inches * 25.4);
        }
        out.push_str("}\n\n");
    }

    /// Une variable par voix, en mode `\fixed`.
    fn write_variables(&self, out: &mut String, structure: &structure::Structure) {
        for staff_index in 0..self.score.staves.len() {
            let voices = voice::written_voices(self.score, staff_index);
            for voice_index in 0..voices {
                let name = voice::voice_variable(staff_index, voice_index);
                let body =
                    voice::render_body(self.score, structure, staff_index, voice_index, voices);
                let _ = writeln!(out, "{name} = \\fixed c' {{");
                out.push_str(&body);
                out.push_str("}\n\n");
            }
        }
    }

    /// Bloc `\score` avec les regroupements de portées.
    fn write_score(&self, out: &mut String) {
        out.push_str("\\score {\n");
        if self.score.playback.articulate {
            out.push_str("  \\articulate {\n");
        }
        out.push_str("  <<\n");
        let mut written = vec![false; self.score.staves.len()];

        for part in &self.score.parts {
            let staves: Vec<usize> = part
                .staves
                .iter()
                .map(|id| id.index())
                .filter(|index| *index < self.score.staves.len())
                .collect();
            if staves.is_empty() {
                continue;
            }
            for index in &staves {
                written[*index] = true;
            }
            let group = group_of(&self.score.staves[staves[0]], staves.len());
            if let Some(group) = group {
                let clause = instrument_clause(part);
                let with = if clause.is_empty() {
                    String::new()
                } else {
                    format!(" \\with {{ {clause} }}")
                };
                let _ = writeln!(out, "    \\new {group}{with} <<");
            }
            for (position, index) in staves.iter().enumerate() {
                self.write_staff(out, *index, position == 0, group.is_some());
            }
            if group.is_some() {
                out.push_str("    >>\n");
            }
        }

        // Portées hors partie (fichier incohérent) : écrites à la fin, une par
        // une, plutôt que perdues.
        for (index, is_written) in written.iter().enumerate() {
            if !is_written {
                self.write_staff(out, index, true, false);
            }
        }

        out.push_str("  >>\n");
        if self.score.playback.articulate {
            out.push_str("  }\n");
        }
        out.push_str("  \\layout { }\n}\n");
    }

    /// Une portée, ses voix et ses blocs de paroles.
    fn write_staff(&self, out: &mut String, staff_index: usize, first: bool, grouped: bool) {
        let indent = if grouped { "      " } else { "    " };
        let staff_data = &self.score.staves[staff_index];
        let part = &self.score.parts[staff_data.part.index()];
        let clause = instrument_clause(part);
        let with = if first && !grouped && !clause.is_empty() {
            format!(" \\with {{ {clause} }}")
        } else {
            String::new()
        };
        let _ = writeln!(
            out,
            "{indent}\\new Staff = \"{}\"{with} <<",
            voice::staff_variable(staff_index)
        );

        let voices = voice::written_voices(self.score, staff_index);
        // Sans paroles, les voix multiples s'écrivent avec le séparateur `\\`
        // (`<< { … } \\ { … } >>`), forme idiomatique ; avec des paroles, il
        // faut des voix nommées pour que `\\lyricsto` puisse s'y accrocher.
        let has_lyrics = (0..voices).any(|voice_index| {
            !voice::lyrics_verses(self.score, staff_index, voice_index).is_empty()
        });
        if voices > 1 && !has_lyrics {
            for voice_index in 0..voices {
                if voice_index > 0 {
                    let _ = writeln!(out, "{indent}  \\\\");
                }
                let variable = voice::voice_variable(staff_index, voice_index);
                let _ = writeln!(out, "{indent}  {{ \\{variable} }}");
            }
        } else {
            for voice_index in 0..voices {
                let variable = voice::voice_variable(staff_index, voice_index);
                let _ = writeln!(
                    out,
                    "{indent}  \\new Voice = \"{variable}\" {{ \\{variable} }}"
                );
            }
        }

        for voice_index in 0..voices {
            let variable = voice::voice_variable(staff_index, voice_index);
            let verses = voice::lyrics_verses(self.score, staff_index, voice_index);
            let multiple = verses.len() > 1;
            for (verse, syllables) in verses {
                let _ = writeln!(out, "{indent}  \\new Lyrics \\lyricsto \"{variable}\" {{");
                // `\stanza` n'existe que depuis LilyPond 2.27 : la forme
                // `\set stanza = "n."`, utilisée par `musicxml2ly`, compile
                // aussi sous 2.26 et est relue par notre lecteur.
                if multiple {
                    let _ = writeln!(out, "{indent}    \\set stanza = \"{}.\"", verse + 1);
                }
                write_tokens(out, &syllables, &format!("{indent}    "));
                let _ = writeln!(out, "{indent}  }}");
            }
        }

        let _ = writeln!(out, "{indent}>>");
    }
}

/// Écrit des jetons par lignes de six, pour garder le fichier lisible.
fn write_tokens(out: &mut String, tokens: &[String], indent: &str) {
    for chunk in tokens.chunks(6) {
        out.push_str(indent);
        out.push_str(&chunk.join(" "));
        out.push('\n');
    }
}

/// Regroupement LilyPond d'une partie, d'après son accolade.
fn group_of(staff: &Staff, count: usize) -> Option<&'static str> {
    if count < 2 {
        return None;
    }
    let bracket = staff
        .brackets
        .iter()
        .filter(|bracket| bracket.span >= count)
        .max_by_key(|bracket| bracket.span);
    match bracket.map(|bracket| bracket.kind) {
        Some(1) => Some("PianoStaff"),
        Some(0) | Some(3) => Some("ChoirStaff"),
        None | Some(_) => Some("StaffGroup"),
    }
}

/// Clause `\with` des noms d'instrument.
///
/// Les parties 3.x n'ont pas toujours de nom long : le nom de piste de
/// l'instrument sert alors de repli, comme `plain_part_name` à l'export.
fn instrument_clause(part: &scoretk_core::model::Part) -> String {
    let long_name = if part.long_name.trim().is_empty() {
        part.instrument.track_name.trim()
    } else {
        part.long_name.trim()
    };
    let short_name = if part.short_name.trim().is_empty() {
        part.instrument.short_name.trim()
    } else {
        part.short_name.trim()
    };
    let mut clauses = Vec::new();
    if !long_name.trim().is_empty() {
        clauses.push(format!(
            "instrumentName = \"{}\"",
            text::escape(long_name.trim())
        ));
    }
    if !short_name.trim().is_empty() {
        clauses.push(format!(
            "shortInstrumentName = \"{}\"",
            text::escape(short_name.trim())
        ));
    }
    clauses.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Part, PartId, StaffBracket};

    #[test]
    fn parties_sans_portee() {
        let score = Score::new();
        assert_eq!(Writer::new(&score).render(), Err(Error::PartitionVide));
    }

    #[test]
    fn regroupement_des_portees() {
        let mut staff = Staff::new(PartId::from_raw(0));
        assert_eq!(group_of(&staff, 1), None);
        staff.brackets.push(StaffBracket {
            kind: 1,
            span: 2,
            visible: true,
        });
        assert_eq!(group_of(&staff, 2), Some("PianoStaff"));
        staff.brackets[0].kind = 3;
        assert_eq!(group_of(&staff, 2), Some("ChoirStaff"));
        staff.brackets.clear();
        assert_eq!(group_of(&staff, 2), Some("StaffGroup"));
    }

    /// `ly/articulate.ly` raccourcit les notes jouées d'un facteur 7/8. Le
    /// modèle porte ce réglage dans `Score::playback.articulate` ; si
    /// l'écriture l'omet, la relecture du `.ly` rejoue des durées pleines
    /// (`22_StMichel.ly`, `GaudeteChristusEstNatus.ly`).
    #[test]
    fn articulation_du_midi_ecrite() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff(part);
        score.playback.articulate = true;
        let rendered = Writer::new(&score).render().expect("écriture");
        assert!(
            rendered.contains("\\include \"articulate.ly\""),
            "l'include doit être écrit : {rendered}"
        );
        assert!(
            rendered.contains("\\articulate {"),
            "la musique doit être enveloppée : {rendered}"
        );
    }

    #[test]
    fn en_tete_minimal() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff(part);
        score.metadata.work_title = "Essai".to_owned();
        let rendered = Writer::new(&score).render().expect("écriture");
        assert!(rendered.contains("\\header {"));
        assert!(rendered.contains("title = \"Essai\""));
        assert!(rendered.contains("\\new Staff = \"staffOne\""));
        assert!(rendered.contains("instrumentName = \"Piano\""));
    }
}
