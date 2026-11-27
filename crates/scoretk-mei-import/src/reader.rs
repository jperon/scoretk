// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` de MuseScore Studio
// 4.7.4, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026. Les noms des méthodes suivent le C++ (`readScore`, `readScoreDef`,
// `readMeasure`, `readLayers`, `readElements`, `addChordRest`, …).
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

//! Structure du lecteur : `readScore`, `readScoreDef`, `readMeasure`.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{
    Accidental, BarLineType, BeamMode, ChordRest, ChordRestKind, Clef, ClefType, Duration,
    Fraction, Key, KeyMode, Layers, Measure, Note, NoteId, Placement, Segment, SegmentItem,
    SegmentType, StaffId, TimeSig, TimeSigType, Track, Transpose, Tuplet, TupletId, VOICES,
};
use scoretk_mei_common::{convert, ids};
use scoretk_xml_common::Element;

use crate::{first_descendant, Error};

/// Key of global `scoreDef` values in the signature maps.
const SCOREDEF_IDX: usize = usize::MAX;

/// Spanner en attente d'une extrémité (`addSpanner` / `findEnd`).
#[derive(Clone, Debug)]
pub(crate) struct PendingSpanner {
    pub(crate) kind: scoretk_core::model::SpannerType,
    pub(crate) number: u8,
    pub(crate) start: (Fraction, Track),
    pub(crate) end_id: Option<String>,
    pub(crate) end_tstamp: Option<f64>,
    pub(crate) measure: usize,
    /// `@curvedir` d'un slur, conservé jusqu'à la résolution de `@endid`.
    pub(crate) placement: Option<Placement>,
}

pub(crate) struct Reader<'a> {
    pub(crate) score: scoretk_core::model::Score,
    pub(crate) root: &'a Element,
    /// `staff@n` → index de portée.
    pub(crate) staff_by_n: BTreeMap<i32, usize>,
    pub(crate) part_by_staff: BTreeMap<i32, usize>,
    pub(crate) clefs: BTreeMap<usize, ClefType>,
    pub(crate) timesigs: BTreeMap<usize, (i32, i32, TimeSigType)>,
    pub(crate) keysigs: BTreeMap<usize, Key>,
    pub(crate) transpose: BTreeMap<usize, Transpose>,
    /// `xml:id` → note, pour les liaisons.
    pub(crate) id_notes: BTreeMap<String, NoteId>,
    /// `xml:id` → (mesure, piste, tick), pour les événements de contrôle.
    pub(crate) id_events: BTreeMap<String, (usize, Track, Fraction)>,
    pub(crate) pending: Vec<PendingSpanner>,
    /// Ties whose end note is in a later measure, with `@curvedir`.
    pub(crate) pending_ties: Vec<(String, String, Option<bool>)>,
    pub(crate) current_time: Fraction,
    /// Rang du `<measure>` MEI courant (indépendant des mesures recréées pour
    /// un multi-repos) : `@n` compte les mesures écrites, pas le modèle.
    pub(crate) mei_measure_ordinal: usize,
    /// Dernier `@n` MEI lu, pour mesurer le saut de numérotation d'un
    /// multi-repos (`Convert::measureToMEI`, meiconverter.cpp:2254).
    pub(crate) previous_measure_n: i64,
    /// Ratios des n-olets ouverts, du plus externe au plus interne : l'avance
    /// d'un événement est divisée par leur produit (`Tuplet::parent`).
    pub(crate) tuplet_ratios: Vec<Fraction>,
    /// Identifiants des n-olets ouverts ; le dernier est le plus interne.
    pub(crate) tuplet_stack: Vec<TupletId>,
    pub(crate) beam_mode: BeamMode,
    /// Alternates the two chords of an `<fTrem>` (`Start`, then `Stop`).
    pub(crate) tremolo_phase: u8,
    /// `<graceGrp>` lu avant l'événement principal, attaché au prochain
    /// accord/rest (`GraceChord` n'a pas de segment propre dans le modèle).
    pub(crate) pending_graces: Vec<scoretk_core::model::GraceChord>,
    /// Signatures must be written in the next measure (initial `scoreDef` or
    /// a `scoreDef` change inside a section).
    pub(crate) signatures_pending: bool,
    /// Dénominateur *non réduit* du mètre de chaque mesure (par index de
    /// mesure). MEI 5.1 compte `@tstamp` en positions métriques : dans une
    /// mesure à `2/4`, `tstamp="2"` est le deuxième temps, soit `1/4` de ronde
    /// (*Guidelines* §4.2.6). Le modèle réduit `2/4` en `1/2` et ne garde que
    /// le numérateur réduit dans `Measure::time_sig_numerator` ; sans cette
    /// table le lecteur ne peut pas retrouver le dénominateur écrit.
    pub(crate) beat_denominators: Vec<i64>,
}

impl<'a> Reader<'a> {
    /// `MeiImporter::read` → `readScore`.
    pub(crate) fn read(root: &'a Element) -> Result<scoretk_core::model::Score, Error> {
        let Some(score_node) = first_descendant(root, "score") else {
            return Err(Error::NoScore);
        };
        let mut reader = Self {
            score: scoretk_core::model::Score::new(),
            root,
            staff_by_n: BTreeMap::new(),
            part_by_staff: BTreeMap::new(),
            clefs: BTreeMap::new(),
            timesigs: BTreeMap::new(),
            keysigs: BTreeMap::new(),
            transpose: BTreeMap::new(),
            id_notes: BTreeMap::new(),
            id_events: BTreeMap::new(),
            pending: Vec::new(),
            pending_ties: Vec::new(),
            current_time: Fraction::new(4, 4),
            mei_measure_ordinal: 0,
            previous_measure_n: 0,
            tuplet_ratios: Vec::new(),
            tuplet_stack: Vec::new(),
            beam_mode: BeamMode::Auto,
            tremolo_phase: 0,
            pending_graces: Vec::new(),
            signatures_pending: true,
            beat_denominators: Vec::new(),
        };
        reader.build_staff_layer_map(score_node);
        reader.read_mei_head();
        reader.read_score(score_node)?;
        reader.add_spanner_ends();
        crate::barlines::finalize(&mut reader.score);
        Ok(reader.score)
    }

