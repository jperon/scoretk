// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Événements datés d'une voix.
//!
//! Une voix LilyPond est une suite linéaire de notes ; les objets du modèle
//! qui ne sont pas des accords — nuances, soufflets, liaisons, changements de
//! clé — doivent donc être rattachés à un accord. Un [`Event`] porte le tick
//! auquel il s'applique et un rang qui fixe son ordre parmi les autres
//! événements du même tick.

use scoretk_core::model::Tick;

/// Événement daté à insérer dans une voix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Event {
    /// Tick absolu de l'événement.
    pub tick: Tick,
    /// Rang d'insertion : les événements d'un même tick sortent par ordre
    /// croissant.
    pub order: u8,
    /// Texte LilyPond.
    pub text: String,
}

impl Event {
    /// Construit un événement.
    pub(crate) fn new(tick: Tick, order: u8, text: impl Into<String>) -> Self {
        Self {
            tick,
            order,
            text: text.into(),
        }
    }
}

/// Ordres d'insertion avant la note.
///
/// Reproduit l'ordre de `musicxml2ly` : les changements de contexte passent
/// avant la note à laquelle ils s'appliquent.
pub(crate) mod before {
    /// `\section` (saut de section).
    pub(crate) const SECTION: u8 = 0;
    /// `\transposition` (avant `\key` : c'est une propriété de contexte).
    pub(crate) const TRANSPOSE: u8 = 25;
    /// `\clef`.
    pub(crate) const CLEF: u8 = 20;
    /// `\key`.
    pub(crate) const KEY: u8 = 30;
    /// `\time`.
    pub(crate) const TIME: u8 = 40;
    /// `\set Timing.measureLength`, après `\time` et avant `\partial`.
    pub(crate) const MEASURE_LENGTH: u8 = 42;
    /// `\partial` (levée), après `\time`.
    pub(crate) const PARTIAL: u8 = 45;
    /// `\ottava`.
    pub(crate) const OTTAVA: u8 = 50;
    /// `\tempo`.
    pub(crate) const TEMPO: u8 = 70;
    /// `\set Staff.midiMinimumVolume` (§24.4.2), avant la nuance.
    pub(crate) const MIDI_MIN_VOLUME: u8 = 4;
    /// `\set Staff.midiMaximumVolume` (§24.4.2), avant la nuance.
    pub(crate) const MIDI_MAX_VOLUME: u8 = 5;
    /// `\mark` (repères de répétition).
    pub(crate) const MARK: u8 = 80;
}

/// Ordres d'insertion après la note.
pub(crate) mod after {
    /// Pédale (`\sustainOn`, `\sustainOff`), au plus près de la note.
    pub(crate) const PEDAL: u8 = 5;
    /// Fermeture de liaison d'expression (`)`), avant une éventuelle
    /// ouverture au même tick.
    pub(crate) const SLUR_STOP: u8 = 19;
    /// Ouverture de liaison d'expression (`(`).
    pub(crate) const SLUR_START: u8 = 20;
    /// Fin de soufflet (`\!`), avant un éventuel départ au même tick.
    pub(crate) const HAIRPIN_STOP: u8 = 29;
    /// Soufflet (`\<`, `\>`).
    pub(crate) const HAIRPIN_START: u8 = 30;
    /// Fin de trille (`\stopTrillSpan`).
    pub(crate) const TRILL_STOP: u8 = 39;
    /// Trille (`\startTrillSpan`).
    pub(crate) const TRILL_START: u8 = 40;
    /// Glissando (`\glissando`).
    pub(crate) const GLISSANDO: u8 = 50;
    /// Nuance (`\p`), au plus près de la note pour que `_`/`^` s'y accroche.
    pub(crate) const DYNAMIC: u8 = 6;
    /// Texte de portée (`^\markup`), accords chiffrés.
    pub(crate) const TEXT: u8 = 56;
    /// Point d'orgue (`\fermata`).
    pub(crate) const FERMATA: u8 = 60;
}
