// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/segment.h`, `engraving/dom/keysig.h`,
// `engraving/dom/timesig.h`, `engraving/dom/clef.h` et
// `engraving/dom/barline.h` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Segments, armures, mesures, clefs et barres de mesure.
//!
//! Portage des objets verticaux de MuseScore : `Segment` et `SegmentType`
//! (`engraving/dom/segment.h`), `KeySig` (`engraving/dom/keysig.h`),
//! `TimeSig` (`engraving/dom/timesig.h`), `Clef` (`engraving/dom/clef.h`) et
//! `BarLine` (`engraving/dom/barline.h`).
//!
//! Un segment contient un élément au plus par piste. La taille du vecteur est
//! `nb_portées * VOICES`, et l'index est [`Track::raw`], exactement comme
//! `Segment::_elist` dans MuseScore.

use super::annotation::{Annotation, Placement};
use super::chord::ChordRest;
use super::fraction::Tick;
use super::ids::{Track, VOICES};

// L1 metric types live in `layers/metric.rs`; re-exported here so that
// `model::segment::{Key, TimeSig, Clef, BarLine, …}` keeps working.
pub use super::layers::metric::{
    BarLine, BarLineType, Clef, ClefSign, ClefType, Key, KeyMode, KeySig, TimeSig, TimeSigType,
};

/// Type de respiration.
///
/// Correspond aux `SymId` de `Breath::BREATH_LIST`
/// (`engraving/dom/breath.cpp:33-44`) : six césures (`isCaesura`) et quatre
/// respirations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BreathType {
    /// Virgule.
    #[default]
    Comma,
    /// Césure normale.
    Caesura,
    /// Césure courbe (`caesuraCurved`).
    CaesuraCurved,
    /// Césure courte (`caesuraShort`).
    CaesuraShort,
    /// Césure épaisse (`caesuraThick`).
    CaesuraThick,
    /// Césure simple trait (`caesuraSingleStroke`).
    CaesuraSingle,
    /// Respiration en tick (`breathMarkTick`).
    Tick,
    /// Coup d'archet (`breathMarkUpbow`).
    Upbow,
    /// Respiration de Salzedo (`breathMarkSalzedo`).
    Salzedo,
}

/// Respiration.
///
/// Correspond à `Breath` dans `engraving/dom/breath.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Breath {
    /// Type de respiration.
    pub kind: BreathType,
    /// Placement vertical (`ChordRest::hasBreathMark` : la voix 0 est
    /// au-dessus, la voix 1 au-dessous).
    pub placement: Placement,
    /// Durée de la pause en millisecondes (`Breath::pause()`,
    /// `<pause>` en secondes, `engraving/dom/breath.cpp`).
    ///
    /// `None` quand le fichier n'en donne pas : la respiration ne rallonge
    /// alors pas la lecture MIDI. Une césure de 0,5 s insère une mesure de
    /// ticks dans `PauseMap` (`Vingt_danses`, `El_Choclo`).
    pub pause_ms: Option<u32>,
}

/// Type de segment.
///
/// Correspond à `SegmentType` dans `engraving/dom/segment.h`. L'ordre de
/// déclaration reproduit celui de MuseScore, ce qui permet de trier les
/// segments d'une mesure par `(tick, type)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SegmentType {
    /// Segment invalide.
    Invalid,
    /// Barre de mesure au début de la mesure.
    BeginBarLine,
    /// Clef d'en-tête de mesure.
    HeaderClef,
    /// Armure.
    KeySig,
    /// Ambitus.
    Ambitus,
    /// Respiration.
    Breath,
    /// Indication de mesure.
    TimeSig,
    /// Barre de début de reprise.
    StartRepeatBarLine,
    /// Clef de début de reprise (annonce).
    ClefStartRepeatAnnounce,
    /// Armure de début de reprise (annonce).
    KeySigStartRepeatAnnounce,
    /// Indication de mesure de début de reprise (annonce).
    TimeSigStartRepeatAnnounce,
    /// Clef.
    Clef,
    /// Barre de mesure.
    BarLine,
    /// Tick de temps (segment sans élément visible).
    TimeTick,
    /// Accord ou silence.
    ChordRest,
    /// Clef répétée (annonce de courtoisie).
    ClefRepeatAnnounce,
    /// Armure répétée (annonce de courtoisie).
    KeySigRepeatAnnounce,
    /// Indication de mesure répétée (annonce de courtoisie).
    TimeSigRepeatAnnounce,
    /// Barre de fin de mesure.
    EndBarLine,
    /// Armure de courtoisie.
    KeySigAnnounce,
    /// Indication de mesure de courtoisie.
    TimeSigAnnounce,
}

