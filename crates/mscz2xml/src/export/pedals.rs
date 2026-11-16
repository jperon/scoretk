//! Pédales à l'export (`ExportMusicXml::pedal`).
//!
//! MuseScore écrit une direction `<pedal>` avec le trait (`line`), le signe
//! (`sign`) et le type (`start`, `stop`, `change`, `resume`, `discontinue`,
//! `sostenuto`). Les positions relèvent de la mise en page et sont omises.

use crate::model::{HookType, PedalInfo, Spanner};

use super::exportmusicxml::{placement_name, Out};

/// Écrit le début d'une pédale (`pedal`, `isStart`).
pub(crate) fn start(out: &mut Out, spanner: &Spanner, xml_staff: usize) {
    if !spanner.visible {
        return;
    }
    let Some(info) = &spanner.pedal else {
        return;
    };
    write_pedal(out, info, true, xml_staff);
}

/// Écrit la fin d'une pédale.
pub(crate) fn stop(out: &mut Out, spanner: &Spanner, xml_staff: usize) {
    if !spanner.visible {
        return;
    }
    let Some(info) = &spanner.pedal else {
        return;
    };
    write_pedal(out, info, false, xml_staff);
}

fn write_pedal(out: &mut Out, info: &PedalInfo, is_start: bool, xml_staff: usize) {
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[("placement", placement_name(info.placement))],
    );
    out.xml.start_element("direction-type", &[]);

    // `Pedal::propertyDefault(BEGIN_TEXT)` vaut
    // `<sym>keyboardPedalPed</sym>` : un début sans texte explicite reste donc
    // un `start` avec `sign="yes"`.
    let kind = if is_start {
        "start"
    } else {
        match info.end_hook {
            HookType::None if info.line_visible && info.end_text.is_empty() => "discontinue",
            _ => "stop",
        }
    };
    let line = if info.line_visible { "yes" } else { "no" };
    let sign = if is_start || !info.end_text.is_empty() {
        "yes"
    } else {
        "no"
    };
    out.xml
        .tag_empty("pedal", &[("type", kind), ("line", line), ("sign", sign)]);
    out.xml.end_element();
    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}
