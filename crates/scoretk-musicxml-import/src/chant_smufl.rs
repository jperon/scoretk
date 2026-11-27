// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture des porteurs SMuFL standard des neumes
// grégoriens (`musicxml-4.0.xsd` : `complexType other-notation`,
// `:5361-5375`, groupe d'attributs `smufl` `:2183-2188`) et des champs de
// métadonnées arbitraires (`identification/miscellaneous-field/@name`,
// `:4327-4348`), réécrits en Rust en 2026.
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

//! Restauration du payload grégorien **sans aucun champ privé `gabc:`**.
//!
//! En mode `full_support`, l'exporteur écrit un `smufl="chant…"` par note
//! chantée, les en-têtes dans `<miscellaneous-field>` et les attributs `[…]`
//! dans `<other-technical>`. Ce module les relit : un consommateur qui ignore
//! scoretk peut donc retrouver les mêmes faits que le lecteur des extensions
//! privées de [`super::chant`]. Aucun document sans glyphe SMuFL de chant
//! n'est modifié.

use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, NoteAttribute, Score, Syllable,
};
use scoretk_gabc_common::syntax::{clef_key, parse_signs};
use scoretk_xml_common::Element;

/// Tente la restauration. `false` si le document ne porte aucun glyphe SMuFL
/// de chant : dans ce cas `score` est laissé intact.
pub(crate) fn restore(root: &Element, score: &mut Score) -> bool {
    let Some(part) = root.children_named("part").next() else {
        return false;
    };
    let mut headers = Vec::new();
    for field in root
        .children_named("identification")
        .flat_map(|identification| identification.children_named("miscellaneous"))
        .flat_map(|miscellaneous| miscellaneous.children_named("miscellaneous-field"))
    {
        if let Some(name) = field.attribute("name") {
            headers.push(GregorianHeader {
                name: name.to_owned(),
                value: field.text_trimmed().to_owned(),
            });
        }
    }

    let mut reader = Reader::default();
    for (index, measure) in part.children_named("measure").enumerate() {
        reader.measure(measure, index);
    }
    if !reader.found {
        return false;
    }
    let clef = reader.clef.clone().unwrap_or_else(|| "c4".to_owned());
    let mut syllables = reader.syllables;
    if let Some(last) = syllables.last_mut() {
        if last.bars.is_empty() {
            last.bars.push(BarKind::DivisioFinalis);
        }
    }
    let gregorian = Gregorian {
        headers,
        staff_lines: reader.staff_lines,
        clefs: vec![clef],
        syllables,
        opening_separator: None,
        derived_modern_view: true,
        ..Gregorian::default()
    };
    let mut rebuilt = gregorian.build_derived_score();
    rebuilt.metadata = score.metadata.clone();
    for loss in &score.report.losses {
        rebuilt.report.losses.push(loss.clone());
    }
    rebuilt.extensions.format = Some(Extension::Gregorian(gregorian));
    *score = rebuilt;
    true
}

#[derive(Default)]
struct Reader {
    clef: Option<String>,
    staff_lines: u8,
    syllables: Vec<Syllable>,
    found: bool,
}

