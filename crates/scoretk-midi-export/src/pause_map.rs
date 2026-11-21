// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/pausemap.cpp` et
// `engraving/dom/tempotimeline.cpp` de MuseScore Studio, Copyright (C)
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

//! Pauses de section et chronologie des tempos, rendues par des ticks et des
//! changements de tempo.
//!
//! Un fichier MIDI ne peut pas contenir de silence de durée libre : MuseScore
//! insère donc une mesure de ticks supplémentaires à chaque pause et encadre
//! cette mesure par deux changements de tempo (`PauseMap` dans
//! `engraving/compat/midi/pausemap.cpp`). Les ticks insérés décalent tous les
//! évènements suivants ; [`PauseMap::tick_with_pauses`] effectue ce décalage.
//!
//! Port fidèle de `PauseMap::calculate` (`pausemap.cpp:47-76`) : la chronologie
//! des tempos est déjà déroulée par `RepeatList` (uticks) et fusionne les
//! changements de tempo et les pauses par tick (`TempoTimeline::rebuild`,
//! `tempotimeline.cpp:134-180`). Il n'y a pas de garde `pause.tick ==
//! segment.tick` dans le C++ : les points sont déjà aux uticks, un par passage.

use std::collections::BTreeMap;

use scoretk_core::model::{Annotation, Score, SegmentItem, SegmentType, TimeSig};

use super::repeat_list::{RepeatList, DIVISION, UNITS_PER_WHOLE};

/// Tempo par défaut, en battements par seconde (`Constants::DEFAULT_TEMPO`).
const DEFAULT_BPS: f64 = 2.0;
/// Bornes de tempo (`constants.h:107-109` : 5 et 999 BPM).
const MIN_BPS: f64 = 5.0 / 60.0;
const MAX_BPS: f64 = 999.0 / 60.0;
/// Durée par défaut d'une pause de section, en secondes (`Sid::sectionPause`).
const SECTION_PAUSE_SECONDS: f64 = 3.0;

/// Point de la chronologie des tempos (`TempoTimePoint`, `tempotimeline.h`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct TempoPoint {
    /// Tick d'exécution **déroulé**, avant insertion des pauses.
    utick: i64,
    /// Battements par seconde en vigueur.
    bps: f64,
    /// Durée de la pause maintenue, ou `0` pour un changement de tempo ordinaire.
    pause: f64,
    /// Tick source de la pause, pour relire l'indication de mesure
    /// (`pausemap.cpp:68-71`). `None` pour un changement de tempo ordinaire.
    pause_source: Option<i64>,
}

/// Pause insérée dans la performance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PauseSpan {
    /// Tick d'exécution du début de pause **avant** décalage (tick déroulé,
    /// sans les pauses).
    pub base_utick: i64,
    /// Tick d'exécution du début de la pause, pauses précédentes comprises.
    pub paused_utick: i64,
    /// Ticks insérés par cette pause.
    pub inserted: i64,
    /// Durée de la pause, en secondes.
    pub seconds: f64,
}

/// Carte des ticks insérés et de tous les changements de tempo.
#[derive(Clone, Debug, Default)]
pub(crate) struct PauseMap {
    /// Tick d'exécution du début de pause → total cumulé de ticks insérés.
    offsets: BTreeMap<i64, i64>,
    /// `m_tempoEvents` : tout changement de tempo, pauses comprises, au tick
    /// d'exécution après insertion des pauses.
    pub(crate) tempo_events: BTreeMap<i64, f64>,
    /// Pauses insérées, dans l'ordre de construction.
    spans: Vec<PauseSpan>,
}

