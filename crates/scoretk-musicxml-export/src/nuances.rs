// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Export MusicXML des nuances et des soufflets.
//!
//! Portage de `ExportMusicXml::dynamic` et `ExportMusicXml::hairpin`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`), y compris la
//! gestion des numéros de soufflets (`m_hairpins`/`m_dashes`) et l'écriture des
//! textes de soufflet (`writeHairpinText`).
//!
//! Les attributs de mise en page (`default-x`/`default-y`, polices, `spread`)
//! ne sont pas modélisés et donc pas écrits.

use std::collections::BTreeSet;

use scoretk_core::model::{
    Dynamic, Hairpin, HairpinType, LineFont, Placement, Score, Spanner, SpannerId, StaffId, Tick,
};

use super::exportmusicxml::Out;

/// Nombre maximal de soufflets simultanés (`MAX_NUMBER_LEVEL`).
const MAX_NUMBER_LEVEL: usize = 32;

/// État des numéros de soufflets et des soufflets déjà arrêtés.
#[derive(Clone, Debug, Default)]
pub(super) struct Nuances {
    /// Soufflets dessinés actifs, par numéro (`m_hairpins`).
    hairpins: [Option<usize>; MAX_NUMBER_LEVEL],
    /// Soufflets en texte actifs, par numéro (`m_dashes`).
    dashes: [Option<usize>; MAX_NUMBER_LEVEL],
    /// Soufflets déjà arrêtés dans la partie courante.
    stopped: BTreeSet<usize>,
    /// Soufflets dont le début a déjà été écrit.
    started: BTreeSet<usize>,
}

impl Nuances {
    /// Réinitialise l'ensemble des soufflets arrêtés au début d'une partie.
    pub(super) fn reset_part(&mut self) {
        self.stopped.clear();
        self.started.clear();
    }

    fn table_mut(&mut self, line: bool) -> &mut [Option<usize>; MAX_NUMBER_LEVEL] {
        if line {
            &mut self.dashes
        } else {
            &mut self.hairpins
        }
    }

    /// Numéro du soufflet dans sa table, en réservant un emplacement au besoin.
    ///
    /// Reproduit `findHairpin`/`findDashes` puis la recherche de `nullptr` :
    /// le numéro écrit est l'indice + 1. `None` si les 32 emplacements sont
    /// occupés (MuseScore abandonne alors l'export du soufflet).
    fn number(&mut self, id: SpannerId, line: bool) -> Option<usize> {
        let index = id.index();
        let table = self.table_mut(line);
        if let Some(position) = table.iter().position(|entry| *entry == Some(index)) {
            return Some(position);
        }
        let position = table.iter().position(Option::is_none)?;
        table[position] = Some(index);
        Some(position)
    }

    /// Libère le numéro d'un soufflet à son arrêt.
    fn release(&mut self, id: SpannerId, line: bool, number: usize) {
        let table = self.table_mut(line);
        if table.get(number) == Some(&Some(id.index())) {
            table[number] = None;
        }
    }

    /// Écrit le début d'un soufflet précis, dans l'ordre des spanners.
    pub(super) fn start_one(
        &mut self,
        score: &Score,
        out: &mut Out,
        index: usize,
        xml_staff: usize,
    ) {
        let Some(spanner) = score.spanners.get(index) else {
            return;
        };
        let Some(kind) = hairpin_kind(spanner) else {
            return;
        };
        // `ExportMusicXml::canWrite` : un soufflet invisible n'est pas exporté.
        if !spanner.visible {
            return;
        }
        let data = spanner.hairpin.clone().unwrap_or_default();
        let line = data.line;
        let Some(number) = self.number(SpannerId::from_index(index), line) else {
            return;
        };
        self.started.insert(index);
        write_hairpin(
            out,
            kind,
            &data,
            true,
            number,
            xml_staff,
            score.style.hairpin_height_sp,
        );
        // L'arrêt a déjà été écrit (voix traitée avant celle du début) :
        // le numéro n'a plus à rester réservé.
        if self.stopped.contains(&index) {
            self.release(SpannerId::from_index(index), line, number);
        }
    }

    /// Écrit les soufflets qui se terminent à ce tick.
    pub(super) fn stop(
        &mut self,
        score: &Score,
        out: &mut Out,
        tick: Tick,
        staff_id: StaffId,
        xml_staff: usize,
    ) {
        for (index, spanner) in score.spanners.iter().enumerate() {
            let Some(kind) = hairpin_kind(spanner) else {
                continue;
            };
            if !spanner.visible {
                continue;
            }
            if spanner.end.tick != tick
                || spanner.start.track.staff() != staff_id
                || !self.stopped.insert(index)
            {
                continue;
            }
            let data = spanner.hairpin.clone().unwrap_or_default();
            let line = data.line;
            let Some(number) = self.number(SpannerId::from_index(index), line) else {
                continue;
            };
            write_hairpin(
                out,
                kind,
                &data,
                false,
                number,
                xml_staff,
                score.style.hairpin_height_sp,
            );
            if self.started.remove(&index) {
                self.release(SpannerId::from_index(index), line, number);
            }
        }
    }
}

