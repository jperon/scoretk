// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances de nuances, tempos et textes s'inspirent de
// `musicxml2ly` de LilyPond, Copyright (C) 2016--2026 John Gourlay et Werner
// Lemberg, GPL-3.0-or-later ; elles sont réécrites en Rust.
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

//! Annotations de segment vers LilyPond.
//!
//! Portage du rendu des objets de `Segment::annotations()`
//! (`engraving/dom/segment.h`) : nuances (`engraving/dom/dynamic.h`), textes
//! (`stafftext.h`, `systemtext.h`), tempos (`tempotext.h`), repères
//! (`rehearsalmark.h`), accords chiffrés (`harmony.h`) et points d'orgue
//! (`fermata.h`).

use mscz2xml::model::{Annotation, FermataType, Placement, Tick};

use crate::event::{after, before, Event};
use crate::text;

/// Nuances prédéfinies de LilyPond (`dynamics`), reprises de `musicxml2ly`.
const PREDEFINED_DYNAMICS: [&str; 22] = [
    "ppppp", "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "fffff", "fp", "sf",
    "sfp", "sff", "sfz", "fz", "sp", "spp", "rfz", "n",
];

/// Événements LilyPond d'une annotation de segment.
///
/// Une annotation sans équivalent (chiffrage figuré, barre de vibrato,
/// annotation inconnue) ne produit aucun événement : la note reste écrite,
/// seule la décoration disparaît.
pub(crate) fn events(annotation: &Annotation, tick: Tick) -> Vec<Event> {
    if !annotation.visible() {
        return Vec::new();
    }
    match annotation {
        Annotation::Dynamic(dynamic) => {
            let name = dynamic.text.trim();
            if name.is_empty() {
                return Vec::new();
            }
            let direction = text::direction(dynamic.placement);
            let rendered = if PREDEFINED_DYNAMICS.contains(&name) {
                format!("{direction}\\{name}")
            } else {
                format!("{direction}{}", text::markup(&text::escape(name)))
            };
            vec![Event::new(tick, after::DYNAMIC, rendered)]
        }
        Annotation::Tempo(tempo) => {
            // Un texte réduit à un symbole musical (`♩ = 80`) ne laisse rien
            // après nettoyage : seule la valeur métronomique est écrite.
            let plain = text::rich_text(&tempo.text);
            let label = if plain.chars().any(char::is_alphabetic) {
                format!("\"{}\"", plain.trim())
            } else {
                String::new()
            };
            let metronome = tempo
                .beats_per_second
                .filter(|beats| *beats > 0.0)
                .map(|beats| format!("4 = {}", (beats * 60.0).round() as i64))
                .unwrap_or_default();
            let command = match (label.is_empty(), metronome.is_empty()) {
                (true, true) => return Vec::new(),
                (true, false) => format!("\\tempo {metronome}"),
                (false, true) => format!("\\tempo {label}"),
                (false, false) => format!("\\tempo {label} {metronome}"),
            };
            vec![Event::new(tick, before::TEMPO, command)]
        }
        Annotation::StaffText(staff_text) => {
            vec![text_event(
                staff_text.text.plain(),
                staff_text.placement,
                tick,
            )]
        }
        Annotation::Expression(staff_text) => {
            vec![text_event(
                staff_text.text.plain(),
                staff_text.placement,
                tick,
            )]
        }
        Annotation::SystemText(system_text) => vec![text_event(
            system_text.text.plain(),
            system_text.placement,
            tick,
        )],
        Annotation::InstrumentChange(change) => {
            vec![text_event(change.text.plain(), change.placement, tick)]
        }
        Annotation::RehearsalMark(mark) => {
            let plain = mark.text.plain().trim();
            if plain.is_empty() {
                return Vec::new();
            }
            vec![Event::new(
                tick,
                before::MARK,
                format!("\\mark {}", text::markup(&text::escape(plain))),
            )]
        }
        Annotation::Harmony(harmony) => {
            let plain = if harmony.text.trim().is_empty() {
                harmony.name.trim()
            } else {
                harmony.text.trim()
            };
            if plain.is_empty() {
                return Vec::new();
            }
            vec![text_event(plain, harmony.placement, tick)]
        }
        Annotation::Fermata(fermata) => match fermata_command(fermata.kind) {
            Some(command) => vec![Event::new(tick, after::FERMATA, command)],
            None => Vec::new(),
        },
        Annotation::Other { text: plain, .. } => {
            let plain = plain.trim();
            if plain.is_empty() {
                Vec::new()
            } else {
                vec![text_event(plain, Placement::Above, tick)]
            }
        }
        Annotation::FiguredBass(_) | Annotation::TremoloBar(_) => Vec::new(),
    }
}

/// Texte attaché à la note courante, du côté demandé.
fn text_event(plain: &str, placement: Placement, tick: Tick) -> Event {
    Event::new(tick, after::TEXT, text::attach_markup(plain, placement))
}

/// Commande LilyPond d'une forme de point d'orgue.
fn fermata_command(kind: FermataType) -> Option<&'static str> {
    let command = match kind {
        FermataType::Normal => "\\fermata",
        FermataType::Short | FermataType::VeryShort => "\\shortfermata",
        FermataType::Long => "\\longfermata",
        FermataType::VeryLong => "\\verylongfermata",
        FermataType::LongHenze => "\\henzelongfermata",
        FermataType::ShortHenze => "\\henzeshortfermata",
        FermataType::Curlew => return None,
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mscz2xml::model::{Dynamic, RehearsalMark, Tempo};

    #[test]
    fn nuance_predefinie() {
        let events = events(&Annotation::Dynamic(Dynamic::new("mf")), Tick::ZERO);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "_\\mf");
    }

    #[test]
    fn nuance_personnalisee() {
        let events = events(&Annotation::Dynamic(Dynamic::new("dolce")), Tick::ZERO);
        assert_eq!(events[0].text, "_\\markup { \"dolce\" }");
    }

    #[test]
    fn tempo_avec_metronome() {
        let mut tempo = Tempo::new("Allegro");
        tempo.beats_per_second = Some(2.0);
        let events = events(&Annotation::Tempo(tempo), Tick::ZERO);
        assert_eq!(events[0].text, "\\tempo \"Allegro\" 4 = 120");
    }

    #[test]
    fn repere_de_repetition() {
        let events = events(
            &Annotation::RehearsalMark(RehearsalMark::new("A")),
            Tick::ZERO,
        );
        assert_eq!(events[0].text, "\\mark \\markup { \"A\" }");
    }
}
