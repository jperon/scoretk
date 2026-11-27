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
//! chantée et les en-têtes dans `<miscellaneous-field>`. Ce module les relit :
//! un consommateur qui ignore scoretk peut donc retrouver les faits portés par
//! la norme. Les `other-technical` dont le texte est une étiquette
//! `letter:`/`glyph:`/`syllable-text:` sont ignorés : ce sont des vidages de la
//! source, pas des porteurs. Les `<words>` qui répètent une `<lyric>` (avec une
//! couleur utilisée comme drapeau privé) sont également ignorés : les paroles
//! restent dans leur porteur XSD `<lyric>` et le texte marqué d'une syllabe
//! sans porteur standard est un défaut, pas un fait à masquer dans une
//! direction imprimée. Aucun document sans glyphe SMuFL de chant n'est modifié.

use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, Score, Syllable,
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
    for text in std::mem::take(&mut reader.standalone_rubrics) {
        reader.syllables.push(Syllable {
            text,
            ..Syllable::default()
        });
    }
    if !reader.pending_rubrics.is_empty() {
        let printed = reader.pending_rubrics.concat();
        if let Some(last) = reader.syllables.last_mut() {
            last.text.push_str(&printed);
        }
    }
    let clef = reader.clef.clone().unwrap_or_else(|| "c4".to_owned());
    let mut syllables = std::mem::take(&mut reader.syllables);
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
    /// Textes imprimés lus dans les `<direction placement="below">` avant la
    /// note suivante (XSD 4.0 `words` :3495). Ils sont réécrits dans le
    /// `<lyric>` du payload L6 comme texte ou comme balise Gregorio.
    pending_rubrics: Vec<String>,
    /// Rubriques imprimées au-dessus de la portée : elles forment une syllabe
    /// sans note, distincte de la syllabe chantée suivante.
    standalone_rubrics: Vec<String>,
    /// La prochaine note doit ouvrir une nouvelle syllabe après une rubrique.
    force_new_syllable: bool,
    /// Alterations GABC lues comme symboles SMuFL positionnés, en attente de
    /// la note chantée qui les suit (`Gabc.tex:512-540`).
    pending_alterations: Vec<(char, String)>,
}

impl Reader {
    fn measure(&mut self, measure: &Element, index: usize) {
        for child in &measure.children {
            match child.name.as_str() {
                "attributes" => self.attributes(child),
                "note" => {
                    let clef = self.clef.clone().unwrap_or_else(|| "c4".to_owned());
                    self.note(child, &clef);
                }
                "barline" => self.barline(child),
                "direction" => self.rubric_direction(child),
                _ => {}
            }
        }
        let _ = index;
    }

