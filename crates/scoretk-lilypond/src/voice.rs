// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/segment.h`, `engraving/dom/measure.h` et
// `engraving/dom/chordrest.h` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026 : parcours d'une voix
// mesure par mesure et rattachement des annotations à la note la plus proche.
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

//! Écriture d'une voix.
//!
//! Une voix LilyPond est une suite linéaire. Le modèle, lui, range les objets
//! en segments verticaux : accords et silences d'un côté, annotations de
//! l'autre, spanners dans une arène à part. Cette module fait la jonction :
//!
//! 1. les accords et silences deviennent la trame de la voix ;
//! 2. les changements de contexte (clé, armure, mesure, levée) sont posés au
//!    début de la mesure concernée ;
//! 3. les annotations et les spanners sont rattachés à la note la plus proche :
//!    la dernière note au tick de fin pour une fermeture (`)`, `\!`), la
//!    première note au tick de début pour une ouverture (`(`, `\<`).
//!
//! Les silences de remplissage sont ajoutés pour que chaque voix couvre
//! exactement la mesure, même quand un fichier ne note pas les silences de ses
//! voix secondaires.

use std::collections::BTreeMap;

use mscz2xml::model::{
    Annotation, BarLineType, ChordRest, Fraction, JumpType, MarkerType, Measure, RepeatMark, Score,
    SegmentType, SpannerType, StaffId, Tick, Track, TupletId, VOICES,
};

use crate::chord;
use crate::duration::{length_token, measure_rest_token};
use crate::event::{before, Event};
use crate::structure::{self, StructToken, Structure};
use crate::text;

/// Un accord ou un silence de la trame d'une voix.
struct Item<'a> {
    /// Mesure d'appartenance.
    measure: usize,
    /// Tick de début.
    tick: Tick,
    /// Tick de fin (durée réelle, mesure comprise).
    end_tick: Tick,
    /// Accord ou silence.
    chord_rest: &'a ChordRest,
}

