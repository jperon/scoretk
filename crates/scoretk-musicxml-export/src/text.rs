// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp`,
// `importexport/musicxml/internal/shared/musicxmlfonthandler.cpp` et
// `engraving/dom/tempotext.cpp` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Textes MusicXML : `<words>`, `<metronome>`, `<rehearsal>` et `<credit>`.
//!
//! Portage de :
//!
//! - `MScoreTextToMusicXml` (`importexport/musicxml/internal/shared/musicxmlfonthandler.cpp`)
//!   pour la restitution des fragments formatés (gras, italique, police,
//!   taille) et de `ExportMusicXml::wordsMetronome`/
//!   `TempoText::findTempoDuration` (`engraving/dom/tempotext.cpp`) pour la
//!   détection de la formule de métronome ;
//! - `ExportMusicXml::words`, `systemText`, `tempoText`, `tempoSound`,
//!   `rehearsal` et `credits`
//!   (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! Les attributs de mise en page (`default-x`, `default-y`, `relative-x`,
//! `relative-y`, `justify` des directions) ne sont pas modélisés : la partition
//! ne porte ni géométrie ni style.

use scoretk_core::model::{
    Credit, CreditJustify, CreditStyle, CreditValign, DurationType, Placement, RehearsalFrame,
    RehearsalMark, RichText, Score, StyleSettings, Tempo, TextFormat, TextFragment,
    DEFAULT_TEXT_FONT, DEFAULT_TEXT_SIZE, MUSICAL_TEXT_FONT,
};

use super::exportmusicxml::{format_number, placement_name, Out};
use super::layout;
use super::pitch;

// ----------------------------------------------------------------------
// Formats
// ----------------------------------------------------------------------

/// Format effectif d'un fragment : balisage explicite fondu dans le style.
#[derive(Clone, Debug, PartialEq)]
struct EffectiveFormat {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    font_family: String,
    font_size: f64,
}

/// Format par défaut du style d'un texte (ce que le fichier `.mscx` ne porte
/// pas : MuseScore le tire de `MStyle`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct StyleFormat {
    font_family: &'static str,
    font_size: f64,
    bold: bool,
}

/// Style des textes de portée et système (`StaffText`/`SystemText`).
const WORDS_STYLE: StyleFormat = StyleFormat {
    font_family: DEFAULT_TEXT_FONT,
    font_size: DEFAULT_TEXT_SIZE,
    bold: false,
};

/// Style des indications de tempo (`TempoText`).
///
/// Le style « Tempo » de MuseScore 4.7 est en gras, taille 12 (voir
/// `testTempoPrecision` dans le corpus).
const TEMPO_STYLE: StyleFormat = StyleFormat {
    font_family: DEFAULT_TEXT_FONT,
    font_size: 12.0,
    bold: true,
};

/// Style des repères de répétition (`RehearsalMark`) : gras, taille 14.
const REHEARSAL_STYLE: StyleFormat = StyleFormat {
    font_family: DEFAULT_TEXT_FONT,
    font_size: 14.0,
    bold: true,
};

/// Style des changements d'instrument (`InstrumentChange`) : MuseScore les
/// affiche en gras, taille 12 (style « Instrument Name »).
pub(crate) const INSTRUMENT_CHANGE_STYLE: StyleFormat = StyleFormat {
    font_family: DEFAULT_TEXT_FONT,
    font_size: 12.0,
    bold: true,
};

/// Format de référence de `MScoreTextToMusicXml` (`formatForWords`) : celui du
/// style des textes de portée. Seules les différences sont écrites.
const BASE_FORMAT: StyleFormat = WORDS_STYLE;

/// Format du style d'un crédit.
///
/// La taille par défaut dépend de la version de MuseScore (le style « title »
/// vaut 24 pt en 3.x et 22 pt en 4.x) : elle vient de [`StyleSettings`], jamais
/// du seul nom du style.
fn credit_style(credit: &Credit, settings: &StyleSettings) -> StyleFormat {
    StyleFormat {
        font_family: DEFAULT_TEXT_FONT,
        font_size: settings.credit_font_size(&credit.style),
        bold: false,
    }
}