    fn attributes(&mut self, attributes: &Element) {
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

    fn barline(&mut self, barline: &Element) {
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

    /// Lit un texte imprimé porté par `direction/words` (`:3495`).
    ///
    /// Seules les directions `placement="below"` sont des rubriques : le texte
    /// `[alt:…]` est écrit `placement="above"` et garde son porteur dédié. La
    /// valeur est le texte **imprimé**, jamais une étiquette du source ; la
    /// table ci-dessous inverse celle de l'exporteur pour les caractères que
    /// GregorioTeX imprime (`Gabc.tex:345-378`).
    fn rubric_direction(&mut self, direction: &Element) {
        let placement = direction.attribute("placement").unwrap_or("");
        for kind in direction.children_named("direction-type") {
            if let (Some(symbol), Some(y)) = (
                kind.children_named("symbol").next(),
                kind.attribute("default-y")
                    .and_then(|y| y.parse::<i32>().ok()),
            ) {
                let raw = match symbol.text_trimmed() {
                    "accidentalFlat" => "x",
                    "accidentalSharp" => "#",
                    "accidentalNatural" => "y",
                    _ => "",
                };
                if let Some(letter) = self.letter_from_y(y) {
                    if !raw.is_empty() {
                        self.pending_alterations.push((letter, raw.to_owned()));
                    }
                }
            }
            for words in kind.children_named("words") {
                // Seul le rouge imprimé est une rubrique : le texte `[alt:…]`
                // est en italique sans couleur, et un `<words>` d'une autre
                // couleur n'est pas un porteur de rubrique (c3c6919f).
                if words.attribute("color") != Some("#FF0000") {
                    continue;
                }
                let printed = words.text_trimmed();
                // Un `<words>` qui contient la balise source elle-même est un
                // vidage, pas une valeur imprimée (revue c3c6919f).
                if printed.is_empty() || printed.contains('<') {
                    continue;
                }
                let value = printed_to_gabc(printed);
                if placement == "above" {
                    self.standalone_rubrics.push(value);
                } else {
                    self.pending_rubrics.push(value);
                }
            }
        }
    }

    /// Inverse de `alteration_y` de l'exporteur : position verticale d'un
    /// symbole → lettre de hauteur GABC (`Gabc.tex:408-443`).
    fn letter_from_y(&self, y: i32) -> Option<char> {
        let clef = self.clef.as_deref().unwrap_or("c4");
        let key = clef_key(clef);
        let first = clef.split('@').next().unwrap_or(clef);
        let line = i32::from(
            first
                .bytes()
                .rev()
                .find(|byte| byte.is_ascii_digit())
                .unwrap_or(b'4')
                - b'0',
        );
        let clef_step = if first.starts_with('f') { 3 } else { 0 };
        let step = clef_step - 2 * (line - 1) + y / 5;
        let height = step + key + 5;
        if !(3..=19).contains(&height) {
            return None;
        }
        let mut offset = height - 3;
        if offset >= 15 {
            offset += 1; // `o` n'est pas une lettre de hauteur GABC.
        }
        Some((b'a' + offset as u8) as char)
    }

    fn note(&mut self, note: &Element, clef: &str) {
        if note.children_named("rest").next().is_some() {
            return;
        }
        let mut glyph = String::new();
        let mut inclinatum = false;
        let mut mora = false;
        let mut episema = false;
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
            // Aucun `technical` n'est lu ici : un `other-technical` est soit
            // un champ privé `gabc:` (lu par `super::chant`), soit un vidage
            // étiqueté sans porteur dans la norme.
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
            .and_then(|(step, octave)| letter_for_pitch(step, octave, clef))
            .unwrap_or('g');
        if inclinatum {
            letter = letter.to_ascii_uppercase();
        }
        for text in std::mem::take(&mut self.standalone_rubrics) {
            self.syllables.push(Syllable {
                text,
                ..Syllable::default()
            });
            self.force_new_syllable = true;
        }
        if self.force_new_syllable
            || self.syllables.is_empty()
            || self.syllables.last().is_some_and(|s| !s.notes.is_empty())
        {
            self.syllables.push(Syllable::default());
            self.force_new_syllable = false;
        }
        let syllable_index = self.syllables.len() - 1;
        let alterations = std::mem::take(&mut self.pending_alterations);
        let mut ends_syllable = false;
        {
            let syllable = &mut self.syllables[syllable_index];
            for (letter, raw) in alterations {
                syllable.notes.push(GregorianNote {
                    letter,
                    signs: parse_signs(&raw),
                    glyph: raw,
                    ..GregorianNote::default()
                });
            }
            syllable.notes.push(GregorianNote {
                letter,
                signs: parse_signs(&glyph),
                glyph,
                ..GregorianNote::default()
            });
            for lyric in note.children_named("lyric") {
                let element = lyric.children_named("text").next();
                let text = element
                    .map(|text| text.text_trimmed().to_owned())
                    .unwrap_or_default();
                // XSD 4.0 `text-formatting` : le style imprimé de la lyric
                // restaure les balises `Gabc.tex:254-270` quand elles
                // enveloppaient toute la syllabe. Aucune direction dupliquée.
                let mut styled = text.clone();
                if element
                    .is_some_and(|element| element.attribute("font-family") == Some("monospace"))
                {
                    styled = format!("<tt>{styled}</tt>");
                }
                if element.is_some_and(|element| element.attribute("underline") == Some("1")) {
                    styled = format!("<ul>{styled}</ul>");
                }
                if element.is_some_and(|element| element.attribute("font-weight") == Some("bold")) {
                    styled = format!("<b>{styled}</b>");
                }
                if element.is_some_and(|element| element.attribute("font-style") == Some("italic"))
                {
                    styled = format!("<i>{styled}</i>");
                }
                if element.is_some_and(|element| element.attribute("color") == Some("#FF0000")) {
                    styled = format!("<c>{styled}</c>");
                }
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
                    syllable.text = styled;
                }
                ends_syllable |= matches!(syllabic, "single" | "end");
                // XSD 4.0 `syllabic` (`single`/`end`) marque la fin d'un mot :
                // c'est aussi la fin de la portée d'une alteration GABC
                // (`Gabc.tex:212-227,512-540`). Sans elle, un bémol fuirait
                // sur toutes les syllabes suivantes.
                syllable.space_after = matches!(syllabic, "single" | "end");
            }
            for printed in std::mem::take(&mut self.pending_rubrics) {
                // Quand le `<words>` imprime la **même** syllabe avec sa
                // typographie d'origine (`{ia}.` pour la lyric `ia.`), il
                // remplace la lyric au lieu de la dupliquer. Un caractère
                // spécial inséré (`<sp>*</sp>`) reste ajouté à la syllabe.
                let plain: String = printed
                    .chars()
                    .filter(|char| !matches!(char, '{' | '}'))
                    .collect();
                let plain = plain
                    .replace("<sp>", "")
                    .replace("</sp>", "")
                    .replace("<v>", "")
                    .replace("</v>", "");
                if !syllable.text.is_empty() && plain.contains(&syllable.text) {
                    syllable.text = printed;
                } else {
                    syllable.text.push_str(&printed);
                }
            }
        }
        if ends_syllable {
            self.syllables.push(Syllable::default());
        }
    }
}

/// Texte imprimé → balise Gregorio d'origine (`Gabc.tex:345-378`).
///
/// L'exporteur n'écrit que des valeurs imprimées dans `<words>` ; cette table
/// en inverse les caractères spéciaux. Tout autre texte reste tel quel et est
/// compté comme paroles/rubrique par le lecteur de faits GABC, sans inventer
/// de balise. Aucun préfixe ni étiquette de vidage n'est utilisé.
fn printed_to_gabc(printed: &str) -> String {
    // Un même `<words>` peut imprimer un caractère spécial suivi de texte
    // (`V/.`, `† Quan`) : on remplace chaque valeur connue, pas seulement la
    // chaîne entière.
    const SPECIALS: &[(&str, &str)] = &[
        ("V/", "<sp>V/</sp>"),
        ("R/", "<sp>R/</sp>"),
        ("A/", "<sp>A/</sp>"),
        ("æ", "<sp>ae</sp>"),
        ("œ", "<sp>oe</sp>"),
        ("'æ", "<sp>'ae</sp>"),
        ("'œ", "<sp>'oe</sp>"),
        ("✶", "<sp>*</sp>"),
        ("†", "<sp>+</sp>"),
        ("✳", "<v>\\greheightstar</v>"),
        ("✦", "<v>\\gresixstar</v>"),
        ("‡", "<v>\\ddag\\ </v>"),
        ("V̄", "<v>\\Vbar</v>"),
        ("★", "<v>$\\star$</v>"),
        ("¶", "<v>\\P\\ </v>"),
        ("X", "<v>\\textcolor{red}{X}</v>"),
        ("M", "<v>\\textcolor{red}{M}</v>"),
        ("P", "<v>\\textcolor{red}{P}</v>"),
        ("A", "<v>\\textcolor{red}{A}</v>"),
        ("H", "<v>\\textcolor{red}{H}</v>"),
        ("G", "<v>\\textcolor{red}{G}</v>"),
    ];
    // Les lettres seules sont ambiguës : ne les remplacer que si le texte est
    // exactement cette lettre (les initiales rouges sont des cas isolés).
    let mut result = printed.to_owned();
    for (glyph, markup) in SPECIALS {
        // Les lettres et ligatures sont ambiguës dans un mot latin (`cælo`) :
        // elles ne sont remplacées que pour une valeur imprimée exacte. Seuls
        // les symboles non alphanumériques sont substitués dans un texte plus
        // long (`V/.`, `† Quan`).
        if glyph.chars().all(|c| c.is_alphanumeric()) {
            if printed == *glyph {
                return (*markup).to_owned();
            }
            continue;
        }
        result = result.replace(glyph, markup);
    }
    if result != printed {
        return result;
    }
    let markup = match printed {
        "V/" => "<sp>V/</sp>",
        "R/" => "<sp>R/</sp>",
        "A/" => "<sp>A/</sp>",
        "æ" => "<sp>ae</sp>",
        "œ" => "<sp>oe</sp>",
        "'æ" => "<sp>'ae</sp>",
        "'œ" => "<sp>'oe</sp>",
        "✶" => "<sp>*</sp>",
        "†" => "<sp>+</sp>",
        "✳" => "<v>\\greheightstar</v>",
        "✦" => "<v>\\gresixstar</v>",
        "‡" => "<v>\\ddag\\ </v>",
        "V̄" => "<v>\\Vbar</v>",
        "★" => "<v>$\\star$</v>",
        "¶" => "<v>\\P\\ </v>",
        "X" => "<v>\\textcolor{red}{X}</v>",
        "M" => "<v>\\textcolor{red}{M}</v>",
        "P" => "<v>\\textcolor{red}{P}</v>",
        "A" => "<v>\\textcolor{red}{A}</v>",
        "H" => "<v>\\textcolor{red}{H}</v>",
        "G" => "<v>\\textcolor{red}{G}</v>",
        _ => return printed.to_owned(),
    };
    markup.to_owned()
}

/// Nom SMuFL officiel du registre « Medieval and Renaissance » → jeton GABC.
///
/// L'exporteur écrit une *séquence* de `<other-notation>` par note
/// (`scoretk-musicxml-export/src/chant.rs`) : le lecteur concatène les jetons
/// dans l'ordre pour retrouver le glyphe source. Les noms `accidental*`
/// appartiennent au registre SMuFL des accidentels (XSD 4.0 les accepte
/// tous), et portent les alterations `x`/`y`/`#` du glyphe GABC.
fn glyph_from_smufl(name: &str) -> Option<(&'static str, bool)> {
    match name {
        "chantQuilisma" | "chantQuilismaAscending" | "chantQuilismaDescending" => {
            Some(("w", false))
        }
        "chantIctusAbove" | "chantIctusBelow" => Some(("'", false)),
        "chantEpisema" => Some(("_", false)),
        "chantAugmentum" => Some((".", false)),
        "chantPunctumVirga" | "chantVirga" => Some(("v", false)),
        "chantPunctumVirgaReversed" => Some(("V", false)),
        "chantStrophicus" => Some(("s", false)),
        "chantOriscusAscending" => Some(("o", false)),
        "chantOriscusDescending" => Some(("O", false)),
        "chantDeminutumLower" | "chantDeminutumUpper" | "chantPunctumDeminutum" => {
            Some(("~", false))
        }
        "chantAuctumAsc" => Some(("<", false)),
        "chantAuctumDesc" => Some((">", false)),
        "chantPunctumCavum" | "chantCavum" => Some(("r", false)),
        "chantPunctumLinea" | "chantLinea" => Some(("=", false)),
        "accidentalFlat" => Some(("x", false)),
        "accidentalSharp" => Some(("#", false)),
        "accidentalNatural" => Some(("y", false)),
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
    fn words_are_not_a_lyric_carrier() {
        // Un `<words>` coloré qui répète la syllabe est un porteur privé
        // déguisé : il s'imprimerait une seconde fois au-dessus de la portée.
        // Seul `<lyric>/<text>` porte les paroles (XSD 4.0 :3495).
        let xml = r##"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><clef><sign>C</sign><line>3</line></clef></attributes>
    <direction><direction-type>
      <words font-style="italic" color="#FF0000">&lt;sp&gt;V/&lt;/sp&gt;.</words>
      <words font-style="italic" color="#0000FF"> </words>
    </direction-type></direction>
    <note><pitch><step>G</step><octave>3</octave></pitch><type>quarter</type>
      <notations><other-notation type="single" smufl="chantPunctum"/></notations>
      <lyric number="1"><syllabic>single</syllabic><text>V/.</text></lyric>
    </note>
  </measure></part>
</score-partwise>"##;
        let root = scoretk_xml_common::parse(xml.as_bytes()).expect("XML");
        let mut score = Score::default();
        assert!(restore(&root, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.syllables[0].text, "V/.");
    }

    #[test]
    fn smufl_sequence_restores_a_compound_glyph() {
        // XSD 4.0 `other-notation` est unbounded : une séquence de noms
        // SMuFL canoniques porte les signes dans l'ordre du source, sans
        // étiquette contenant le jeton GABC brut.
        let xml = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><clef><sign>C</sign><line>3</line></clef></attributes>
    <note><pitch><step>G</step><octave>3</octave></pitch><type>quarter</type>
      <notations>
        <other-notation type="single" smufl="chantPunctumVirga"/>
        <other-notation type="single" smufl="chantAugmentum"/>
        <other-notation type="single" smufl="accidentalFlat"/>
      </notations>
    </note>
  </measure></part>
</score-partwise>"#;
        let root = scoretk_xml_common::parse(xml.as_bytes()).expect("XML");
        let mut score = Score::default();
        assert!(restore(&root, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.syllables[0].notes[0].glyph, "v.x");
    }

    #[test]
    fn technical_dump_labels_are_not_carriers() {
        // `letter:`, `glyph:` and `syllable-text:` are dumps of the source
        // under a generic xs:string. They must not override what the standard
        // carriers (pitch, SMuFL name, lyric) actually say.
        let xml = r#"<score-partwise version="4.0">
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
        assert_eq!(payload.syllables[0].notes[0].letter, 'e'); // clef + pitch
        assert_eq!(payload.syllables[0].notes[0].glyph, "w");
        assert_eq!(payload.syllables[0].text, "Ve");
    }

    #[test]
    fn placed_symbol_and_words_restore_alteration_and_rubric() {
        // Le lecteur standard ne voit que des porteurs XSD : `symbol` SMuFL
        // positionné et `words` imprimé. Il reconstruit le glyphe `x` à sa
        // lettre exacte et la rubrique `<sp>V/</sp>.` sans champ privé.
        let xml = r##"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><clef><sign>C</sign><line>4</line></clef></attributes>
    <direction placement="above"><direction-type><words color="#FF0000">V/.</words></direction-type></direction>
    <direction placement="above"><direction-type default-y="25"><symbol>accidentalFlat</symbol></direction-type></direction>
    <note><pitch><step>B</step><octave>3</octave></pitch><type>quarter</type>
      <notations><other-notation type="single" smufl="chantPunctum"/></notations>
      <lyric number="1"><syllabic>end</syllabic><text>ne</text></lyric>
    </note>
  </measure></part>
</score-partwise>"##;
        let root = scoretk_xml_common::parse(xml.as_bytes()).expect("XML");
        let mut score = Score::default();
        assert!(restore(&root, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.syllables[0].text, "<sp>V/</sp>.");
        assert_eq!(payload.syllables[1].notes[0].letter, 'i');
        assert_eq!(payload.syllables[1].notes[0].glyph, "x");
        assert!(payload.syllables[1].space_after);
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
