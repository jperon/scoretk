// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
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

//! Export MusicXML des notations : articulations, points d'orgue, liaisons
//! d'expression, ornements, trilles, trémolos, techniques, arpèges,
//! glissandos, respirations et lignes d'accord.
//!
//! Portage de `ExportMusicXml::chordAttributes`, `fermata`, `symIdToArtic`,
//! `symIdToOrnam`, `symIdToTechn`, `writeBreathMark`, `writeChordLines`,
//! `tremoloSingleStartStop`, `wavyLineStartStop`, `GlissandoHandler`,
//! `SlurHandler` et `arpeggiate`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! Les attributs de mise en page (`default-x`/`default-y`, `relative-x`/
//! `relative-y`, `bezier-*`, couleur, police) ne sont pas modélisés et ne sont
//! donc pas écrits.

use std::collections::BTreeMap;

use crate::model::{
    Accidental, Annotation, Articulation, ArticulationAnchor, ArticulationType, Breath, BreathType,
    Chord, ChordLineType, Fermata, FermataType, Glissando, GlissandoType, LineStyle, Measure, Note,
    Ornament, OrnamentType, Placement, Score, Segment, SpannerNotation, SpannerType, Tick, Track,
    TremoloKind, Trill, TrillType,
};

use super::exportmusicxml::{accidental_name, Notations};
use super::writer::XmlWriter;

/// Nombre maximal de spanners simultanés (`MAX_NUMBER_LEVEL`).
const MAX_NUMBER_LEVEL: usize = 32;

/// État des notations à numéroter, conservé pendant tout l'export.
#[derive(Clone, Debug, Default)]
pub(crate) struct NotationsState {
    /// Liaisons d'expression actives, par numéro (`SlurHandler::m_slur`).
    slur_slots: [Option<usize>; MAX_NUMBER_LEVEL],
    /// Vrai quand la liaison a déjà écrit son départ.
    slur_started: [bool; MAX_NUMBER_LEVEL],
    /// Glissandos droits actifs (`m_slideNote`).
    slide_slots: [Option<usize>; MAX_NUMBER_LEVEL],
    /// Glissandos ondulés actifs (`m_glissNote`).
    wavy_slots: [Option<usize>; MAX_NUMBER_LEVEL],
    /// Arpèges déjà numérotés, par tick (`m_measArpeggios`).
    arpeggios: BTreeMap<Tick, Vec<(u32, usize)>>,
}

impl NotationsState {
    /// Réinitialise l'état au début d'une partie.
    pub(crate) fn reset_part(&mut self) {
        self.slur_slots = [None; MAX_NUMBER_LEVEL];
        self.slur_started = [false; MAX_NUMBER_LEVEL];
        self.slide_slots = [None; MAX_NUMBER_LEVEL];
        self.wavy_slots = [None; MAX_NUMBER_LEVEL];
        self.arpeggios.clear();
    }

    /// Réinitialise les numéros d'arpèges au début d'une mesure.
    pub(crate) fn start_measure(&mut self) {
        self.arpeggios.clear();
    }

    fn slur_slot(&self, id: usize) -> Option<usize> {
        self.slur_slots.iter().position(|slot| *slot == Some(id))
    }

    fn allocate_slur(&mut self, id: usize) -> Option<usize> {
        let index = self.slur_slots.iter().position(Option::is_none)?;
        self.slur_slots[index] = Some(id);
        Some(index)
    }

    fn glissando_table(&mut self, wavy: bool) -> &mut [Option<usize>; MAX_NUMBER_LEVEL] {
        if wavy {
            &mut self.wavy_slots
        } else {
            &mut self.slide_slots
        }
    }

    fn glissando_number(&mut self, id: usize, wavy: bool) -> Option<usize> {
        let table = self.glissando_table(wavy);
        if let Some(index) = table.iter().position(|slot| *slot == Some(id)) {
            return Some(index);
        }
        let index = table.iter().position(Option::is_none)?;
        table[index] = Some(id);
        Some(index)
    }

    fn release_glissando(&mut self, id: usize, wavy: bool) {
        let table = self.glissando_table(wavy);
        if let Some(index) = table.iter().position(|slot| *slot == Some(id)) {
            table[index] = None;
        }
    }

    /// Numéro d'un arpège au tick donné (`arpeggiate`).
    fn arpeggio_number(&mut self, tick: Tick, track: Track) -> usize {
        let entries = self.arpeggios.entry(tick).or_default();
        if let Some((_, number)) = entries
            .iter()
            .find(|(entry_track, _)| *entry_track == track.raw())
        {
            return *number;
        }
        let number = entries.iter().map(|(_, number)| *number).max().unwrap_or(0) + 1;
        entries.push((track.raw(), number));
        number
    }
}