/// Fond le balisage d'un fragment dans le style du texte.
fn effective_format(format: &TextFormat, style: &StyleFormat) -> EffectiveFormat {
    // `updateFormat` remplace la police `ScoreText` des symboles par la police
    // de texte musical, qui est celle du style.
    let font_family = match format.font_family.as_deref() {
        None | Some(MUSICAL_TEXT_FONT) => style.font_family.to_owned(),
        Some(family) => family.to_owned(),
    };
    EffectiveFormat {
        bold: format.bold || style.bold,
        italic: format.italic,
        underline: format.underline,
        strike: format.strike,
        font_family,
        font_size: format.font_size.unwrap_or(style.font_size),
    }
}

/// Formate une taille de police comme `String::number`/`QString::arg(double)`
/// (six chiffres significatifs, zéros finaux retirés).
pub(crate) fn format_font_size(value: f64) -> String {
    if !value.is_finite() || value == 0.0 {
        return "0".to_owned();
    }
    let magnitude = value.abs().log10().floor() as i32;
    let decimals = (6 - 1 - magnitude).max(0) as usize;
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

// ----------------------------------------------------------------------
// Écriture des fragments
// ----------------------------------------------------------------------

/// Écrit une liste de fragments formatés (`MScoreTextToMusicXml::writeTextFragments`).
///
/// `attributes` n'est écrit que sur la première balise, comme `firstTime` dans
/// le C++. Les attributs de format ne portent que les changements par rapport
/// au fragment précédent, la référence initiale étant [`BASE_FORMAT`].
fn write_fragments(
    out: &mut Out,
    tag: &str,
    attributes: &[(&str, &str)],
    fragments: &[TextFragment],
    style: &StyleFormat,
) {
    if fragments.is_empty() {
        return;
    }
    let mut previous = effective_format(&TextFormat::default(), &BASE_FORMAT);
    let mut first = true;

    for fragment in fragments {
        let current = effective_format(&fragment.format, style);
        let mut changes: Vec<(&str, String)> = Vec::new();
        if current.bold != previous.bold {
            changes.push((
                "font-weight",
                if current.bold { "bold" } else { "normal" }.to_owned(),
            ));
        }
        if current.italic != previous.italic {
            changes.push((
                "font-style",
                if current.italic { "italic" } else { "normal" }.to_owned(),
            ));
        }
        if current.underline != previous.underline {
            changes.push((
                "underline",
                if current.underline { "1" } else { "0" }.to_owned(),
            ));
        }
        if current.strike != previous.strike {
            changes.push((
                "line-through",
                if current.strike { "1" } else { "0" }.to_owned(),
            ));
        }
        if current.font_family != previous.font_family {
            changes.push(("font-family", current.font_family.clone()));
        }
        // MuseScore tolère 1 % d'écart, pour éviter les attributs dus aux
        // arrondis de taille.
        if current.font_size < 0.99 * previous.font_size
            || current.font_size > 1.01 * previous.font_size
        {
            changes.push(("font-size", format_font_size(current.font_size)));
        }

        let mut all: Vec<(&str, &str)> = Vec::new();
        if first {
            all.extend_from_slice(attributes);
        }
        for (name, value) in &changes {
            all.push((name, value.as_str()));
        }
        out.xml.tag_raw(tag, &all, &fragment.text);
        previous = current;
        first = false;
    }
}

// ----------------------------------------------------------------------
// Métronome
// ----------------------------------------------------------------------

/// Point de code du point d'augmentation de métronome (`metAugmentationDot`).
const METRONOME_DOT: char = '\u{ECB7}';

/// Motif de durée de métronome.
///
/// Table `tp` de `engraving/dom/tempotext.cpp`, dans le même ordre : MuseScore
/// retient le premier motif qui correspond, pas la correspondance la plus à
/// gauche.
struct TempoPattern {
    /// Point de code MuseScore de la figure (`metNoteQuarterUp`…).
    symbol: char,
    /// Points d'augmentation.
    dots: u8,
    /// Figure correspondante.
    kind: DurationType,
}

/// Motifs de `TempoText::findTempoDuration`, dans l'ordre du C++.
const TEMPO_PATTERNS: &[TempoPattern] = &[
    // double croche pointée deux fois / pointée / simple
    TempoPattern {
        symbol: '\u{ECA5}',
        dots: 2,
        kind: DurationType::Quarter,
    },
    TempoPattern {
        symbol: '\u{ECA5}',
        dots: 1,
        kind: DurationType::Quarter,
    },
    TempoPattern {
        symbol: '\u{ECA5}',
        dots: 0,
        kind: DurationType::Quarter,
    },
    TempoPattern {
        symbol: '\u{ECA3}',
        dots: 2,
        kind: DurationType::Half,
    },
    TempoPattern {
        symbol: '\u{ECA3}',
        dots: 1,
        kind: DurationType::Half,
    },
    TempoPattern {
        symbol: '\u{ECA3}',
        dots: 0,
        kind: DurationType::Half,
    },
    TempoPattern {
        symbol: '\u{ECA7}',
        dots: 2,
        kind: DurationType::Eighth,
    },
    TempoPattern {
        symbol: '\u{ECA7}',
        dots: 1,
        kind: DurationType::Eighth,
    },
    TempoPattern {
        symbol: '\u{ECA7}',
        dots: 0,
        kind: DurationType::Eighth,
    },
    TempoPattern {
        symbol: '\u{ECA2}',
        dots: 1,
        kind: DurationType::Whole,
    },
    TempoPattern {
        symbol: '\u{ECA2}',
        dots: 0,
        kind: DurationType::Whole,
    },
    TempoPattern {
        symbol: '\u{ECA9}',
        dots: 1,
        kind: DurationType::Sixteenth,
    },
    TempoPattern {
        symbol: '\u{ECA9}',
        dots: 0,
        kind: DurationType::Sixteenth,
    },
    TempoPattern {
        symbol: '\u{ECAB}',
        dots: 1,
        kind: DurationType::ThirtySecond,
    },
    TempoPattern {
        symbol: '\u{ECAB}',
        dots: 0,
        kind: DurationType::ThirtySecond,
    },
    TempoPattern {
        symbol: '\u{ECA1}',
        dots: 0,
        kind: DurationType::Breve,
    },
    TempoPattern {
        symbol: '\u{ECA0}',
        dots: 0,
        kind: DurationType::Breve,
    },
    TempoPattern {
        symbol: '\u{ECAD}',
        dots: 0,
        kind: DurationType::SixtyFourth,
    },
    TempoPattern {
        symbol: '\u{ECAF}',
        dots: 0,
        kind: DurationType::OneHundredTwentyEighth,
    },
    TempoPattern {
        symbol: '\u{ECB1}',
        dots: 0,
        kind: DurationType::TwoHundredFiftySixth,
    },
    TempoPattern {
        symbol: '\u{ECB3}',
        dots: 0,
        kind: DurationType::FiveHundredTwelfth,
    },
    TempoPattern {
        symbol: '\u{ECB5}',
        dots: 0,
        kind: DurationType::OneThousandTwentyFourth,
    },
];

/// Correspondance d'un motif à partir d'une position (`std::regex_search`).
fn match_pattern(text: &[char], from: usize, pattern: &TempoPattern) -> Option<(usize, usize)> {
    'search: for start in from..text.len() {
        if text[start] != pattern.symbol {
            continue;
        }
        let mut index = start + 1;
        for _ in 0..pattern.dots {
            while index < text.len() && text[index].is_whitespace() {
                index += 1;
            }
            if index >= text.len() || text[index] != METRONOME_DOT {
                continue 'search;
            }
            index += 1;
        }
        return Some((start, index - start));
    }
    None
}