    /// `MeiImporter::buildStaffLayerMap` (`meiimporter.cpp:3309`).
    ///
    /// Le C++ collecte `//staff/@n` dans un `std::set` **ordonné**, puis
    /// `getStaffIndex` (`:211`) renvoie la position de `@n` dans cet ensemble.
    /// L'ordre des `<staffDef>` (qui peuvent être regroupés par `staffGrp`
    /// dans un ordre différent, par exemple dans l'export de scoretk) n'est
    /// donc pas l'index de portée : utiliser l'ordre des `staffDef` décalait
    /// toutes les notes d'un fichier MEI externe dont les parties ne sont pas
    /// dans l'ordre numérique des portées.
    fn build_staff_layer_map(&mut self, score_node: &Element) {
        let mut staff_ns: BTreeSet<i32> = BTreeSet::new();
        for staff in descendants_named(score_node, "staff") {
            let n = staff
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(1);
            staff_ns.insert(n);
        }
        for (index, n) in staff_ns.into_iter().enumerate() {
            self.staff_by_n.insert(n, index);
        }
    }

    /// `MeiImporter::readScore` (`meiimporter.cpp:1011`).
    fn read_score(&mut self, score_node: &Element) -> Result<(), Error> {
        let score_def = score_node.child("scoreDef");
        if let Some(score_def) = score_def {
            self.read_score_def(score_def, true);
        }
        for section in score_node.children_named("section") {
            self.read_section_elements(section);
        }
        Ok(())
    }

