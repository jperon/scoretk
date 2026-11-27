// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : il porte les conventions d'interopérabilité
// grégorienne de MusicXML 4.0 (`musicxml-4.0.xsd` : `other-notation`,
// `other-direction`, `other-technical`, `notehead`, `time/senza-misura`) et
// la syntaxe de Gregorio (`doc/Gabc.tex`, commit b63f074), réécrits en Rust
// en 2026.
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

//! Transport des faits grégoriens que MusicXML peut porter.
//!
//! Un `.gabc` n'a ni mesure métrique ni durée fixe (`Gabc.tex:801-845`), mais
//! ses faits **structurés** — en-têtes, clé, syllabes, lettres de hauteur,
//! glyphes de neume, signes et barres de séparation — ont un equivalent
//! MusicXML : `<lyric>` (`syllabic`), `<barline>`, `<clef>`, `<notehead>` pour
//! les formes que l'énumération couvre, puis
//! `<other-notation>`/`<other-direction>`/`<other-technical>` pour ce que la
//! norme n'énumère pas (XSD 4.0 : « extended representation, though without
//! application interoperability »). Chaque champ est écrit **séparément** :
//! aucune source GABC brute n'est copiée dans le XML (voir le test
//! `no_raw_gabc_source_in_xml`).
//!
//! Ce qui ne peut pas être exprimé est déclaré hors de portée dans
//! `docs/agents/gabc.md` §10 : custos et sauts de ligne (`Gabc.tex:872-906`),
//! attributs TeX entre crochets (`:689-700,908-931`), coupures NABC
//! (`gabc-score-determination.y:870-907`) et ordre exact des barres à
//! l'intérieur d'un groupe.

use scoretk_core::model::{Gregorian, GregorianNote, NeumeSign, NoteSource, SignKind, Syllable};

use crate::exportmusicxml::Out;

/// Préfixe des champs grégoriens dans un élément d'extension MusicXML.
///
/// Le préfixe permet à un lecteur tiers d'ignorer ces données sans confondre
/// un texte grégorien avec une indication d'exécution ordinaire.
pub(crate) const PREFIX: &str = "gabc:";

