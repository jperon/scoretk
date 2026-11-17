// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlnoteduration.cpp`
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
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

//! Durées d'une note MusicXML (`MusicXmlNoteDuration`).
//!
//! Deux durées coexistent, comme dans le C++ : la durée *spécifiée*
//! (`<duration>`, en divisions) et la durée *calculée* à partir de `<type>`,
//! des `<dot>` et du `<time-modification>`. La durée calculée l'emporte quand
//! elle est valide ; c'est elle que MuseScore stocke dans `TDuration`.

use mscz2xml::model::{Duration, DurationType, Fraction};

use crate::xml::Element;

/// Valeurs lues dans une note et durée retenue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteTiming {
    /// Durée retenue, en fractions de ronde (`MusicXmlNoteDuration::duration`).
    pub dura: Fraction,
    /// Durée spécifiée par `<duration>`, avant application du n-olet.
    pub spec: Option<Fraction>,
    /// Durée calculée à partir de `<type>`, des points et du n-olet.
    pub calc: Option<Fraction>,
    /// Nombre de `<dot>`.
    pub dots: u8,
    /// Ratio `normal / actual` du `<time-modification>`.
    pub time_mod: Option<Fraction>,
    /// `<normal-type>` du `<time-modification>`, si présent.
    pub normal_type: Option<Fraction>,
}

/// Fraction d'une figure MusicXML (`noteTypeToFraction`).
pub fn type_fraction(name: &str) -> Option<Fraction> {
    let denominator = match name {
        "1024th" => 1024,
        "512th" => 512,
        "256th" => 256,
        "128th" => 128,
        "64th" => 64,
        "32nd" => 32,
        "16th" => 16,
        "eighth" => 8,
        "quarter" => 4,
        "half" => 2,
        "whole" => 1,
        "breve" => return Some(Fraction::from_integer(2)),
        "long" => return Some(Fraction::from_integer(4)),
        "maxima" => return Some(Fraction::from_integer(8)),
        _ => return None,
    };
    Some(Fraction::new(1, denominator))
}

/// Figure MuseScore correspondant à un nom MusicXML.
pub fn duration_type(name: &str) -> DurationType {
    match name {
        "1024th" => DurationType::OneThousandTwentyFourth,
        "512th" => DurationType::FiveHundredTwelfth,
        "256th" => DurationType::TwoHundredFiftySixth,
        "128th" => DurationType::OneHundredTwentyEighth,
        "64th" => DurationType::SixtyFourth,
        "32nd" => DurationType::ThirtySecond,
        "16th" => DurationType::Sixteenth,
        "eighth" => DurationType::Eighth,
        "quarter" => DurationType::Quarter,
        "half" => DurationType::Half,
        "whole" => DurationType::Whole,
        "breve" => DurationType::Breve,
        "long" => DurationType::Long,
        _ => DurationType::Invalid,
    }
}

/// Lit le timing d'une note (`MusicXmlNoteDuration::checkTiming`).
pub fn read(note: &Element, divisions: u32) -> NoteTiming {
    let mut dots: u8 = 0;
    let mut spec = None;
    let mut time_mod = None;
    let mut normal_type = None;

    for child in &note.children {
        match child.name.as_str() {
            "dot" => dots = dots.saturating_add(1),
            "duration" => {
                if divisions > 0 {
                    if let Ok(ticks) = child.text_trimmed().parse::<i64>() {
                        spec = Some(Fraction::from_ticks(ticks, divisions * 4));
                    }
                }
            }
            "time-modification" => {
                let actual = child.child_int("actual-notes").unwrap_or(0);
                let normal = child.child_int("normal-notes").unwrap_or(0);
                if actual > 0 && normal > 0 {
                    time_mod = Some(Fraction::new(normal, actual));
                } else {
                    time_mod = Some(Fraction::ONE);
                }
                normal_type = child
                    .child_text("normal-type")
                    .filter(|name| *name != "measure")
                    .and_then(type_fraction);
            }
            _ => {}
        }
    }

    let mut calc = None;
    if let Some(figure) = note.child_text("type").and_then(type_fraction) {
        let mut total = figure;
        let mut increment = figure * Fraction::HALF;
        for _ in 0..dots {
            total += increment;
            increment *= Fraction::HALF;
        }
        if let Some(modification) = time_mod {
            total *= modification;
        }
        calc = Some(total);
    }

    let rest = note.has_child("rest");
    let grace = note.has_child("grace");
    let chord = note.has_child("chord");
    let whole_rest =
        rest && note.child_text("type") == Some("whole") && spec.is_some() && calc.is_some();
    let grace_zero = grace && spec == Some(Fraction::ZERO);

    let dura = match (calc, spec) {
        (Some(calc), Some(spec)) if calc != spec && !whole_rest && !grace_zero => {
            // Durée calculée retenue, comme le C++ (« using calculated
            // duration ») : le `<type>` fait foi pour la figure.
            calc
        }
        (Some(calc), _) => calc,
        (None, Some(spec)) => spec,
        (None, None) => Fraction::ONE,
    };

    // Un accord ou un ornement ne fait pas avancer le temps.
    let dura = if chord || grace { Fraction::ZERO } else { dura };

    NoteTiming {
        dura,
        spec,
        calc,
        dots,
        time_mod,
        normal_type,
    }
}

/// Figure et points d'une `<type>` avec ses `<dot>`, si elle est reconnue.
pub fn figure(note: &Element) -> Option<Duration> {
    let kind = note.child_text("type").map(duration_type)?;
    if kind == DurationType::Invalid {
        return None;
    }
    let dots = note
        .children
        .iter()
        .filter(|child| child.name == "dot")
        .count()
        .try_into()
        .unwrap_or(0);
    Some(Duration::with_dots(kind, dots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse;

    #[test]
    fn duree_calculee() {
        let root = parse(br#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>eighth</type></note>"#).unwrap();
        let timing = read(&root, 1);
        assert_eq!(timing.dura, Fraction::new(1, 8));
        assert_eq!(timing.spec, Some(Fraction::new(2, 4)));
    }

    #[test]
    fn point_et_nolet() {
        let root = parse(
            br#"<note><duration>1</duration><type>quarter</type><dot/><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>"#,
        )
        .unwrap();
        let timing = read(&root, 1);
        assert_eq!(timing.calc, Some(Fraction::new(1, 4)));
        assert_eq!(timing.time_mod, Some(Fraction::new(2, 3)));
    }
}