/// Écrit les notations d'un accord (`chordAttributes`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn chord_attributes(
    score: &Score,
    xml: &mut XmlWriter,
    chord: &Chord,
    grace: bool,
    measure: &Measure,
    segment: Option<&Segment>,
    segment_tick: Tick,
    track: Track,
    end_tick: Tick,
    notations: &mut Notations,
) {
    // Points d'orgue d'abord (`fermatas`), jamais sur un ornement.
    if let (false, Some(segment)) = (grace, segment) {
        for annotation in segment.annotations_on(track) {
            if let Annotation::Fermata(fermata) = annotation {
                notations.tag_element(xml, None, fermata.visible);
                write_fermata(xml, fermata);
            }
        }
    }

    // Puis les articulations, dont les enfants vont dans `<articulations>`.
    for articulation in &chord.articulations {
        if is_technical(&articulation.kind) || is_ornament(&articulation.kind) {
            continue;
        }
        write_articulation(xml, articulation, notations);
    }

    if let Some(breath) = breath_at(measure, track, end_tick) {
        write_breath(xml, breath, notations);
    }

    if let Some(kind) = chord.chord_line {
        notations.tag(xml, Some("articulations"));
        write_chord_line(xml, kind);
    }

    notations.close_container(xml);

    // Ornements autonomes, trémolo et lignes ondulées.
    for ornament in &chord.ornaments {
        notations.tag_element(xml, Some("ornaments"), ornament.visible);
        write_ornament(xml, ornament);
    }
    if let Some(tremolo) = chord.tremolo {
        notations.tag(xml, Some("ornaments"));
        let kind = match tremolo.kind {
            TremoloKind::Single => "single",
            TremoloKind::Start => "start",
            TremoloKind::Stop => "stop",
            TremoloKind::Unmeasured => "unmeasured",
        };
        xml.tag_with("tremolo", &[("type", kind)], &tremolo.count.to_string());
    }
    write_trills(score, xml, segment_tick, track, notations);
    notations.close_container(xml);

    // Enfin les éléments de `<technical>`.
    for articulation in &chord.articulations {
        if let Some(name) = technical_name(&articulation.kind) {
            notations.tag_element(xml, Some("technical"), articulation.visible);
            write_technical(xml, name, articulation);
        }
    }
    notations.close_container(xml);
}

/// Écrit les doigtés d'une note (`writeFingering`).
pub(crate) fn fingering(xml: &mut XmlWriter, note: &Note, notations: &mut Notations) {
    for fingering in &note.fingerings {
        notations.tag_element(xml, Some("technical"), fingering.visible);
        let tag = if fingering.text.len() == 1
            && matches!(fingering.text.as_str(), "p" | "i" | "m" | "a" | "c")
        {
            "pluck"
        } else {
            "fingering"
        };
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        if let Some(placement) = fingering.placement {
            attributes.push(("placement", placement_name(placement)));
        }
        xml.tag_with(tag, &attributes, &fingering.text);
    }
}

/// Écrit l'arpège d'un accord sur une note (`arpeggiate`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn arpeggio(
    xml: &mut XmlWriter,
    state: &mut NotationsState,
    chord: &Chord,
    tick: Tick,
    track: Track,
    first: bool,
    last: bool,
    notations: &mut Notations,
) {
    let Some(arpeggio) = chord.arpeggio else {
        return;
    };
    let number = state.arpeggio_number(tick, track);
    let number_text = number.to_string();
    let visible = arpeggio.visible;
    match arpeggio.kind {
        crate::model::ArpeggioType::Normal => {
            notations.tag_element(xml, None, visible);
            xml.tag_empty("arpeggiate", &[("number", &number_text)]);
        }
        crate::model::ArpeggioType::Up => {
            notations.tag_element(xml, None, visible);
            xml.tag_empty(
                "arpeggiate",
                &[("direction", "up"), ("number", &number_text)],
            );
        }
        crate::model::ArpeggioType::Down => {
            notations.tag_element(xml, None, visible);
            xml.tag_empty(
                "arpeggiate",
                &[("direction", "down"), ("number", &number_text)],
            );
        }
        crate::model::ArpeggioType::Bracket => {
            if first {
                notations.tag_element(xml, None, visible);
                xml.tag_empty(
                    "non-arpeggiate",
                    &[("type", "bottom"), ("number", &number_text)],
                );
            }
            if last {
                notations.tag_element(xml, None, visible);
                xml.tag_empty(
                    "non-arpeggiate",
                    &[("type", "top"), ("number", &number_text)],
                );
            }
        }
    }
}