impl PauseMap {
    /// Construit la chronologie, puis applique `PauseMap::calculate`
    /// (`pausemap.cpp:47-76`).
    pub(crate) fn build(
        score: &Score,
        repeat_list: &RepeatList,
        time_signatures: &[(i64, TimeSig)],
    ) -> Self {
        let mut map = Self {
            offsets: BTreeMap::from([(0, 0)]), // « can't start with a pause »
            tempo_events: BTreeMap::new(),
            spans: Vec::new(),
        };
        let timeline = tempo_timeline(score, repeat_list);
        for point in &timeline {
            let utick = point.utick;
            if point.pause <= 0.0 {
                // Changement de tempo ordinaire : `m_tempoEvents[tickWithPauses(utick)] = bps`.
                map.tempo_events
                    .insert(map.tick_with_pauses(utick), point.bps);
                continue;
            }

            // Pause : `PauseMap::calculate` (`pausemap.cpp:66-75`) relit
            // l'indication de mesure au **tick source** de la pause, pas à
            // l'utick déroulé.
            let quarter_notes =
                quarter_notes_at(score, time_signatures, point.pause_source.unwrap_or(0));
            // pausemap.cpp:70 : `int ticksPerMeasure = quarterNotesPerMeasure
            // * Constants::DIVISION;` — le double est TRONQUÉ en int (pas
            // arrondi). Le tempo (`:71`) garde la valeur fractionnaire.
            let ticks_per_measure = (quarter_notes * f64::from(DIVISION)) as i64;
            let before = map.tick_with_pauses(utick);
            map.tempo_events
                .insert(before, quarter_notes / point.pause.max(f64::EPSILON));
            let total = ticks_per_measure + map.offset_at(utick);
            map.offsets.insert(utick, total);
            // Le second `tickWithPauses(utick)` est calculé **après** l'insertion
            // du décalage : le tempo est restauré à la fin de la mesure insérée.
            let after = map.tick_with_pauses(utick);
            map.tempo_events.insert(after, point.bps);
            map.spans.push(PauseSpan {
                base_utick: utick,
                paused_utick: before,
                inserted: after - before,
                seconds: point.pause,
            });
        }
        map
    }

    /// Total des ticks insérés avant `utick` (le tick de pause inclus).
    fn offset_at(&self, utick: i64) -> i64 {
        self.offsets
            .range(..=utick)
            .next_back()
            .map_or(0, |(_, offset)| *offset)
    }

    /// Tick d'exécution réel (`PauseMap::tickWithPauses`).
    pub(crate) fn tick_with_pauses(&self, utick: i64) -> i64 {
        utick + self.offset_at(utick)
    }

    /// Tick d'exécution réel, pauses **strictement** antérieures seulement.
    ///
    /// Sert aux fins de mesures : une pause qui ouvre la mesure suivante n'est
    /// pas comptée dans la fin de la mesure courante, ce qui évite de compter
    /// deux fois la pause à la frontière.
    pub(crate) fn tick_with_pauses_before(&self, utick: i64) -> i64 {
        utick
            + self
                .offsets
                .range(..utick)
                .next_back()
                .map_or(0, |(_, o)| *o)
    }

    /// Pauses insérées, dans l'ordre de construction.
    pub(crate) fn spans(&self) -> &[PauseSpan] {
        &self.spans
    }
}

/// Tempos écrits dans la partition, au tick source (`TempoValues`).
///
/// MuseScore initialise toujours la chronologie à 2 battements/s ; les
/// indications `<Tempo>` (2.x) ou `< Tempo>`/annotations (3.x/4.x) écrasent la
/// valeur à leur tick, la dernière gagnant (`resampleTempoValues`,
/// `tempotimeline.cpp:39-57`).
fn source_tempos(score: &Score) -> BTreeMap<i64, f64> {
    let mut values = BTreeMap::from([(0i64, DEFAULT_BPS)]);
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if let Annotation::Tempo(tempo) = &entry.annotation {
                    if let Some(beats_per_second) = tempo.beats_per_second {
                        values.insert(segment.tick.ticks(UNITS_PER_WHOLE), beats_per_second);
                    }
                }
            }
        }
    }
    values
}

