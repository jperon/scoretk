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

//! Silences multi-mesures portés par une mesure, **par portée**.
//!
//! Portage de `Measure::isEmpty(staffIdx)` (`engraving/dom/measure.cpp:2286-2330`)
//! et de `MMRestLayout::createMMRest` (`engraving/rendering/score/mmrestlayout.cpp:161-206`) :
//! MuseScore regroupe les mesures vides pour une portée donnée ; une même
//! mesure peut donc ouvrir un `<multiple-rest>` sur une portée et porter des
//! notes sur une autre. Le champ historique `Measure::multi_measure_rest`
//! reste la valeur globale utilisée par les autres écrivains ; celui-ci
//! conserve l'information par portée lue dans les fichiers MuseScore.

use super::annotation::Annotation;
use super::chord::ChordRestKind;
use super::fraction::Fraction;
use super::ids::{StaffId, Track, VOICES};
use super::repeat::RepeatMark;
use super::score::Score;
use super::segment::{BarLineType, SegmentType};

/// Nombre de mesures regroupées par un silence multi-mesures, par portée.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultiMeasureRests {
    /// Paires `(portée, nombre)`, triées par portée.
    entries: Vec<(StaffId, u16)>,
}

impl MultiMeasureRests {
    /// Nouveau groupe vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre le nombre de mesures regroupées pour `staff`.
    pub fn set(&mut self, staff: StaffId, count: u16) {
        if count == 0 {
            self.remove(staff);
            return;
        }
        match self.entries.binary_search_by_key(&staff, |(id, _)| *id) {
            Ok(index) => self.entries[index].1 = count,
            Err(index) => self.entries.insert(index, (staff, count)),
        }
    }

    /// Retire l'entrée de `staff`, si elle existe.
    pub fn remove(&mut self, staff: StaffId) {
        if let Ok(index) = self.entries.binary_search_by_key(&staff, |(id, _)| *id) {
            self.entries.remove(index);
        }
    }

    /// Nombre de mesures regroupées pour `staff`, s'il y en a.
    pub fn count_for(&self, staff: StaffId) -> Option<u16> {
        self.entries
            .binary_search_by_key(&staff, |(id, _)| *id)
            .ok()
            .map(|index| self.entries[index].1)
    }

    /// Première valeur enregistrée, pour les écrivains qui n'ont pas de portée.
    pub fn primary(&self) -> Option<u16> {
        self.entries.first().map(|(_, count)| *count)
    }

    /// Vrai si aucune portée n'ouvre de multi-repos ici.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Itère sur les paires `(portée, nombre)`.
    pub fn iter(&self) -> impl Iterator<Item = (StaffId, u16)> + '_ {
        self.entries.iter().copied()
    }
}

/// Groupes de silences multi-mesures déduits des mesures vides.
///
/// Portage minimal de `MeasureLayout::createMultiMeasureRestsIfNeed` et
/// `MeasureLayout::validMMRestMeasure`
/// (`engraving/rendering/score/measurelayout.cpp:849-902` et `:580-641`) sans
/// la mise en page : MuseScore remplace une suite de mesures vides par un
/// silence multi-mesures à la lecture, même quand le `.mscx` n'écrit aucun
/// marqueur `<multiMeasureRest>`. Le regroupement est **global à la
/// partition** (la même valeur est écrite dans toutes les parties,
/// `measureStyle` `exportmusicxml.cpp`), et n'examine que les portées
/// affichées : `Staff::show()` vaut `part()->show() && visible()`
/// (`engraving/dom/staff.cpp:1420-1423`).
///
/// Le résultat est indexé par mesure : `Some(n)` sur la première mesure d'un
/// groupe de `n` mesures, `None` ailleurs. Il est vide quand
/// `createMultiMeasureRests` est faux, et quand la partition porte déjà des
/// marqueurs `<multiMeasureRest>` (voir le commentaire dans le corps). Les
/// ruptures de `breakMultiMeasureRest` que le modèle ne porte pas (spanners
/// volta/tempo progressif/textline, drapeau `<breakMultiMeasureRest>`, clefs
/// de courtoisie) ne sont pas reproduites ; elles ne peuvent que scinder un
/// groupe davantage.
pub fn derived_mmrest_runs(score: &Score) -> Vec<Option<u16>> {
    let mut runs = vec![None; score.measures.len()];
    if !score.style.create_multi_measure_rests {
        return runs;
    }
    // Un `.mscx` qui porte des marqueurs `<multiMeasureRest>` a déjà enregistré
    // la disposition des multi-repos. La compléter depuis les mesures vides
    // sur-groupe : les marqueurs d'un fichier 3.x peuvent être partiels, et la
    // disposition dépend aussi des cadres et des systèmes (`testTboxWords1`
    // attend un groupe de 2 là où la règle de vacuité en trouve 3). Les
    // marqueurs lus restent donc la seule source quand ils existent.
    if score
        .measures
        .iter()
        .any(|measure| measure.multi_measure_rest.is_some() || !measure.mmrests.is_empty())
    {
        return runs;
    }
    let minimum = score.style.min_empty_measures.max(1);
    let mut index = 0usize;
    while index < score.measures.len() {
        if !valid_mmrest_measure(score, index) {
            index += 1;
            continue;
        }
        let first = index;
        let mut last = index;
        while last + 1 < score.measures.len()
            && valid_mmrest_measure(score, last + 1)
            && !break_mmrest(score, last + 1)
        {
            last += 1;
        }
        let count = last - first + 1;
        if count >= minimum {
            runs[first] = Some(u16::try_from(count).unwrap_or(u16::MAX));
        }
        index = last + 1;
    }
    runs
}