/// Écrit le corps d'une variable de voix, sans l'enveloppe
/// `\fixed c' { … }`.
///
/// `voice_count` est le nombre de voix écrites pour la portée : au-delà d'une,
/// chaque voix reçoit `\voiceOne`, `\voiceTwo`… pour que LilyPond gère les
/// collisions et les hampes comme MuseScore.
pub(crate) fn render_body(
    score: &Score,
    structure: &Structure,
    staff: usize,
    voice: usize,
    voice_count: usize,
) -> String {
    let items = collect_items(score, staff, voice);
    let mut pre_by_measure: Vec<Vec<Event>> = vec![Vec::new(); score.measures.len()];
    let mut pre_by_item: Vec<Vec<Event>> = vec![Vec::new(); items.len()];
    let mut post_by_item: Vec<Vec<Event>> = vec![Vec::new(); items.len()];
    let has_voice = |candidate: usize| voice_has_content(score, staff, candidate);

    // Changements de contexte portés par les segments de la portée : ils
    // n'appartiennent qu'à la voix 0 dans les fichiers MuseScore.
    for (measure_index, measure) in score.measures.iter().enumerate() {
        for segment in &measure.segments {
            if segment.kind.is_bar_line_type() || segment.kind == SegmentType::Invalid {
                continue;
            }
            for (element_track, item) in segment.iter_elements() {
                if element_track.staff().index() != staff || element_track.voice() as usize != voice
                {
                    continue;
                }
                let text = match item {
                    mscz2xml::model::SegmentItem::Clef(clef) => crate::pitch::clef_command(clef),
                    mscz2xml::model::SegmentItem::KeySig(key) => {
                        Some(crate::pitch::key_signature(key))
                    }
                    mscz2xml::model::SegmentItem::TimeSig(time) => {
                        // Indication locale de MuseScore : la portée affiche
                        // 3/4 mais partage une mesure de 9/8 avec les autres.
                        // `measureLength` garde la mesure réelle, sinon
                        // LilyPond signale un contrôle de mesure raté.
                        let mut command = format!("\\time {}/{}", time.numerator, time.denominator);
                        if time.measure_len() != measure.len && measure.len.is_positive() {
                            command.push_str(&format!(
                                " \\set Timing.measureLength = #{}/{}",
                                measure.len.numerator(),
                                measure.len.denominator()
                            ));
                        }
                        Some(command)
                    }
                    _ => None,
                };
                if let Some(text) = text {
                    pre_by_measure[measure_index].push(Event::new(
                        segment.tick,
                        match item {
                            mscz2xml::model::SegmentItem::Clef(_) => before::CLEF,
                            mscz2xml::model::SegmentItem::KeySig(_) => before::KEY,
                            _ => before::TIME,
                        },
                        text,
                    ));
                }
            }
        }
    }

    // Levée : première mesure plus courte que sa durée nominale.
    if voice == 0 {
        if let Some(first) = score.measures.first() {
            if first.len.is_positive() && first.len < first.nominal_len {
                pre_by_measure[0].push(Event::new(
                    first.start_tick,
                    before::PARTIAL,
                    format!("\\partial {}", length_token(first.len)),
                ));
            }
        }
    }

    // Sauts de section (voix 0 uniquement : le saut est global).
    if voice == 0 {
        for (measure_index, measure) in score.measures.iter().enumerate() {
            if measure.section_break_before {
                pre_by_measure[measure_index].push(Event::new(
                    measure.start_tick,
                    before::SECTION,
                    "\\section",
                ));
            }
        }
    }

    // Textes de cadre (`TBox`) : accrochés à la première voix de la première
    // portée, à la mesure qu'ils précèdent. Un `\mark` y serait rejeté quand
    // un repère de répétition tombe au même endroit : le texte passe donc par
    // un `\markup` attaché à la note.
    if staff == 0 && voice == 0 {
        for tbox in &score.tbox_texts {
            let measure_index = tbox.measure.min(score.measures.len().saturating_sub(1));
            if let Some(measure) = score.measures.get(measure_index) {
                let plain = tbox.text.plain().trim();
                if !plain.is_empty() {
                    attach_post(
                        &mut post_by_item,
                        &items,
                        Event::new(
                            measure.start_tick,
                            crate::event::after::TEXT,
                            text::attach_markup(plain, tbox.placement),
                        ),
                    );
                }
            }
        }
    }

    // Annotations de segment.
    for measure in &score.measures {
        for segment in &measure.segments {
            for annotation in &segment.annotations {
                let declared = annotation.track;
                if declared.staff().index() != staff {
                    continue;
                }
                let target = if has_voice(declared.voice() as usize) {
                    declared.voice() as usize
                } else {
                    0
                };
                if target != voice {
                    continue;
                }
                // Tempo et repères de répétition sont des événements de
                // système : seule la première voix de la portée les écrit.
                let system_level = matches!(
                    &annotation.annotation,
                    Annotation::Tempo(_) | Annotation::RehearsalMark(_)
                );
                if system_level && voice != 0 {
                    continue;
                }
                let mut events = crate::annotations::events(&annotation.annotation, segment.tick);
                if events.is_empty() {
                    continue;
                }
                let measure_index = structure::measure_at(&score.measures, segment.tick);
                for event in events.drain(..) {
                    if system_level {
                        // Posé au tick réel : deux repères d'une même mesure
                        // resteraient sinon sur le même temps, ce que
                        // LilyPond refuse.
                        let attached = if event.text.starts_with("\\mark ") {
                            attach_mark(&mut pre_by_item, &mut post_by_item, &items, event.clone())
                        } else {
                            attach_pre(&mut pre_by_item, &items, event.clone())
                        };
                        if !attached {
                            pre_by_measure[measure_index].push(event);
                        }
                    } else {
                        attach_post(&mut post_by_item, &items, event);
                    }
                }
            }
        }
    }

    // Marqueurs et sauts de reprise (`Measure::el()`) : MuseScore les range
    // dans la première partie, et LilyPond n'a qu'un `\mark` par instant.
    if voice == 0 && staff == 0 {
        for measure in &score.measures {
            for mark in &measure.repeat_marks {
                let event = match mark {
                    RepeatMark::Marker(marker) => {
                        let plain = if marker.text.trim().is_empty() {
                            marker_name(marker.kind).to_owned()
                        } else {
                            marker.text.trim().to_owned()
                        };
                        let tick = if marker.kind.is_start_marker() {
                            measure.start_tick
                        } else {
                            measure.end_tick()
                        };
                        Event::new(
                            tick,
                            before::MARK,
                            format!("\\mark {}", text::markup(&text::escape(&plain))),
                        )
                    }
                    RepeatMark::Jump(jump) => {
                        let plain = if jump.text.trim().is_empty() {
                            jump_name(jump.kind).to_owned()
                        } else {
                            jump.text.trim().to_owned()
                        };
                        Event::new(
                            measure.end_tick(),
                            before::MARK,
                            format!("\\mark {}", text::markup(&text::escape(&plain))),
                        )
                    }
                };
                if event.text == "\\mark \\markup { \"\" }" {
                    continue;
                }
                attach_mark(&mut pre_by_item, &mut post_by_item, &items, event);
            }
        }
    }

    // Spanners.
    for (spanner_index, spanner) in score.spanners.iter().enumerate() {
        if spanner.start.track.staff().index() != staff {
            continue;
        }
        let staff_level = crate::spanners::is_staff_level(&spanner.kind);
        let declared = spanner.start.track.voice() as usize;
        let target = if staff_level {
            0
        } else if has_voice(declared) {
            declared
        } else {
            0
        };
        if target != voice {
            continue;
        }
        // Un soufflet dont le départ et l'arrivée tombent sur le même silence
        // n'a pas de longueur : LilyPond le refuse (« crescendo non
        // terminé ») et il ne s'entend pas. On l'omet.
        if matches!(spanner.kind, SpannerType::Hairpin(_)) {
            let start = post_target(&items, spanner.start.tick);
            let end = post_target(&items, spanner.end.tick);
            if start.is_some() && start == end {
                if let Some(index) = start {
                    if items[index].chord_rest.is_rest() {
                        continue;
                    }
                }
            }
        }
        for event in crate::spanners::events(spanner, spanner_index + 1) {
            attach_post(&mut post_by_item, &items, event);
        }
    }

    // Tri des événements et écriture.
    for events in &mut pre_by_measure {
        events.sort_by_key(|event| (event.tick, event.order));
    }
    for events in &mut pre_by_item {
        events.sort_by_key(|event| (event.tick, event.order));
    }
    for events in &mut post_by_item {
        events.sort_by_key(|event| event.order);
    }

    let mut emitter = Emitter::new();
    if voice_count > 1 {
        emitter.line(&format!("\\voice{}", voice_name(voice)));
    }

    let mut item_index = 0usize;
    for (measure_index, measure) in score.measures.iter().enumerate() {
        for token in structure.before_measure(measure_index) {
            emitter.structure(*token);
        }
        for event in &pre_by_measure[measure_index] {
            emitter.token(&event.text);
        }
        emitter.flush_line();

        let bar_check = time_signature_matches(measure, staff);
        let mut cursor = measure.start_tick;
        let measure_items_start = item_index;
        let mut tuplets: Vec<OpenTuplet> = Vec::new();

        // Silence multi-mesures : le marqueur est posé sur la première mesure
        // du groupe et les suivantes n'existent pas dans le modèle. On écrit
        // `R` avec le multiplicateur, à condition que la voix n'y ait que des
        // silences.
        let mut multi_rest_done = false;
        if let Some(count) = measure.multi_measure_rest.filter(|count| *count > 1) {
            let end_of_measure = item_index
                + items[item_index..]
                    .iter()
                    .take_while(|item| item.measure == measure_index)
                    .count();
            let all_rests = items[item_index..end_of_measure]
                .iter()
                .all(|item| item.chord_rest.is_rest());
            if all_rests {
                let len = if measure.len.is_positive() {
                    measure.len
                } else {
                    measure.nominal_len
                };
                if len.is_positive() {
                    emitter.token(&format!("{}*{count}", measure_rest_token(len)));
                }
                item_index = end_of_measure;
                multi_rest_done = true;
            }
        }

        while item_index < items.len() && items[item_index].measure == measure_index {
            let item = &items[item_index];
            let chain = match item.chord_rest.tuplet {
                Some(id) => tuplet_chain(score, id),
                None => Vec::new(),
            };
            close_tuplets_until(&chain, &mut tuplets, &mut emitter);
            if item.tick > cursor {
                emitter.token(&fill_rest(cursor, item.tick, measure));
            }
            for event in &pre_by_item[item_index] {
                emitter.token(&event.text);
            }
            open_tuplets(score, &chain, &mut tuplets, &mut emitter);

            let transpose = transpose_at(score, staff, item.tick);
            let mut line = chord::render(score, item.chord_rest, measure.len, transpose);
            for event in &post_by_item[item_index] {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(&event.text);
            }
            emitter.token(&line);

            cursor = item.end_tick.max(item.tick);
            item_index += 1;
        }

        if multi_rest_done {
            // Rien : le silence multi-mesures couvre déjà la mesure.
        } else if item_index == measure_items_start {
            let len = if measure.len.is_positive() {
                measure.len
            } else {
                measure.nominal_len
            };
            if len.is_positive() {
                emitter.token(&measure_rest_token(len));
            }
        } else if cursor < measure.end_tick() {
            let end = measure.end_tick();
            if end > cursor {
                emitter.token(&fill_rest(cursor, end, measure));
            }
        }
        // Un n-olet ne franchit pas une barre de mesure dans MuseScore : tout
        // ce qui reste ouvert est fermé ici, même si le modèle lui donne un
        // tick de fin incohérent.
        close_tuplets_until(&[], &mut tuplets, &mut emitter);

        emitter.token(&bar_line_token(
            measure,
            measure_index + 1 == score.measures.len(),
            bar_check,
        ));
        emitter.flush_line();
        if voice == 0 && measure.line_break && measure_index + 1 < score.measures.len() {
            emitter.line("\\break");
        }
        for token in structure.after_measure(measure_index) {
            emitter.structure(*token);
        }
    }

    emitter.finish()
}