/// Pauses de section, indexées par tick **source** (`Score::rebuildTempoAndTimeSigMaps`,
/// `engraving/dom/score.cpp:522-530` dans 4.7.4 : la rupture est portée par la
/// mesure précédente et la pause est inscrite au tick de la mesure suivante).
/// Le modèle marque déjà la mesure qui suit la rupture (`section_break_before`).
fn raw_pauses(score: &Score) -> BTreeMap<i64, f64> {
    let mut pauses = BTreeMap::new();
    for (index, measure) in score.measures.iter().enumerate() {
        // `can't start with a pause` (`pausemap.cpp:56`) et pas de
        // `measure->prev()` pour la première mesure.
        if index == 0 || !measure.section_break_before {
            continue;
        }
        // `LayoutBreak::pause()` (`layoutbreak.cpp:138-140`) : la propriété
        // `Pid::PAUSE` peut valoir 0, ce qui supprime la pause.
        let seconds = measure.section_pause.unwrap_or(SECTION_PAUSE_SECONDS);
        if seconds <= 0.0 {
            continue;
        }
        pauses.insert(measure.start_tick.ticks(UNITS_PER_WHOLE), seconds);
    }
    // Respiration avec `<pause>` (`rebuildTempoAndTimeSigMaps`,
    // `score.cpp:531-545`) : la plus longue respiration du tick gagne et
    // écrase une éventuelle pause de section au même tick.
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.kind != SegmentType::Breath {
                continue;
            }
            let mut pause_ms = 0u32;
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::Breath(breath) = item {
                    if let Some(value) = breath.pause_ms {
                        pause_ms = pause_ms.max(value);
                    }
                }
            }
            if pause_ms > 0 {
                pauses.insert(
                    segment.tick.ticks(UNITS_PER_WHOLE),
                    f64::from(pause_ms) / 1000.0,
                );
            }
        }
    }
    pauses
}

/// Déroule les pauses brutes comme `PauseMap::calculate` (`pausemap.cpp:62-90`) :
/// chaque segment parcourt `[tick, endTick]` **borne incluse** (`ee =
/// lower_bound(endTick + 1)`) et la pause tombe à `tick + (utick - tick)`.
///
/// Le C++ saute une pause au premier tick du segment courant (« incluse dans
/// le précédent »). Cette hypothèse ne tient pas quand un saut rompt la
/// contiguïté : `Vingt_danses` a une pause à la fin du segment 15 (722880)
/// **et** à l'ouverture du segment 17 (837600), que MuseScore 4.7.4 écrit
/// toutes les deux. On ne saute donc que si le segment précédent finissait
/// réellement sur ce tick.
fn unfold_pauses(score: &Score, repeat_list: &RepeatList) -> Vec<(i64, i64, f64)> {
    let raw = raw_pauses(score);
    let mut result = Vec::new();
    for segment in &repeat_list.segments {
        let end = repeat_list.segment_end(segment);
        let offset = segment.utick - segment.tick;
        for (tick, seconds) in &raw {
            if *tick < segment.tick || *tick > end {
                continue;
            }
            if *tick == segment.tick {
                continue; // déjà émise par le segment précédent
            }
            result.push((tick + offset, *tick, *seconds));
        }
    }
    result.sort_by_key(|(utick, _, _)| *utick);
    result
}

/// Déroule une carte indexée par tick source à travers [`RepeatList`].
fn unfold(values: &BTreeMap<i64, f64>, repeat_list: &RepeatList) -> BTreeMap<i64, f64> {
    let mut result = BTreeMap::new();
    for segment in &repeat_list.segments {
        let end = repeat_list.segment_end(segment);
        let offset = segment.utick - segment.tick;
        for (tick, value) in values {
            if *tick >= segment.tick && *tick < end {
                result.insert(tick + offset, *value);
            }
        }
    }
    result
}

/// Fusion des tempos et des pauses par utick (`TempoTimeline::rebuild`,
/// `tempotimeline.cpp:134-180`). Un point de pause porte le bps du point qui
/// le précède, éventuellement le changement de tempo de ce même tick.
fn tempo_timeline(score: &Score, repeat_list: &RepeatList) -> Vec<TempoPoint> {
    let raw_tempos = source_tempos(score);
    let tempos = unfold(&raw_tempos, repeat_list);
    let pauses = unfold_pauses(score, repeat_list);
    let mut values = tempos.into_iter().peekable();
    let mut rests = pauses.into_iter().peekable();
    let mut points = Vec::new();
    while values.peek().is_some() || rests.peek().is_some() {
        let value_is_next = rests.peek().is_none()
            || (values.peek().is_some()
                && values.peek().as_ref().unwrap().0 <= rests.peek().as_ref().unwrap().0);
        let utick = if value_is_next {
            values.peek().unwrap().0
        } else {
            rests.peek().unwrap().0
        };
        if value_is_next {
            let (at, bps) = values.next().unwrap();
            points.push(TempoPoint {
                utick: at,
                bps: bps.clamp(MIN_BPS, MAX_BPS),
                pause: 0.0,
                pause_source: None,
            });
        }
        if rests.peek().is_some() && rests.peek().as_ref().unwrap().0 == utick {
            let (at, source, seconds) = rests.next().unwrap();
            // `it->second.tempo` (`pausemap.cpp:73`) : le tempo écrit en
            // vigueur au tick source de la pause.
            let bps = raw_tempos
                .range(..=source)
                .next_back()
                .map_or(DEFAULT_BPS, |(_, bps)| *bps)
                .clamp(MIN_BPS, MAX_BPS);
            points.push(TempoPoint {
                utick: at,
                bps,
                pause: seconds,
                pause_source: Some(source),
            });
        }
    }
    points
}