/// Échappe les séparateurs et les retours à la ligne d'un champ structuré.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            // Certains lecteurs XML (quick-xml 0.42) perdent `&gt;` dans le
            // texte : on encode donc `>` comme une séquence propre au champ,
            // sans changer l'échappement global de l'écrivain (qui doit rester
            // identique aux références MuseScore 4.7.4).
            '>' => out.push_str("\\g"),
            '<' => out.push_str("\\l"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// Structuré `clé=valeur|clé=valeur` d'un champ d'extension.
pub(crate) fn field(key: char, value: &str) -> String {
    format!("{key}={}", escape(value))
}

/// En-têtes et clef : directions non imprimées au début de la première mesure.
///
/// `<other-direction>` est le point d'extension normalisé pour une direction
/// que MusicXML ne décrit pas (XSD 4.0, `complexType other-direction`).
pub(crate) fn write_header_directions(
    out: &mut Out,
    gregorian: &Gregorian,
    private: bool,
) {
    if !private {
        return;
    }
    out.attr(false);
    for header in &gregorian.headers {
        let text = format!(
            "{PREFIX}header:{}|{}",
            field('n', &header.name),
            field('v', &header.value)
        );
        write_direction(out, &text);
    }
    // Une source sans jeton de clé (`( )`) ne doit pas en recevoir une par
    // défaut : la clé absente est elle-même une donnée du payload.
    if let Some(clef) = gregorian.clefs.first() {
        let text = format!(
            "{PREFIX}clef:{}|{}",
            field('t', clef),
            field('l', &gregorian.staff_lines.to_string())
        );
        write_direction(out, &text);
    }
}

/// Porteurs standard des en-têtes grégoriens (`xsd:miscellaneous-field/@name`,
/// XSD 4.0 `:4327-4348`). Additif au mode `full_support` : les champs privés
/// `gabc:` restent le porteur exact par défaut, mais un lecteur conforme peut
/// relire chaque nom/valeur sans les connaître.
pub(crate) fn write_miscellaneous_headers(out: &mut Out, gregorian: &Gregorian) {
    for header in &gregorian.headers {
        out.xml.tag_with(
            "miscellaneous-field",
            &[("name", &header.name)],
            &header.value,
        );
    }
}

/// Valeur imprimée d'un caractère spécial Gregorio (`Gabc.tex:345-378`).
///
/// La table ne contient que des caractères que GregorioTeX imprime réellement :
/// les noms de commandes TeX ne sont pas des valeurs imprimées. Elle est
/// l'inverse de la table de lecture de `scoretk-musicxml-import::chant_smufl`.
fn printed_special(raw: &str) -> Option<&'static str> {
    match raw {
        "V/" => Some("V/"),
        "R/" => Some("R/"),
        "A/" => Some("A/"),
        "ae" => Some("æ"),
        "oe" => Some("œ"),
        "'ae" => Some("'æ"),
        "'oe" => Some("'œ"),
        "*" => Some("✶"),
        "+" => Some("†"),
        "\\greheightstar" => Some("✳"),
        "\\gresixstar" => Some("✦"),
        "\\ddag\\ " => Some("‡"),
        "\\Vbar" => Some("V̄"),
        "$\\star$" => Some("★"),
        "\\P\\ " => Some("¶"),
        "(" => Some("("),
        ")" => Some(")"),
        _ => {
            if let Some(inner) = raw
                .strip_prefix("\\textcolor{red}{")
                .and_then(|s| s.strip_suffix('}'))
            {
                match inner {
                    "X" => Some("X"),
                    "M" => Some("M"),
                    "P" => Some("P"),
                    "A" => Some("A"),
                    "H" => Some("H"),
                    "G" => Some("G"),
                    "*" => Some("✳"),
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

/// Un fragment de texte imprimé par la source, avec l'indication s'il vient
/// d'une balise (`<sp>`, `<v>`) ou du texte brut de la syllabe.
#[derive(Clone, Debug)]
struct PrintedFragment {
    value: String,
    markup: bool,
}

/// Fragments imprimés d'une syllabe, dans l'ordre du source.
///
/// `Gabc.tex:303-309,345-378` : `<sp>` et `<v>` ne sont **pas** des paroles
/// chantées (`plain_lyric` les retire) ; ils impriment un caractère ou un
/// texte. Les autres balises de style laissent leur contenu dans la lyric.
fn printed_fragments(text: &str) -> Vec<PrintedFragment> {
    let mut result = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        if bytes[index] == b'<' {
            if let Some(relative) = text[index..].find('>') {
                let end = index + relative;
                let tag = text[index + 1..end].trim();
                if matches!(tag, "sp" | "v") {
                    let closing = format!("</{tag}>");
                    if let Some(relative) = text[end + 1..].find(&closing) {
                        let content_end = end + 1 + relative;
                        let printed = printed_special(&text[end + 1..content_end]);
                        if let Some(value) = printed {
                            if !plain.is_empty() {
                                result.push(PrintedFragment {
                                    value: std::mem::take(&mut plain),
                                    markup: false,
                                });
                            }
                            result.push(PrintedFragment {
                                value: value.to_owned(),
                                markup: true,
                            });
                        }
                        index = content_end + closing.len();
                        continue;
                    }
                }
                index = end + 1;
                continue;
            }
        }
        plain.push(bytes[index] as char);
        index += 1;
    }
    if !plain.is_empty() {
        result.push(PrintedFragment {
            value: plain,
            markup: false,
        });
    }
    result
}

/// Vrai si le texte est une rubrique **imprimée** et non une parole chantée
/// (`Gabc.tex:297-378` ; cf. la règle homologue de `full_support.py::facts`).
///
/// Seul le texte que GregorioTeX imprime réellement est une rubrique : les
/// numéros de verset, `V/`, `R/`, `Ant.`, `Ps.` et les signes `*`/`†`. Les
/// accolades `{}` sont du centrage de voyelle (`Gabc.tex:314-324`) et les
/// backslashes du TeX brut : ni l'un ni l'autre n'est imprimé, et ni l'un ni
/// l'autre ne transforme une syllabe chantée en rubrique.
fn printed_rubric_text(value: &str) -> bool {
    value.starts_with("V/")
        || value.starts_with("R/")
        || value.starts_with("Ant.")
        || value.starts_with("Ps.")
        || value.starts_with('+')
        || value
            .strip_suffix('.')
            .is_some_and(|head| !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()))
}

/// Clé d'une clé GABC (`src/struct.c:1273-1288`).
fn clef_key(clef: &str) -> i32 {
    let first = clef.split('@').next().unwrap_or(clef);
    let line = first
        .bytes()
        .rev()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    if first.starts_with('f') {
        2 * i32::from(line) - 4
    } else {
        2 * i32::from(line) - 1
    }
}

/// Position verticale imprimée d'un signe d'alteration, en dixièmes
/// (`musicxml-4.0.xsd` `print-style` : `default-y`).
///
/// La ligne inférieure est l'index diatonique `clef_step - 2*(lines-1)`
/// (C4 = 0, F4 = 3) et chaque degré vaut 5 dixièmes. Le lecteur inverse
/// retrouve la lettre GABC exacte du signe (`Gabc.tex:408-443,481-489`).
fn alteration_y(letter: char, clef: &str) -> i32 {
    let low = letter.to_ascii_lowercase();
    let height = i32::from(low as u8 - b'a') + 3 + i32::from(low > 'o');
    let step = height - clef_key(clef) - 5;
    let first = clef.split('@').next().unwrap_or(clef);
    let line = first
        .bytes()
        .rev()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    let clef_step = if first.starts_with('f') { 3 } else { 0 };
    let bottom = clef_step - 2 * (i32::from(line) - 1);
    (step - bottom) * 5
}

/// Porteur XSD 4.0 d'une alteration GABC non chantée.
///
/// `Gabc.tex:512-540` : `x`/`X`, `#`/`##`, `y`/`Y` sont des glyphes, pas des
/// notes. XSD 4.0 `direction-type/symbol` (`:3530`, `formatted-symbol-id`
/// `:2596`) imprime un symbole SMuFL canonique ; `default-y` le place sur la
/// ligne voulue. Un tiers voit donc le bémol, le dièse ou le bécarre à sa
/// position, et la lecture inverse restaure la lettre GABC du signe.
pub(crate) fn write_alteration_symbols(
    out: &mut Out,
    gregorian: &Gregorian,
    syllable_index: usize,
    note_index: usize,
) {
    let Some(syllable) = gregorian.syllables.get(syllable_index) else {
        return;
    };
    let clef = gregorian.clefs.first().map_or("c4", String::as_str);
    let mut index = note_index;
    while index > 0 {
        index -= 1;
        let Some(note) = syllable.notes.get(index) else {
            break;
        };
        let Some(sign) = note
            .signs
            .iter()
            .find(|sign| sign.kind == SignKind::Alteration)
        else {
            break;
        };
        let name = match sign.raw.chars().next() {
            Some('x' | 'X') => "accidentalFlat",
            Some('#') => "accidentalSharp",
            Some('y' | 'Y') => "accidentalNatural",
            _ => break,
        };
        let y = alteration_y(note.letter, clef);
        out.xml
            .start_element("direction", &[("placement", "above")]);
        out.xml
            .start_element("direction-type", &[("default-y", &y.to_string())]);
        out.xml.tag("symbol", name);
        out.xml.end_element();
        out.xml.end_element();
    }
}

/// Typographie XSD 4.0 d'une lyric entièrement stylée (`text-formatting`,
/// `:2237`, `font` `:2005`).
///
/// `Gabc.tex:254-270` : les balises `<b>`, `<c>`, `<i>`, `<tt>` et `<ul>`
/// appliquent un style au texte chanté. Quand une de ces balises enveloppe
/// toute la syllabe, le style est porté par l'attribut standard du `<text>`
/// de la lyric ; aucun texte n'est dupliqué dans une direction. Les balises
/// partielles n'ont pas de porteur XSD (une seule `<text>` par syllabe) et
/// restent des défauts.
pub(crate) fn lyric_style(syllable: &Syllable) -> Vec<(&'static str, &'static str)> {
    let text = syllable.text.trim();
    let mut style = Vec::new();
    for (tag, attribute, value) in [
        ("b", "font-weight", "bold"),
        ("i", "font-style", "italic"),
        ("c", "color", "#FF0000"),
        ("tt", "font-family", "monospace"),
        ("ul", "underline", "1"),
    ] {
        if text.starts_with(&format!("<{tag}>")) && text.ends_with(&format!("</{tag}>")) {
            style.push((attribute, value));
        }
    }
    style
}

/// Porteur XSD 4.0 `words` (`formatted-text`, `:3495`, `:2610`) du texte que
/// la source imprime réellement : caractères spéciaux `<sp>`/`<v>` et rubriques
/// sans note chantée. La valeur est le texte imprimé, jamais une étiquette du
/// source, jamais une copie d'une `<lyric>` et jamais un texte vide.
pub(crate) fn write_printed_rubrics(out: &mut Out, syllable: &Syllable) {
    let fragments = printed_fragments(&syllable.text);
    let sung = syllable.notes.iter().any(|note| {
        !note
            .signs
            .iter()
            .any(|sign| sign.kind == SignKind::Alteration)
    });
    let values: Vec<String> = if sung {
        // Une syllabe chantée ne porte dans `<words>` que les caractères que
        // GregorioTeX imprime **hors** de la lyric (`Gabc.tex:297-309,345-378`) :
        // un `<sp>`/`<v>` inséré. Tout le texte brut — y compris les accolades
        // de centrage, invisibles (`Gabc.tex:314-324`) — reste dans la seule
        // `<lyric>` : le réimprimer serait une duplication avec balise source.
        fragments
            .into_iter()
            .filter(|fragment| fragment.markup)
            .map(|fragment| fragment.value)
            .collect()
    } else {
        let joined = fragments
            .iter()
            .map(|fragment| fragment.value.as_str())
            .collect::<String>();
        let value = joined.trim();
        if value.is_empty()
            || !fragments
                .iter()
                .any(|fragment| fragment.markup || printed_rubric_text(value))
        {
            Vec::new()
        } else {
            vec![value.to_owned()]
        }
    };
    for value in values {
        // Une rubrique sans note chantée est imprimée au-dessus de la portée
        // (numéro de verset, `V/`, `Ps.`) ; un caractère spécial inséré dans
        // une syllabe est imprimé avec la lyric, au-dessous. Cette distinction
        // est une position d'impression XSD (`placement`), pas un drapeau.
        let placement = if sung { "below" } else { "above" };
        out.xml
            .start_element("direction", &[("placement", placement)]);
        out.xml.start_element("direction-type", &[]);
        out.xml.tag_with("words", &[("color", "#FF0000")], &value);
        out.xml.end_element();
        out.xml.end_element();
    }
}

/// Écrit le payload L6 complet des syllabes dans la première mesure.
///
/// Une mesure dérivée se termine sur une barre réelle, pas sur une syllabe :
/// il y a donc moins de mesures que de syllabes. Les directions privées sont
/// regroupées au début du document pour que l'aller-retour L6 ne dépende pas
/// du nombre de mesures projetées.
pub(crate) fn write_all_syllable_directions(
    out: &mut Out,
    gregorian: &Gregorian,
    styled: bool,
    private: bool,
) {
    for index in 0..gregorian.syllables.len() {
        write_syllable_direction(out, gregorian, index, styled, private);
    }
}

/// Une syllabe et ses barres, indexée par sa position source.
pub(crate) fn write_syllable_direction(
    out: &mut Out,
    gregorian: &Gregorian,
    index: usize,
    styled: bool,
    private: bool,
) {
    let Some(syllable) = gregorian.syllables.get(index) else {
        return;
    };
    if private {
        let mut text = format!(
            "{PREFIX}syllable:{}|{}|{}",
            field('i', &index.to_string()),
            field('t', &syllable.text),
            field('w', if syllable.space_after { "1" } else { "0" })
        );
        if let Some(clef) = &syllable.clef {
            text.push('|');
            text.push_str(&field('c', clef));
        }
        if let Some(rubric) = &syllable.rubric {
            text.push('|');
            text.push_str(&field('r', rubric));
        }
        if let Some(above) = &syllable.above_text {
            text.push('|');
            text.push_str(&field('A', above));
        }
        for bar in &syllable.bars {
            text.push('|');
            text.push_str(&field('b', bar.as_str()));
        }
        for sign in &syllable.signs {
            text.push('|');
            text.push_str(&field('z', &sign.raw));
        }
        out.attr(false);
        write_direction(out, &text);
    }
    if let Some(above) = &syllable.above_text {
        // XSD 4.0 `words` / `placement=above` : le texte `[alt:…]` devient
        // lisible par n'importe quel lecteur MusicXML, sans le préfixe GABC.
        out.xml
            .start_element("direction", &[("placement", "above")]);
        out.xml.start_element("direction-type", &[]);
        // `Gabc.tex:339-343` : le texte de traduction est imprimé en italique.
        // L'attribut est additif au mode full support : les octets par défaut
        // restent ceux de la référence 4.7.4.
        if styled {
            out.xml
                .tag_with("words", &[("font-style", "italic")], above);
        } else {
            out.xml.tag_with("words", &[], above);
        }
        out.xml.end_element();
        out.xml.end_element();
    }
    // Une alteration (`Gabc.tex:481-489,512-540`) est un glyphe, pas une note
    // chantée : la projection dérivée ne produit donc aucun `<note>` pour
    // elle. Son rang source est transporté séparément pour que l'aller-retour
    // L6 reste exact ; aucun consommateur tiers n'a besoin de ce champ.
    if private {
        for (note_index, note) in syllable.notes.iter().enumerate() {
            if note
                .signs
                .iter()
                .any(|sign| sign.kind == SignKind::Alteration)
            {
                let mut text = format!(
                    "{PREFIX}alter:{}|{}|{}|{}",
                    field('i', &index.to_string()),
                    field('a', &note_index.to_string()),
                    field('l', &note.letter.to_string()),
                    field('g', &note.glyph)
                );
                if let Some(offset) = note.source_offset {
                    text.push('|');
                    text.push_str(&field('o', &offset.to_string()));
                }
                if let Some(end) = note.source_end {
                    text.push('|');
                    text.push_str(&field('e', &end.to_string()));
                }
                out.attr(false);
                write_direction(out, &text);
            }
        }
    }
}

/// Lettre et glyphe d'une note : `<other-notation>` non imprimé.
///
/// Les signes rythmiques vont dans `<technical><other-technical>` ci-dessous ;
/// les formes que MusicXML énumère vont dans `<notehead>` (XSD 4.0,
/// `notehead-value`).
pub(crate) fn write_note_notation(
    out: &mut Out,
    note: &GregorianNote,
    syllable_index: usize,
    note_index: usize,
    private: bool,
) {
    if !private {
        return;
    }
    let mut text = format!(
        "{PREFIX}note:{}|{}|{}|{}",
        field('l', &note.letter.to_string()),
        field('g', &note.glyph),
        field('i', &syllable_index.to_string()),
        field('a', &note_index.to_string())
    );
    // Positions source en unités UTF-16 : elles traversent le pont privé pour
    // que l'identifiant et la timeline restent les mêmes après un aller-retour.
    if let Some(offset) = note.source_offset {
        text.push('|');
        text.push_str(&field('o', &offset.to_string()));
    }
    if let Some(end) = note.source_end {
        text.push('|');
        text.push_str(&field('e', &end.to_string()));
    }
    out.xml
        .start_element("notations", &[("print-object", "no")]);
    out.xml
        .tag_with("other-notation", &[("type", "single")], &text);
    out.xml.end_element();
}

/// Vrai si une respiration visible doit être écrite sur cette note d'après
/// les barres du payload L6 (`docs/agents/gabc.md` §9.1).
///
/// Une barre qui appelle une respiration (`BarKind::has_breath`, gabctk
/// `gabctk.py:930-936`) la porte sur sa dernière note chantée. Quand le groupe
/// de barre ne contient aucune note (`(,)` avant un mot), la respiration est
/// reportée sur la dernière note chantée **précédant** la barre, jamais sur la
/// suivante : un `<breath-mark>` posé sur une note MusicXML se dessine après
/// elle, donc `x(g) (,) do(h)` placerait la virgule après `do`, de l'autre côté
/// de la divisio. S'aucune note chantée ne précède la barre (juste après la
/// clé), aucune respiration n'est émise. Une barre finale `::` n'en porte pas
/// non plus : le `<barline>` final la remplace.
pub(crate) fn bar_breath(gregorian: &Gregorian, source: &NoteSource) -> bool {
    let is_alteration = |note: &GregorianNote| {
        note.signs
            .iter()
            .any(|sign| sign.kind == SignKind::Alteration)
    };
    let Some(syllable) = gregorian.syllables.get(source.gregorian_syllable) else {
        return false;
    };
    // La note courante doit être la dernière chantée de son groupe : sinon
    // c'est la note suivante, plus proche de la barre, qui en héritera.
    let last_sung = syllable
        .notes
        .iter()
        .skip(source.gregorian_note + 1)
        .all(is_alteration);
    if !last_sung {
        return false;
    }
    // Une barre dans le même groupe est dessinée après cette dernière note.
    if syllable.bars.iter().any(|bar| bar.has_breath()) {
        return true;
    }
    // Un groupe de barres sans note après celui-ci remonte la respiration
    // jusqu'à la note courante, dernière chantée avant la barre. On s'arrête
    // à la première syllabe chantée suivante : elle est après la barre et ne
    // doit donc pas recevoir sa virgule.
    for following in gregorian
        .syllables
        .iter()
        .skip(source.gregorian_syllable + 1)
    {
        if following.notes.iter().any(|note| !is_alteration(note)) {
            return false;
        }
        if following.bars.iter().any(|bar| bar.has_breath()) {
            return true;
        }
    }
    false
}

/// Signes attachés à une note : `<other-technical>` (XSD 4.0,
/// `complexType other-technical`).
/// Noms SMuFL 1.4 d'un signe du glyphe GABC, dans l'ordre source.
///
/// Le registre officiel (`glyphnames.json`, ranges
/// `medievalAndRenaissancePlainchant{SingleNote,MultipleNote,Articulation}`)
/// donne un nom canonique à chaque forme : `chantQuilisma`, `chantStrophicus`,
/// `chantAuctumDesc`, `chantAugmentum`, `accidentalFlat`, … Un élément
/// `<other-notation>` porte **un** de ces noms (XSD 4.0 `:5361-5375`,
/// `maxOccurs="unbounded"`) : la séquence complète est donc lisible par un
/// tiers sans champ privé. Les chiffres de position GABC (`.0`, `_502`) et
/// les caractères d'espacement/fusion (`/`, `!`, `*`) n'ont aucun nom SMuFL :
/// ils ne sont pas simulés ici et restent des défauts mesurés.
fn smufl_signs(sign: &NeumeSign) -> Vec<&'static str> {
    if sign.raw.chars().any(|c| c.is_ascii_digit()) {
        return Vec::new();
    }
    match sign.kind {
        SignKind::Quilisma => vec!["chantQuilisma"; sign.raw.chars().count()],
        SignKind::Virga => vec![if sign.raw.starts_with('V') {
            "chantPunctumVirgaReversed"
        } else {
            "chantPunctumVirga"
        }],
        SignKind::Stropha => vec!["chantStrophicus"; sign.raw.chars().count()],
        SignKind::Oriscus => vec![if sign.raw.starts_with('O') {
            "chantOriscusDescending"
        } else {
            "chantOriscusAscending"
        }],
        SignKind::Liquescentia => match sign.raw.chars().next() {
            Some('<') => vec!["chantAuctumAsc"],
            Some('>') => vec!["chantAuctumDesc"],
            _ => vec!["chantPunctumDeminutum"],
        },
        SignKind::Linea => vec!["chantPunctumLinea"],
        SignKind::Cavum => vec!["chantPunctumCavum"],
        SignKind::PunctumMora => vec!["chantAugmentum"; sign.raw.matches('.').count()],
        SignKind::Ictus => vec!["chantIctusAbove"; sign.raw.matches('\'').count()],
        SignKind::HorizontalEpisema => vec!["chantEpisema"; sign.raw.matches('_').count()],
        SignKind::Alteration => match sign.raw.chars().next() {
            Some('x' | 'X') => vec!["accidentalFlat"; sign.raw.chars().count()],
            Some('#') => vec!["accidentalSharp"; sign.raw.matches('#').count()],
            Some('y' | 'Y') => vec!["accidentalNatural"],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Séquence SMuFL complète d'une note chantée : le nom de tête, puis un nom
/// par signe de son glyphe, dans l'ordre exact du source (`Gabc.tex:452-797`).
fn smufl_neumes(note: &GregorianNote) -> Vec<&'static str> {
    // La tête porte la position exacte sur la portée : `chantPunctum` est le
    // punctum quadratum (lettre minuscule du GABC), `chantPunctumInclinatum`
    // le punctum inclinatum (lettre majuscule, `Gabc.tex:452-468`). Clé + tête
    // + hauteur restaurent donc la lettre source, comme le demande
    // `musicxml-4.0.xsd` `clef`/`staff-lines`/`pitch/step`.
    let mut names = vec![if note.letter.is_ascii_uppercase() {
        "chantPunctumInclinatum"
    } else {
        "chantPunctum"
    }];
    for sign in &note.signs {
        names.extend(smufl_signs(sign));
    }
    names
}

/// Contexte d'une note grégorienne pour l'écriture de ses porteurs standard.
pub(crate) struct NoteSigns<'a> {
    pub note: &'a GregorianNote,
    pub syllable_index: usize,
    pub breath: bool,
    pub smufl: bool,
    pub private: bool,
}

pub(crate) fn write_note_signs(out: &mut Out, signs: &NoteSigns<'_>) {
    let NoteSigns {
        note,
        syllable_index,
        breath,
        smufl,
        private,
    } = *signs;
    if smufl {
        // XSD 4.0 `complexType other-notation` : l'attribut `smufl` donne une
        // **interopérabilité applicative** (« allowing application
        // interoperability »), contrairement aux champs privés `gabc:`.
        // `other-notation` est `maxOccurs="unbounded"` : un neume composé est
        // une séquence de noms canoniques, pas une étiquette contenant le
        // jeton source.
        let names = smufl_neumes(note);
        out.xml.start_element("notations", &[]);
        for name in names {
            out.xml
                .tag_with("other-notation", &[("type", "single"), ("smufl", name)], "");
        }
        out.xml.end_element();
    }
    // Un `other-technical` dont le texte est `letter: e`, `glyph: …` ou
    // `syllable-text: …` serait un vidage de la source, pas un porteur : aucun
    // autre outil n'y donne de sens. La lettre est déjà dans `<pitch>`, les
    // paroles dans `<lyric>` et la forme du neume dans le nom SMuFL. Les
    // attributs `[…]` et l'orthographe exacte du glyphe n'ont pas de porteur
    // dans XSD 4.0 : ils sont comptés comme défauts, pas simulés par un label.
    if note.signs.is_empty() && !breath {
        return;
    }
    if private && !note.signs.is_empty() {
        out.xml
            .start_element("notations", &[("print-object", "no")]);
        out.xml.start_element("technical", &[]);
        for sign in &note.signs {
            out.xml.tag_with(
                "other-technical",
                &[("print-object", "no")],
                &format!(
                    "{PREFIX}sign:{}|{}",
                    field('s', &sign.raw),
                    field('i', &syllable_index.to_string())
                ),
            );
        }
        out.xml.end_element();
        out.xml.end_element();
    }
    if breath {
        // XSD 4.0 `breath-mark` : une barre de séparation est une respiration
        // (`Gabc.tex:801-845`), pas une fin de mesure métrique. Le MIDI
        // correspondant porte la pause dans `SegmentItem::Breath`.
        out.xml.start_element("notations", &[]);
        // XSD 4.0 `group:articulation` : `breath-mark` est un enfant de
        // `<articulations>`, pas de `<notations>` directement. Sans cette
        // enveloppe, un lecteur conforme (et notre propre lecteur) ignore
        // la respiration.
        out.xml.start_element("articulations", &[]);
        out.xml.tag_empty("breath-mark", &[]);
        out.xml.end_element();
        out.xml.end_element();
    }
}

/// Liure d'un neume : une `<notations><slur>` ouverte sur la première note
/// chantée du groupe et fermée sur la dernière (`Gabc.tex:36-58,452-700`).
pub(crate) fn write_note_slur(out: &mut Out, start: bool, stop: bool) {
    if !start && !stop {
        return;
    }
    out.xml.start_element("notations", &[]);
    if start {
        out.xml
            .tag_empty("slur", &[("number", "1"), ("type", "start")]);
    }
    if stop {
        out.xml
            .tag_empty("slur", &[("number", "1"), ("type", "stop")]);
    }
    out.xml.end_element();
}

fn write_direction(out: &mut Out, text: &str) {
    out.xml
        .start_element("direction", &[("print-object", "no")]);
    out.xml.start_element("direction-type", &[]);
    out.xml
        .tag_with("other-direction", &[("print-object", "no")], text);
    out.xml.end_element();
    out.xml.end_element();
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, NeumeSign, SignKind, Syllable};

    #[test]
    fn escape_keeps_field_boundaries_unambiguous() {
        assert_eq!(escape("a|b\\c\nd>e"), "a\\|b\\\\c\\nd\\ge");
        assert_eq!(field('t', "<sp>R/</sp>"), "t=\\lsp\\gR/\\l/sp\\g");
    }

    #[test]
    fn no_raw_gabc_source_in_xml() {
        use scoretk_core::model::{
            Extension, GregorianHeader, GregorianNote, LyricsSyllabic, Syllable,
        };

        let gregorian = Gregorian {
            headers: vec![
                GregorianHeader {
                    name: "name".to_owned(),
                    value: "Veni Sancte Spiritus".to_owned(),
                },
                GregorianHeader {
                    name: "language".to_owned(),
                    value: "latin".to_owned(),
                },
            ],
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "<sc>Ve</sc>ni".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'g',
                    glyph: "h_".to_owned(),
                    signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                    ..GregorianNote::default()
                }],
                space_after: true,
                bars: vec![BarKind::DivisioFinalis],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let mut score = gregorian.build_derived_score();
        score.extensions.format = Some(Extension::Gregorian(gregorian.clone()));
        let xml = crate::export_musicxml(&score).expect("MusicXML export");
        // Not a single raw GABC delimiter or source group is copied: each
        // fact is rebuilt into its own MusicXML element/field.
        assert!(!xml.contains("%%"), "{xml}");
        assert!(!xml.contains("(g"), "{xml}");
        assert!(!xml.contains("<sc>Ve</sc>ni(h_')"), "{xml}");
        assert!(xml.contains("gabc:syllable:i=0"), "{xml}");
        assert!(xml.contains("gabc:note:l=g|g=h_"), "{xml}");
        assert!(xml.contains("gabc:sign:s='"), "{xml}");
        assert!(xml.contains("<senza-misura"), "{xml}");
        assert_eq!(
            score.measures[0]
                .segments
                .iter()
                .filter_map(|segment| {
                    match segment.element(scoretk_core::model::Track::new(
                        scoretk_core::model::StaffId::from_raw(0),
                        0,
                    )) {
                        Some(scoretk_core::model::SegmentItem::ChordRest(chord_rest)) => {
                            Some(chord_rest)
                        }
                        _ => None,
                    }
                })
                .filter(|chord_rest| {
                    chord_rest
                        .lyrics
                        .first()
                        .is_some_and(|lyric| lyric.syllabic == LyricsSyllabic::Single)
                })
                .count(),
            1
        );
    }

    /// Out-of-reach L6 facts are declared, not silently counted as gains:
    /// custos and line breaks (`Gabc.tex:872-906`), bracketed TeX/layout
    /// attributes (`:689-700,908-931`), NABC cuts
    /// (`gabc-score-determination.y:870-907`) and the exact order of bars
    /// inside a group. None of them is a sung fact and none gets an XML field.
    #[test]
    fn out_of_reach_gabc_facts_have_no_xml_field() {
        let mut out = Out::new_with(false, false);
        let gregorian = Gregorian {
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "<sp>V/</sp>.".to_owned(),
                bars: vec![BarKind::DivisioFinalis],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        write_header_directions(&mut out, &gregorian, true);
        write_syllable_direction(&mut out, &gregorian, 0, false, true);
        let xml = out.xml.finish();
        for absent in [
            "gabc:custos",
            "gabc:line-break",
            "gabc:nabc",
            "gabc:attribute",
        ] {
            assert!(
                !xml.contains(absent),
                "{absent} must not be invented: {xml}"
            );
        }
    }

    #[test]
    fn a_syllable_direction_carries_each_fact_separately() {
        let gregorian = Gregorian {
            syllables: vec![Syllable {
                text: "De|us".to_owned(),
                clef: Some("c3".to_owned()),
                rubric: Some("<sp>R/</sp>".to_owned()),
                bars: vec![BarKind::DivisioFinalis, BarKind::Dominican(2)],
                signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                space_after: true,
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let mut out = Out::new_with(false, false);
        write_syllable_direction(&mut out, &gregorian, 0, false, true);
        let xml = out.xml.finish();
        assert!(xml.contains("gabc:syllable:i=0|t=De\\|us|w=1"), "{xml}");
        assert!(xml.contains("|c=c3"), "{xml}");
        assert!(xml.contains("|r=\\lsp\\gR/\\l/sp\\g"), "{xml}");
        assert!(xml.contains("|b=::"), "{xml}");
        assert!(xml.contains("|b=;2"), "{xml}");
        assert!(xml.contains("|z='"), "{xml}");
    }

    #[test]
    fn printed_rubric_is_a_non_blank_words_carrier() {
        // XSD 4.0 `words` (:3495) imprime du texte : la valeur est celle que
        // le source imprime (`V/.`), jamais le jeton `<sp>` et jamais un
        // drapeau de couleur. Aucune lyric n'est dupliquée.
        let syllable = Syllable {
            text: "<sp>V/</sp>.".to_owned(),
            ..Syllable::default()
        };
        let mut out = Out::new_with(false, false);
        write_printed_rubrics(&mut out, &syllable);
        let xml = out.xml.finish();
        assert!(
            xml.contains(r##"<words color="#FF0000">V/.</words>"##),
            "{xml}"
        );
        assert!(!xml.contains("<sp>"), "{xml}");
        assert!(!xml.contains("<lyric"), "{xml}");
    }

    #[test]
    fn centering_braces_are_not_a_printed_rubric() {
        // `Gabc.tex:314-324` : `{}` force le centrage de la voyelle et ne
        // sont **jamais** imprimés. `mi(h){n}us(h)` est la syllabe chantée
        // « nus », pas une rubrique : aucun `<words>` rouge ne doit porter
        // `{n}us` en double de la `<lyric>`.
        let syllable = Syllable {
            text: "{n}us".to_owned(),
            notes: vec![
                scoretk_core::model::GregorianNote {
                    letter: 'h',
                    glyph: String::new(),
                    ..scoretk_core::model::GregorianNote::default()
                },
                scoretk_core::model::GregorianNote {
                    letter: 'h',
                    glyph: String::new(),
                    ..scoretk_core::model::GregorianNote::default()
                },
            ],
            ..Syllable::default()
        };
        let mut out = Out::new_with(false, false);
        write_printed_rubrics(&mut out, &syllable);
        assert!(
            !out.xml.finish().contains("<words"),
            "centering markup must never be a <words> carrier"
        );
        assert!(!printed_rubric_text("{n}us"));
        assert!(!printed_rubric_text("{ia}."));
        assert!(!printed_rubric_text("5. C{ui}"));
    }

    #[test]
    fn alteration_symbol_is_placed_at_its_staff_position() {
        // XSD 4.0 `direction-type/symbol` (:3530) + `default-y` portent le
        // bémol de `ixi` sur la lettre `i` (25 dixièmes sous `c4`).
        use scoretk_core::model::GregorianNote;
        let gregorian = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![Syllable {
                notes: vec![
                    GregorianNote {
                        letter: 'i',
                        glyph: "x".to_owned(),
                        signs: vec![NeumeSign::new(SignKind::Alteration, "x")],
                        ..GregorianNote::default()
                    },
                    GregorianNote {
                        letter: 'h',
                        ..GregorianNote::default()
                    },
                ],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let mut out = Out::new_with(false, false);
        write_alteration_symbols(&mut out, &gregorian, 0, 1);
        let xml = out.xml.finish();
        assert!(xml.contains(r#"default-y="25""#), "{xml}");
        assert!(xml.contains("<symbol>accidentalFlat</symbol>"), "{xml}");
    }
}