impl SegmentType {
    /// Vrai pour un segment porteur d'accords ou de silences.
    pub const fn is_chord_rest_type(self) -> bool {
        matches!(self, Self::ChordRest)
    }

    /// Vrai pour un segment de tick de temps.
    pub const fn is_time_tick_type(self) -> bool {
        matches!(self, Self::TimeTick)
    }

    /// Vrai pour un segment porteur d'une durée.
    pub const fn has_duration(self) -> bool {
        matches!(self, Self::ChordRest | Self::TimeTick)
    }

    /// Vrai pour une barre de mesure.
    pub const fn is_bar_line_type(self) -> bool {
        matches!(
            self,
            Self::BeginBarLine | Self::StartRepeatBarLine | Self::BarLine | Self::EndBarLine
        )
    }

    /// Vrai pour une clef (y compris les annonces de reprise et de courtoisie).
    pub const fn is_clef_type(self) -> bool {
        matches!(
            self,
            Self::HeaderClef
                | Self::Clef
                | Self::ClefStartRepeatAnnounce
                | Self::ClefRepeatAnnounce
        )
    }

    /// Vrai pour une armure (y compris les annonces de reprise et de
    /// courtoisie).
    pub const fn is_key_sig_type(self) -> bool {
        matches!(
            self,
            Self::KeySig
                | Self::KeySigStartRepeatAnnounce
                | Self::KeySigRepeatAnnounce
                | Self::KeySigAnnounce
        )
    }

    /// Vrai pour une indication de mesure (y compris les annonces de reprise
    /// et de courtoisie).
    pub const fn is_time_sig_type(self) -> bool {
        matches!(
            self,
            Self::TimeSig
                | Self::TimeSigStartRepeatAnnounce
                | Self::TimeSigRepeatAnnounce
                | Self::TimeSigAnnounce
        )
    }
}

/// Élément rangé dans un segment.
///
/// Pendant de `EngravingItem` dans `engraving/dom/engravingitem.h`, limité aux
/// types utiles à l'export MusicXML.
#[derive(Clone, Debug, PartialEq)]
pub enum SegmentItem {
    /// Accord ou silence.
    ChordRest(ChordRest),
    /// Armure.
    KeySig(KeySig),
    /// Indication de mesure.
    TimeSig(TimeSig),
    /// Clef.
    Clef(Clef),
    /// Barre de mesure.
    BarLine(BarLine),
    /// Respiration.
    Breath(Breath),
}

impl SegmentItem {
    /// Accès à l'accord ou au silence, si l'élément en est un.
    pub fn as_chord_rest(&self) -> Option<&ChordRest> {
        match self {
            Self::ChordRest(value) => Some(value),
            _ => None,
        }
    }