/// Nombre de noires de la mesure qui contient `tick`.
///
/// MuseScore 4.7.4 (référence de conformité) tire la durée de la pause de la
/// **longueur réelle** de la mesure, pas de son indication de mesure : le
/// fichier 2.x `Vingt_danses_enfantines` a des mesures `len="2/4"` qui
/// déclarent `3/4`, et la référence y joue la pause sur 2 noires à 40 BPM.
/// Le code amont 5.0 (`pausemap.cpp:68-71`) lit `sigmap` ; en cas de divergence
/// c'est 4.7.4 qui fait foi (`docs/agents/upstream.md`). À défaut de mesure
/// couvrant le tick, on retombe sur l'indication de mesure.
fn quarter_notes_at(score: &Score, time_signatures: &[(i64, TimeSig)], tick: i64) -> f64 {
    for measure in &score.measures {
        let start = measure.start_tick.ticks(UNITS_PER_WHOLE);
        if start <= tick && tick < start + measure.len.ticks(UNITS_PER_WHOLE) {
            let quarters = measure.len.ticks(UNITS_PER_WHOLE) as f64 / f64::from(DIVISION);
            if quarters > 0.0 {
                return quarters;
            }
        }
    }
    quarter_notes_per_measure(time_signatures, tick)
}

/// Nombre de noires par mesure de l'indication de mesure en vigueur.
fn quarter_notes_per_measure(time_signatures: &[(i64, TimeSig)], tick: i64) -> f64 {
    let time_sig = time_signatures
        .iter()
        .rev()
        .find(|(at, _)| *at <= tick)
        .map_or(4.0, |(_, time_sig)| {
            if time_sig.denominator > 0 {
                4.0 * f64::from(time_sig.numerator) / f64::from(time_sig.denominator)
            } else {
                4.0
            }
        });
    time_sig.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repeat_list::RepeatList;
    use scoretk_core::model::{
        BarLineType, Breath, BreathType, ChordRest, Duration, DurationType, Fermata, FermataType,
        Fraction, Measure, Placement, RepeatMark, Segment, SegmentItem, SegmentType, TimeSig,
        Track,
    };

    /// Partition de deux rondes 4/4, la première portant un point d'orgue et
    /// une césure.
    fn score_avec_point_d_orgue_et_cesure(quarter_notes: Fraction) -> Score {
        let mut score = Score::new();
        let whole = quarter_notes;
        for number in 0..2 {
            let mut measure =
                Measure::new(number, whole * Fraction::from_integer(number as i64), whole);
            let chord_rest = ChordRest::rest(Duration::new(DurationType::Whole));
            let start = measure.start_tick;
            let mut segment = Segment::new(SegmentType::ChordRest, start, 1);
            segment.set_element(Track::from_raw(0), SegmentItem::ChordRest(chord_rest));
            if number == 0 {
                segment.add_annotation(
                    Track::from_raw(0),
                    Annotation::Fermata(Fermata {
                        kind: FermataType::Normal,
                        placement: Placement::Above,
                        visible: true,
                    }),
                );
            }
            measure.add_segment(segment);
            if number == 0 {
                let mut breath = Segment::new(SegmentType::Breath, measure.end_tick(), 1);
                breath.set_element(
                    Track::from_raw(0),
                    SegmentItem::Breath(Breath {
                        kind: BreathType::CaesuraCurved,
                        placement: Placement::Above,
                        pause_ms: None,
                    }),
                );
                measure.add_segment(breath);
            }
            score.add_measure(measure);
        }
        score
    }

    /// Ni le point d'orgue ni la césure n'allongent le rendu MIDI : aucun
    /// `PausesMap` n'est construit pour eux dans le portage (le module
    /// *playback* amont manque). La chronologie contient en revanche le tempo
    /// par défaut, comme `resampleTempoValues` (`tempotimeline.cpp:44-46`).
    #[test]
    fn points_d_orgue_et_respirations_n_allongent_pas_le_rendu() {
        let score = score_avec_point_d_orgue_et_cesure(Fraction::ONE);
        let repeat_list = RepeatList::unwind(&score);
        let map = PauseMap::build(&score, &repeat_list, &[]);
        assert!(map.spans().is_empty());
        assert_eq!(map.tempo_events, BTreeMap::from([(0, 2.0)]));
        assert_eq!(map.tick_with_pauses(1920), 1920);
        assert_eq!(repeat_list.ticks(), 3840);
    }

    /// Une mesure de 5/8 (2,5 noires) garde sa longueur fractionnaire : la
    /// pause insère 2,5 × 480 = 1200 ticks, sans troncature à 2 noires.
    #[test]
    fn pause_en_cinq_huit_sans_troncature() {
        let mut score = score_avec_point_d_orgue_et_cesure(Fraction::new(5, 8));
        score.measures[1].section_break_before = true;
        let repeat_list = RepeatList::unwind(&score);
        let time_sigs = [(0, TimeSig::new(5, 8))];
        let map = PauseMap::build(&score, &repeat_list, &time_sigs);
        assert_eq!(map.spans().len(), 1);
        assert_eq!(map.spans()[0].inserted, 1200);
        assert_eq!(map.tempo_events.get(&1200), Some(&(2.5 / 3.0)));
        assert_eq!(map.tempo_events.get(&2400), Some(&2.0));
    }

    /// La première mesure n'a pas de `measure->prev()` : un marqueur de
    /// rupture posé dessus n'insère aucune pause (`collectPauses`, `:1105`).
    #[test]
    fn premiere_mesure_sans_pause_de_section() {
        let mut score = score_avec_point_d_orgue_et_cesure(Fraction::ONE);
        score.measures[0].section_break_before = true;
        let repeat_list = RepeatList::unwind(&score);
        let map = PauseMap::build(&score, &repeat_list, &[]);
        assert!(map.spans().is_empty(), "aucune pause avant la mesure 0");
        assert_eq!(map.tick_with_pauses(1920), 1920);
    }

    /// Un saut de section insère une mesure entière de ticks. La pause est
    /// ancrée sur la mesure qui ouvre la nouvelle section.
    #[test]
    fn un_saut_de_section_insere_toujours_une_pause() {
        // Une mesure de 2/4 (comme celles du piano 2.x) : la pause dure 2
        // noires, soit 960 ticks, et son tempo est 2 / 3 bps (40 BPM).
        let mut score = score_avec_point_d_orgue_et_cesure(Fraction::new(1, 2));
        score.measures[1].section_break_before = true;
        score.measures[1].start_bar_line = BarLineType::StartRepeat;
        let repeat_list = RepeatList::unwind(&score);
        let time_sigs = [(0, TimeSig::new(3, 4))];
        let map = PauseMap::build(&score, &repeat_list, &time_sigs);
        assert_eq!(map.spans().len(), 1);
        assert_eq!(map.spans()[0].inserted, 960);
        assert_eq!(map.spans()[0].paused_utick, 960);
        // quarterNotes (2) / 3 s = 2/3 bps à l'ouverture, retour à 2 bps après
        // les 960 ticks insérés (pausemap.cpp:72-74). L'indication 3/4 est
        // ignorée : c'est la longueur réelle de la mesure qui fait foi (4.7.4).
        assert_eq!(map.tempo_events.get(&960), Some(&(2.0 / 3.0)));
        assert_eq!(map.tempo_events.get(&1920), Some(&2.0));
    }

    /// Une respiration avec `<pause>0.5</pause>` insère une mesure de ticks
    /// au tempo `quarterNotesPerMeasure / pause` (`El_Choclo` : césure de
    /// 0,5 s avant une mesure de 3/8, 720 ticks à 180 BPM).
    #[test]
    fn respiration_avec_pause_insere_une_mesure_de_ticks() {
        let mut score = score_avec_point_d_orgue_et_cesure(Fraction::ONE);
        score.measures[1].len = Fraction::new(3, 8);
        let breath = score.measures[0]
            .segments
            .iter_mut()
            .find(|segment| segment.kind == SegmentType::Breath)
            .expect("respiration");
        breath.set_element(
            Track::from_raw(0),
            SegmentItem::Breath(Breath {
                kind: BreathType::CaesuraCurved,
                placement: Placement::Above,
                pause_ms: Some(500),
            }),
        );
        let repeat_list = RepeatList::unwind(&score);
        let map = PauseMap::build(&score, &repeat_list, &[]);
        assert_eq!(map.spans().len(), 1, "pauses {:?}", map.spans());
        assert_eq!(map.spans()[0].base_utick, 1920);
        assert_eq!(map.spans()[0].inserted, 720);
        assert_eq!(map.tempo_events.get(&1920), Some(&3.0));
        assert_eq!(map.tempo_events.get(&2640), Some(&2.0));
    }

    /// Une pause de section explicitement nulle (`<pause>0</pause>`,
    /// `Dominus_fredipi`) n'insère ni tick ni changement de tempo.
    #[test]
    fn pause_de_section_nulle_n_insere_rien() {
        let mut score = score_avec_point_d_orgue_et_cesure(Fraction::ONE);
        score.measures[1].section_break_before = true;
        score.measures[1].section_pause = Some(0.0);
        let repeat_list = RepeatList::unwind(&score);
        let map = PauseMap::build(&score, &repeat_list, &[]);
        assert!(map.spans().is_empty());
        assert_eq!(map.tick_with_pauses(1920), 1920);
    }

    fn marqueur(label: &str) -> scoretk_core::model::Marker {
        scoretk_core::model::Marker {
            kind: scoretk_core::model::MarkerType::Segno,
            label: label.into(),
            text: label.into(),
            placement: Placement::Above,
            track: Track::from_raw(0),
            visible: true,
        }
    }

    /// Saut dont la mesure porte aussi la rupture de section : MuseScore 4.7.4
    /// joue la pause **au moment du saut** (fin du segment qui contient la
    /// mesure du saut), pas à l'ouverture de la section suivante.
    ///
    /// Réduction du cas `Vingt_danses` : le fichier réduit
    /// `jump-ref/jump.mscx` exporté par 4.7.4 donne six mesures de 4/4, une
    /// pause de 1920 ticks à 80 BPM (4/3 bps) à 7680, puis la cible du saut.
    #[test]
    fn pause_du_saut_au_moment_du_saut() {
        let mut score = Score::new();
        let whole = Fraction::ONE;
        for number in 0..6 {
            let mut measure =
                Measure::new(number, whole * Fraction::from_integer(number as i64), whole);
            match number {
                0 => measure.repeat_marks = vec![RepeatMark::Marker(marqueur("A"))],
                2 => measure.repeat_marks = vec![RepeatMark::Marker(marqueur("B"))],
                3 => {
                    measure.repeat_marks = vec![RepeatMark::Jump(scoretk_core::model::Jump::new(
                        "D.C.",
                        "A",
                        "B",
                        "C",
                        Placement::Above,
                        Track::from_raw(0),
                    ))]
                }
                4 => {
                    measure.section_break_before = true;
                    measure.repeat_marks = vec![RepeatMark::Marker(marqueur("C"))];
                }
                _ => {}
            }
            score.add_measure(measure);
        }
        let repeat_list = RepeatList::unwind(&score);
        let map = PauseMap::build(&score, &repeat_list, &[]);
        assert_eq!(map.spans().len(), 1, "pauses {:?}", map.spans());
        assert_eq!(map.spans()[0].base_utick, 4 * 1920);
        assert_eq!(map.spans()[0].inserted, 1920);
        assert_eq!(map.tempo_events.get(&(4 * 1920)), Some(&(4.0 / 3.0)));
        assert_eq!(map.tempo_events.get(&(5 * 1920)), Some(&2.0));
    }
}
