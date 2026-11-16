//! Lignes de texte et changements d'instrument à l'export.
//!
//! Portage de `ExportMusicXml::textLine`, `writeHairpinText`,
//! `tboxTextAsWords`, `writeInstrumentChange` et `writeInstrumentDetails`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! Les positions (`default-x`, `default-y`, `relative-x`, `relative-y`) et les
//! couleurs relèvent de la mise en page et ne sont pas modélisées : seuls les
//! attributs structurels (type, number, line-end, line-type, end-length) sont
//! écrits.

use crate::model::{
    Fraction, HookType, Instrument, InstrumentChange, LineFont, Spanner, TextLineInfo,
    TextLineStyle,
};

use super::exportmusicxml::{format_number, placement_name, Context, Out};

/// Écrit le début d'une ligne de texte (`textLine` avec `isStart`).
pub(crate) fn start(out: &mut Out, spanner: &Spanner, xml_staff: usize) {
    let Some(info) = &spanner.text_line else {
        return;
    };
    write_text_line(out, spanner, info, true, xml_staff);
}

/// Écrit la fin d'une ligne de texte (`textLine` appelé par `spannerStop`).
pub(crate) fn stop(out: &mut Out, spanner: &Spanner, xml_staff: usize) {
    let Some(info) = &spanner.text_line else {
        return;
    };
    write_text_line(out, spanner, info, false, xml_staff);
}