    /// `MeiImporter::readSectionElements` (`meiimporter.cpp:1346`).
    pub(crate) fn read_section_elements(&mut self, parent: &Element) {
        for child in &parent.children {
            match child.name.as_str() {
                "scoreDef" => {
                    // MEI 5.1 permet le `<scoreDef>` initial à l'intérieur
                    // d'une `<section>` (exemples officiels `beamSpans`,
                    // `slur_element`, `Editorial_markup`). Tant qu'aucune
                    // portée n'a été construite, c'est cette définition qui
                    // porte les `staffDef` : la traiter comme non initiale
                    // laissait `score.staves` vide et perdait notes, stems
                    // et silences. Un `<scoreDef>` ultérieur reste un
                    // changement local.
                    let initial = self.score.staves.is_empty();
                    self.read_score_def(child, initial);
                }
                "section" => self.read_section_elements(child),
                "measure" => {
                    self.read_measure(child);
                    // MuseScore 4.7.4 can write a measure inside another one;
                    // read those too, as the reference model has them all.
                    for nested in child.children_named("measure") {
                        self.read_measure(nested);
                    }
                }
                "ending" => {
                    // MuseScore writes the ending numbers as `@type`
                    // values (`Convert::endingToMEI`, meiconverter.cpp:1448).
                    let typed = convert::get_type_values_with_prefix(
                        child.attribute("type").unwrap_or_default(),
                        scoretk_mei_common::ids::ENDING_REPEAT_TYPE,
                    )
                    .filter_map(|value| value.parse::<u8>().ok())
                    .collect::<Vec<_>>();
                    let endings = if typed.is_empty() {
                        child
                            .attribute("n")
                            .unwrap_or("1")
                            .split(',')
                            .filter_map(|value| value.trim().parse::<u8>().ok())
                            .collect::<Vec<_>>()
                    } else {
                        typed
                    };
                    // `MeiImporter::readSectionElements` traite aussi les
                    // `<scoreDef>` (et `<pb>`/`<sb>`) contenus dans un
                    // `<ending>` : MuseScore 4.7.4 y écrit par exemple un
                    // changement de metre (`measureToMEI` garde `@n` et
                    // `writeEnding` englobe la mesure). Ne lire que les
                    // mesures decorale la mesure suivante d'un quart de temps.
                    let first = self.score.measures.len();
                    self.read_section_elements(child);
                    let last = self.score.measures.len();
                    if last > first {
                        let start = self.score.measures[first].start_tick;
                        let end = self.score.measures[last - 1].end_tick();
                        // MuseScore attaches the volta to the staff of the
                        // first measure; keep the same anchor.
                        let track = Track::new(StaffId::from_index(0), 0);
                        let mut spanner = scoretk_core::model::Spanner::new(
                            scoretk_core::model::SpannerType::Volta,
                            scoretk_core::model::SpannerAnchor::new(start, track),
                            scoretk_core::model::SpannerAnchor::new(end, track),
                        );
                        spanner.volta = Some(scoretk_core::model::VoltaInfo {
                            endings,
                            closed: true,
                        });
                        self.score.add_spanner(spanner);
                    }
                }
                "pb" if child.attribute("type") == Some("mscore-manual") => {
                    if let Some(measure) = self.score.measures.last_mut() {
                        measure.section_break_before = true;
                    }
                }
                "sb" if child.attribute("type") == Some("mscore-manual") => {
                    if let Some(measure) = self.score.measures.last_mut() {
                        measure.line_break = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// `MeiImporter::readMeasure` (`meiimporter.cpp:1434`).
    fn read_measure(&mut self, node: &Element) {
        let info = convert::measure_from_mei(
            node.attribute("n"),
            node.attribute("left"),
            node.attribute("right"),
            node.attribute("type"),
        );
        let ordinal = self.mei_measure_ordinal;
        self.mei_measure_ordinal += 1;
        let index = self.score.measures.len();
        let (numerator, denominator, kind) = self.current_timesig();
        debug_assert_eq!(index, self.beat_denominators.len());
        self.beat_denominators.push(denominator);
        let nominal = Fraction::try_new(numerator, denominator).unwrap_or(Fraction::new(4, 4));
        let start = self.score.end_tick();
        let mut measure = Measure::new(index, start, nominal);
        // `Measure::new` stocke le numérateur **réduit** (`4/4` → `1`), comme
        // le lecteur `.mscx` de MuseScore. L'ancienne affectation du `@n`
        // écrit (`4`) rendait `Convert::tstampFromFraction` et son inverse
        // asymétriques : un `@tstamp` écrit avec le dénominateur réduit était
        // relu avec le dénominateur non réduit (par exemple 1.750 relu comme
        // 19/16 au lieu de 7/4 dans `v3/testWedge4`).
        measure.time_sig_numerator = nominal.numerator();
        measure.number_offset = if info.irregular {
            0
        } else {
            info.n as i32 - 1 - ordinal as i32
        };
        if info.irregular {
            measure.exclude_from_numbering = true;
        }
        measure.start_bar_line = if info.repeat_start {
            scoretk_core::model::BarLineType::StartRepeat
        } else {
            scoretk_core::model::BarLineType::Normal
        };
        measure.end_bar_line = if info.repeat_end {
            scoretk_core::model::BarLineType::EndRepeat
        } else {
            info.end_bar_line
        };
        measure.repeat_count = info.repeat_count;

        // Initial signatures of every staff, only in the first measure after
        // a scoreDef (MuseScore writes them once, not in every measure).
        if self.signatures_pending {
            let staff_count = self.score.staves.len().max(1);
            for staff_idx in 0..staff_count {
                if let Some(clef) = self.clefs.get(&staff_idx).copied() {
                    Self::insert_segment(
                        &mut measure,
                        staff_count,
                        start,
                        SegmentType::Clef,
                        staff_idx,
                        SegmentItem::Clef(Clef::new(clef)),
                    );
                }
                // `Convert::keyFromMEI` (meiconverter.cpp:2066) ne construit
                // qu'une `engraving::Key` (nombre de quintes) : aucun mode
                // n'est porté par @keysig, donc le mode reste inconnu, comme
                // dans le modèle .mscx. Do majeur est implicite quand ni
                // `<staffDef>` ni `<scoreDef>` n'écrit @keysig.
                let key = self
                    .keysigs
                    .get(&staff_idx)
                    .or_else(|| self.keysigs.get(&SCOREDEF_IDX))
                    .copied()
                    .unwrap_or(Key::C);
                Self::insert_segment(
                    &mut measure,
                    staff_count,
                    start,
                    SegmentType::KeySig,
                    staff_idx,
                    SegmentItem::KeySig(scoretk_core::model::KeySig {
                        mode: KeyMode::Unknown,
                        ..scoretk_core::model::KeySig::new(key)
                    }),
                );
                // `staffDef @meter.count/@meter.unit` (`att.staffDef.log`)
                // peut donner à une portée sa propre mesure locale ; le
                // `<scoreDef>` global ne s'applique qu'aux autres portées.
                let (sig_num, sig_den, sig_kind) = self
                    .timesigs
                    .get(&staff_idx)
                    .or_else(|| self.timesigs.get(&SCOREDEF_IDX))
                    .copied()
                    .unwrap_or((numerator as i32, denominator as i32, kind));
                Self::insert_segment(
                    &mut measure,
                    staff_count,
                    start,
                    SegmentType::TimeSig,
                    staff_idx,
                    SegmentItem::TimeSig(TimeSig {
                        numerator: sig_num,
                        denominator: sig_den,
                        kind: sig_kind,
                        ..TimeSig::new(sig_num, sig_den)
                    }),
                );
            }
            self.signatures_pending = false;
        }

        self.score.add_measure(measure);
        self.read_staves(node, index);
        self.read_control_events(node, index);
        // Several staves may share the last tick: the measure length is the
        // longest voice, not the sum of all tracks. A control event anchored
        // at the last beat creates a `ChordRest` segment holding only an
        // annotation: such a segment must not extend the measure. MEI 5.1
        // places control events after the staves (`format-ref/mei.md` §2/§10)
        // and their position is independent of the sounding events.
        let len = self.score.measures[index]
            .segments
            .iter()
            .rfind(|segment| {
                segment
                    .iter_elements()
                    .any(|(_, item)| item.as_chord_rest().is_some())
            })
            .map_or(Fraction::ZERO, |segment| {
                segment
                    .iter_elements()
                    .filter_map(|(_, item)| item.as_chord_rest())
                    .map(|cr| cr.ticks_in_measure(nominal))
                    .max()
                    .unwrap_or(Fraction::ZERO)
                    + (segment.tick - start)
            });
        // The MuseScore `.mscx` reader defaults `Measure::len` to the nominal
        // length; only an irregular measure (`@metcon="false"` or no `@n`)
        // carries its real length. Match that so ticks stay aligned.
        let irregular = info.irregular || node.attribute("metcon") == Some("false");
        let gap = info.n.saturating_sub(ordinal + 1) as i64;
        let jump = if info.n > 0 && self.previous_measure_n > 0 {
            info.n as i64 - self.previous_measure_n
        } else {
            1
        };
        if info.n > 0 {
            self.previous_measure_n = info.n as i64;
        }
        // `@n` saute les mesures couvertes ; l'écart avec le rang MEI et le
        // saut de numérotation donnent deux bornes du nombre de mesures
        // absentes. La plus large restaure le plus de mesures sans décaler
        // les suivantes (v4/mmrest, v3/testTboxWords1).
        let hidden = gap.max(jump).max(1) as usize;
        let explicit_rest = self.score.measures[index].multi_measure_rest.is_some();
        if explicit_rest {
            // Un `<multiRest @n>` écrit par scoretk : la mesure ne saute pas
            // son `@n`, mais porte explicitement le nombre de mesures couvertes.
            self.score.measures[index].len = nominal;
        } else if (node.attribute("metcon") == Some("false") || jump > 1) && hidden > 1 {
            // `Convert::measureToMEI` (meiconverter.cpp:2254) writes a single
            // `<measure>` for a whole multi-measure rest, and `writeScore`
            // (meiexporter.cpp:292) iterates layout measures: the covered
            // measures are absent from the reference. `@n` is the last
            // displayed number of the group, so `gap` measures are hidden.
            // Recreate them explicitly with a full-measure rest each, as the
            // `.mscx` model has them, while keeping the group's total length.
            let end_bar_line = self.score.measures[index].end_bar_line;
            let repeat_count = self.score.measures[index].repeat_count;
            self.score.measures[index].multi_measure_rest = Some(hidden as u16);
            self.score.measures[index].len = nominal;
            self.score.measures[index].end_bar_line = BarLineType::Normal;
            self.score.measures[index].repeat_count = 0;
            self.expand_mmrest(index, nominal, hidden, end_bar_line, repeat_count);
        } else {
            self.score.measures[index].len = if irregular && len > Fraction::ZERO {
                len
            } else {
                nominal
            };
        }
    }

    /// Recrée les mesures couvertes par un multi-repos (`@n` qui saute).
    fn expand_mmrest(
        &mut self,
        index: usize,
        nominal: Fraction,
        count: usize,
        end_bar_line: BarLineType,
        repeat_count: u8,
    ) {
        let start = self.score.measures[index].start_tick;
        let staff_count = self.score.staves.len().max(1);
        let shift = nominal * Fraction::from_integer((count - 1) as i64);
        // Le contenu écrit dans la mesure MEI appartient à la *dernière*
        // mesure du groupe : MuseScore place le multi-repos au début et les
        // événements visibles à la fin (`v3/testTboxWords1`).
        let mut moved = Vec::new();
        {
            let first = &mut self.score.measures[index];
            let mut kept = Vec::new();
            for mut segment in std::mem::take(&mut first.segments) {
                if segment.kind == SegmentType::ChordRest || segment.tick > start {
                    segment.tick += shift;
                    moved.push(segment);
                } else {
                    kept.push(segment);
                }
            }
            first.segments = kept;
        }
        for (event_measure, _, tick) in self.id_events.values_mut() {
            if *event_measure == index && *tick >= start && *tick < start + shift {
                *tick += shift;
            }
        }
        for offset in 1..count {
            let tick = start + nominal * Fraction::from_integer(offset as i64);
            let mut measure = Measure::new(index + offset, tick, nominal);
            measure.len = nominal;
            if offset + 1 == count {
                measure.end_bar_line = end_bar_line;
                measure.repeat_count = repeat_count;
                for segment in moved.drain(..) {
                    measure.add_segment(segment);
                }
            }
            if !Self::has_chord_rest(&measure, tick) {
                Self::insert_measure_rest(&mut measure, tick, staff_count, nominal);
            }
            self.score.add_measure(measure);
        }
        let denominator = self.beat_denominators[index];
        self.beat_denominators
            .extend(std::iter::repeat_n(denominator, count - 1));
        if !Self::has_chord_rest(&self.score.measures[index], start) {
            Self::insert_measure_rest(&mut self.score.measures[index], start, staff_count, nominal);
        }
    }

    fn has_chord_rest(measure: &Measure, tick: Fraction) -> bool {
        measure.segments.iter().any(|segment| {
            segment.tick == tick
                && segment
                    .element(Track::new(StaffId::from_index(0), 0))
                    .and_then(SegmentItem::as_chord_rest)
                    .is_some()
        })
    }

    /// Un silence de mesure entière sur la première portée, comme dans le
    /// modèle `.mscx` (`Rest` avec `durationType=measure`).
    fn insert_measure_rest(
        measure: &mut Measure,
        tick: Fraction,
        staff_count: usize,
        nominal: Fraction,
    ) {
        let mut segment = Segment::new(SegmentType::ChordRest, tick, staff_count);
        let mut rest = ChordRest::rest(Duration::measure());
        rest.ticks = Some(nominal);
        segment.set_element(
            Track::new(StaffId::from_index(0), 0),
            SegmentItem::ChordRest(rest),
        );
        measure.add_segment(segment);
    }

    /// Insert (or complete) a segment at `tick` for `staff_idx`.
    pub(crate) fn insert_segment(
        measure: &mut Measure,
        staff_count: usize,
        tick: Fraction,
        kind: SegmentType,
        staff_idx: usize,
        item: SegmentItem,
    ) {
        let position = measure
            .segments
            .iter()
            .position(|segment| segment.tick == tick && segment.kind == kind);
        let track = Track::new(StaffId::from_index(staff_idx), 0);
        if let Some(position) = position {
            measure.segments[position].set_element(track, item);
        } else {
            let mut segment = Segment::new(kind, tick, staff_count.max(1));
            segment.set_element(track, item);
            measure.add_segment(segment);
        }
    }

    /// `MeiImporter::readStaves` (`meiimporter.cpp:1544`).
    fn read_staves(&mut self, parent: &Element, measure: usize) {
        for (order, staff) in parent.children_named("staff").enumerate() {
            let n = staff
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(order as i32 + 1);
            let staff_idx = self.staff_by_n.get(&n).copied().unwrap_or(order);
            for layer in staff.children_named("layer") {
                let layer_n = layer
                    .attribute("n")
                    .and_then(|value| value.parse::<u8>().ok())
                    .unwrap_or(1);
                if layer_n == 0 || layer_n as usize > VOICES {
                    self.score
                        .report
                        .record(Layers::EXTENSIONS, "layer-beyond-voices", None, None);
                    continue;
                }
                let track = Track::new(StaffId::from_index(staff_idx), layer_n - 1);
                let mut ticks = Fraction::ZERO;
                self.read_elements(layer, measure, track, &mut ticks);
            }
        }
    }

    /// `MeiImporter::readElements` (`meiimporter.cpp:1654`).
    pub(crate) fn read_elements(
        &mut self,
        parent: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        for node in &parent.children {
            match node.name.as_str() {
                "beam" => self.read_beam(node, measure, track, ticks),
                "chord" => {
                    self.read_chord(node, measure, track, ticks);
                }
                "note" => {
                    self.read_note(node, measure, track, ticks);
                }
                "rest" => self.read_rest(node, measure, track, ticks),
                "space" => self.read_space(node, measure, track, ticks),
                "mRest" => self.read_m_rest(node, measure, track, ticks),
                "multiRest" => self.read_multi_rest(node, measure, track, ticks),
                "mRpt" => self.read_m_rpt(node, measure, track, ticks),
                "tuplet" => self.read_tuplet(node, measure, track, ticks),
                "clef" => self.read_clef_element(node, measure, track, ticks),
                "barLine" => self.read_bar_line(node, measure),
                "meterSig" => self.read_meter_sig(node, measure, track, ticks),
                "bTrem" | "fTrem" => self.read_tremolo(node, measure, track, ticks),
                "graceGrp" => self.read_grace_group(node, measure, track, ticks),
                "dir"
                    if node.attribute("type").is_some_and(|typ| {
                        typ.starts_with(ids::CHORD_LINE_TYPE)
                            || typ.starts_with(ids::TREMOLO_BAR_TYPE)
                    }) =>
                {
                    // Les deux porteurs additionnels sont écrits comme des
                    // `<dir>` frères de l'événement dans la couche. Les autres
                    // événements de contrôle suivent le chemin 4.7.4.
                    self.read_control_event(node, node, measure);
                }
                "accid" | "artic" | "arpeg" | "fermata" | "fing" | "dir" | "dynam" | "reh"
                | "harm" | "trill" | "mordent" | "turn" | "ornam" => {
                    self.read_control_events(node, measure);
                }
                _ => {}
            }
        }
    }

    /// `MeiImporter::readBeam` (`meiimporter.cpp:1784`). The 4.7.4 importer
    /// relies on automatic beaming for a `<beam>` container; only the
    /// `mscore-beam-*` `@type` values set an explicit mode.
    fn read_beam(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let previous = self.beam_mode;
        self.read_elements(node, measure, track, ticks);
        self.beam_mode = previous;
    }

    /// `MeiImporter::addChordRest` (`meiimporter.cpp:242`).
    pub(crate) fn add_chord_rest(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: Fraction,
        duration: Duration,
        is_rest: bool,
    ) -> usize {
        let start = self.score.measures[measure].start_tick;
        let absolute_for_index = start + ticks;
        let mut cr = if is_rest {
            ChordRest::rest(duration)
        } else {
            ChordRest::chord(duration)
        };
        if !self.pending_graces.is_empty() {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.grace_chords = std::mem::take(&mut self.pending_graces);
            }
        }
        cr.ticks = Some(gestural_duration_of(node).unwrap_or(duration).ticks());
        // L'événement appartient au n-olet le plus interne (`Tuplet::parent`
        // dans le modèle). Les n-olets externes ne reçoivent que les
        // événements qui ne sont pas déjà dans un n-olet imbriqué.
        cr.tuplet = self.tuplet_stack.last().copied();
        if let Some(stem) = convert::stem_from_mei(node.attribute("stem.dir").unwrap_or_default()) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.engraving.stem = stem;
            }
        }
        let beam =
            convert::beam_from_mei(node.attribute("type").unwrap_or_default(), "mscore-beam");
        if beam != BeamMode::Auto {
            cr.engraving.beam_mode = beam;
        } else if let Some(breaksec) = node
            .attribute("breaksec")
            .and_then(|v| v.parse::<i32>().ok())
        {
            cr.engraving.beam_mode = convert::breaksec_from_mei(breaksec);
        }
        let absolute = start + ticks;
        let segment = Segment::new(
            SegmentType::ChordRest,
            absolute,
            self.score.staves.len().max(1),
        );
        let mut segment = segment;
        segment.set_element(track, SegmentItem::ChordRest(cr));
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|existing| {
                existing.tick == absolute && existing.kind == SegmentType::ChordRest
            });
        let position = match position {
            Some(position) => {
                self.score.measures[measure].segments[position].set_element(
                    track,
                    segment.element(track).cloned().expect("new chord rest"),
                );
                position
            }
            None => {
                self.score.measures[measure].add_segment(segment);
                self.score.measures[measure]
                    .segments
                    .iter()
                    .position(|existing| {
                        existing.tick == absolute_for_index
                            && existing.kind == SegmentType::ChordRest
                    })
                    .expect("inserted chord rest segment")
            }
        };
        let id = node.attribute("id").map(str::to_owned);
        if let Some(id) = id {
            self.id_events.insert(id, (measure, track, absolute));
        }
        position
    }

    /// `MeiImporter::readNote` (`meiimporter.cpp:2040`).
    fn read_note(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, false);
        let accid_node = first_descendant(node, "accid");
        let accid = accid_node.unwrap_or(node);
        let staff_idx = track.staff().raw() as usize;
        let transpose = self.transpose.get(&staff_idx).copied().unwrap_or_default();
        let info = convert::pitch_from_mei(
            node.attribute("pname"),
            node.attribute("oct").and_then(|value| value.parse().ok()),
            accid.attribute("accid"),
            accid.attribute("accid.ges"),
            transpose.chromatic,
        );
        let mut note = Note::new(info.pitch, info.tpc);
        if let Some(group) = note_head_group(node) {
            note.head_group = group;
        }
        note.accidental = info.accidental.map(|kind| Accidental {
            kind,
            bracket: accid.attribute("enclose") == Some("brack"),
            parentheses: accid.attribute("enclose") == Some("paren"),
            visible: node.attribute("visible") != Some("false"),
            user: accid.attribute("accid").is_some(),
            small: false,
        });
        if let Some(vel) = node
            .attribute("vel")
            .and_then(|value| value.parse::<u8>().ok())
        {
            note.playback.user_velocity = Some(i32::from(vel));
        }
        let note_id = self.score.add_note(note);
        if let Some(id) = node.attribute("id") {
            self.id_notes.insert(id.to_owned(), note_id);
        }
        if let Some(SegmentItem::ChordRest(cr)) =
            self.score.measures[measure].segments[index].element_mut(track)
        {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.notes.push(note_id);
                chord
                    .articulations
                    .extend(crate::controls::articulations_of(node));
            }
        }
        self.read_verses(node, measure, track, ticks);
        self.read_control_events(node, measure);
        self.advance_event(ticks, node, duration, measure);
    }

    /// `MeiImporter::readChord` (`meiimporter.cpp:1835`).
    fn read_chord(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, false);
        let staff_idx = track.staff().raw() as usize;
        let transpose = self.transpose.get(&staff_idx).copied().unwrap_or_default();
        let mut notes = Vec::new();
        for note_node in node.children_named("note") {
            let accid = first_descendant(note_node, "accid").unwrap_or(note_node);
            let info = convert::pitch_from_mei(
                note_node.attribute("pname"),
                note_node
                    .attribute("oct")
                    .and_then(|value| value.parse().ok()),
                accid.attribute("accid"),
                accid.attribute("accid.ges"),
                transpose.chromatic,
            );
            let mut note = Note::new(info.pitch, info.tpc);
            if let Some(group) = note_head_group(note_node) {
                note.head_group = group;
            }
            note.accidental = info.accidental.map(|kind| Accidental {
                kind,
                bracket: accid.attribute("enclose") == Some("brack"),
                parentheses: accid.attribute("enclose") == Some("paren"),
                visible: note_node.attribute("visible") != Some("false"),
                user: accid.attribute("accid").is_some(),
                small: false,
            });
            let note_id = self.score.add_note(note);
            if let Some(id) = note_node.attribute("id") {
                self.id_notes.insert(id.to_owned(), note_id);
                // Un slur peut s'ancrer sur un `<note>` enfant d'un `<chord>`
                // (`@endid="#n…"`) : enregistrer aussi son id comme événement,
                // avec le tick et la piste de l'accord qui le contient.
                self.id_events.insert(
                    id.to_owned(),
                    (
                        measure,
                        track,
                        self.score.measures[measure].start_tick + *ticks,
                    ),
                );
            }
            notes.push(note_id);
        }
        let segment = &mut self.score.measures[measure].segments[index];
        if let Some(SegmentItem::ChordRest(cr)) = segment.element_mut(track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.notes = notes;
                chord
                    .articulations
                    .extend(crate::controls::articulations_of(node));
            }
        }
        self.read_verses(node, measure, track, ticks);
        self.read_control_events(node, measure);
        self.advance_event(ticks, node, duration, measure);
    }

    /// `MeiImporter::readRest` (`meiimporter.cpp:2139`).
    fn read_rest(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, true);
        if node.attribute("visible") == Some("false") {
            if let Some(SegmentItem::ChordRest(cr)) =
                self.score.measures[measure].segments[index].element_mut(track)
            {
                if let ChordRestKind::Rest(rest) = &mut cr.kind {
                    rest.visible = false;
                }
            }
        }
        self.advance_event(ticks, node, duration, measure);
    }

    /// `MeiImporter::readSpace` (`meiimporter.cpp:2160`): an invisible rest.
    fn read_space(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, true);
        if let Some(SegmentItem::ChordRest(cr)) =
            self.score.measures[measure].segments[index].element_mut(track)
        {
            if let ChordRestKind::Rest(rest) = &mut cr.kind {
                rest.gap = true;
                rest.visible = false;
            }
        }
        self.advance_event(ticks, node, duration, measure);
    }

    /// `MeiImporter::readMRest` (`meiimporter.cpp:1972`).
    fn read_m_rest(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        self.add_chord_rest(node, measure, track, *ticks, Duration::measure(), true);
        let nominal = self.score.measures[measure].nominal_len;
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            cr.ticks = Some(nominal);
        }
        *ticks += nominal;
    }

    /// `<multiRest @n>` (MEI 5.1 `att.numbered`, `mei_multiRest`) porte le
    /// nombre de mesures couvertes, sans décaler la numérotation `@n`.
    fn read_multi_rest(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        let count = node
            .attribute("n")
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|count| *count > 0)
            .unwrap_or(1);
        self.score.measures[measure].multi_measure_rest = Some(count);
        self.read_m_rest(node, measure, track, ticks);
    }

    /// `MeiImporter::readMRpt` (`meiimporter.cpp:2010`).
    fn read_m_rpt(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let nominal = self.score.measures[measure].nominal_len;
        // L'écriture additive place `<mRpt>` en fin de couche avec un
        // `@tstamp` explicite (`att.timestamp`) : le symbole appartient au
        // début de la mesure et ne doit pas hériter du curseur final.
        let additive = node
            .attribute("tstamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|stamp| {
                convert::tstamp_to_fraction(
                    stamp,
                    convert::tstamp_denominator(&self.score.measures[measure]),
                )
            });
        // Sans `@tstamp`, `<mRpt>` est un événement rythmique de l'importeur
        // 4.7.4 (`readMRpt`) : il crée un silence d'une mesure. Notre porteur
        // additionnel ne porte que le symbole (`MeasureRepeatMark`, L1) et ne
        // doit pas écraser un accord déjà présent au même tick.
        if let Some(offset) = additive {
            *ticks = offset;
        } else {
            self.add_chord_rest(node, measure, track, *ticks, Duration::whole(), true);
        }
        let count = node
            .attribute("n")
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|count| *count > 0)
            .unwrap_or(1);
        self.score.measures[measure].measure_repeat =
            Some(scoretk_core::model::MeasureRepeatMark {
                staff: StaffId::from_index(track.staff().raw() as usize),
                count,
            });
        *ticks += nominal;
    }

    /// `MeiImporter::readTuplet` (`meiimporter.cpp:2200`). MuseScore 4.7.4
    /// rejette les n-olets imbriqués à l'import, mais son export en écrit
    /// (`Convert::tupletToMEI`, meiconverter.cpp:3639) et la partition de
    /// référence en contient : le lecteur les conserve en empilant les ratios.
    /// Les événements appartiennent au n-olet le plus interne ; la durée d'un
    /// n-olet est la somme de ses propres événements, divisée par son ratio.
    fn read_tuplet(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let num = node
            .attribute("num")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(1);
        let base = node
            .attribute("numbase")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(1);
        let ratio = Fraction::try_new(num, base).unwrap_or(Fraction::ONE);
        let start_tick = self.score.measures[measure].start_tick + *ticks;
        let base_len = node
            .children
            .iter()
            .find_map(duration_of_opt)
            .map(|duration| duration.ticks())
            .unwrap_or(Fraction::QUARTER);
        // `TRead::read(Tuplet*)` : `len = baseLen * normalNotes`, avec le
        // dénominateur non réduit lu dans `@numbase` (read4xx/mod.rs:1963).
        let len = base_len * Fraction::from_integer(base);
        let mut tuplet = Tuplet::new(ratio, base_len, start_tick, len);
        tuplet.actual_notes = num;
        tuplet.normal_notes = base;
        tuplet.parent = self.tuplet_stack.last().copied();
        // `Convert::tupletToMEI` (meiconverter.cpp:3649) écrit
        // `@bracket.place` dès que la direction est haute ou basse, et
        // `@bracket.visible="false"` seulement pour un crochet explicitement
        // masqué.
        tuplet.bracket = match node.attribute("bracket.visible") {
            Some("false") => false,
            Some("true") => true,
            _ => node.attribute("bracket.place").is_some(),
        };
        tuplet.bracket_explicit = node.attribute("bracket.visible").is_some();
        let tuplet_id = self.score.add_tuplet(tuplet);
        self.tuplet_ratios.push(ratio);
        self.tuplet_stack.push(tuplet_id);
        self.read_elements(node, measure, track, ticks);
        self.tuplet_ratios.pop();
        self.tuplet_stack.pop();
        if !self.tuplet_stack.is_empty() {
            self.score.report.record(
                Layers::EXTENSIONS,
                "nested-tuplet",
                Some(start_tick),
                Some(track),
            );
        }
    }

    /// `<meterSig>` (MEI 5.1, `model.meterSigLike`): an in-measure meter change.
    fn read_meter_sig(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        let unit = node
            .attribute("unit")
            .and_then(|value| value.parse::<i32>().ok());
        let (_, kind) =
            convert::meter_from_mei(node.attribute("count"), unit, node.attribute("sym"));
        let numerator = node
            .attribute("count")
            .and_then(|value| value.split('+').next())
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(4);
        let denominator = unit.unwrap_or(4);
        let offset = node
            .attribute("tstamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|stamp| {
                convert::tstamp_to_fraction(
                    stamp,
                    convert::tstamp_denominator(&self.score.measures[measure]),
                )
            })
            .unwrap_or(*ticks);
        let time = scoretk_core::model::TimeSig {
            numerator,
            denominator,
            kind,
            ..scoretk_core::model::TimeSig::new(1, 1)
        };
        let staff_count = self.score.staves.len();
        let tick = self.score.measures[measure].start_tick + offset;
        Self::insert_segment(
            &mut self.score.measures[measure],
            staff_count,
            tick,
            SegmentType::TimeSig,
            track.staff().raw() as usize,
            SegmentItem::TimeSig(time),
        );
    }

    /// `<barLine @form>` (MEI 5.1 `mei_barLine`, `data.BARRENDITION`) lève
    /// l'ambiguïté de `@right="dotted"` : `Dashed` et `Tick` sont deux types
    /// du modèle que 4.7.4 écrit tous deux comme `dotted`. L'élément explicite
    /// utilise la valeur `dashed` de la RNG, pas l'extension `@type`.
    fn read_bar_line(&mut self, node: &Element, measure: usize) {
        let kind = match node.attribute("form") {
            Some("dashed") => BarLineType::Dashed,
            Some(form) => convert::barline_from_mei(form),
            None => return,
        };
        self.score.measures[measure].end_bar_line = kind;
    }

    /// `MeiImporter::readClef` (`meiimporter.cpp:1879`).
    fn read_clef_element(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        let clef = convert::clef_from_mei(
            node.attribute("shape"),
            node.attribute("line")
                .and_then(|value| value.parse().ok())
                .unwrap_or(2),
            node.attribute("dis"),
            node.attribute("dis.place"),
        );
        let offset = node
            .attribute("tstamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|stamp| {
                convert::tstamp_to_fraction(
                    stamp,
                    convert::tstamp_denominator(&self.score.measures[measure]),
                )
            })
            .unwrap_or(*ticks);
        let tick = self.score.measures[measure].start_tick + offset;
        let staff_count = self.score.staves.len();
        let staff_idx = track.staff().raw() as usize;
        let duplicate = self.score.measures[measure].segments.iter().any(|segment| {
            segment.tick == tick
                && segment.kind == SegmentType::Clef
                && segment
                    .element(Track::new(StaffId::from_index(staff_idx), 0))
                    .is_some()
        });
        if duplicate {
            // Deux clés au même tick sont légales en MEI 5.1 (`<staffDef>`
            // puis `<clef>` dans la couche) et le modèle les conserve dans
            // deux segments distincts : `insert_segment` écraserait la
            // première au lieu d'ajouter la seconde.
            let mut segment = Segment::new(SegmentType::Clef, tick, staff_count.max(1));
            segment.set_element(
                Track::new(StaffId::from_index(staff_idx), 0),
                SegmentItem::Clef(Clef::new(clef)),
            );
            self.score.measures[measure].add_segment(segment);
        } else {
            Self::insert_segment(
                &mut self.score.measures[measure],
                staff_count,
                tick,
                SegmentType::Clef,
                staff_idx,
                SegmentItem::Clef(Clef::new(clef)),
            );
        }
    }

    /// `MeiImporter::readBTrem`/`readFTrem` (`meiimporter.cpp:1812`, `:1910`).
    fn read_tremolo(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let before = self.score.measures[measure].start_tick + *ticks;
        let two_chord = node.name == "fTrem";
        let kind = if two_chord {
            let kind = if self.tremolo_phase == 0 {
                scoretk_core::model::TremoloKind::Start
            } else {
                scoretk_core::model::TremoloKind::Stop
            };
            self.tremolo_phase = (self.tremolo_phase + 1) % 2;
            kind
        } else {
            self.tremolo_phase = 0;
            scoretk_core::model::TremoloKind::Single
        };
        self.read_elements(node, measure, track, ticks);
        for segment in &mut self.score.measures[measure].segments {
            if segment.tick < before {
                continue;
            }
            if let Some(SegmentItem::ChordRest(cr)) = segment.element_mut(track) {
                if let ChordRestKind::Chord(chord) = &mut cr.kind {
                    chord.tremolo = Some(scoretk_core::model::Tremolo { kind, count: 1 });
                    chord.tremolo_two_chord = two_chord;
                }
            }
        }
    }

    /// `MeiImporter::readGraceGrp` (`meiimporter.cpp:1938`).
    fn read_grace_group(
        &mut self,
        node: &Element,
        _measure: usize,
        track: Track,
        _ticks: &mut Fraction,
    ) {
        use scoretk_core::model::{GraceChord, GraceType};
        // `@type="mscore-grace-*"` (porteur additionnel) garde la variante
        // exacte ; `@dur`/`@dots` portent la durée écrite, `@grace` la
        // direction standard (`acc`/`unacc`). RNG `mei_graceGrp` : la durée
        // ne peut être portée que par un `<note>` enfant (porteur additif) ;
        // le repli la lit donc aussi sur la première note.
        let grace = node
            .attribute("type")
            .and_then(|value| value.strip_prefix(ids::GRACE_TYPE))
            .and_then(convert::grace_type_from_name)
            .unwrap_or_else(|| {
                if node.attribute("grace") == Some("unacc") {
                    GraceType::Grace8After
                } else {
                    GraceType::Acciaccatura
                }
            });
        let mut chord = scoretk_core::model::Chord {
            grace: Some(grace),
            ..Default::default()
        };
        let staff_idx = track.staff().raw() as usize;
        let transpose = self.transpose.get(&staff_idx).copied().unwrap_or_default();
        for note_node in descendants_named(node, "note") {
            let accid = first_descendant(note_node, "accid").unwrap_or(note_node);
            let info = convert::pitch_from_mei(
                note_node.attribute("pname"),
                note_node.attribute("oct").and_then(|v| v.parse().ok()),
                accid.attribute("accid"),
                accid.attribute("accid.ges"),
                transpose.chromatic,
            );
            let id = self.score.add_note(Note::new(info.pitch, info.tpc));
            if let Some(xml_id) = note_node.attribute("id") {
                self.id_notes.insert(xml_id.to_owned(), id);
            }
            chord.notes.push(id);
        }
        let duration = duration_of_opt(node)
            .or_else(|| {
                descendants_named(node, "note")
                    .first()
                    .and_then(|note| duration_of_opt(note))
            })
            .unwrap_or_else(Duration::quarter);
        self.pending_graces.push(GraceChord {
            duration,
            played_offset: None,
            in_tuplet: false,
            chord,
        });
    }

    /// Advance by the performed duration when `@dur.ges` is present.
    fn advance_event(
        &self,
        ticks: &mut Fraction,
        node: &Element,
        duration: Duration,
        measure: usize,
    ) {
        self.advance(
            ticks,
            gestural_duration_of(node).unwrap_or(duration),
            measure,
        );
    }

    /// `MeiImporter::addChordRest`: time advance, scaled inside a tuplet.
    fn advance(&self, ticks: &mut Fraction, duration: Duration, _measure: usize) {
        let mut advance = duration.ticks();
        for ratio in &self.tuplet_ratios {
            advance /= *ratio;
        }
        *ticks += advance;
    }

    /// Last chord/rest written on a track, used for attached notations.
    ///
    /// At the start of a measure, the initial clef/key/time signatures share
    /// the chord's tick. Searching for the last segment with *any* element on
    /// the track picks a signature segment and returns no chord rest: lyrics,
    /// grace notes, articulations and tremolos attached to the first event
    /// were then silently dropped. Search explicitly for a `ChordRest`.
    pub(crate) fn last_chord_rest_mut(
        &mut self,
        measure: usize,
        track: Track,
    ) -> Option<&mut ChordRest> {
        let position = self.score.measures[measure]
            .segments
            .iter()
            .rposition(|segment| {
                segment
                    .element(track)
                    .is_some_and(|item| item.as_chord_rest().is_some())
            })?;
        self.score.measures[measure].segments[position]
            .element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
    }

    /// Chord rest at an exact anchor (`@startid` resolved by `anchor`),
    /// falling back to the last event of the track for external MEI that
    /// only gives `@tstamp`. `MeiImporter::findStart`, meiimporter.cpp:620.
    pub(crate) fn chord_rest_at_mut(
        &mut self,
        measure: usize,
        tick: Fraction,
        track: Track,
    ) -> Option<&mut ChordRest> {
        let segments = &self.score.measures[measure].segments;
        let has_chord_rest = |segment: &Segment| {
            segment
                .element(track)
                .is_some_and(|item| item.as_chord_rest().is_some())
        };
        let position = segments
            .iter()
            .position(|segment| segment.tick == tick && has_chord_rest(segment))
            .or_else(|| segments.iter().rposition(has_chord_rest))?;
        self.score.measures[measure].segments[position]
            .element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
    }

    /// Current time signature as `(numerator, denominator, kind)`.
    pub(crate) fn current_timesig(&self) -> (i64, i64, TimeSigType) {
        let (numerator, denominator, kind) = self
            .timesigs
            .get(&SCOREDEF_IDX)
            .copied()
            .or_else(|| self.timesigs.iter().next().map(|(_, value)| *value))
            .unwrap_or((4, 4, TimeSigType::Normal));
        (i64::from(numerator), i64::from(denominator), kind)
    }
}

