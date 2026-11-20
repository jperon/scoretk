// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/connectorinfowriter.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026 : écriture des moitiés de spanners (`<Spanner type="…">`).
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

//! Écriture des spanners.
//!
//! Un spanner s'écrit en deux moitiés, chacune à sa position dans une voix :
//! la moitié de départ porte la charge (`<HairPin>`, `<Ottava>`, `<Pedal>`…) et
//! `<next>`, la moitié d'arrivée seulement `<prev>`. Les deux portent la
//! `<location>` relative de l'autre extrémité, ce qui permet au lecteur de les
//! apparier (`ConnectorInfoReader::connect`).

use scoretk_core::model::{
    Glissando, GlissandoType, LineStyle, SpannerNotation, SpannerType, Trill, TrillType,
};

use crate::write::context::Half;
use crate::write::twrite::{
    spanner_type_name, write_hairpin, write_location_element, write_notation, write_pedal,
    write_text_line, write_volta,
};
use crate::xmlwriter::XmlWriter;

/// Écrit une moitié de spanner.
pub(crate) fn write_spanner_half(
    xml: &mut XmlWriter,
    half: &Half<'_>,
    score: &scoretk_core::model::Score,
) {
    let spanner = half.spanner;
    let Some(type_name) = spanner_type_name(spanner) else {
        return;
    };
    xml.start_element("Spanner", &[("type", type_name)]);
    if half.start {
        write_payload(xml, spanner);
    }
    let (tick, track) = if half.start {
        (spanner.start.tick, spanner.start.track)
    } else {
        (spanner.end.tick, spanner.end.track)
    };
    write_location_element(
        xml,
        score,
        half.other_tick,
        tick,
        half.other_track,
        track,
        (half.start, None),
    );
    xml.end_element();
}

/// Écrit la charge propre au type de spanner.
fn write_payload(xml: &mut XmlWriter, spanner: &scoretk_core::model::Spanner) {
    match &spanner.kind {
        SpannerType::Slur => {
            let notation = spanner
                .notation
                .clone()
                .unwrap_or(SpannerNotation::Slur(Default::default()));
            write_notation(xml, &notation);
        }
        SpannerType::Hairpin(kind) => {
            let hairpin = spanner.hairpin.clone().unwrap_or_default();
            write_hairpin(xml, *kind, &hairpin, spanner.visible);
        }
        SpannerType::Ottava(kind) => {
            xml.start_element("Ottava", &[]);
            xml.tag("subtype", crate::names::ottava_subtype(*kind));
            if !spanner.visible {
                xml.tag("visible", "0");
            }
            xml.end_element();
        }
        SpannerType::Pedal => {
            // `PedalInfo::default` n'a pas le trait visible : `PedalInfo::new`
            // est la pédale dessinée par défaut de MuseScore.
            let pedal = match &spanner.pedal {
                Some(pedal) => pedal.clone(),
                None => scoretk_core::model::PedalInfo::new(),
            };
            write_pedal(xml, &pedal, spanner.visible);
        }
        SpannerType::Volta => {
            let volta = spanner.volta.clone().unwrap_or_default();
            write_volta(xml, &volta, spanner.text.as_deref());
        }
        SpannerType::TextLine => {
            let info = spanner.text_line.clone().unwrap_or_default();
            write_text_line(xml, &info);
        }
        SpannerType::Trill => {
            let notation = spanner
                .notation
                .clone()
                .unwrap_or_else(|| SpannerNotation::Trill(Trill::new(TrillType::TrillLine)));
            write_notation(xml, &notation);
        }
        SpannerType::Glissando => match &spanner.notation {
            Some(notation) => write_notation(xml, notation),
            None => write_notation(
                xml,
                &SpannerNotation::Glissando(Glissando::new(GlissandoType::Straight(
                    LineStyle::Solid,
                ))),
            ),
        },
        SpannerType::Other(_) => {}
    }
}