/// Vrai si `annotation` interdit le regroupement en multi-repos
/// (`BREAK_TYPES`, `measurelayout.cpp:147-163`).
fn is_break_type(annotation: &Annotation) -> bool {
    match annotation {
        Annotation::Tempo(_)
        | Annotation::RehearsalMark(_)
        | Annotation::Harmony(_)
        | Annotation::StaffText(_)
        | Annotation::Expression(_)
        | Annotation::Dynamic(_)
        | Annotation::SystemText(_)
        | Annotation::InstrumentChange(_) => true,
        Annotation::Other { kind, .. } => matches!(
            kind.as_str(),
            "TripletFeel"
                | "PlayTechAnnotation"
                | "Capo"
                | "StringTunings"
                | "Symbol"
                | "FretDiagram"
                | "HarpDiagram"
                | "PlayCountText"
                | "Breath"
        ),
        _ => false,
    }
}

/// `MeasureLayout::validMMRestMeasure` (`measurelayout.cpp:580-641`).
fn valid_mmrest_measure(score: &Score, index: usize) -> bool {
    let measure = &score.measures[index];
    if measure.is_irregular() || measure.measure_repeat.is_some() {
        return false;
    }
    let mut rest_segments = 0usize;
    for segment in &measure.segments {
        let mid_measure = segment.tick > measure.start_tick;
        for entry in &segment.annotations {
            if !score.staff_shown(entry.track().staff()) || !entry.annotation().visible() {
                continue;
            }
            // Une annotation qui coupe le multi-repos n'est admise qu'au
            // début de la mesure.
            if mid_measure && is_break_type(entry.annotation()) {
                return false;
            }
        }
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        // « measure is not empty if there is more than one rest » : une
        // mesure dont les silences occupent plusieurs positions rythmiques
        // n'est pas regroupable.
        if segment
            .annotations
            .iter()
            .any(|entry| matches!(entry.annotation(), Annotation::Fermata(_)))
        {
            return false;
        }
        let mut rest_found = false;
        for staff in 0..score.staves.len() {
            let staff_id = StaffId::from_index(staff);
            if !score.staff_shown(staff_id) {
                continue;
            }
            for voice in 0..VOICES {
                let Some(item) = segment.element(Track::new(staff_id, voice as u8)) else {
                    continue;
                };
                let Some(chord_rest) = item.as_chord_rest() else {
                    continue;
                };
                match &chord_rest.kind {
                    ChordRestKind::Chord(_) => return false,
                    ChordRestKind::Rest(_) => rest_found = true,
                }
            }
        }
        if rest_found {
            rest_segments += 1;
            if rest_segments > 1 {
                return false;
            }
        }
    }
    true
}