impl Reader {
    fn measure(&mut self, measure: &Element, index: usize) {
        for attributes in measure.children_named("attributes") {
            for clef in attributes.children_named("clef") {
                let sign = clef
                    .children_named("sign")
                    .next()
                    .map(|sign| sign.text_trimmed());
                let line = clef
                    .children_named("line")
                    .next()
                    .and_then(|line| line.text_trimmed().parse::<u8>().ok());
                if let (Some(sign @ ("C" | "F")), Some(line)) = (sign, line) {
                    if (1..=5).contains(&line) {
                        self.clef = Some(format!("{}{}", sign.to_ascii_lowercase(), line));
                    }
                }
            }
            for details in attributes.children_named("staff-details") {
                if let Some(lines) = details
                    .children_named("staff-lines")
                    .next()
                    .and_then(|lines| lines.text_trimmed().parse::<u8>().ok())
                {
                    self.staff_lines = lines.clamp(2, 5);
                }
            }
        }

        let clef = self.clef.clone().unwrap_or_else(|| "c4".to_owned());
        for note in measure.children_named("note") {
            if note.children_named("rest").next().is_some() {
                continue;
            }
            let mut glyph = String::new();
            let mut inclinatum = false;
            let mut attributes = Vec::new();
            let mut mora = false;
            let mut episema = false;
            let mut exact_glyph: Option<String> = None;
            let mut exact_letter: Option<char> = None;
            let mut syllable_text: Option<String> = None;
            for duration_type in note.children_named("type") {
                // XSD 4.0 `note/type` : une figure doublée est un punctum mora
                // projection `Gabc.tex:742-755`.
                mora |= duration_type.text_trimmed() == "half";
            }
            for notations in note.children_named("notations") {
                for articulations in notations.children_named("articulations") {
                    episema |= articulations.children_named("tenuto").next().is_some();
                }
                for other in notations.children_named("other-notation") {
                    if let Some(name) = other.attribute("smufl") {
                        if let Some((g, upper)) = glyph_from_smufl(name) {
                            glyph.push_str(g);
                            inclinatum |= upper;
                            self.found = true;
                        }
                    }
                }
                for technical in notations.children_named("technical") {
                    for other in technical.children_named("other-technical") {
                        let text = other.text_trimmed();
                        if text.starts_with('[') && text.ends_with(']') {
                            attributes.push(NoteAttribute {
                                after: glyph.chars().count(),
                                text: text.to_owned(),
                            });
                            self.found = true;
                        } else if let Some(value) = text.strip_prefix("glyph: ") {
                            exact_glyph = Some(glyph_from_label(value));
                            self.found = true;
                        } else if let Some(value) = text.strip_prefix("letter: ") {
                            exact_letter = value.chars().next();
                            self.found = true;
                        } else if let Some(value) = text.strip_prefix("syllable-text: ") {
                            syllable_text = Some(value.to_owned());
                            self.found = true;
                        }
                    }
                }
            }
            if let Some(exact) = &exact_glyph {
                glyph = exact.clone();
            }
            if glyph.is_empty() {
                if mora {
                    glyph.push('.');
                }
                if episema {
                    glyph.push('_');
                }
            }
            let pitch = note.children_named("pitch").next();
            let step = pitch
                .and_then(|pitch| pitch.children_named("step").next())
                .map(|step| step.text_trimmed());
            let octave = pitch
                .and_then(|pitch| pitch.children_named("octave").next())
                .and_then(|octave| octave.text_trimmed().parse::<i32>().ok());
            let mut letter = step
                .zip(octave)
                .and_then(|(step, octave)| letter_for_pitch(step, octave, &clef))
                .unwrap_or('g');
            if inclinatum {
                letter = letter.to_ascii_uppercase();
            }
            if let Some(exact) = exact_letter {
                letter = exact;
            }
            if self.syllables.is_empty()
                || self.syllables.last().is_some_and(|s| !s.notes.is_empty())
            {
                self.syllables.push(Syllable::default());
            }
            let syllable_index = self.syllables.len() - 1;
            let mut ends_syllable = false;
            {
                let syllable = &mut self.syllables[syllable_index];
                syllable.notes.push(GregorianNote {
                    letter,
                    signs: parse_signs(&glyph),
                    glyph,
                    attributes,
                    ..GregorianNote::default()
                });
                for lyric in note.children_named("lyric") {
                    let text = lyric
                        .children_named("text")
                        .next()
                        .map(|text| text.text_trimmed().to_owned())
                        .unwrap_or_default();
                    let syllabic = lyric
                        .children_named("syllabic")
                        .next()
                        .map(|syllabic| syllabic.text_trimmed())
                        .unwrap_or("single");
                    if !text.is_empty() {
                        // Des paroles seules ne prouvent pas qu'il s'agit
                        // d'un chant grégorien : sinon tout MusicXML chanté
                        // serait reconstruit en charge L6 (aller-retour
                        // `changevisibility`, `repeat65`, …).
                        syllable.text = text;
                    }
                    ends_syllable |= matches!(syllabic, "single" | "end");
                }
                if let Some(text) = &syllable_text {
                    // Le texte source, balises `<sp>` comprises, est porté par
                    // `other-technical` (xs:string) et relu sans préfixe privé.
                    syllable.text = text.clone();
                }
            }
            if ends_syllable {
                self.syllables.push(Syllable::default());
            }
        }
        for barline in measure.children_named("barline") {
            let style = barline
                .children_named("bar-style")
                .next()
                .map(|style| style.text_trimmed())
                .unwrap_or("regular");
            let bar = match style {
                "tick" => BarKind::DivisioMinima,
                "short" => BarKind::DivisioMinor,
                "light-light" => BarKind::DivisioMaior,
                "light-heavy" => BarKind::DivisioFinalis,
                _ => BarKind::DivisioMinima,
            };
            if self.syllables.is_empty() {
                self.syllables.push(Syllable::default());
            }
            self.syllables.last_mut().expect("syllable").bars.push(bar);
        }
        let _ = index;
    }
}

/// Nom lisible d'un caractère de glyphe GABC → caractère source.
fn glyph_char(name: &str) -> Option<char> {
    const NAMES: &[(&str, char)] = &[
        ("augmentum", '.'), ("ictus", '\''), ("episema", '_'), ("quilisma", 'w'),
        ("spacing", '/'), ("zero-space", '!'), ("deminutum", '~'), ("virga", 'v'),
        ("flat", 'x'), ("asterisk", '*'), ("diminutum", '>'), ("oriscus", 'o'),
        ("one", '1'), ("zero", '0'), ("natural", 'y'), ("virga-reversa", 'V'),
        ("stropha", 's'), ("oriscus-scampus", 'O'), ("two", '2'), ("cavum", 'r'),
        ("virgula", '`'), ("hyphen", '-'), ("plus", '+'), ("colon", ':'),
        ("brace-open", '{'), ("brace-close", '}'), ("fusion", '@'), ("three", '3'),
        ("auctum", '<'), ("u", 'u'), ("quote", '"'), ("dagger", '†'),
        ("sharp", '#'), ("linea", '='),
    ];
    NAMES.iter().find(|(label, _)| *label == name).map(|(_, gabc)| *gabc)
}