/// Cherche la partie « durée » d'un tempo (`TempoText::findTempoDuration`).
///
/// Renvoie la position, la longueur, la figure et le nombre de points.
fn find_tempo_duration(text: &[char], from: usize) -> Option<(usize, usize, DurationType, u8)> {
    for pattern in TEMPO_PATTERNS {
        if let Some((position, length)) = match_pattern(text, from, pattern) {
            return Some((position, length, pattern.kind, pattern.dots));
        }
    }
    None
}

/// Cherche `\s*=\s*` à partir d'une position, comme `indexOf`.
fn find_equation(text: &[char], from: usize) -> Option<(usize, usize)> {
    for start in from..text.len() {
        let mut index = start;
        while index < text.len() && text[index].is_whitespace() {
            index += 1;
        }
        if index < text.len() && text[index] == '=' {
            let mut end = index + 1;
            while end < text.len() && text[end].is_whitespace() {
                end += 1;
            }
            return Some((start, end - start));
        }
    }
    None
}

/// Cherche un nombre `\d+([,.]\d+)?` au début du texte.
fn find_number(text: &[char]) -> Option<(usize, usize)> {
    let mut index = 0;
    while index < text.len() && text[index].is_ascii_digit() {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    if index + 1 < text.len() && (text[index] == ',' || text[index] == '.') {
        let mut decimals = index + 1;
        while decimals < text.len() && text[decimals].is_ascii_digit() {
            decimals += 1;
        }
        if decimals > index + 1 {
            index = decimals;
        }
    }
    Some((0, index))
}

/// Formule de métronome trouvée dans un texte.
struct Metronome {
    /// Parenthèses autour de la formule (`parentheses="yes"`).
    parentheses: bool,
    /// Figure de gauche.
    beat_unit: DurationType,
    /// Points de la figure de gauche.
    beat_dots: u8,
    /// Figure de droite, quand « = ♩ » répète une durée.
    second_beat: Option<(DurationType, u8)>,
    /// Nombre de battements par minute.
    per_minute: Option<String>,
    /// Texte à gauche de la formule.
    left: Vec<TextFragment>,
    /// Texte à droite de la formule.
    right: Vec<TextFragment>,
}

/// Découpe une liste de fragments aux positions demandées (`split`).
///
/// La partie centrale est écartée : elle correspond à la formule de métronome.
fn split_fragments(
    fragments: &[TextFragment],
    position: usize,
    length: usize,
) -> (Vec<TextFragment>, Vec<TextFragment>) {
    let end = position + length;
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut offset = 0usize;

    for fragment in fragments {
        let characters: Vec<char> = fragment.text.chars().collect();
        let fragment_end = offset + characters.len();

        if offset < position {
            let take = position.min(fragment_end) - offset;
            if take > 0 {
                left.push(TextFragment::new(
                    characters[..take].iter().collect::<String>(),
                    fragment.format.clone(),
                ));
            }
        }
        if fragment_end > end {
            let skip = end.saturating_sub(offset);
            right.push(TextFragment::new(
                characters[skip..].iter().collect::<String>(),
                fragment.format.clone(),
            ));
        }
        offset = fragment_end;
    }

    (left, right)
}

/// Cherche une formule de métronome (`findMetronome`).
fn find_metronome(fragments: &[TextFragment]) -> Option<Metronome> {
    let characters: Vec<char> = fragments
        .iter()
        .flat_map(|fragment| fragment.text.chars())
        .collect();
    let (position, length, beat_unit, beat_dots) = find_tempo_duration(&characters, 0)?;

    let mut start = position;
    let mut span = length;
    let mut second_beat = None;
    let mut per_minute = None;

    if let Some((equation, equation_length)) = find_equation(&characters, position + length) {
        if equation == position + length {
            let after = &characters[equation + equation_length..];
            if let Some((found, found_length, kind, dots)) = find_tempo_duration(after, 0) {
                if found != 0 {
                    return None;
                }
                second_beat = Some((kind, dots));
                span = length + equation_length + found_length;
            } else if let Some((found, found_length)) = find_number(after) {
                if found != 0 {
                    return None;
                }
                per_minute = Some(after[..found_length].iter().collect::<String>());
                span = length + equation_length + found_length;
            } else {
                return None;
            }
        }
    }

    let mut parentheses = false;
    if start > 0
        && characters[start - 1] == '('
        && characters.len() > start + span
        && characters[start + span] == ')'
    {
        parentheses = true;
        start -= 1;
        span += 2;
    }

    let (left, right) = split_fragments(fragments, start, span);
    Some(Metronome {
        parentheses,
        beat_unit,
        beat_dots,
        second_beat,
        per_minute,
        left,
        right,
    })
}

/// Écrit `<beat-unit>` avec ses points (`beatUnit`).
fn write_beat_unit(out: &mut Out, kind: DurationType, dots: u8) {
    let name = pitch::duration_type_name(kind);
    if name.is_empty() {
        return;
    }
    out.xml.tag("beat-unit", name);
    for _ in 0..dots {
        out.xml.tag_empty("beat-unit-dot", &[]);
    }
}

/// Écrit le contenu d'une direction textuelle (`wordsMetronome`).
///
/// Une formule de métronome reconnue dans les fragments donne un
/// `<metronome>`, entouré des `<words>` de gauche et de droite ; sinon tout le
/// texte est écrit en un ou plusieurs `<words>`.
fn write_words_metronome(out: &mut Out, text: &RichText, style: &StyleFormat, offset: i64) {
    match find_metronome(text.fragments()) {
        Some(metronome) => {
            if !metronome.left.is_empty() {
                out.xml.start_element("direction-type", &[]);
                write_fragments(out, "words", &[], &metronome.left, style);
                out.xml.end_element();
            }

            out.xml.start_element("direction-type", &[]);
            out.xml.start_element(
                "metronome",
                &[(
                    "parentheses",
                    if metronome.parentheses { "yes" } else { "no" },
                )],
            );
            write_beat_unit(out, metronome.beat_unit, metronome.beat_dots);
            if let Some((kind, dots)) = metronome.second_beat {
                write_beat_unit(out, kind, dots);
            }
            if let Some(per_minute) = &metronome.per_minute {
                out.xml.tag("per-minute", per_minute);
            }
            out.xml.end_element();
            out.xml.end_element();

            if !metronome.right.is_empty() {
                out.xml.start_element("direction-type", &[]);
                write_fragments(out, "words", &[], &metronome.right, style);
                out.xml.end_element();
            }
        }
        None => {
            if text.fragments().is_empty() {
                return;
            }
            out.xml.start_element("direction-type", &[]);
            write_fragments(out, "words", &[], text.fragments(), style);
            out.xml.end_element();
        }
    }

    if offset != 0 {
        out.xml
            .tag_with("offset", &[("sound", "yes")], &offset.to_string());
    }
}

// ----------------------------------------------------------------------
// Directions
// ----------------------------------------------------------------------

/// Écrit un texte de portée ou système (`ExportMusicXml::words`, `systemText`).
///
/// `system` vaut vrai pour un texte système : MusicXML le marque
/// `system="only-top"` (MuseScore n'écrit pas `also-top` sans parties liées).
pub(crate) fn write_words(
    out: &mut Out,
    text: &RichText,
    placement: Placement,
    system: bool,
    xml_staff: usize,
    offset: i64,
) {
    write_words_styled(
        out,
        text,
        placement,
        system,
        xml_staff,
        offset,
        &WORDS_STYLE,
    );
}

/// Variante de [`write_words`] avec le style du texte (`wordsMetronome`).
pub(crate) fn write_words_styled(
    out: &mut Out,
    text: &RichText,
    placement: Placement,
    system: bool,
    xml_staff: usize,
    offset: i64,
    style: &StyleFormat,
) {
    if text.is_empty() {
        return;
    }
    out.attr(false);
    let mut attributes: Vec<(&str, &str)> = vec![("placement", placement_name(placement))];
    if system {
        attributes.push(("system", "only-top"));
    }
    out.xml.start_element("direction", &attributes);
    write_words_metronome(out, text, style, offset);
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

/// Écrit `<sound><swing>` (`ExportMusicXml::swingSound`).
///
/// `offset` ajoute `<offset sound="yes">` à l'intérieur de `<sound>`, pour un
/// texte système invisible.
pub(crate) fn write_swing_sound(
    out: &mut Out,
    swing: &scoretk_core::model::SwingParameters,
    offset: Option<i64>,
) {
    use scoretk_core::model::SwingUnit;

    out.xml.start_element("sound", &[]);
    out.xml.start_element("swing", &[]);
    match swing.unit {
        SwingUnit::Straight => out.xml.tag_empty("straight", &[]),
        SwingUnit::Eighth | SwingUnit::Sixteenth => {
            let ratio = i64::from(swing.ratio);
            let divisor = gcd(ratio, 100).max(1);
            out.xml.tag("first", &(100 / divisor).to_string());
            out.xml.tag("second", &(ratio / divisor).to_string());
            let kind = if swing.unit == SwingUnit::Eighth {
                "eighth"
            } else {
                "16th"
            };
            out.xml.tag("swing-type", kind);
        }
    }
    out.xml.end_element();
    if let Some(offset) = offset {
        out.xml
            .tag_with("offset", &[("sound", "yes")], &offset.to_string());
    }
    out.xml.end_element();
}

/// Plus grand commun diviseur (signe ignoré).
fn gcd(left: i64, right: i64) -> i64 {
    let (mut left, mut right) = (left.abs(), right.abs());
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Écrit un texte système (`ExportMusicXml::systemText`), swing compris.
pub(crate) fn write_system_text(
    out: &mut Out,
    text: &scoretk_core::model::SystemText,
    xml_staff: usize,
    offset: i64,
) {
    if text.text.is_empty() {
        return;
    }
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[
            ("placement", placement_name(text.placement)),
            ("system", "only-top"),
        ],
    );
    write_words_metronome(out, &text.text, &WORDS_STYLE, offset);
    if let Some(swing) = &text.swing {
        write_swing_sound(out, swing, None);
    }
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

/// Écrit le texte d'un cadre `TBox` (`ExportMusicXml::tboxTextAsWords`).
///
/// MuseScore l'écrit en `<words valign="top">` dans une direction au début de
/// la mesure qui suit le cadre ; les positions (`default-x`, `default-y`) sont
/// de la mise en page et ne sont pas modélisées.
pub(crate) fn write_tbox(out: &mut Out, tbox: &scoretk_core::model::TboxText) {
    if tbox.text.is_empty() {
        return;
    }
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[("placement", placement_name(tbox.placement))],
    );
    out.xml.start_element("direction-type", &[]);
    let mut style = WORDS_STYLE;
    if let Some(font_size) = tbox.font_size {
        style.font_size = font_size;
    }
    write_fragments(
        out,
        "words",
        &[("valign", "top")],
        tbox.text.fragments(),
        &style,
    );
    out.xml.end_element();
    out.xml.end_element();
}

/// Écrit `<sound tempo=…>` (`tempoSound`).
pub(crate) fn write_tempo_sound(out: &mut Out, tempo: &Tempo) {
    write_tempo_sound_value(out, tempo.beats_per_second);
}

/// Écrit `<sound tempo=…>` à partir d'une valeur en battements par seconde.
pub(crate) fn write_tempo_sound_value(out: &mut Out, beats_per_second: Option<f64>) {
    let Some(beats_per_second) = beats_per_second else {
        return;
    };
    // MuseScore arrondit à deux décimales : les fichiers stockent parfois
    // 91,9998 pour 92.
    let bpm = (beats_per_second * 60.0 * 100.0).round() / 100.0;
    let value = format_number(bpm, 2);
    out.xml.tag_empty("sound", &[("tempo", &value)]);
}

/// Écrit une indication de tempo (`ExportMusicXml::tempoText`).
pub(crate) fn write_tempo(out: &mut Out, tempo: &Tempo, xml_staff: usize, offset: i64) {
    if tempo.text.is_empty() && tempo.beats_per_second.is_none() {
        return;
    }
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[
            ("placement", placement_name(tempo.placement)),
            ("system", "only-top"),
        ],
    );
    write_words_metronome(out, &tempo.text, &TEMPO_STYLE, offset);
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    write_tempo_sound(out, tempo);
    out.xml.end_element();
}

/// Écrit un repère de répétition (`ExportMusicXml::rehearsal`).
pub(crate) fn write_rehearsal(out: &mut Out, mark: &RehearsalMark, xml_staff: usize, offset: i64) {
    if mark.text.is_empty() {
        return;
    }
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[
            ("placement", placement_name(mark.placement)),
            ("system", "only-top"),
        ],
    );
    out.xml.start_element("direction-type", &[]);
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    match mark.frame {
        // `enclosure="square"` est la valeur par défaut de MusicXML, que
        // MuseScore n'écrit pas quand le repère est encadré.
        RehearsalFrame::Rectangle => {}
        RehearsalFrame::Circle => attributes.push(("enclosure", "circle")),
        RehearsalFrame::NoFrame => attributes.push(("enclosure", "none")),
    }
    // `Sid::rehearsalMarkAlign` vaut `HCENTER` par défaut : MuseScore écrit
    // `justify="center"` pour un repère centré.
    attributes.push(("justify", "center"));
    write_fragments(
        out,
        "rehearsal",
        &attributes,
        mark.text.fragments(),
        &REHEARSAL_STYLE,
    );
    out.xml.end_element();
    if offset != 0 {
        out.xml.tag("offset", &offset.to_string());
    }
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

// ----------------------------------------------------------------------
// Crédits
// ----------------------------------------------------------------------

/// Écrit les `<credit>` de la partition (`ExportMusicXml::credits`).
///
/// MuseScore parcourt les cadres verticaux de toutes les pages, dans l'ordre
/// des pages ; `Credit::page` vient des `<LayoutBreak>` lus par le lecteur.
pub(crate) fn write_credits(out: &mut Out, score: &Score) {
    for credit in &score.credits {
        write_credit(out, credit, &score.style);
    }

    // MuseScore ajoute le copyright en bas au centre de chaque page.
    if !score.metadata.copyright.is_empty() {
        for page in 1..=score.style.page_count.max(1) {
            let credit = Credit {
                text: RichText::new(score.metadata.copyright.clone()),
                style: CreditStyle::Other("rights".to_owned()),
                justify: CreditJustify::Center,
                valign: CreditValign::Bottom,
                page,
                font_size: None,
                vbox_height: 10.0,
                extra_words: Vec::new(),
            };
            write_credit(out, &credit, &score.style);
        }
    }
}

/// Écrit un `<credit>` (`creditWords`).
///
/// La position est celle que calcule `textAsCreditWords` : `default-x` selon
/// l'alignement et les marges, `default-y` depuis le haut de la page et la
/// marge haute (voir [`layout::credit_position`]).
fn write_credit(out: &mut Out, credit: &Credit, settings: &StyleSettings) {
    if credit.text.is_empty() {
        return;
    }
    let page = credit.page.to_string();
    out.xml.start_element("credit", &[("page", &page)]);
    // MuseScore passe le type au crédit lui-même : `tidToCreditType` pour les
    // textes de cadre, `rights` pour le copyright.
    let is_rights = matches!(&credit.style, CreditStyle::Other(name) if name == "rights");
    let kind = if is_rights {
        Some("rights")
    } else {
        credit.style.credit_type()
    };
    if let Some(kind) = kind {
        out.xml.tag("credit-type", kind);
    }

    let (x, y) = if is_rights {
        layout::rights_position(settings)
    } else {
        layout::credit_position(credit, settings)
    };
    let x = format_number(x, 2);
    let y = format_number(y, 2);
    let attributes: Vec<(&str, &str)> = vec![
        ("default-x", &x),
        ("default-y", &y),
        ("justify", credit.justify.musicxml_name()),
        ("valign", credit.valign.musicxml_name()),
    ];
    let mut style = credit_style(credit, settings);
    if let Some(font_size) = credit.font_size {
        style.font_size = font_size;
    }
    write_fragments(
        out,
        "credit-words",
        &attributes,
        credit.text.fragments(),
        &style,
    );
    for extra in &credit.extra_words {
        out.xml.tag("credit-words", extra.plain());
    }
    out.xml.end_element();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragments(markup: &str) -> Vec<TextFragment> {
        RichText::new(markup).fragments().to_vec()
    }

    #[test]
    fn formule_de_metronome_simple() {
        let metronome = find_metronome(&fragments("<sym>metNoteQuarterUp</sym> = 80")).unwrap();
        assert_eq!(metronome.beat_unit, DurationType::Quarter);
        assert_eq!(metronome.beat_dots, 0);
        assert_eq!(metronome.per_minute.as_deref(), Some("80"));
        assert!(!metronome.parentheses);
        assert!(metronome.left.is_empty());
        assert!(metronome.right.is_empty());
    }

    #[test]
    fn formule_avec_texte_a_gauche_et_parentheses() {
        let metronome = find_metronome(&fragments(
            "Allegro (<sym>metNoteHalfUp</sym> = 60) ma non troppo",
        ))
        .unwrap();
        assert!(metronome.parentheses);
        assert_eq!(metronome.beat_unit, DurationType::Half);
        assert_eq!(
            metronome
                .left
                .iter()
                .map(|fragment| fragment.text.as_str())
                .collect::<String>(),
            "Allegro "
        );
        assert_eq!(
            metronome
                .right
                .iter()
                .map(|fragment| fragment.text.as_str())
                .collect::<String>(),
            " ma non troppo"
        );
    }

    #[test]
    fn formule_pointee_et_second_beat_unit() {
        let metronome = find_metronome(&fragments(
            "<sym>metNoteQuarterUp</sym><sym>metAugmentationDot</sym> = <sym>metNote8thUp</sym>",
        ))
        .unwrap();
        assert_eq!(metronome.beat_dots, 1);
        assert_eq!(metronome.second_beat, Some((DurationType::Eighth, 0)));
        assert!(metronome.per_minute.is_none());
    }

    #[test]
    fn texte_sans_metronome() {
        assert!(find_metronome(&fragments("Allegro")).is_none());
        // « = » sans nombre ni durée à droite : pas une formule.
        assert!(find_metronome(&fragments("<sym>metNoteQuarterUp</sym> = ")).is_none());
    }

    #[test]
    fn decoupage_des_fragments() {
        let (left, right) = split_fragments(&fragments("abcDEFghi"), 3, 3);
        assert_eq!(left[0].text, "abc");
        assert_eq!(right[0].text, "ghi");
    }
}
