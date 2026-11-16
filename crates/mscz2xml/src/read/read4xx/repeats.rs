//! Reprises en 4.x : marqueurs, sauts et voltas.
//!
//! Portage de `TRead::read(Marker*)`, `TRead::read(Jump*)`
//! (`engraving/rw/read400/tread.cpp`) et de la lecture des `Spanner`
//! `Volta` par `ConnectorInfoReader` (`engraving/rw/read400/
//! connectorinforeader.cpp`).
//!
//! En 4.x, un marqueur ou un saut est un enfant direct de `<Measure>` ; une
//! volta est un `<Spanner type="Volta">` écrit en deux moitiés, chacune portant
//! l'extrémité implicite (sa position) et l'autre dans `<prev>`/`<next>`. Les
//! moitiés sont appariées en fin de lecture.

use super::dom::Element;
use super::{placement, Reader};
use crate::model::{
    Fraction, Jump, Marker, MarkerType, Spanner, SpannerAnchor, SpannerType, Tick, Track,
    VoltaInfo, VOICES,
};

/// Piste d'attache d'un marqueur ou d'un saut.
///
/// `MeasureRead::readMeasure` pose `ctx.track()` (première piste de la portée)
/// puis ramène à zéro les objets système du haut de système ; dans le modèle
/// la piste de la portée suffit, l'export filtrant sur la première piste.
fn measure_track(staff_index: usize) -> Track {
    Track::from_raw((staff_index * VOICES) as u32)
}

/// Lit un `<Marker>` enfant de `<Measure>`.
pub(super) fn read_marker(element: &Element, staff_index: usize) -> Marker {
    let label = element
        .child("label")
        .map(|node| node.text().to_owned())
        .unwrap_or_default();
    // `TRead::read(Marker*)` part de `MarkerType::SEGNO` ; le type n'est
    // écrasé que par la conversion du libellé, qui donne `USER` s'il est
    // inconnu.
    let kind = if label.is_empty() {
        MarkerType::Segno
    } else {
        MarkerType::from_label(&label)
    };
    Marker {
        kind,
        label,
        text: element
            .child("text")
            .map(|node| node.text().to_owned())
            .unwrap_or_default(),
        placement: placement(element.child("placement")),
        track: measure_track(staff_index),
    }
}

/// Lit un `<Jump>` enfant de `<Measure>`.
pub(super) fn read_jump(element: &Element, staff_index: usize) -> Jump {
    let text = |name: &str| {
        element
            .child(name)
            .map(|node| node.text().to_owned())
            .unwrap_or_default()
    };
    Jump::new(
        text("text"),
        text("jumpTo"),
        text("playUntil"),
        text("continueAt"),
        placement(element.child("placement")),
        measure_track(staff_index),
    )
}

/// `<location>` relative d'une extrémité de spanner.
#[derive(Clone, Copy, Default)]
struct Location {
    measures: i64,
    fractions: Fraction,
    staves: i64,
    voices: i64,
}

/// Moitié de `<Spanner type="Volta">` en attente d'appariement.
pub(super) struct VoltaHalf {
    tick: Tick,
    measure: usize,
    track: Track,
    prev: Option<Location>,
    next: Option<Location>,
    info: Option<VoltaInfo>,
    text: Option<String>,
}

impl VoltaHalf {
    /// Position absolue d'une extrémité, comme `Location::toAbsolute`.
    fn absolute(&self, location: Location, measures: &[crate::model::Measure]) -> (Tick, Track) {
        let target = (self.measure as i64 + location.measures).max(0) as usize;
        let (base, current_base) = match (measures.get(target), measures.get(self.measure)) {
            (Some(target), Some(current)) => (target.start_tick, current.start_tick),
            _ => (Fraction::ZERO, Fraction::ZERO),
        };
        let tick = base + (self.tick - current_base) + location.fractions;
        let raw_track =
            i64::from(self.track.raw()) + location.voices + location.staves * VOICES as i64;
        (tick, Track::from_raw(raw_track.max(0) as u32))
    }
}

/// Lit un `<Spanner type="Volta">` rencontré dans une voix.
pub(super) fn read_spanner(
    reader: &mut Reader,
    element: &Element,
    tick: Tick,
    track: Track,
    measure_index: usize,
) {
    let Some(kind) = element.attr("type") else {
        return;
    };
    if kind != "Volta" {
        // Les autres spanners (soufflets, liaisons d'expression…) ne sont pas
        // du périmètre de cette famille.
        return;
    }

    let mut half = VoltaHalf {
        tick,
        measure: measure_index,
        track,
        prev: None,
        next: None,
        info: None,
        text: None,
    };
    for child in &element.children {
        match child.name.as_str() {
            "prev" => half.prev = child.child("location").map(read_location),
            "next" => half.next = child.child("location").map(read_location),
            "Volta" => {
                let mut info = VoltaInfo::default();
                let mut text = None;
                for property in &child.children {
                    match property.name.as_str() {
                        "endings" => info.endings = read_endings(property.text()),
                        "endHookType" => {
                            info.closed = property.int().unwrap_or(0) != 0;
                        }
                        "beginText" => text = Some(property.text().to_owned()),
                        "text" if text.is_none() => text = Some(property.text().to_owned()),
                        _ => {}
                    }
                }
                info.endings.sort_unstable();
                half.info = Some(info);
                half.text = text;
            }
            _ => {}
        }
    }
    reader.voltas.push(half);
}

fn read_location(element: &Element) -> Location {
    let mut location = Location::default();
    for child in &element.children {
        match child.name.as_str() {
            "measures" => location.measures = child.int().unwrap_or(0),
            "fractions" => location.fractions = child.fraction().unwrap_or(Fraction::ZERO),
            "staves" => location.staves = child.int().unwrap_or(0),
            "voices" => location.voices = child.int().unwrap_or(0),
            _ => {}
        }
    }
    location
}

/// Analyse `<endings>` comme `TConv::fromXml(String, vector<int>)`.
fn read_endings(text: &str) -> Vec<u8> {
    text.split(',')
        .filter_map(|part| part.trim().parse::<u8>().ok())
        .collect()
}

/// Apparie les moitiés de voltas et les ajoute à la partition.
pub(super) fn resolve(reader: &mut Reader) {
    let halves = std::mem::take(&mut reader.voltas);
    let measures = &reader.score.measures;
    let mut spanners: Vec<Spanner> = Vec::new();

    for (index, first) in halves.iter().enumerate() {
        let Some(next) = first.next else {
            continue;
        };
        let (next_tick, next_track) = first.absolute(next, measures);
        for (other_index, second) in halves.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Some(prev) = second.prev else {
                continue;
            };
            let (prev_tick, prev_track) = second.absolute(prev, measures);
            if prev_tick != first.tick || prev_track != first.track {
                continue;
            }
            if next_tick != second.tick || next_track != second.track {
                continue;
            }
            let info = first.info.clone().or_else(|| second.info.clone());
            let text = first.text.clone().or_else(|| second.text.clone());
            let mut spanner = Spanner::new(
                SpannerType::Volta,
                SpannerAnchor::new(first.tick, first.track),
                SpannerAnchor::new(second.tick, second.track),
            );
            spanner.text = text;
            spanner.volta = info;
            spanners.push(spanner);
            break;
        }
    }

    reader.score.spanners.extend(spanners);
}