/// N-olet ouvert.
struct OpenTuplet {
    id: TupletId,
}

/// Ouvre les n-olets de la chaîne qui ne le sont pas encore.
fn open_tuplets(
    score: &Score,
    chain: &[TupletId],
    open: &mut Vec<OpenTuplet>,
    emitter: &mut Emitter,
) {
    for tuplet_id in chain {
        if open.iter().any(|entry| entry.id == *tuplet_id) {
            continue;
        }
        let tuplet = score.tuplet(*tuplet_id);
        let (actual, normal) = chord::tuplet_ratio(tuplet);
        emitter.line(&format!("\\tuplet {actual}/{normal} {{"));
        emitter.depth += 1;
        open.push(OpenTuplet { id: *tuplet_id });
    }
}

/// Ferme les n-olets ouverts qui ne contiennent pas la chaîne donnée.
fn close_tuplets_until(chain: &[TupletId], open: &mut Vec<OpenTuplet>, emitter: &mut Emitter) {
    while let Some(entry) = open.last() {
        if chain.contains(&entry.id) {
            break;
        }
        open.pop();
        emitter.depth = emitter.depth.saturating_sub(1);
        emitter.line("}");
    }
}

/// Chaîne des n-olets d'un accord, du plus externe au plus interne.
fn tuplet_chain(score: &Score, id: TupletId) -> Vec<TupletId> {
    let mut chain = Vec::new();
    let mut current = Some(id);
    let mut guard = 0;
    while let Some(tuplet_id) = current {
        chain.push(tuplet_id);
        current = score.tuplet(tuplet_id).parent;
        guard += 1;
        if guard > 8 {
            break;
        }
    }
    chain.reverse();
    chain
}