/// Écrit les glissandos qui commencent ou finissent à ce tick (`GlissandoHandler`).
pub(crate) fn glissandi(
    score: &Score,
    xml: &mut XmlWriter,
    state: &mut NotationsState,
    tick: Tick,
    track: Track,
    notations: &mut Notations,
) {
    // Les arrêts d'abord, comme `doSlurs` : une demi-liaison rencontrée avant
    // son départ réserve son numéro.
    for index in spanner_indices(score, SpannerType::Glissando, tick, track, false) {
        let Some(SpannerNotation::Glissando(glissando)) = &score.spanners[index].notation else {
            continue;
        };
        let wavy = matches!(glissando.kind, GlissandoType::Wavy);
        let Some(number) = state.glissando_number(index, wavy) else {
            continue;
        };
        write_glissando(xml, glissando, number, false, notations);
        state.release_glissando(index, wavy);
    }
    for index in spanner_indices(score, SpannerType::Glissando, tick, track, true) {
        let Some(SpannerNotation::Glissando(glissando)) = &score.spanners[index].notation else {
            continue;
        };
        let wavy = matches!(glissando.kind, GlissandoType::Wavy);
        let Some(number) = state.glissando_number(index, wavy) else {
            continue;
        };
        write_glissando(xml, glissando, number, true, notations);
    }
}

/// Écrit les liaisons d'expression qui commencent ou finissent à cet accord
/// (`SlurHandler::doSlurs`).
pub(crate) fn slurs(
    score: &Score,
    xml: &mut XmlWriter,
    state: &mut NotationsState,
    tick: Tick,
    track: Track,
    notations: &mut Notations,
) {
    // `doSlurs` boucle deux fois : arrêts, puis départs.
    for starting in [false, true] {
        for index in spanner_indices(score, SpannerType::Slur, tick, track, starting) {
            let Some(SpannerNotation::Slur(slur)) = &score.spanners[index].notation else {
                continue;
            };
            if starting {
                if let Some(slot) = state.slur_slot(index) {
                    // La fin a déjà été rencontrée (voix traitées dans un ordre
                    // différent) : le départ réutilise son numéro.
                    state.slur_slots[slot] = None;
                    state.slur_started[slot] = false;
                    write_slur_start(xml, slur, slot, notations);
                } else if let Some(slot) = state.allocate_slur(index) {
                    state.slur_started[slot] = true;
                    write_slur_start(xml, slur, slot, notations);
                }
            } else if let Some(slot) = state.slur_slot(index) {
                notations.tag_element(xml, None, slur.visible);
                let number = (slot + 1).to_string();
                xml.tag_empty("slur", &[("type", "stop"), ("number", &number)]);
                if state.slur_started[slot] {
                    state.slur_slots[slot] = None;
                    state.slur_started[slot] = false;
                }
            } else if let Some(slot) = state.allocate_slur(index) {
                // Arrêt avant le départ : le numéro reste réservé pour le
                // départ à venir.
                state.slur_started[slot] = false;
                notations.tag_element(xml, None, slur.visible);
                let number = (slot + 1).to_string();
                xml.tag_empty("slur", &[("type", "stop"), ("number", &number)]);
            }
        }
    }
}

/// Écrit le départ d'une liaison (`doSlurStart`).
fn write_slur_start(
    xml: &mut XmlWriter,
    slur: &crate::model::Slur,
    slot: usize,
    notations: &mut Notations,
) {
    notations.tag_element(xml, None, slur.visible);
    let number = (slot + 1).to_string();
    let mut attributes: Vec<(&str, String)> = vec![("type", "start".to_owned())];
    if let Some(placement) = slur.placement {
        attributes.push((
            "orientation",
            if placement == Placement::Above {
                "over".to_owned()
            } else {
                "under".to_owned()
            },
        ));
        attributes.push(("placement", placement_name(placement).to_owned()));
    }
    attributes.push(("number", number));
    let borrowed: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    xml.tag_empty("slur", &borrowed);
}

/// Écrit les notations d'un silence : points d'orgue, respiration, trilles et
/// liaisons.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rest_notations(
    score: &Score,
    xml: &mut XmlWriter,
    measure: &Measure,
    state: &mut NotationsState,
    segment: Option<&Segment>,
    segment_tick: Tick,
    end_tick: Tick,
    track: Track,
    notations: &mut Notations,
) {
    if let Some(segment) = segment {
        for annotation in segment.annotations_on(track) {
            if let Annotation::Fermata(fermata) = annotation {
                notations.tag_element(xml, None, fermata.visible);
                write_fermata(xml, fermata);
            }
        }
    }
    if let Some(breath) = breath_at(measure, track, end_tick) {
        write_breath(xml, breath, notations);
    }
    write_trills(score, xml, segment_tick, track, notations);
    slurs(score, xml, state, segment_tick, track, notations);
}

/// Indices des spanners d'un type dont une extrémité est au tick et à la piste
/// donnés.
fn spanner_indices(
    score: &Score,
    kind: SpannerType,
    tick: Tick,
    track: Track,
    start: bool,
) -> Vec<usize> {
    score
        .spanners
        .iter()
        .enumerate()
        .filter(|(_, spanner)| {
            spanner.kind == kind
                && if start {
                    spanner.start.tick == tick && spanner.start.track == track
                } else {
                    spanner.end.tick == tick && spanner.end.track == track
                }
        })
        .map(|(index, _)| index)
        .collect()
}

