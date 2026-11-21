// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/measure.h` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Mesures.
//!
//! Portage de `Measure` (`engraving/dom/measure.h`). Une mesure possède une
//! position de départ, une durée réelle (`len`), une durée nominale
//! (`nominal_len`, l'ancien `m_timesig`) et ses segments triés.
//!
//! L'irrégularité n'est pas un champ stocké : comme `Measure::isIrregular()`
//! dans MuseScore, elle se déduit de `len != nominal_len`.

use super::fraction::{Fraction, Tick};
use super::ids::StaffId;
use super::repeat::RepeatMark;
use super::segment::{BarLineType, Segment, SegmentType};

/// Marque de répétition de mesure.
///
/// Correspond au `MeasureRepeat` de MuseScore (`engraving/dom/measurerepeat.h`)
/// et aux compteurs `Measure::measureRepeatNumMeasures()` /
/// `Measure::isMeasureRepeatGroup()` qu'utilise `ExportMusicXml::measureRepeat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasureRepeatMark {
    /// Portée qui répète.
    pub staff: StaffId,
    /// Nombre de mesures répétées (`numMeasures`).
    pub count: u16,
}

/// Mesure.
///
/// Correspond à `Measure` dans `engraving/dom/measure.h`.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    /// Numéro de mesure à partir de zéro, comme `Measure::measureNumber()`
    /// dans MuseScore. Le numéro affiché en MusicXML est calculé par l'export.
    pub number: usize,
    /// Décalage de numérotation affichée (`Measure::measureNumberOffset`,
    /// `<noOffset>`). Additionné au numéro courant à l'export MusicXML.
    pub number_offset: i32,
    /// Tick de début.
    pub start_tick: Tick,
    /// Durée réelle de la mesure.
    pub len: Fraction,
    /// Durée nominale donnée par l'indication de mesure.
    pub nominal_len: Fraction,
    /// Numérateur **non réduit** de l'indication de mesure.
    ///
    /// `ReducedFraction` de MuseScore conserve 4/4 comme `(4, 4)`, alors que
    /// [`Fraction`] le réduit à `1/1`. Les fonctions `isDuple`, `isCompound`,
    /// `beatLength`… (`importmidi_meter.cpp`) dépendent de ce numérateur brut.
    pub time_sig_numerator: i64,
    /// Dénominateur non réduit de l'indication de mesure.
    pub time_sig_denominator: i64,
    /// Mesure exclue de la numérotation (`Measure::excludeFromNumbering()`),
    /// c'est-à-dire une anacrouse ou une mesure irrégulière marquée
    /// `<irregular>1</irregular>` (3.x) ou `@len` (4.x). MuseScore l'exporte
    /// avec `number="0" implicit="yes"`.
    pub exclude_from_numbering: bool,
    /// Barre de mesure au début.
    pub start_bar_line: BarLineType,
    /// Barre de mesure à la fin.
    pub end_bar_line: BarLineType,
    /// Nombre de lectures de la reprise de fin (`<endRepeat>N</endRepeat>`).
    ///
    /// Zéro quand la mesure ne termine pas de reprise. MuseScore n'écrit
    /// l'attribut `times` de `<repeat>` que si ce nombre dépasse deux.
    pub repeat_count: u8,
    /// Nombre de mesures regroupées par un silence multi-mesures.
    ///
    /// Vaut `Some(n)` sur la première mesure d'un groupe, comme
    /// `Measure::mmRestCount()` sur la mesure qui remplace le groupe ; `None`
    /// quand la mesure n'ouvre aucun `<multiple-rest>`.
    pub multi_measure_rest: Option<u16>,
    /// Vrai quand un saut de section précède la mesure.
    ///
    /// MuseScore recommence alors la numérotation affichée à 1 et réécrit les
    /// `<attributes>` de début de section (`<divisions>` compris).
    pub section_break_before: bool,
    /// Durée de la pause du saut de section, en secondes
    /// (`LayoutBreak::pause()`, propriété `Pid::PAUSE`, défaut
    /// `Sid::sectionPause` = 3 s ; `layoutbreak.cpp:138-140, 209`).
    ///
    /// `None` quand le saut ne porte pas de `<pause>` explicite : la valeur par
    /// défaut du style s'applique. Un fichier peut la mettre à `0` pour ne pas
    /// interrompre la lecture (`Dominus_fredipi`, 3.x).
    pub section_pause: Option<f64>,
    /// Un saut de section avec `startWithMeasureOne="0"` conserve la
    /// numérotation au lieu de la relancer à 1
    /// (`MeasureNumberStateHandler::updateForMeasure`, `exportmusicxml.cpp:8274-8286`).
    pub section_keeps_numbering: bool,
    /// Mesure répétée (`<RepeatMeasure>`/`<MeasureRepeat>`), avec la portée
    /// concernée et le nombre de mesures répétées.
    ///
    /// Le contenu de la mesure a été recopié depuis la source ; l'export écrit
    /// en plus `<measure-style><measure-repeat>` (`measureRepeat`).
    pub measure_repeat: Option<MeasureRepeatMark>,
    /// Vrai quand la mesure finit un système (`<LayoutBreak>` `line` ou
    /// `page`).
    ///
    /// Sans moteur de mise en page, ce saut explicite est la seule façon de
    /// savoir qu'une mesure ouvre un nouveau système : MuseScore y place alors
    /// une barre de courtoisie (`light-light`) quand la mesure suivante change
    /// d'armure ou d'indication de mesure.
    pub line_break: bool,
    /// Marqueurs et sauts attachés à la mesure, dans l'ordre du fichier
    /// (`Measure::el()` dans MuseScore).
    pub repeat_marks: Vec<RepeatMark>,
    /// Segments de la mesure, triés par `(tick, type)`.
    pub segments: Vec<Segment>,
}