/// Relit une orthographe de glyphe écrite en noms lisibles.
fn glyph_from_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(|word| glyph_char(word).map_or_else(|| word.to_owned(), |gabc| gabc.to_string()))
        .collect()
}

/// Nom SMuFL officiel du registre « Medieval and Renaissance » → jeton GABC.
/// Un seul glyphe est porté par note (`scoretk-musicxml-export/src/chant.rs`) ;
/// les formes composées restent donc hors de portée de ce seul attribut.
fn glyph_from_smufl(name: &str) -> Option<(&'static str, bool)> {
    match name {
        "chantQuilisma" | "chantQuilismaAscending" | "chantQuilismaDescending" => {
            Some(("w", false))
        }
        "chantIctusAbove" | "chantIctusBelow" => Some(("'", false)),
        "chantEpisema" => Some(("_", false)),
        "chantPunctumVirga" | "chantVirga" => Some(("v", false)),
        "chantOriscusAscending" | "chantOriscusDescending" => Some(("o", false)),
        "chantDeminutumLower" | "chantDeminutumUpper" => Some(("~", false)),
        "chantCavum" => Some(("r", false)),
        "chantLinea" => Some(("=", false)),
        "chantPunctumInclinatum" => Some(("", true)),
        "chantPunctum" => Some(("", false)),
        _ => None,
    }
}

/// Inverse de `scoretk_gabc_common::syntax::diatonic_step` : hauteur diatonique
/// (C4 = 0) → lettre de position sur la portée (`Gabc.tex:408-443`).
fn letter_for_pitch(step: &str, octave: i32, clef: &str) -> Option<char> {
    const STEPS: &str = "cdefgab";
    let index = STEPS.find(step.to_ascii_lowercase().chars().next()?)?;
    let diatonic = (octave - 4) * 7 + index as i32;
    let height = diatonic + clef_key(clef) + 5;
    if !(3..=19).contains(&height) {
        return None;
    }
    let mut offset = height - 3;
    if offset >= 15 {
        offset += 1; // `o` is not a GABC pitch letter.
    }
    Some((b'a' + offset as u8) as char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_smufl_names_map_to_their_gabc_glyph() {
        assert_eq!(glyph_from_smufl("chantQuilisma"), Some(("w", false)));
        assert_eq!(glyph_from_smufl("chantIctusAbove"), Some(("'", false)));
        assert_eq!(glyph_from_smufl("chantPunctumInclinatum"), Some(("", true)));
        assert_eq!(glyph_from_smufl("pianoPedalPed"), None);
    }

    #[test]
    fn readable_technical_labels_restore_exact_letter_and_glyph() {
        let xml = r#"<score-partwise version="4.0">
  <identification><miscellaneous>
    <miscellaneous-field name="language">latin</miscellaneous-field>
  </miscellaneous></identification>
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><clef><sign>C</sign><line>3</line></clef></attributes>
    <note><pitch><step>G</step><octave>3</octave></pitch><type>quarter</type>
      <notations><other-notation type="single" smufl="chantQuilisma"/></notations>
      <notations><technical>
        <other-technical>letter: e</other-technical>
        <other-technical>glyph: virga augmentum spacing</other-technical>
        <other-technical>syllable-text: &lt;sp&gt;Ve&lt;/sp&gt;</other-technical>
      </technical></notations>
      <lyric number="1"><syllabic>single</syllabic><text>Ve</text></lyric>
    </note>
  </measure></part>
</score-partwise>"#;
        let root = scoretk_xml_common::parse(xml.as_bytes()).expect("XML");
        let mut score = Score::default();
        assert!(restore(&root, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.syllables[0].notes[0].letter, 'e');
        assert_eq!(payload.syllables[0].notes[0].glyph, "v./");
        assert_eq!(payload.syllables[0].text, "<sp>Ve</sp>");
        assert!(!xml.contains("gabc:"), "no private carrier in the source");
    }

    #[test]
    fn pitch_and_clef_reconstruct_the_staff_position() {
        // `Gabc.tex:847-870`, `src/struct.c:1273-1288` : sous `c3`, un `e`
        // écrit sur la première ligne est la lettre `e` du payload.
        assert_eq!(letter_for_pitch("G", 3, "c3"), Some('e'));
        assert_eq!(letter_for_pitch("C", 4, "c4"), Some('j'));
        assert_eq!(letter_for_pitch("H", 99, "c3"), None);
    }
}