/// Remplit un trou par un silence de la bonne longueur.
fn fill_rest(start: Tick, end: Tick, measure: &Measure) -> String {
    let len = end - start;
    if len <= Fraction::ZERO {
        return String::new();
    }
    if start == measure.start_tick && len == measure.len {
        measure_rest_token(len)
    } else {
        format!("r{}", length_token(len))
    }
}

/// Barre de fin de mesure.
///
/// Le contrôle de mesure (`|`) est omis quand l'indication de mesure de la
/// portée ne correspond pas à la longueur réelle de la mesure : c'est le cas
/// des indications locales de MuseScore, où une portée en 3/4 partage une
/// mesure de 9/8 avec les autres. LilyPond signalerait alors un échec de
/// contrôle à chaque barre sans que la musique soit fausse.
fn bar_line_token(measure: &Measure, last: bool, bar_check: bool) -> String {
    let explicit = match measure.end_bar_line {
        BarLineType::End => Some("\\bar \"|.\""),
        BarLineType::Double => Some("\\bar \"||\""),
        BarLineType::Dotted | BarLineType::DoubleDotted => Some("\\bar \";\""),
        _ => None,
    };
    if let Some(token) = explicit {
        return token.to_owned();
    }
    if last {
        return "\\bar \"|.\"".to_owned();
    }
    if bar_check {
        "|".to_owned()
    } else {
        String::new()
    }
}