/// Écrit les lignes ondulées des trilles actifs (`wavyLineStartStop`).
fn write_trills(
    score: &Score,
    xml: &mut XmlWriter,
    tick: Tick,
    track: Track,
    notations: &mut Notations,
) {
    let starting = spanner_indices(score, SpannerType::Trill, tick, track, true);
    let stopping = spanner_indices(score, SpannerType::Trill, tick, track, false);
    for (index, spanner) in score.spanners.iter().enumerate() {
        if !starting.contains(&index) && !stopping.contains(&index) {
            continue;
        }
        let Some(SpannerNotation::Trill(trill)) = &spanner.notation else {
            continue;
        };
        let number = trill_number(score, index);
        let number_text = number.to_string();
        if starting.contains(&index) {
            notations.tag(xml, Some("ornaments"));
            write_trill_symbol(xml, trill);
            xml.tag_empty("wavy-line", &[("type", "start"), ("number", &number_text)]);
            if let Some(accidental) = &trill.accidental {
                write_accidental_mark(xml, accidental, Placement::Above);
            }
        }
        if stopping.contains(&index) {
            notations.tag(xml, Some("ornaments"));
            xml.tag_empty("wavy-line", &[("type", "stop"), ("number", &number_text)]);
        }
    }
}

/// Numéro d'un trille : rang du spanner parmi les trilles de la partition.
fn trill_number(score: &Score, index: usize) -> usize {
    score
        .spanners
        .iter()
        .enumerate()
        .filter(|(_, spanner)| spanner.kind == SpannerType::Trill)
        .position(|(candidate, _)| candidate == index)
        .map(|position| position + 1)
        .unwrap_or(1)
}

/// Écrit le symbole d'un trille (`wavyLineStart`).
fn write_trill_symbol(xml: &mut XmlWriter, trill: &Trill) {
    match trill.kind {
        TrillType::TrillLine => xml.tag_empty("trill-mark", &[]),
        TrillType::PureLine => {}
        TrillType::UpperPrallLine => {
            xml.tag_empty(
                "other-ornament",
                &[("smufl", "ornamentTopLeftConvexStroke")],
            );
        }
        TrillType::PrallPrallLine => {}
        TrillType::DownPrallLine => {
            xml.tag_empty("other-ornament", &[("smufl", "ornamentLeftVerticalStroke")]);
        }
    }
}

/// Écrit un point d'orgue (`fermata`).
pub(crate) fn write_fermata(xml: &mut XmlWriter, fermata: &Fermata) {
    let kind = if fermata.placement == Placement::Above {
        "upright"
    } else {
        "inverted"
    };
    let body = match fermata.kind {
        FermataType::Normal => None,
        FermataType::Short => Some("angled"),
        FermataType::Long => Some("square"),
        FermataType::VeryShort => Some("double-angled"),
        FermataType::VeryLong => Some("double-square"),
        FermataType::LongHenze => Some("double-dot"),
        FermataType::ShortHenze => Some("half-curve"),
        FermataType::Curlew => Some("curlew"),
    };
    match body {
        Some(body) => xml.tag_with("fermata", &[("type", kind)], body),
        None => xml.tag_empty("fermata", &[("type", kind)]),
    }
}

/// Vrai pour une articulation écrite dans `<technical>`.
fn is_technical(kind: &ArticulationType) -> bool {
    matches!(
        kind,
        ArticulationType::UpBow
            | ArticulationType::DownBow
            | ArticulationType::Harmonic
            | ArticulationType::Open
            | ArticulationType::Stopped
            | ArticulationType::SnapPizzicato
    )
}

/// Vrai pour un ornement rangé dans les articulations du modèle.
fn is_ornament(kind: &ArticulationType) -> bool {
    matches!(
        kind,
        ArticulationType::Fermata
            | ArticulationType::FermataShort
            | ArticulationType::FermataLong
            | ArticulationType::FermataVeryShort
            | ArticulationType::FermataVeryLong
    )
}

/// Nom MusicXML d'une articulation technique (`symIdToTechn`).
fn technical_name(kind: &ArticulationType) -> Option<&'static str> {
    match kind {
        ArticulationType::UpBow => Some("up-bow"),
        ArticulationType::DownBow => Some("down-bow"),
        ArticulationType::Harmonic => Some("harmonic"),
        ArticulationType::Open => Some("open"),
        ArticulationType::Stopped => Some("stopped"),
        ArticulationType::SnapPizzicato => Some("snap-pizzicato"),
        _ => None,
    }
}