fn write_text_line(
    out: &mut Out,
    spanner: &Spanner,
    info: &TextLineInfo,
    is_start: bool,
    xml_staff: usize,
) {
    let text = if is_start {
        info.begin_text.as_ref()
    } else {
        info.end_text.as_ref()
    };

    if !info.line_visible {
        let Some(text) = text else {
            return;
        };
        let font = if is_start {
            &info.begin_font
        } else {
            &info.end_font
        };
        out.attr(false);
        let placement = placement_name(info.placement);
        let mut attributes: Vec<(&str, &str)> = vec![("placement", placement)];
        if info.system {
            attributes.push(("system", "only-top"));
        }
        out.xml.start_element("direction", &attributes);
        out.xml.start_element("direction-type", &[]);
        write_line_words(out, text.plain(), font);
        out.xml.end_element();
        // `spannerStop` passe la position sentinelle `Fraction(-1, 1)` à
        // `textLine`, d'où l'`<offset>` négatif des fins de ligne invisibles.
        let tick = if is_start {
            spanner.start.tick
        } else {
            Fraction::from_integer(-1)
        };
        let offset = out.duration_in_divisions(tick - out.tick);
        if offset != 0 {
            out.xml
                .tag_with("offset", &[("sound", "yes")], &offset.to_string());
        }
        if xml_staff > 0 {
            out.xml.tag("staff", &xml_staff.to_string());
        }
        out.xml.end_element();
        return;
    }

    // Cas particulier : une ligne en traits sans crochet devient `<dashes>`.
    let is_dashes = info.line_style == TextLineStyle::Dashed
        && info.begin_hook == HookType::None
        && info.end_hook == HookType::None;

    let kind = if is_start { "start" } else { "stop" };
    let hook = if is_start {
        info.begin_hook
    } else {
        info.end_hook
    };
    let hook_height = if is_start {
        info.begin_hook_height
    } else {
        info.end_hook_height
    };
    let (line_end, end_length) = hook_name(hook, hook_height, info.placement);

    out.attr(false);
    let mut attributes: Vec<(&str, &str)> = vec![("placement", placement_name(info.placement))];
    if info.system {
        attributes.push(("system", "only-top"));
    }
    out.xml.start_element("direction", &attributes);

    if is_start {
        if let Some(begin) = &info.begin_text {
            out.xml.start_element("direction-type", &[]);
            out.xml.tag_raw("words", &[], begin.plain());
            out.xml.end_element();
        }
    }

    out.xml.start_element("direction-type", &[]);
    if is_dashes {
        out.xml.tag_empty("dashes", &[("type", kind), ("number", "1")]);
    } else {
        let number = "1".to_owned();
        let end_length = end_length.map(|value| format_number(value, 0));
        let mut attributes: Vec<(&str, &str)> = vec![
            ("type", kind),
            ("number", &number),
            ("line-end", line_end),
        ];
        // `line-type` n'est écrit qu'au début de la ligne.
        let line_type = if is_start {
            info.line_style.musicxml_name()
        } else {
            ""
        };
        if !line_type.is_empty() {
            attributes.push(("line-type", line_type));
        }
        if let Some(end_length) = &end_length {
            attributes.push(("end-length", end_length));
        }
        out.xml.tag_empty("bracket", &attributes);
    }
    out.xml.end_element();

    if !is_start {
        if let Some(end) = &info.end_text {
            out.xml.start_element("direction-type", &[]);
            out.xml.tag_raw("words", &[], end.plain());
            out.xml.end_element();
        }
    }

    let tick = if is_start {
        spanner.start.tick
    } else {
        spanner.end.tick
    };
    let offset = out.duration_in_divisions(tick - out.tick);
    if offset != 0 {
        out.xml
            .tag_with("offset", &[("sound", "yes")], &offset.to_string());
    }
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

/// Nom MusicXML du crochet et longueur associée (`textLine`).
fn hook_name(
    hook: HookType,
    height: f64,
    placement: crate::model::Placement,
) -> (&'static str, Option<f64>) {
    match hook {
        HookType::Hook90 => (
            if placement == crate::model::Placement::Below {
                "up"
            } else {
                "down"
            },
            Some(height.abs() * 10.0),
        ),
        HookType::Hook90T => ("both", Some(height.abs() * 20.0)),
        HookType::Arrow | HookType::ArrowFilled => ("arrow", None),
        HookType::None => ("none", None),
    }
}

/// Écrit `<words>` avec la police d'une extrémité de ligne
/// (`writeHairpinText`).
fn write_line_words(out: &mut Out, text: &str, font: &LineFont) {
    let family = font.family.clone();
    let size = super::text::format_font_size(font.size);
    let weight = if font.bold { Some("bold") } else { None };
    let style = if font.italic { Some("italic") } else { None };
    let mut attributes: Vec<(&str, &str)> = vec![
        ("font-family", &family),
        ("font-size", &size),
    ];
    if let Some(weight) = weight {
        attributes.push(("font-weight", weight));
    }
    if let Some(style) = style {
        attributes.push(("font-style", style));
    }
    if font.underline {
        attributes.push(("underline", "1"));
    }
    if font.strike {
        attributes.push(("line-through", "1"));
    }
    out.xml.tag_raw("words", &attributes, text);
}

/// Écrit le texte d'un cadre `TBox` au début de sa mesure
/// (`tboxTextAsWords`), pour la première partie seulement.
pub(crate) fn write_tboxes(out: &mut Out, context: &Context<'_>, measure_index: usize) {
    for tbox in &context.score.tbox_texts {
        if tbox.measure == measure_index {
            super::text::write_tbox(out, tbox);
        }
    }
}

/// Écrit les détails d'un changement d'instrument (`writeInstrumentChange`).
///
/// Écrit `<print>` (noms affichés), la transposition dans `<attributes>` et
/// `<sound><instrument-change>` ; le texte lui-même est une direction écrite
/// par `annotations`.
pub(crate) fn write_instrument_change(
    out: &mut Out,
    context: &Context<'_>,
    change: &InstrumentChange,
    part: usize,
    instrument_number: usize,
) {
    let Some(instrument) = &change.instrument else {
        return;
    };

    // Un changement peut survenir n'importe où dans la mesure : on ferme
    // toujours les attributs en cours.
    out.attr(false);
    out.xml.start_element("print", &[]);
    if !instrument.long_name.is_empty() {
        out.xml.start_element("part-name-display", &[]);
        write_display_name(out, &instrument.long_name);
        out.xml.end_element();
    }
    if !instrument.short_name.is_empty() {
        out.xml.start_element("part-abbreviation-display", &[]);
        write_display_name(out, &instrument.short_name);
        out.xml.end_element();
    }
    out.xml.end_element();

    write_transpose(out, instrument, context.concert_pitch());

    out.xml.start_element("sound", &[]);
    let id = format!("P{}-I{}", part + 1, instrument_number);
    if instrument.id.is_empty() {
        out.xml.tag_empty("instrument-change", &[("id", &id)]);
    } else {
        out.xml
            .start_element("instrument-change", &[("id", &id)]);
        out.xml.tag("instrument-sound", &instrument.id);
        out.xml.end_element();
    }
    out.xml.end_element();
}

/// Écrit `<display-text>`/`<accidental-text>` (`writeDisplayName`).
fn write_display_name(out: &mut Out, name: &str) {
    let mut display = String::new();
    for character in name.chars() {
        match character {
            '♭' => {
                if !display.is_empty() {
                    write_display_text(out, &display);
                    display.clear();
                }
                out.xml.tag("accidental-text", "flat");
            }
            '♯' => {
                if !display.is_empty() {
                    write_display_text(out, &display);
                    display.clear();
                }
                out.xml.tag("accidental-text", "sharp");
            }
            other => display.push(other),
        }
    }
    if !display.is_empty() {
        write_display_text(out, &display);
    }
}

/// Écrit un `<display-text>`, guillemets échappés comme `XmlWriter::xmlString`.
fn write_display_text(out: &mut Out, text: &str) {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    out.xml.tag_raw("display-text", &[], &escaped);
}

/// Écrit la transposition d'un instrument (`writeInstrumentDetails`).
fn write_transpose(out: &mut Out, instrument: &Instrument, concert_pitch: bool) {
    let Some(transpose) = instrument.transpose else {
        return;
    };
    if transpose.chromatic == 0 {
        return;
    }
    out.attr(true);
    if concert_pitch {
        out.xml.start_element("for-part", &[]);
        out.xml.start_element("part-transpose", &[]);
    } else {
        out.xml.start_element("transpose", &[]);
    }
    out.xml
        .tag("diatonic", &(transpose.diatonic % 7).to_string());
    out.xml
        .tag("chromatic", &(transpose.chromatic % 12).to_string());
    let octave_change = transpose.chromatic / 12;
    if octave_change != 0 {
        out.xml.tag("octave-change", &octave_change.to_string());
    }
    out.xml.end_element();
    if concert_pitch {
        out.xml.end_element();
    }
    out.attr(false);
}