/// Vrai si l'indication de mesure de la portée correspond à la mesure.
fn time_signature_matches(measure: &Measure, staff: usize) -> bool {
    for segment in &measure.segments {
        for (track, item) in segment.iter_elements() {
            if track.staff().index() != staff {
                continue;
            }
            if let mscz2xml::model::SegmentItem::TimeSig(time) = item {
                if time.measure_len() != measure.len {
                    return false;
                }
            }
        }
    }
    true
}

/// Accords et silences d'une voix, dans l'ordre.
fn collect_items(score: &Score, staff: usize, voice: usize) -> Vec<Item<'_>> {
    let track = Track::new(StaffId::from_index(staff), voice as u8);
    let mut items = Vec::new();
    for (measure_index, measure) in score.measures.iter().enumerate() {
        let mut last_tick: Option<Tick> = None;
        for segment in &measure.segments {
            let Some(chord_rest) = segment.chord_rest(track) else {
                continue;
            };
            if last_tick == Some(segment.tick) {
                continue;
            }
            last_tick = Some(segment.tick);
            let length = chord_rest.ticks_in_measure(measure.len);
            items.push(Item {
                measure: measure_index,
                tick: segment.tick,
                end_tick: segment.tick + length,
                chord_rest,
            });
        }
    }
    items
}

/// Rattache un événement « après la note » à la dernière note au plus tard au
/// tick de l'événement.
fn attach_post(post_by_item: &mut [Vec<Event>], items: &[Item<'_>], event: Event) {
    if let Some(index) = post_target(items, event.tick) {
        post_by_item[index].push(event);
    }
}

/// Nom affichable d'un marqueur quand le fichier n'en donne pas.
fn marker_name(kind: MarkerType) -> &'static str {
    match kind {
        MarkerType::Segno | MarkerType::VarSegno => "Segno",
        MarkerType::Coda | MarkerType::VarCoda => "Coda",
        MarkerType::Codetta => "Codetta",
        MarkerType::Fine => "Fine",
        MarkerType::ToCoda | MarkerType::ToCodaSym => "To Coda",
        MarkerType::DaCoda => "Da Coda",
        MarkerType::DaDoubleCoda => "Da Double Coda",
        MarkerType::User => "",
    }
}

/// Nom affichable d'un saut quand le fichier n'en donne pas.
fn jump_name(kind: JumpType) -> &'static str {
    match kind {
        JumpType::Dc => "D.C.",
        JumpType::DcAlFine => "D.C. al Fine",
        JumpType::DcAlCoda => "D.C. al Coda",
        JumpType::DsAlCoda => "D.S. al Coda",
        JumpType::DsAlFine => "D.S. al Fine",
        JumpType::Ds => "D.S.",
        JumpType::DcAlDoubleCoda => "D.C. al Double Coda",
        JumpType::DsAlDoubleCoda => "D.S. al Double Coda",
        JumpType::Dss => "D.S.S.",
        JumpType::DssAlCoda => "D.S.S. al Coda",
        JumpType::DssAlDoubleCoda => "D.S.S. al Double Coda",
        JumpType::DssAlFine => "D.S.S. al Fine",
        JumpType::User => "",
    }
}

