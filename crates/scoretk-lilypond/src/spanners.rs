// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances de soufflets, ottavas et trilles reprennent le
// comportement de `musicxml2ly` de LilyPond, Copyright (C) 2016--2026 John
// Gourlay et Werner Lemberg, GPL-3.0-or-later ; elles sont réécrites en Rust.
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

//! Spanners vers LilyPond.
//!
//! Portage du rendu des objets dérivés de `Spanner`
//! (`engraving/dom/spanner.h`) : liaisons d'expression (`slur.h`), soufflets
//! (`hairpin.h`), ottavas (`ottava.h`), pédales (`pedal.h`), trilles
//! (`trill.h`), glissandos (`glissando.h`) et lignes de texte
//! (`textlinebase.h`). Les volatas sont traitées par
//! [`crate::structure`] : elles changent la structure des reprises, pas la
//! suite des notes.

use mscz2xml::model::{HairpinType, OttavaType, Spanner, SpannerType};

use crate::event::{Event, after, before};
use crate::text;

/// Événements LilyPond d'un spanner.
///
/// Les extrémités sont rendues telles quelles : c'est [`crate::voice`] qui
/// décide à quelle voix les rattacher et sur quelle note les poser.
pub(crate) fn events(spanner: &Spanner, number: usize) -> Vec<Event> {
    match &spanner.kind {
        // Les liaisons d'expression sont numérotées (`\=1(`) : c'est la seule
        // façon pour LilyPond de distinguer deux liaisons qui se chevauchent.
        SpannerType::Slur => vec![
            Event::new(
                spanner.start.tick,
                after::SLUR_START,
                format!("\\={number}("),
            ),
            Event::new(
                spanner.end.tick,
                after::SLUR_STOP,
                format!("\\={number})"),
            ),
        ],
        SpannerType::Hairpin(kind) => {
            let start = match (kind, spanner.hairpin.as_ref()) {
                (HairpinType::Crescendo, Some(hairpin)) if hairpin.line => "\\cresc",
                (HairpinType::Diminuendo, Some(hairpin)) if hairpin.line => "\\decresc",
                (HairpinType::Crescendo, _) => "\\<",
                (HairpinType::Diminuendo, _) => "\\>",
            };
            vec![
                Event::new(spanner.start.tick, after::HAIRPIN_START, start),
                Event::new(spanner.end.tick, after::HAIRPIN_STOP, "\\!"),
            ]
        }
        SpannerType::Ottava(kind) => {
            if !spanner.visible {
                return Vec::new();
            }
            let shift = match kind {
                OttavaType::OctaveUp => 1,
                OttavaType::OctaveDown => -1,
                OttavaType::FifteenthUp => 2,
                OttavaType::FifteenthDown => -2,
            };
            vec![
                Event::new(spanner.start.tick, before::OTTAVA, format!("\\ottava {shift}")),
                Event::new(spanner.end.tick, before::OTTAVA, "\\ottava 0"),
            ]
        }
        SpannerType::Pedal => vec![
            Event::new(spanner.start.tick, after::PEDAL, "\\sustainOn"),
            Event::new(spanner.end.tick, after::PEDAL, "\\sustainOff"),
        ],
        SpannerType::Trill => vec![
            Event::new(spanner.start.tick, after::TRILL_START, "\\startTrillSpan"),
            Event::new(spanner.end.tick, after::TRILL_STOP, "\\stopTrillSpan"),
        ],
        SpannerType::Glissando => vec![Event::new(
            spanner.start.tick,
            after::GLISSANDO,
            "\\glissando",
        )],
        SpannerType::TextLine => text_line_events(spanner),
        SpannerType::Volta | SpannerType::Other(_) => Vec::new(),
    }
}

/// Textes de début et de fin d'une ligne de texte.
fn text_line_events(spanner: &Spanner) -> Vec<Event> {
    let Some(info) = &spanner.text_line else {
        return Vec::new();
    };
    let mut events = Vec::new();
    if let Some(begin) = &info.begin_text {
        let plain = begin.plain().trim();
        if !plain.is_empty() {
            events.push(Event::new(
                spanner.start.tick,
                after::TEXT,
                text::attach_markup(plain, info.placement),
            ));
        }
    }
    if let Some(end) = &info.end_text {
        let plain = end.plain().trim();
        if !plain.is_empty() {
            events.push(Event::new(
                spanner.end.tick,
                after::TEXT,
                text::attach_markup(plain, info.placement),
            ));
        }
    }
    events
}

/// Vrai si le spanner s'applique à toute la portée et non à une voix.
///
/// Ottavas et pédales sont lus sur la piste de la voix 0 par convention ;
/// les rattacher à la première voix de la portée évite de les perdre quand
/// cette voix n'a pas de note à l'endroit voulu.
pub(crate) fn is_staff_level(kind: &SpannerType) -> bool {
    matches!(kind, SpannerType::Ottava(_) | SpannerType::Pedal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mscz2xml::model::{SpannerAnchor, Track};

    fn spanner(kind: SpannerType) -> Spanner {
        Spanner::new(
            kind,
            SpannerAnchor::new(Tick::ZERO, Track::from_raw(0)),
            SpannerAnchor::new(Tick::from_integer(1), Track::from_raw(0)),
        )
    }

    #[test]
    fn liaison_et_soufflet() {
        let events = events(&spanner(SpannerType::Slur), 1);
        assert_eq!(events[0].text, "\\=1(");
        assert_eq!(events[1].text, "\\=1)");
        let events = events(&spanner(SpannerType::Hairpin(HairpinType::Crescendo)), 1);
        assert_eq!(events[0].text, "\\<");
        assert_eq!(events[1].text, "\\!");
    }

    #[test]
    fn ottava() {
        let events = events(&spanner(SpannerType::Ottava(OttavaType::OctaveDown)), 1);
        assert_eq!(events[0].text, "\\ottava -1");
        assert_eq!(events[1].text, "\\ottava 0");
    }
}