/// `@head.shape` (MEI 5.1 `data.HEADSHAPE`) ou `@glyph.name` SMuFL
/// (`att.noteHeads`, `@glyph.auth="smufl"`). Les têtes sans terme standard
/// dans `HEADSHAPE` passent par le nom de glyphe, comme dans
/// `Convert::headShapeFromMEI` (meiconverter.cpp:3547 pour les glyphes des
/// autres objets).
fn note_head_group(node: &Element) -> Option<scoretk_core::model::NoteHeadGroup> {
    if let Some(shape) = node
        .attribute("head.shape")
        .and_then(convert::head_shape_from_mei)
    {
        return Some(shape);
    }
    if matches!(node.attribute("glyph.auth"), Some("smufl") | None) {
        return node
            .attribute("glyph.name")
            .and_then(convert::head_shape_from_smufl);
    }
    None
}

/// `Duration` from `@dur` and `@dots`, defaulting to a quarter note
/// (`Convert::durFromMEI` + `AttAugmentDots`).
pub(crate) fn duration_of(node: &Element) -> Duration {
    duration_of_opt(node).unwrap_or(Duration::quarter())
}

fn duration_of_opt(node: &Element) -> Option<Duration> {
    let kind = convert::dur_from_mei(node.attribute("dur")?)?;
    let dots = node
        .attribute("dots")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    Some(Duration::with_dots(kind, dots.min(4)))
}

/// `att.gestural.log/@dur.ges` + `@dots.ges` (MEI 5.1).
fn gestural_duration_of(node: &Element) -> Option<Duration> {
    let kind = convert::dur_from_mei(node.attribute("dur.ges")?)?;
    let dots = node
        .attribute("dots.ges")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    Some(Duration::with_dots(kind, dots.min(4)))
}

/// All descendants with the given name, in document order.
pub(crate) fn descendants_named<'a>(node: &'a Element, name: &str) -> Vec<&'a Element> {
    let mut result = Vec::new();
    collect_descendants(node, name, &mut result);
    result
}

fn collect_descendants<'a>(node: &'a Element, name: &str, result: &mut Vec<&'a Element>) {
    for child in &node.children {
        if child.name == name {
            result.push(child);
        }
        collect_descendants(child, name, result);
    }
}