/// Écrit une articulation (`symIdToArtic`).
fn write_articulation(xml: &mut XmlWriter, articulation: &Articulation, notations: &mut Notations) {
    use ArticulationType as T;
    let placement = match articulation.anchor {
        ArticulationAnchor::Up => Some("above"),
        ArticulationAnchor::Down => Some("below"),
        ArticulationAnchor::Auto => None,
    };
    let mut names: Vec<&str> = Vec::new();
    let mut marcatto = false;
    match &articulation.kind {
        T::Staccato => names.push("staccato"),
        T::Staccatissimo => names.push("staccatissimo"),
        T::Accent => names.push("accent"),
        T::Marcato => {
            names.push("strong-accent");
            marcatto = true;
        }
        T::Tenuto => names.push("tenuto"),
        T::Portato => names.push("detached-legato"),
        T::SoftAccent => names.push("soft-accent"),
        T::SoftAccentStaccato => {
            names.push("soft-accent");
            names.push("staccato");
        }
        T::SoftAccentTenuto => {
            names.push("soft-accent");
            names.push("tenuto");
        }
        T::SoftAccentTenutoStaccato => {
            names.push("soft-accent");
            names.push("detached-legato");
        }
        T::Stress => names.push("stress"),
        T::Unstress => names.push("unstress"),
        T::Spiccato => names.push("spiccato"),
        T::BreathMark => {
            notations.tag_element(xml, Some("articulations"), articulation.visible);
            xml.tag_with("breath-mark", &[], "comma");
            return;
        }
        T::Caesura => {
            notations.tag_element(xml, Some("articulations"), articulation.visible);
            xml.tag_empty("caesura", &[]);
            return;
        }
        T::Other(name) => {
            notations.tag_element(xml, Some("articulations"), articulation.visible);
            xml.tag_empty("other-articulation", &[("smufl", name)]);
            return;
        }
        _ => return,
    }
    notations.tag_element(xml, Some("articulations"), articulation.visible);
    for name in names {
        if marcatto && name == "strong-accent" {
            let direction = if articulation.up { "up" } else { "down" };
            xml.tag_empty(name, &[("type", direction)]);
        } else if let Some(placement) = placement {
            xml.tag_empty(name, &[("placement", placement)]);
        } else {
            xml.tag_empty(name, &[]);
        }
    }
}

/// Écrit une respiration ou une césure (`writeBreathMark`).
fn write_breath(xml: &mut XmlWriter, breath: &Breath, notations: &mut Notations) {
    notations.tag(xml, Some("articulations"));
    let (tag, body) = match breath.kind {
        BreathType::Caesura => ("caesura", None),
        BreathType::CaesuraCurved => ("caesura", Some("curved")),
        BreathType::CaesuraShort => ("caesura", Some("short")),
        BreathType::CaesuraThick => ("caesura", Some("thick")),
        BreathType::CaesuraSingle => ("caesura", Some("single")),
        BreathType::Tick => ("breath-mark", Some("tick")),
        BreathType::Upbow => ("breath-mark", Some("upbow")),
        BreathType::Comma => ("breath-mark", Some("comma")),
    };
    let placement = if breath.placement == Placement::Below {
        Some(("placement", "below"))
    } else {
        None
    };
    let attributes: Vec<(&str, &str)> = placement.into_iter().collect();
    match body {
        Some(body) => {
            if attributes.is_empty() {
                xml.tag_with(tag, &[], body);
            } else {
                xml.tag_with(tag, &attributes, body);
            }
        }
        None => xml.tag_empty(tag, &attributes),
    }
}

/// Respiration attachée à un accord ou un silence (`ChordRest::hasBreathMark`).
fn breath_at(measure: &Measure, track: Track, end_tick: Tick) -> Option<&Breath> {
    let segment = measure.segments.iter().find(|segment| {
        segment.kind == crate::model::SegmentType::Breath && segment.tick == end_tick
    })?;
    match segment.element(track) {
        Some(crate::model::SegmentItem::Breath(breath)) => Some(breath),
        _ => None,
    }
}

/// Écrit une ligne d'accord (`writeChordLines`).
fn write_chord_line(xml: &mut XmlWriter, kind: ChordLineType) {
    let name = match kind {
        ChordLineType::Fall => "falloff",
        ChordLineType::Doit => "doit",
        ChordLineType::Plop => "plop",
        ChordLineType::Scoop => "scoop",
    };
    xml.tag_empty(name, &[]);
}