    /// Accès mutable à l'accord ou au silence, si l'élément en est un.
    pub fn as_chord_rest_mut(&mut self) -> Option<&mut ChordRest> {
        match self {
            Self::ChordRest(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SegmentAnnotation {
    /// Piste à laquelle l'annotation est rattachée.
    pub track: Track,
    /// Annotation elle-même.
    pub annotation: Annotation,
}

impl SegmentAnnotation {
    /// Construit une annotation de segment rattachée à une piste.
    pub fn new(track: Track, annotation: Annotation) -> Self {
        Self { track, annotation }
    }

    /// Piste de l'annotation.
    pub const fn track(&self) -> Track {
        self.track
    }

    /// Annotation.
    pub fn annotation(&self) -> &Annotation {
        &self.annotation
    }

    /// Texte affiché, si l'annotation en a un.
    pub fn text(&self) -> &str {
        self.annotation.text()
    }
}

/// Segment vertical d'une mesure.
///
/// Correspond à `Segment` dans `engraving/dom/segment.h`. Tous les éléments
/// d'un segment commencent au même tick ; `elements` est indexé par
/// [`Track::raw`], avec `len = nb_portées * VOICES`. Les annotations portent
/// aussi leur piste, comme `EngravingItem::track()` dans MuseScore.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// Tick du segment.
    pub tick: Tick,
    /// Type du segment.
    pub kind: SegmentType,
    /// Annotations attachées au segment, avec leur piste.
    pub annotations: Vec<SegmentAnnotation>,
    elements: Vec<Option<SegmentItem>>,
}

impl Segment {
    /// Construit un segment vide pour un nombre donné de portées.
    pub fn new(kind: SegmentType, tick: Tick, staff_count: usize) -> Self {
        Self {
            tick,
            kind,
            annotations: Vec::new(),
            elements: vec![None; staff_count * VOICES],
        }
    }

    /// Élément de la piste, s'il existe.
    pub fn element(&self, track: Track) -> Option<&SegmentItem> {
        self.elements.get(track.index()).and_then(Option::as_ref)
    }

    /// Élément mutable de la piste, s'il existe.
    pub fn element_mut(&mut self, track: Track) -> Option<&mut SegmentItem> {
        self.elements
            .get_mut(track.index())
            .and_then(Option::as_mut)
    }

    /// Place un élément sur une piste, en agrandissant le vecteur si besoin.
    pub fn set_element(&mut self, track: Track, item: SegmentItem) -> Option<SegmentItem> {
        let index = track.index();
        if index >= self.elements.len() {
            self.elements.resize(index + 1, None);
        }
        self.elements[index].replace(item)
    }

    /// Retire l'élément d'une piste.
    pub fn take_element(&mut self, track: Track) -> Option<SegmentItem> {
        self.elements.get_mut(track.index()).and_then(Option::take)
    }

    /// Accès à l'accord ou au silence d'une piste.
    pub fn chord_rest(&self, track: Track) -> Option<&ChordRest> {
        self.element(track).and_then(SegmentItem::as_chord_rest)
    }

    /// Accès mutable à l'accord ou au silence d'une piste.
    pub fn chord_rest_mut(&mut self, track: Track) -> Option<&mut ChordRest> {
        self.element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
    }

    /// Ajoute une annotation au segment, rattachée à une piste.
    ///
    /// La piste joue le rôle de `EngravingItem::track()` dans MuseScore :
    /// elle indique la portée et la voix auxquelles l'export doit rattacher
    /// l'annotation (`Segment::annotations()` dans
    /// `engraving/dom/segment.h`).
    pub fn add_annotation(&mut self, track: Track, annotation: Annotation) {
        self.annotations
            .push(SegmentAnnotation::new(track, annotation));
    }

    /// Annotations d'une piste donnée.
    pub fn annotations_on(&self, track: Track) -> impl Iterator<Item = &Annotation> + '_ {
        self.annotations
            .iter()
            .filter(move |entry| entry.track == track)
            .map(|entry| &entry.annotation)
    }

    /// Éléments indexés par piste.
    pub fn elements(&self) -> &[Option<SegmentItem>] {
        &self.elements
    }

    /// Itère sur les pistes occupées.
    pub fn iter_elements(&self) -> impl Iterator<Item = (Track, &SegmentItem)> + '_ {
        self.elements
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item.as_ref()
                    .map(|item| (Track::from_raw(index as u32), item))
            })
    }

    /// Ajuste la taille du vecteur au nombre de portées.
    pub fn resize_staves(&mut self, staff_count: usize) {
        self.elements.resize(staff_count * VOICES, None);
    }
}