/// `MeasureLayout::breakMultiMeasureRest` (`measurelayout.cpp:649-822`),
/// limité aux conditions que le modèle porte.
fn break_mmrest(score: &Score, index: usize) -> bool {
    let measure = &score.measures[index];
    if measure.start_bar_line.is_repeat_start() {
        return true;
    }
    let previous = &score.measures[index - 1];
    if previous.end_bar_line.is_repeat_end() || measure.section_break_before {
        return true;
    }
    // Marqueur dans la mesure, saut dans la mesure précédente.
    if !measure.repeat_marks.is_empty()
        || previous
            .repeat_marks
            .iter()
            .any(|mark| matches!(mark, RepeatMark::Jump(_) | RepeatMark::Marker(_)))
    {
        return true;
    }
    // Barre de mesure de fin inhabituelle dans la mesure précédente.
    if !matches!(
        previous.end_bar_line,
        BarLineType::Normal | BarLineType::Broken | BarLineType::Dotted
    ) {
        return true;
    }
    // Annotation qui coupe le multi-repos, à n'importe quel tick.
    for segment in &measure.segments {
        for entry in &segment.annotations {
            if score.staff_shown(entry.track().staff())
                && entry.annotation().visible()
                && is_break_type(entry.annotation())
            {
                return true;
            }
        }
    }
    // Changements d'armure, d'indication de mesure et de clef portés par un
    // élément non engendré d'une portée affichée (`measurelayout.cpp:785-800`).
    let starts_at_zero = measure.start_tick == Fraction::ZERO;
    for segment in &measure.segments {
        if !matches!(
            segment.kind,
            SegmentType::KeySig
                | SegmentType::TimeSig
                | SegmentType::Clef
                | SegmentType::HeaderClef
                | SegmentType::StartRepeatBarLine
        ) {
            continue;
        }
        let mut on_shown_staff = false;
        for staff in 0..score.staves.len() {
            let staff_id = StaffId::from_index(staff);
            if score.staff_shown(staff_id) && segment.element(Track::new(staff_id, 0)).is_some() {
                on_shown_staff = true;
                break;
            }
        }
        if !on_shown_staff {
            continue;
        }
        match segment.kind {
            SegmentType::StartRepeatBarLine => return true,
            // Une indication de mesure ou une armure dans la mesure coupe le
            // groupe, même à son début ; la première mesure de la partition
            // est exclue.
            SegmentType::KeySig | SegmentType::TimeSig if !starts_at_zero => return true,
            // Une clef coupe le groupe sauf à la fin de la mesure (clef de
            // courtoisie de la mesure suivante).
            SegmentType::Clef | SegmentType::HeaderClef
                if !starts_at_zero && segment.tick != measure.end_tick() =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ChordRest, Duration, Fraction, Measure, Part, Score, Segment, SegmentItem, StyleSettings,
    };

    /// Partition à `measures` mesures de 4/4, une par `(portée, contenu)`.
    fn partition(
        staves: &[(bool, bool)],
        measures: usize,
        empty: &[usize],
        chords_on_hidden: bool,
    ) -> Score {
        let mut score = Score::new();
        score.style = StyleSettings::for_version(3);
        score.style.create_multi_measure_rests = true;
        let mut staff_ids = Vec::new();
        for &(visible, part_visible) in staves {
            let mut part = Part::new("P", "P");
            part.visible = part_visible;
            let part_id = score.add_part(part);
            let staff_id = score.add_staff(part_id);
            score.staff_mut(staff_id).visible = visible;
            staff_ids.push(staff_id);
        }
        let staff_count = score.staves.len();
        for index in 0..measures {
            let tick = Fraction::from_integer(index as i64);
            let mut measure = Measure::new(index, tick, Fraction::ONE);
            let mut segment = Segment::new(SegmentType::ChordRest, tick, staff_count);
            for (position, staff_id) in staff_ids.iter().enumerate() {
                let is_empty = empty.contains(&index);
                for voice in 0..VOICES {
                    let value = if build_chord(is_empty, position, chords_on_hidden) {
                        SegmentItem::ChordRest(ChordRest::chord(Duration::quarter()))
                    } else {
                        SegmentItem::ChordRest(ChordRest::full_measure_rest())
                    };
                    segment.set_element(Track::new(*staff_id, voice as u8), value);
                }
            }
            measure.add_segment(segment);
            score.add_measure(measure);
        }
        score
    }

    /// Vrai si l'emplacement reçoit un accord plutôt qu'un silence.
    fn build_chord(empty: bool, position: usize, chords_on_hidden: bool) -> bool {
        if empty {
            // Les portées impaires simulent la partie masquée (`show=0`).
            return chords_on_hidden && position % 2 == 1;
        }
        position == 0
    }

    #[test]
    fn groupe_les_mesures_vides_des_portees_affichees() {
        // Deux portées, la seconde dans une partie masquée (`show=0`) :
        // `Staff::show()` est faux, ses accords ne comptent pas
        // (`validMMRestMeasure`, `measurelayout.cpp:590-620`).
        let score = partition(&[(true, true), (true, false)], 5, &[1, 2, 3], true);
        let runs = derived_mmrest_runs(&score);
        assert_eq!(runs, vec![None, Some(3), None, None, None]);
    }

    #[test]
    fn un_accord_sur_portee_affichee_coupe_le_groupe() {
        let mut score = partition(&[(true, true), (true, false)], 5, &[1, 2, 3], true);
        score.style.min_empty_measures = 1;
        // Un accord sur la portée visible de la troisième mesure.
        let segment = &mut score.measures[2].segments[0];
        segment.set_element(
            Track::new(StaffId::from_index(0), 0),
            SegmentItem::ChordRest(ChordRest::chord(Duration::quarter())),
        );
        let runs = derived_mmrest_runs(&score);
        assert_eq!(runs, vec![None, Some(1), None, Some(1), None]);
    }

    #[test]
    fn le_minimum_depend_de_la_version_du_style() {
        let score = partition(&[(true, true)], 3, &[1], false);
        let mut v4 = score.clone();
        v4.style = StyleSettings::for_version(4);
        v4.style.create_multi_measure_rests = true;
        assert_eq!(derived_mmrest_runs(&score), vec![None, None, None]);
        assert_eq!(derived_mmrest_runs(&v4), vec![None, Some(1), None]);
    }

    #[test]
    fn rien_sans_create_multi_measure_rests() {
        let mut score = partition(&[(true, true)], 3, &[1], false);
        score.style.min_empty_measures = 1;
        score.style.create_multi_measure_rests = false;
        assert_eq!(derived_mmrest_runs(&score), vec![None, None, None]);
    }

    #[test]
    fn une_mesure_irreguliere_ne_groupe_pas() {
        let mut score = partition(&[(true, true)], 4, &[1, 2], false);
        score.style.min_empty_measures = 1;
        score.measures[2].len = Fraction::new(3, 4);
        let runs = derived_mmrest_runs(&score);
        assert_eq!(runs, vec![None, Some(1), None, None]);
    }

    #[test]
    fn la_mesure_vide_doit_etre_un_seul_silence() {
        let mut score = partition(&[(true, true)], 3, &[1], false);
        score.style.min_empty_measures = 1;
        // Deuxième segment de silence dans la mesure vide : « measure is not
        // empty if there is more than one rest » (`measurelayout.cpp:629-635`).
        let tick = Fraction::from_integer(1) + Fraction::new(1, 2);
        let mut segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
        segment.set_element(
            Track::new(StaffId::from_index(0), 0),
            SegmentItem::ChordRest(ChordRest::rest(Duration::half())),
        );
        score.measures[1].add_segment(segment);
        assert_eq!(derived_mmrest_runs(&score), vec![None, None, None]);
    }

    #[test]
    fn un_changement_de_mesure_coupe_le_groupe() {
        // Runs de mesures 1..4 vides, changement d'indication au milieu de la
        // mesure 2 : le groupe repart à la mesure 2 (`breakMultiMeasureRest`,
        // `measurelayout.cpp:668-682`).
        let mut score = partition(&[(true, true)], 6, &[1, 2, 3, 4], false);
        let tick = Fraction::from_integer(2) + Fraction::new(1, 2);
        let mut segment = Segment::new(SegmentType::TimeSig, tick, score.staves.len());
        segment.set_element(
            Track::new(StaffId::from_index(0), 0),
            SegmentItem::TimeSig(crate::model::TimeSig::new(3, 4)),
        );
        score.measures[2].add_segment(segment);
        assert_eq!(
            derived_mmrest_runs(&score),
            vec![None, None, Some(3), None, None, None]
        );
    }

    #[test]
    fn un_marqueur_explicite_empeche_la_derivation() {
        // Le `.mscx` a déjà enregistré un groupe : la règle de vacuité ne doit
        // pas en ajouter d'autres (`testTboxWords1`).
        let mut score = partition(&[(true, true)], 5, &[1, 2, 3], false);
        score.measures[1].mmrests.set(StaffId::from_index(0), 2);
        assert_eq!(derived_mmrest_runs(&score), vec![None; 5]);
    }

    #[test]
    fn un_point_d_orgue_coupe_le_groupe() {
        let mut score = partition(&[(true, true)], 3, &[1], false);
        score.style.min_empty_measures = 1;
        let fermata = crate::model::Fermata::new(crate::model::FermataType::Normal);
        score.measures[1].segments[0].add_annotation(
            Track::new(StaffId::from_index(0), 0),
            crate::model::Annotation::Fermata(fermata),
        );
        assert_eq!(derived_mmrest_runs(&score), vec![None, None, None]);
    }
}