impl Measure {
    /// Construit une mesure vide dont la durée réelle égale la durée nominale.
    pub fn new(number: usize, start_tick: Tick, nominal_len: Fraction) -> Self {
        Self {
            number,
            number_offset: 0,
            start_tick,
            len: nominal_len,
            nominal_len,
            time_sig_numerator: nominal_len.numerator(),
            time_sig_denominator: nominal_len.denominator(),
            exclude_from_numbering: false,
            start_bar_line: BarLineType::Normal,
            end_bar_line: BarLineType::Normal,
            repeat_count: 0,
            multi_measure_rest: None,
            section_break_before: false,
            section_pause: None,
            section_keeps_numbering: false,
            measure_repeat: None,
            line_break: false,
            repeat_marks: Vec::new(),
            segments: Vec::new(),
        }
    }

    /// Remplace la durée réelle (mesure d'anacrouse, mesure irrégulière).
    pub fn with_len(mut self, len: Fraction) -> Self {
        self.len = len;
        self
    }

    /// Pose l'indication de mesure non réduite (4/4, 12/8…).
    ///
    /// La durée nominale reste la valeur réduite ; seul le numérateur brut est
    /// conservé pour les calculs métriques de l'import MIDI.
    pub fn with_raw_timesig(mut self, numerator: i64, denominator: i64) -> Self {
        self.time_sig_numerator = numerator;
        self.time_sig_denominator = denominator;
        self
    }

    /// Tick de fin (exclu).
    pub fn end_tick(&self) -> Tick {
        self.start_tick + self.len
    }

    /// Vrai si la durée réelle diffère de la durée nominale.
    pub fn is_irregular(&self) -> bool {
        self.len != self.nominal_len
    }

    /// Vrai si la mesure commence une reprise.
    pub fn repeat_start(&self) -> bool {
        self.start_bar_line.is_repeat_start()
    }

    /// Vrai si la mesure termine une reprise.
    pub fn repeat_end(&self) -> bool {
        self.end_bar_line.is_repeat_end()
    }

    /// Insère un segment en conservant l'ordre `(tick, type)`.
    ///
    /// L'ordre est celui de MuseScore : à tick égal, les types de segment se
    /// succèdent selon l'ordre de [`SegmentType`].
    pub fn add_segment(&mut self, segment: Segment) {
        let position = self.segments.partition_point(|existing| {
            (existing.tick, existing.kind) <= (segment.tick, segment.kind)
        });
        self.segments.insert(position, segment);
    }

    /// Premier segment de la mesure, s'il existe.
    pub fn first_segment(&self) -> Option<&Segment> {
        self.segments.first()
    }

    /// Dernier segment de la mesure, s'il existe.
    pub fn last_segment(&self) -> Option<&Segment> {
        self.segments.last()
    }

    /// Premier segment d'un type donné.
    pub fn first_segment_of_type(&self, kind: SegmentType) -> Option<&Segment> {
        self.segments.iter().find(|segment| segment.kind == kind)
    }

    /// Segments d'un type donné.
    pub fn segments_of_type(&self, kind: SegmentType) -> impl Iterator<Item = &Segment> + '_ {
        self.segments
            .iter()
            .filter(move |segment| segment.kind == kind)
    }
}