/// Sens du soufflet, ou `None` pour un autre type de spanner.
fn hairpin_kind(spanner: &Spanner) -> Option<HairpinType> {
    match spanner.kind {
        scoretk_core::model::SpannerType::Hairpin(kind) => Some(kind),
        _ => None,
    }
}

/// Écrit un `<direction>` de soufflet (`ExportMusicXml::hairpin`).
fn write_hairpin(
    out: &mut Out,
    kind: HairpinType,
    data: &Hairpin,
    is_start: bool,
    number: usize,
    xml_staff: usize,
    hairpin_height: f64,
) {
    // MuseScore n'écrit rien pour un soufflet de ligne invisible sans texte.
    if data.line && data.begin_text.is_empty() && data.end_text.is_empty() {
        return;
    }

    out.attr(false);
    out.xml.start_element(
        "direction",
        &[("placement", placement_name(data.placement))],
    );

    if is_start {
        write_hairpin_text(out, &data.begin_text, &data.begin_font);
    }

    if data.line {
        let number = (number + 1).to_string();
        out.xml.start_element("direction-type", &[]);
        if is_start {
            out.xml
                .tag_empty("dashes", &[("type", "start"), ("number", &number)]);
        } else {
            out.xml
                .tag_empty("dashes", &[("type", "stop"), ("number", &number)]);
        }
        out.xml.end_element();
    } else {
        out.xml.start_element("direction-type", &[]);
        let mut attributes: Vec<(&str, &str)> = vec![(
            "type",
            if is_start {
                match kind {
                    HairpinType::Crescendo => "crescendo",
                    HairpinType::Diminuendo => "diminuendo",
                }
            } else {
                "stop"
            },
        )];
        let niente = data.niente
            && (is_start && matches!(kind, HairpinType::Crescendo)
                || !is_start && matches!(kind, HairpinType::Diminuendo));
        if niente {
            attributes.push(("niente", "yes"));
        }
        // `spread` : `hairpinHeight * 10`, sur un crescendo au départ et un
        // diminuendo à l'arrivée.
        let spread_needed = (is_start && matches!(kind, HairpinType::Crescendo))
            || (!is_start && matches!(kind, HairpinType::Diminuendo));
        let spread = super::exportmusicxml::format_number(hairpin_height * 10.0, 2);
        if spread_needed {
            attributes.push(("spread", &spread));
        }
        let number = (number + 1).to_string();
        attributes.push(("number", &number));
        out.xml.tag_empty("wedge", &attributes);
        out.xml.end_element();
    }

    if !is_start {
        write_hairpin_text(out, &data.end_text, &data.end_font);
    }

    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    out.xml.end_element();
}

/// Écrit les textes de début ou de fin d'un soufflet (`writeHairpinText`).
///
/// Le texte peut mêler du texte libre et des `<sym>` MuseScore : chaque
/// séquence correspondant à une nuance connue devient un `<dynamics>`, le
/// reste un `<words>`.
fn write_hairpin_text(out: &mut Out, text: &str, font: &LineFont) {
    let mut rest = text;
    while !rest.is_empty() {
        let found = find_dynamic_in_string(rest);
        match found {
            Some((0, length, name)) => {
                out.xml.start_element("direction-type", &[]);
                out.xml.start_element("dynamics", &[]);
                out.xml.tag_empty(name, &[]);
                out.xml.end_element();
                out.xml.end_element();
                rest = &rest[length..];
            }
            Some((position, _, _)) => {
                let (before, after) = rest.split_at(position);
                out.xml.start_element("direction-type", &[]);
                write_words(out, before, font);
                out.xml.end_element();
                rest = after;
            }
            None => {
                out.xml.start_element("direction-type", &[]);
                write_words(out, rest, font);
                out.xml.end_element();
                rest = "";
            }
        }
    }
}

/// Écrit `<words>` avec la police du soufflet (`fontStyleToXML`).
fn write_words(out: &mut Out, text: &str, font: &LineFont) {
    let family = font.family.clone();
    let size = crate::text::format_font_size(font.size);
    let weight = if font.bold { Some("bold") } else { None };
    let style = if font.italic { Some("italic") } else { None };
    let mut attributes: Vec<(&str, &str)> = vec![("font-family", &family), ("font-size", &size)];
    if let Some(weight) = weight {
        attributes.push(("font-weight", weight));
    }
    if let Some(style) = style {
        attributes.push(("font-style", style));
    }
    out.xml.tag_with("words", &attributes, text);
}

