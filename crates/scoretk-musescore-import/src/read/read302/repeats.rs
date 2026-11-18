// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/tread.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Reprises en 3.x : marqueurs, sauts et propriétés de volta.
//!
//! Portage de `TRead::read(Marker*)` et `TRead::read(Jump*)`
//! (`engraving/rw/read400/tread.cpp`) et de `TRead::read(Volta*)` pour les
//! champs lus dans un `.mscx`. En 3.x comme en 4.x, `<Marker>` et `<Jump>` sont
//! des enfants directs de `<Measure>` ; la volta est l'enfant `<Volta>` d'un
//! `<Spanner type="Volta">` (voir `super::spanner`).

use super::tables;
use super::xml::Node;
use scoretk_core::model::{Jump, Marker, MarkerType, Placement, Track, VoltaInfo, VOICES};

/// Piste d'attache d'un marqueur ou d'un saut.
///
/// `MeasureRead::readMeasure` pose `ctx.track()`, c'est-à-dire la première
/// piste de la portée (`staffIdx * VOICES`), avant de lire les voix.
fn measure_track(staff: usize) -> Track {
    Track::from_raw((staff * VOICES) as u32)
}

/// Lit un `<Marker>` enfant de `<Measure>`.
pub(crate) fn read_marker(node: &Node, staff: usize) -> Marker {
    let label = node
        .child("label")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    // `TRead::read(Marker*)` part de `MarkerType::SEGNO`.
    let kind = if label.is_empty() {
        MarkerType::Segno
    } else {
        MarkerType::from_label(&label)
    };
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()))
        .unwrap_or(Placement::Above);
    Marker {
        kind,
        label,
        text: node
            .child("text")
            .map(Node::trimmed_text)
            .unwrap_or_default(),
        placement,
        track: measure_track(staff),
    }
}

/// Lit un `<Jump>` enfant de `<Measure>`.
pub(crate) fn read_jump(node: &Node, staff: usize) -> Jump {
    let text = |name: &str| node.child(name).map(Node::trimmed_text).unwrap_or_default();
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()))
        .unwrap_or(Placement::Above);
    Jump::new(
        text("text"),
        text("jumpTo"),
        text("playUntil"),
        text("continueAt"),
        placement,
        measure_track(staff),
    )
}

/// Lit les propriétés d'une `<Volta>` contenue dans un `<Spanner>`.
pub(crate) fn read_volta(node: &Node) -> VoltaInfo {
    let mut info = VoltaInfo::default();
    for child in node.children() {
        match child.name() {
            "endings" => info.endings = read_endings(&child.trimmed_text()),
            // `HookType` est écrit sous forme numérique ; `NONE` (0) laisse la
            // volta ouverte.
            "endHookType" => info.closed = child.int_text().unwrap_or(0) != 0,
            _ => {}
        }
    }
    info.endings.sort_unstable();
    info
}

/// Analyse `<endings>` comme `TConv::fromXml(String, vector<int>)`.
fn read_endings(text: &str) -> Vec<u8> {
    text.split(',')
        .filter_map(|part| part.trim().parse::<u8>().ok())
        .collect()
}
