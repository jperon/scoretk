// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_operations.h` et `importmidi_operation.h` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026. Seules les valeurs par défaut sont portées : la
// bibliothèque n'expose pas le panneau d'options de l'interface.
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

//! Options de l'import MIDI, aux valeurs par défaut de MuseScore.
//!
//! Portage des valeurs de `MidiOperations::Opers` (`importmidi_operations.h`)
//! qui influencent le résultat : quantification à la double croche, recherche
//! des n-olets 3/5/7/9, points de prolongation, staccato, séparation des mains,
//! quatre voix au plus, changement de clef, affichage du tempo.

/// Plus petite valeur de quantification (`QuantValue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantValue {
    /// Noire.
    Q4,
    /// Croche.
    Q8,
    /// Double croche.
    Q16,
    /// Triple croche.
    Q32,
    /// Quadruple croche.
    Q64,
    /// 128e.
    Q128,
    /// 256e.
    Q256,
    /// 512e.
    Q512,
    /// 1024e.
    Q1024,
}

/// Nombre maximal de voix (`VoiceCount`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceCount {
    /// Une voix.
    V1,
    /// Deux voix.
    V2,
    /// Trois voix.
    V3,
    /// Quatre voix.
    V4,
}

impl VoiceCount {
    /// Nombre de voix correspondant.
    pub const fn limit(self) -> usize {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
            Self::V4 => 4,
        }
    }
}

/// Options d'import MIDI.
///
/// Les valeurs par défaut sont celles de `MidiOperations::Opers`, la
/// quantification étant fixée à la double croche comme
/// `defaultQuantValueFromPreferences` (valeur de
/// `midiShortestNote = DIVISION / 4`).
#[derive(Clone, Debug)]
pub struct ImportOptions {
    /// Quantification de base.
    pub quant_value: QuantValue,
    /// Cherche les n-olets.
    pub search_tuplets: bool,
    /// Note les n-olets détectés (`<time-modification>`), opt-in.
    ///
    /// Le filtre complet de MuseScore (`filterTuplets`) n'est pas porté :
    /// l'option permet de mesurer le détecteur sans changer le comportement
    /// par défaut.
    pub tuplet_notation: bool,
    /// Cherche les duolets (`search2plets`, faux par défaut).
    pub search_2plets: bool,
    /// Cherche les triolets.
    pub search_3plets: bool,
    /// Cherche les quadruplets (`search4plets`, vrai par défaut).
    pub search_4plets: bool,
    /// Cherche les quintolet.
    pub search_5plets: bool,
    /// Cherche les septolets.
    pub search_7plets: bool,
    /// Cherche les nonolets.
    pub search_9plets: bool,
    /// Utilise les points de prolongation.
    pub use_dots: bool,
    /// Affiche un point d'articulation sur les notes piquées.
    pub show_staccato: bool,
    /// Sépare la main droite et la main gauche d'un piano.
    pub do_staff_split: bool,
    /// Nombre maximal de voix.
    pub max_voice_count: VoiceCount,
    /// Change la clef selon la hauteur des notes.
    pub change_clef: bool,
    /// Affiche le texte de tempo.
    pub show_tempo_text: bool,
    /// Simplifie les durées des pistes de percussion.
    pub simplify_durations: bool,
    /// Supprime les silences des pistes de percussion.
    pub remove_drum_rests: bool,
    /// Cherche une mesure d'anacrouse.
    pub search_pickup_measure: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            quant_value: QuantValue::Q16,
            search_tuplets: true,
            tuplet_notation: false,
            search_2plets: false,
            search_3plets: true,
            search_4plets: true,
            search_5plets: true,
            search_7plets: true,
            search_9plets: true,
            use_dots: true,
            show_staccato: true,
            do_staff_split: false,
            max_voice_count: VoiceCount::V4,
            change_clef: true,
            show_tempo_text: true,
            simplify_durations: true,
            remove_drum_rests: true,
            search_pickup_measure: true,
        }
    }
}