/// Écrit un ornement (`symIdToOrnam`) et ses altérations.
fn write_ornament(xml: &mut XmlWriter, ornament: &Ornament) {
    let placement = ornament
        .placement
        .map(|placement| [("placement", placement_name(placement))]);
    let attributes: Vec<(&str, &str)> = placement.map(|value| value.to_vec()).unwrap_or_default();
    match &ornament.kind {
        OrnamentType::TrillMark => xml.tag_empty("trill-mark", &attributes),
        OrnamentType::Turn => xml.tag_empty("turn", &attributes),
        OrnamentType::InvertedTurn => xml.tag_empty("inverted-turn", &attributes),
        OrnamentType::TurnSlash => {
            let mut attributes = attributes;
            attributes.push(("slash", "yes"));
            xml.tag_empty("turn", &attributes);
        }
        OrnamentType::VerticalTurn => xml.tag_empty("vertical-turn", &attributes),
        OrnamentType::InvertedVerticalTurn => xml.tag_empty("inverted-vertical-turn", &attributes),
        OrnamentType::Mordent => xml.tag_empty("mordent", &attributes),
        OrnamentType::InvertedMordent => xml.tag_empty("inverted-mordent", &attributes),
        OrnamentType::InvertedMordentLong => {
            long_ornament(xml, "inverted-mordent", &attributes, None, None);
        }
        OrnamentType::MordentLong => {
            long_ornament(xml, "mordent", &attributes, None, None);
        }
        OrnamentType::MordentLongApproachBelow => {
            long_ornament(xml, "mordent", &attributes, Some("below"), None);
        }
        OrnamentType::MordentLongApproachAbove => {
            long_ornament(xml, "mordent", &attributes, Some("above"), None);
        }
        OrnamentType::InvertedMordentLongApproachBelow => {
            long_ornament(xml, "inverted-mordent", &attributes, Some("below"), None);
        }
        OrnamentType::InvertedMordentLongApproachAbove => {
            long_ornament(xml, "inverted-mordent", &attributes, Some("above"), None);
        }
        OrnamentType::InvertedMordentLongDepartureBelow => {
            long_ornament(xml, "inverted-mordent", &attributes, None, Some("below"));
        }
        OrnamentType::InvertedMordentLongDepartureAbove => {
            long_ornament(xml, "inverted-mordent", &attributes, None, Some("above"));
        }
        OrnamentType::Haydn => xml.tag_empty("haydn", &attributes),
        OrnamentType::Schleifer => xml.tag_empty("schleifer", &attributes),
        OrnamentType::Other(name) => {
            let mut attributes = attributes;
            attributes.push(("smufl", name));
            xml.tag_empty("other-ornament", &attributes);
        }
    }
    for accidental in &ornament.accidentals {
        write_accidental_mark(xml, &accidental.accidental, accidental.placement);
    }
}

/// Écrit un ornement long avec ses attributs `approach`/`departure`.
fn long_ornament(
    xml: &mut XmlWriter,
    name: &str,
    base: &[(&str, &str)],
    approach: Option<&str>,
    departure: Option<&str>,
) {
    let mut attributes = base.to_vec();
    attributes.push(("long", "yes"));
    if let Some(approach) = approach {
        attributes.push(("approach", approach));
    }
    if let Some(departure) = departure {
        attributes.push(("departure", departure));
    }
    xml.tag_empty(name, &attributes);
}

/// Écrit une `<accidental-mark>` (`writeAccidental`).
fn write_accidental_mark(xml: &mut XmlWriter, accidental: &Accidental, placement: Placement) {
    if !accidental.visible {
        return;
    }
    let Some(name) = accidental_name(accidental.kind) else {
        return;
    };
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    if accidental.bracket {
        attributes.push(("bracket", "yes"));
    } else if accidental.parentheses {
        attributes.push(("parentheses", "yes"));
    }
    attributes.push(("placement", placement_name(placement)));
    xml.tag_with("accidental-mark", &attributes, name);
}

/// Écrit une technique (`symIdToTechn`).
fn write_technical(xml: &mut XmlWriter, name: &str, articulation: &Articulation) {
    let placement = match articulation.anchor {
        ArticulationAnchor::Up => Some(("placement", "above")),
        ArticulationAnchor::Down => Some(("placement", "below")),
        ArticulationAnchor::Auto => None,
    };
    let attributes: Vec<(&str, &str)> = placement.into_iter().collect();
    if name == "harmonic" {
        xml.start_element("harmonic", &attributes);
        xml.tag_empty("natural", &[]);
        xml.end_element();
    } else if attributes.is_empty() {
        xml.tag_empty(name, &[]);
    } else {
        xml.tag_empty(name, &attributes);
    }
}

/// Écrit un glissando (`glissando` de `GlissandoHandler`).
fn write_glissando(
    xml: &mut XmlWriter,
    glissando: &Glissando,
    number: usize,
    start: bool,
    notations: &mut Notations,
) {
    let number_text = (number + 1).to_string();
    let (tag, line_type) = match glissando.kind {
        GlissandoType::Straight(style) => (
            "slide",
            match style {
                LineStyle::Solid => "solid",
                LineStyle::Dashed => "dashed",
                LineStyle::Dotted => "dotted",
            },
        ),
        GlissandoType::Wavy => ("glissando", "wavy"),
    };
    let kind = if start { "start" } else { "stop" };
    notations.tag_element(xml, None, glissando.visible);
    let attributes = [
        ("line-type", line_type),
        ("number", number_text.as_str()),
        ("type", kind),
    ];
    if start && glissando.show_text && !glissando.text.is_empty() {
        xml.tag_with(tag, &attributes, &glissando.text);
    } else {
        xml.tag_empty(tag, &attributes);
    }
}