/// Rattache un repère de répétition (`\mark`).
///
/// LilyPond n'accepte qu'un seul repère par instant : un repère identique est
/// ignoré, un repère différent au même endroit devient un `\markup` au-dessus
/// de la note.
fn attach_mark(
    pre_by_item: &mut [Vec<Event>],
    post_by_item: &mut [Vec<Event>],
    items: &[Item<'_>],
    event: Event,
) -> bool {
    if items.is_empty() {
        return false;
    }
    let index = items
        .partition_point(|item| item.tick < event.tick)
        .min(items.len() - 1);
    if let Some(existing) = pre_by_item[index]
        .iter()
        .find(|existing| existing.order == before::MARK)
    {
        if existing.text == event.text {
            return true;
        }
        let markup = event.text.trim_start_matches("\\mark ").to_owned();
        post_by_item[index].push(Event::new(
            event.tick,
            crate::event::after::TEXT,
            format!("^{markup}"),
        ));
        return true;
    }
    pre_by_item[index].push(event);
    true
}

/// Rattache un événement « avant la note » à la première note au tick de
/// l'événement ou, à défaut, à la suivante. Renvoie faux s'il n'y a aucune
/// note.
fn attach_pre(pre_by_item: &mut [Vec<Event>], items: &[Item<'_>], event: Event) -> bool {
    let position = items.partition_point(|item| item.tick < event.tick);
    let Some(index) = items.get(position).map(|_| position).or_else(|| {
        if items.is_empty() {
            None
        } else {
            Some(items.len() - 1)
        }
    }) else {
        return false;
    };
    pre_by_item[index].push(event);
    true
}

/// Indice de la note à laquelle s'attache un événement au tick donné : la
/// dernière note au plus tard à ce tick.
fn post_target(items: &[Item<'_>], tick: Tick) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    Some(
        items
            .partition_point(|item| item.tick <= tick)
            .saturating_sub(1),
    )
}

/// Transposition de l'instrument en vigueur à un tick.
pub(crate) fn transpose_at(score: &Score, staff: usize, tick: Tick) -> i32 {
    let Some(staff_data) = score.staves.get(staff) else {
        return 0;
    };
    let part = score.part(staff_data.part);
    part.instrument_at(tick)
        .transpose
        .map_or(0, |transpose| transpose.chromatic)
}

/// Vrai si une voix contient au moins un accord ou un silence.
pub(crate) fn voice_has_content(score: &Score, staff: usize, voice: usize) -> bool {
    let track = Track::new(StaffId::from_index(staff), voice as u8);
    score.measures.iter().any(|measure| {
        measure
            .segments
            .iter()
            .any(|segment| segment.chord_rest(track).is_some())
    })
}

/// Construit le texte d'une variable de voix, avec indentation.
///
/// Les jetons d'une mesure s'accumulent sur une ligne, coupée à 78 colonnes ;
/// les commandes de structure (`\repeat`, `\alternative`) et les n-olets
/// forcent une ligne à eux.
struct Emitter {
    out: String,
    depth: usize,
    line: String,
}

/// Largeur visée pour les lignes de musique.
const LINE_WIDTH: usize = 78;

impl Emitter {
    fn new() -> Self {
        Self {
            out: String::new(),
            depth: 0,
            line: String::new(),
        }
    }

    /// Indentation d'une ligne au niveau courant.
    fn indent(&self) -> String {
        "  ".repeat(self.depth + 1)
    }

    /// Ajoute un jeton à la ligne courante, en coupant si elle devient trop
    /// longue.
    fn token(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let width = self.indent().len() + self.line.len() + 1 + text.len();
        if !self.line.is_empty() && width > LINE_WIDTH {
            self.flush_line();
        }
        if !self.line.is_empty() {
            self.line.push(' ');
        }
        self.line.push_str(text);
    }

    /// Termine la ligne courante.
    fn flush_line(&mut self) {
        if self.line.is_empty() {
            return;
        }
        for _ in 0..self.depth + 1 {
            self.out.push_str("  ");
        }
        self.out.push_str(&self.line);
        self.out.push('\n');
        self.line.clear();
    }

    /// Écrit une ligne complète, après avoir terminé la ligne courante.
    fn line(&mut self, text: &str) {
        self.flush_line();
        if text.is_empty() {
            return;
        }
        for _ in 0..=self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Écrit un jeton de structure et ajuste l'indentation.
    fn structure(&mut self, token: StructToken) {
        match token {
            StructToken::OpenRepeat(plays) => {
                self.line(&format!("\\repeat volta {plays} {{"));
                self.depth += 1;
            }
            StructToken::CloseRepeatBody | StructToken::CloseEnding => {
                self.depth = self.depth.saturating_sub(1);
                self.line("}");
            }
            StructToken::OpenAlternative => {
                self.line("\\alternative {");
                self.depth += 1;
            }
            StructToken::OpenEnding => {
                self.line("{");
                self.depth += 1;
            }
            StructToken::CloseAlternative => {
                self.depth = self.depth.saturating_sub(1);
                self.line("}");
            }
        }
    }

    fn finish(mut self) -> String {
        self.flush_line();
        self.out
    }
}

/// Paroles d'une voix, par couplet.
///
/// Renvoie une entrée `(couplet, syllabes)` par couplet, dans l'ordre des
/// numéros. Chaque accord sans syllabe pour un couplet donne `\skip 1`, pour
/// que `\lyricsto` reste aligné ; les silences sont ignorés, LilyPond les
/// saute de lui-même.
pub(crate) fn lyrics_verses(
    score: &Score,
    staff: usize,
    voice: usize,
) -> BTreeMap<usize, Vec<String>> {
    let items = collect_items(score, staff, voice);
    let mut verses: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for item in &items {
        for lyrics in &item.chord_rest.lyrics {
            verses.entry(lyrics.verse).or_default();
        }
    }
    if verses.is_empty() {
        return verses;
    }
    for (verse, syllables) in &mut verses {
        for item in &items {
            if !item.chord_rest.is_chord() {
                continue;
            }
            match item
                .chord_rest
                .lyrics
                .iter()
                .find(|lyrics| lyrics.verse == *verse)
            {
                Some(lyrics) => syllables.push(syllable(lyrics)),
                None => syllables.push("\\skip 1".to_owned()),
            }
        }
    }
    verses
}

/// Syllabe LilyPond d'une entrée de paroles.
fn syllable(lyrics: &mscz2xml::model::Lyrics) -> String {
    let mut text_value = text::escape(&lyrics.text);
    match lyrics.syllabic {
        mscz2xml::model::LyricsSyllabic::Begin | mscz2xml::model::LyricsSyllabic::Middle => {
            text_value.push_str(" --");
        }
        mscz2xml::model::LyricsSyllabic::Single | mscz2xml::model::LyricsSyllabic::End => {}
    }
    if lyrics.melisma.is_some() {
        text_value.push_str(" __");
    }
    text_value
}

/// Nom de voix LilyPond (`One`, `Two`…).
pub(crate) fn voice_name(voice: usize) -> &'static str {
    match voice {
        0 => "One",
        1 => "Two",
        2 => "Three",
        _ => "Four",
    }
}

/// Nombre de voix écrites pour une portée.
pub(crate) fn written_voices(score: &Score, staff: usize) -> usize {
    let mut count = 1;
    for voice in 1..VOICES {
        if voice_has_content(score, staff, voice) {
            count = voice + 1;
        }
    }
    count
}

/// Numéral anglais utilisé dans les noms de variables (`One`, `Two`…).
fn number_word(index: usize) -> String {
    const WORDS: [&str; 10] = [
        "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
    ];
    WORDS
        .get(index)
        .map_or_else(|| (index + 1).to_string(), |word| (*word).to_owned())
}

/// Nom de variable d'une portée (`staffOne`, `staffTwo`…).
pub(crate) fn staff_variable(staff: usize) -> String {
    format!("staff{}", number_word(staff))
}

/// Nom de variable d'une voix (`staffOneVoiceTwo`…).
///
/// La même chaîne sert de `\new Voice = "…"` et de cible à `\lyricsto`.
pub(crate) fn voice_variable(staff: usize, voice: usize) -> String {
    format!("staff{}Voice{}", number_word(staff), number_word(voice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mscz2xml::model::{
        ChordRest, Duration, Measure, Part, Segment, SegmentItem, SegmentType, Track,
    };

    fn score_avec_deux_voix() -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        segment.set_element(
            Track::new(staff, 0),
            SegmentItem::ChordRest(ChordRest::rest(Duration::quarter())),
        );
        segment.set_element(
            Track::new(staff, 1),
            SegmentItem::ChordRest(ChordRest::full_measure_rest()),
        );
        measure.add_segment(segment);
        score.add_measure(measure);
        score
    }

    #[test]
    fn voix_ecrites() {
        let score = score_avec_deux_voix();
        // La voix 1 porte un silence de mesure entière : c'est un contenu
        // explicite, donc une voix écrite.
        assert_eq!(written_voices(&score, 0), 2);
    }

    #[test]
    fn noms_de_voix() {
        assert_eq!(voice_name(0), "One");
        assert_eq!(voice_name(1), "Two");
        assert_eq!(voice_name(3), "Four");
    }

    #[test]
    fn remplissage_des_mesures() {
        let measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        assert_eq!(fill_rest(Fraction::ZERO, Fraction::ONE, &measure), "R1");
        assert_eq!(fill_rest(Fraction::ZERO, Fraction::QUARTER, &measure), "r4");
        assert_eq!(fill_rest(Fraction::ONE, Fraction::ONE, &measure), "");
    }
}