/// Plus long texte de nuance reconnu au plus tôt dans `text`.
///
/// Reproduit `findDynamicInString` : à position égale, la correspondance la
/// plus longue (`pp` plutôt que `p`) l'emporte.
fn find_dynamic_in_string(text: &str) -> Option<(usize, usize, &'static str)> {
    let mut found: Option<(usize, usize, &'static str)> = None;
    for (name, markup) in DYNAMIC_MARKUP {
        let Some(position) = text.find(markup) else {
            continue;
        };
        let length = markup.len();
        let better = match found {
            None => true,
            Some((best_position, best_length, _)) => {
                position < best_position || (position == best_position && length > best_length)
            }
        };
        if better {
            found = Some((position, length, name));
        }
    }
    found
}

/// Texte MuseScore de chaque nuance, dans l'ordre de `Dynamic::DYN_LIST`.
///
/// Sert uniquement à retrouver les nuances insérées dans un texte de soufflet.
const DYNAMIC_MARKUP: &[(&str, &str)] = &[
    ("pppppp", "<sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("ppppp", "<sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("pppp", "<sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("ppp", "<sym>dynamicPiano</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("pp", "<sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("p", "<sym>dynamicPiano</sym>"),
    ("mp", "<sym>dynamicMezzo</sym><sym>dynamicPiano</sym>"),
    ("mf", "<sym>dynamicMezzo</sym><sym>dynamicForte</sym>"),
    ("f", "<sym>dynamicForte</sym>"),
    ("ff", "<sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("fff", "<sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("ffff", "<sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("fffff", "<sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("ffffff", "<sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("fp", "<sym>dynamicForte</sym><sym>dynamicPiano</sym>"),
    ("pf", "<sym>dynamicPiano</sym><sym>dynamicForte</sym>"),
    ("sf", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym>"),
    ("sfz", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicZ</sym>"),
    ("sff", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("sffz", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicZ</sym>"),
    ("sfff", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"),
    ("sfffz", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicZ</sym>"),
    ("sfp", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicPiano</sym>"),
    ("sfpp", "<sym>dynamicSforzando</sym><sym>dynamicForte</sym><sym>dynamicPiano</sym><sym>dynamicPiano</sym>"),
    ("rfz", "<sym>dynamicRinforzando</sym><sym>dynamicForte</sym><sym>dynamicZ</sym>"),
    ("rf", "<sym>dynamicRinforzando</sym><sym>dynamicForte</sym>"),
    ("fz", "<sym>dynamicForte</sym><sym>dynamicZ</sym>"),
    ("m", "<sym>dynamicMezzo</sym>"),
    ("r", "<sym>dynamicRinforzando</sym>"),
    ("s", "<sym>dynamicSforzando</sym>"),
    ("z", "<sym>dynamicZ</sym>"),
    ("n", "<sym>dynamicNiente</sym>"),
];

/// Écrit une nuance (`ExportMusicXml::dynamic`).
pub(super) fn write_dynamic(out: &mut Out, dynamic: &Dynamic, xml_staff: usize) {
    if !dynamic.visible {
        return;
    }
    out.attr(false);
    out.xml.start_element(
        "direction",
        &[("placement", placement_name(dynamic.placement))],
    );
    out.xml.start_element("direction-type", &[]);
    out.xml.start_element("dynamics", &[]);
    if !dynamic.custom && is_musicxml_dynamic(&dynamic.text) {
        out.xml.tag_empty(&dynamic.text, &[]);
    } else if !dynamic.text.is_empty() {
        out.xml.tag("other-dynamics", &dynamic.text);
    }
    out.xml.end_element();
    out.xml.end_element();

    if xml_staff > 0 {
        out.xml.tag("staff", &xml_staff.to_string());
    }
    if let Some(velocity) = dynamic.velocity {
        if velocity > 0 {
            let value = f64::from(velocity) * 100.0 / 90.0;
            out.xml
                .tag_empty("sound", &[("dynamics", &format_fixed(value, 2))]);
        }
    }
    out.xml.end_element();
}

/// Nuances reconnues par MusicXML 4.0 **dans MuseScore 4.7.4**.
///
/// Cette version n'écrit ni `<n/>`, ni `<pf/>`, ni `<sfzp/>` : ces textes
/// passent par `<other-dynamics>`.
fn is_musicxml_dynamic(text: &str) -> bool {
    matches!(
        text,
        "f" | "ff"
            | "fff"
            | "ffff"
            | "fffff"
            | "ffffff"
            | "fp"
            | "fz"
            | "mf"
            | "mp"
            | "p"
            | "pp"
            | "ppp"
            | "pppp"
            | "ppppp"
            | "pppppp"
            | "rf"
            | "rfz"
            | "sf"
            | "sffz"
            | "sfp"
            | "sfpp"
            | "sfz"
    )
}

fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

/// Nombre à deux décimales, comme `String::number(valeur, 2)`.
fn format_fixed(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}