/// Nom MusicXML d'un placement.
fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

#[cfg(test)]
mod tests {
    use super::super::export_musicxml;
    use crate::model::{
        tpc, Annotation, Arpeggio, ArpeggioType, Articulation, ArticulationAnchor,
        ArticulationType, Breath, BreathType, Chord, ChordLineType, ChordRest, Duration, Fermata,
        FermataType, Fingering, Glissando, GlissandoType, LineStyle, Measure, Note, Ornament,
        OrnamentAccidental, OrnamentType, Part, Placement, Score, Segment, SegmentItem,
        SegmentType, Slur, Spanner, SpannerAnchor, SpannerNotation, SpannerType, StaffId, Tick,
        Track, Tremolo, TremoloKind, Trill, TrillType,
    };

    /// Partition à une portée et une mesure 4/4 de quatre noires.
    fn partition() -> (Score, StaffId) {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Tick::ZERO, Duration::measure().ticks());
        measure.len = Duration::measure().ticks();
        for index in 0..4 {
            let mut chord_rest = ChordRest::chord(Duration::quarter());
            let note = score.add_note(Note::new(60 + index as u8, tpc::C));
            chord_rest.chord_mut().expect("accord").notes.push(note);
            let mut segment = Segment::new(
                SegmentType::ChordRest,
                Tick::new(i64::from(index), 4),
                score.staves.len(),
            );
            segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord_rest));
            measure.add_segment(segment);
        }
        score.add_measure(measure);
        (score, staff)
    }

    fn accord(score: &mut Score, index: usize) -> &mut Chord {
        score.measures[0].segments[index]
            .chord_rest_mut(Track::new(StaffId::from_index(0), 0))
            .and_then(ChordRest::chord_mut)
            .expect("accord")
    }

    fn segment(score: &mut Score, index: usize) -> &mut Segment {
        &mut score.measures[0].segments[index]
    }

    fn exporter(score: &Score) -> String {
        export_musicxml(score).expect("export")
    }

    #[test]
    fn articulations_et_point_d_orgue() {
        let (mut score, _) = partition();
        {
            let chord = accord(&mut score, 0);
            let mut staccato = Articulation::new(ArticulationType::Staccato);
            staccato.anchor = ArticulationAnchor::Up;
            chord.articulations.push(staccato);
            chord
                .articulations
                .push(Articulation::new(ArticulationType::Marcato));
            chord
                .articulations
                .push(Articulation::new(ArticulationType::Portato));
            chord
                .articulations
                .push(Articulation::new(ArticulationType::Other(
                    "articStressAbove".to_owned(),
                )));
        }
        segment(&mut score, 0).add_annotation(
            Track::new(StaffId::from_index(0), 0),
            Annotation::Fermata(Fermata::new(FermataType::Long)),
        );

        let xml = exporter(&score);
        assert!(
            xml.contains("<fermata type=\"upright\">square</fermata>"),
            "{xml}"
        );
        assert!(xml.contains("<staccato placement=\"above\"/>"), "{xml}");
        assert!(xml.contains("<strong-accent type=\"up\"/>"), "{xml}");
        assert!(xml.contains("<detached-legato/>"), "{xml}");
        assert!(
            xml.contains("<other-articulation smufl=\"articStressAbove\"/>"),
            "{xml}"
        );
    }

    #[test]
    fn ornements_avec_alterations() {
        let (mut score, _) = partition();
        let mut ornament = Ornament::new(OrnamentType::Turn);
        ornament.accidentals.push(OrnamentAccidental {
            accidental: crate::model::Accidental::new(crate::model::AccidentalType::Sharp),
            placement: Placement::Above,
        });
        ornament.accidentals.push(OrnamentAccidental {
            accidental: crate::model::Accidental::new(crate::model::AccidentalType::Flat),
            placement: Placement::Below,
        });
        accord(&mut score, 0).ornaments.push(ornament);

        let xml = exporter(&score);
        assert!(xml.contains("<ornaments>\n            <turn/>"), "{xml}");
        assert!(
            xml.contains("<accidental-mark placement=\"above\">sharp</accidental-mark>"),
            "{xml}"
        );
        assert!(
            xml.contains("<accidental-mark placement=\"below\">flat</accidental-mark>"),
            "{xml}"
        );
    }

    #[test]
    fn tremolos_simple_et_double() {
        let (mut score, _) = partition();
        accord(&mut score, 0).tremolo = Some(Tremolo {
            kind: TremoloKind::Single,
            count: 2,
        });
        accord(&mut score, 1).tremolo = Some(Tremolo {
            kind: TremoloKind::Start,
            count: 3,
        });
        accord(&mut score, 2).tremolo = Some(Tremolo {
            kind: TremoloKind::Stop,
            count: 3,
        });

        let xml = exporter(&score);
        assert!(
            xml.contains("<tremolo type=\"single\">2</tremolo>"),
            "{xml}"
        );
        assert!(xml.contains("<tremolo type=\"start\">3</tremolo>"), "{xml}");
        assert!(xml.contains("<tremolo type=\"stop\">3</tremolo>"), "{xml}");
    }

    #[test]
    fn arpeges_simple_et_crochet() {
        let (mut score, _) = partition();
        accord(&mut score, 0).arpeggio = Some(Arpeggio::new(ArpeggioType::Up));
        // Une deuxième note dans le même accord.
        let note = score.add_note(Note::new(64, tpc::E));
        accord(&mut score, 0).notes.push(note);

        let xml = exporter(&score);
        assert_eq!(
            xml.matches("<arpeggiate direction=\"up\" number=\"1\"/>")
                .count(),
            2,
            "{xml}"
        );

        accord(&mut score, 1).arpeggio = Some(Arpeggio::new(ArpeggioType::Bracket));
        let note = score.add_note(Note::new(67, tpc::G));
        accord(&mut score, 1).notes.push(note);
        let xml = exporter(&score);
        assert!(
            xml.contains("<non-arpeggiate type=\"bottom\" number=\"1\"/>"),
            "{xml}"
        );
        assert!(
            xml.contains("<non-arpeggiate type=\"top\" number=\"1\"/>"),
            "{xml}"
        );
    }

    #[test]
    fn liaisons_et_glissandos() {
        let (mut score, _) = partition();
        let mut slur = Spanner::new(
            SpannerType::Slur,
            SpannerAnchor::new(Tick::ZERO, Track::new(StaffId::from_index(0), 0)),
            SpannerAnchor::new(Tick::new(1, 4), Track::new(StaffId::from_index(0), 0)),
        );
        slur.notation = Some(SpannerNotation::Slur(Slur {
            placement: Some(Placement::Below),
            visible: true,
        }));
        score.add_spanner(slur);

        let mut glissando = Glissando::new(GlissandoType::Straight(LineStyle::Dashed));
        glissando.text = "gliss.".to_owned();
        let mut spanner = Spanner::new(
            SpannerType::Glissando,
            SpannerAnchor::new(Tick::ZERO, Track::new(StaffId::from_index(0), 0)),
            SpannerAnchor::new(Tick::new(1, 4), Track::new(StaffId::from_index(0), 0)),
        );
        spanner.notation = Some(SpannerNotation::Glissando(glissando));
        score.add_spanner(spanner);

        let xml = exporter(&score);
        assert!(
            xml.contains(
                "<slur type=\"start\" orientation=\"under\" placement=\"below\" number=\"1\"/>"
            ),
            "{xml}"
        );
        assert!(xml.contains("<slur type=\"stop\" number=\"1\"/>"), "{xml}");
        assert!(
            xml.contains("<slide line-type=\"dashed\" number=\"1\" type=\"start\">gliss.</slide>"),
            "{xml}"
        );
        assert!(
            xml.contains("<slide line-type=\"dashed\" number=\"1\" type=\"stop\"/>"),
            "{xml}"
        );
    }

    #[test]
    fn trille_et_doigtes() {
        let (mut score, _) = partition();
        let mut trill = Spanner::new(
            SpannerType::Trill,
            SpannerAnchor::new(Tick::ZERO, Track::new(StaffId::from_index(0), 0)),
            SpannerAnchor::new(Tick::new(1, 4), Track::new(StaffId::from_index(0), 0)),
        );
        trill.notation = Some(SpannerNotation::Trill(Trill::new(TrillType::TrillLine)));
        score.add_spanner(trill);

        let note_id = *accord(&mut score, 0).notes.first().expect("note");
        score.note_mut(note_id).fingerings = vec![Fingering::new("2"), Fingering::new("p")];

        let xml = exporter(&score);
        assert!(xml.contains("<trill-mark/>"), "{xml}");
        assert!(
            xml.contains("<wavy-line type=\"start\" number=\"1\"/>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wavy-line type=\"stop\" number=\"1\"/>"),
            "{xml}"
        );
        assert!(xml.contains("<fingering>2</fingering>"), "{xml}");
        assert!(xml.contains("<pluck>p</pluck>"), "{xml}");
    }

    #[test]
    fn respirations_et_lignes_d_accord() {
        let (mut score, _) = partition();
        let mut segment = Segment::new(SegmentType::Breath, Tick::new(1, 4), score.staves.len());
        segment.set_element(
            Track::new(StaffId::from_index(0), 0),
            SegmentItem::Breath(Breath {
                kind: BreathType::CaesuraCurved,
                placement: Placement::Below,
            }),
        );
        score.measures[0].add_segment(segment);
        accord(&mut score, 0).chord_line = Some(ChordLineType::Fall);

        let xml = exporter(&score);
        assert!(
            xml.contains("<caesura placement=\"below\">curved</caesura>"),
            "{xml}"
        );
        assert!(xml.contains("<falloff/>"), "{xml}");
    }
}
