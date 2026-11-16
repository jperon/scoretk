//! Lignes de texte (`TextLine`) et textes de cadres `TBox` en 3.x.
//!
//! Portage de `TRead::read(TextLineBase*)` et de `TRead::read(TBox*)`
//! (`engraving/rw/read400/tread.cpp`, réutilisé par `Read302`). Les moitiés de
//! `<Spanner type="TextLine">` sont appariées par le mécanisme générique de
//! `spanner.rs`.

use super::xml::Node;
use crate::model::{
    HookType, LineFont, RichText, SwingParameters, SwingUnit, TboxText, TextLineInfo, TextLineStyle,
};

/// Lit les propriétés d'une `<TextLine>` (`TRead::read(TextLineBase*)`).
pub(crate) fn read_text_line(node: &Node) -> TextLineInfo {
    let mut info = TextLineInfo {
        system: node
            .attr("system")
            .is_some_and(|value| value != "0" && !value.is_empty()),
        placement: node
            .child("placement")
            .map(|child| super::tables::placement(&child.trimmed_text()))
            .unwrap_or_default(),
        ..TextLineInfo::default()
    };
    for child in node.children() {
        match child.name() {
            "lineVisible" => info.line_visible = child.int_text().unwrap_or(0) != 0,
            "lineStyle" => {
                info.line_style = TextLineStyle::from_mcsx(child.int_text().unwrap_or(0));
            }
            "beginHookType" => {
                info.begin_hook = HookType::from_mcsx(child.int_text().unwrap_or(0));
            }
            "endHookType" => {
                info.end_hook = HookType::from_mcsx(child.int_text().unwrap_or(0));
            }
            "beginHookHeight" => {
                if let Ok(value) = child.trimmed_text().parse::<f64>() {
                    info.begin_hook_height = value;
                }
            }
            "endHookHeight" => {
                if let Ok(value) = child.trimmed_text().parse::<f64>() {
                    info.end_hook_height = value;
                }
            }
            "beginText" => {
                let text = RichText::from_markup(&child.inner_markup());
                if !text.is_empty() {
                    info.begin_text = Some(text);
                }
            }
            "endText" => {
                let text = RichText::from_markup(&child.inner_markup());
                if !text.is_empty() {
                    info.end_text = Some(text);
                }
            }
            "beginFontFace" => info.begin_font.family = child.trimmed_text(),
            "endFontFace" => info.end_font.family = child.trimmed_text(),
            "beginFontSize" => {
                if let Ok(value) = child.trimmed_text().parse::<f64>() {
                    info.begin_font.size = value;
                }
            }
            "endFontSize" => {
                if let Ok(value) = child.trimmed_text().parse::<f64>() {
                    info.end_font.size = value;
                }
            }
            "beginFontStyle" => {
                apply_font_style(&mut info.begin_font, child.int_text().unwrap_or(0))
            }
            "endFontStyle" => apply_font_style(&mut info.end_font, child.int_text().unwrap_or(0)),
            _ => {}
        }
    }
    info
}

/// Applique un `FontStyle` (bits `Bold`, `Italic`, `Underline`, `Strike`).
fn apply_font_style(font: &mut LineFont, style: i32) {
    font.bold = style & 1 != 0;
    font.italic = style & 2 != 0;
    font.underline = style & 4 != 0;
    font.strike = style & 8 != 0;
}

/// Lit les paramètres de swing d'un texte (`<swing unit="…" ratio="…"/>`).
pub(crate) fn read_swing(node: &Node) -> Option<SwingParameters> {
    let swing = node.child("swing")?;
    let unit = match swing.attr("unit") {
        Some("eighth") => SwingUnit::Eighth,
        Some("16th") => SwingUnit::Sixteenth,
        _ => SwingUnit::Straight,
    };
    let ratio = swing
        .attr("ratio")
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(60);
    Some(SwingParameters { unit, ratio })
}

/// Lit le texte d'un cadre `TBox` (`TBox::text()`).
pub(crate) fn read_tbox(node: &Node, measure_index: usize, below: bool) -> Option<TboxText> {
    for text_node in node.children_named("Text") {
        let markup = text_node
            .child("text")
            .map_or(String::new(), |child| child.inner_markup());
        let text = RichText::from_markup(&markup);
        if text.is_empty() {
            continue;
        }
        let target = if below {
            measure_index.saturating_sub(1)
        } else {
            measure_index
        };
        let mut tbox = TboxText::new(text, target);
        if below {
            tbox.placement = crate::model::Placement::Below;
        }
        // Style « Frame » de MuseScore 3.x : 12 points.
        tbox.font_size = Some(12.0);
        if let Some(offset) = text_node.child("offset") {
            if offset
                .attr("y")
                .and_then(|value| value.trim().parse::<f64>().ok())
                .is_some_and(|y| y > 0.0)
            {
                tbox.placement = crate::model::Placement::Below;
            }
        }
        return Some(tbox);
    }
    None
}
