// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// L'écriture suit la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) et le comportement de
// `xml2abc.py` de Willem Vree (LGPL) ; aucune ligne de ce script n'est copiée.
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

//! Écriture d'une voix ABC (une portée).
//!
//! Le modèle range les objets en segments verticaux, ABC en une suite
//! linéaire : ce module fait la jonction, mesure par mesure. Les accords et
//! silences forment la trame ; les annotations et les spanners sont rattachés
//! à la note la plus proche, comme le fait `xml2abc.py` pour les directions
//! MusicXML.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{
    Annotation, Chord, ChordRest, Fraction, HairpinType, KeySig, LyricsSyllabic, Measure, Note,
    Score, SpannerType, StaffId, Tick, Track, TupletId,
};

use crate::writer;
use scoretk_abc_common::decorations;
use scoretk_abc_common::duration;
use scoretk_abc_common::pitch;
use scoretk_abc_common::text;

/// Changement de contexte en cours de mesure.
type ContextChange = (
    Tick,
    u8,
    Option<KeySig>,
    Option<scoretk_core::model::ClefType>,
    Option<scoretk_core::model::TimeSig>,
);

/// Un accord ou silence de la trame d'une voix.
struct Item<'a> {
    /// Mesure d'appartenance.
    measure: usize,
    /// Tick de début.
    tick: Tick,
    /// Tick de fin.
    end_tick: Tick,
    /// Accord ou silence.
    chord_rest: &'a ChordRest,
}

/// Écrit une voix complète, en-tête `V:` compris.
pub(crate) fn render_voice(
    score: &Score,
    spec: writer::VoiceSpec,
    initial_key: &KeySig,
    out: &mut String,
) {
    let staff = spec.staff;
    let model_voice = spec.voice;
    out.push_str(&voice_header(score, spec));
    out.push('\n');

    let items = collect_items(score, staff, model_voice);
    // Port de `xml2abc.py::writePart` (lignes 331-334) : les couplets sont
    // collectés sur toute la voix, pas seulement sur la mesure courante,
    // pour qu'une mesure chantée sans paroles porte quand même son `w:`.
    let all_verses: BTreeSet<usize> = items
        .iter()
        .flat_map(|item| item.chord_rest.lyrics.iter().map(|lyrics| lyrics.verse))
        .collect();
    let tuplet_counts = tuplet_counts(score, &items);
    let mut ranges = Vec::with_capacity(score.measures.len());
    let mut cursor = 0usize;
    for measure_index in 0..score.measures.len() {
        let start = cursor;
        while cursor < items.len() && items[cursor].measure == measure_index {
            cursor += 1;
        }
        ranges.push(start..cursor);
    }

    let context_staff = writer::context_staff(score, staff);
    let mut key = initial_key.clone();
    let mut time = writer::initial_time(score);
    let mut clef = writer::initial_clef(score, staff);

    let mut open_tuplets: Vec<TupletId> = Vec::new();
    let mut pending_word = false;
    for (measure_index, measure) in score.measures.iter().enumerate() {
        let range = ranges[measure_index].clone();
        let mut before: Vec<Vec<String>> = vec![Vec::new(); range.len()];
        let mut after: Vec<Vec<String>> = vec![Vec::new(); range.len()];

        draw_chords(score, &items, range.clone(), &mut before, &mut after);
        draw_annotations(
            measure,
            staff,
            model_voice,
            score,
            &items,
            range.clone(),
            &mut before,
        );
        draw_spanners(
            score,
            measure,
            staff,
            model_voice,
            &items,
            range.clone(),
            &mut before,
            &mut after,
            measure_index + 1 == score.measures.len(),
        );
        draw_context_changes(
            score,
            measure,
            staff,
            context_staff,
            &mut key,
            &mut clef,
            &items,
            range.clone(),
            &mut before,
        );
        // `abc2midi` refuse un jeton court (`.`, `u`, `v`) immédiatement suivi
        // d'une décoration `!…!` (`.!mp!d` est « malformed note »). Comme
        // `xml2abc.py` (`doDirection` puis `addChord`), les `!…!` sont écrits
        // d'abord, les jetons courts juste avant la note.
        for tokens in &mut before {
            tokens
                .sort_by_key(|token| !matches!(token.as_bytes().first(), Some(b'!') | Some(b'"')));
        }

        let mut line = String::new();
        let mut prefixes: Vec<String> = Vec::new();

        let (new_clef, _, _) = writer::measure_context(score, measure_index, Some(staff));
        let (_, new_key, new_time) =
            writer::measure_context(score, measure_index, Some(context_staff));
        let mut key_changed = false;
        if let Some(value) = new_key {
            if value.concert_key != key.concert_key || value.mode != key.mode {
                key = value;
                key_changed = true;
            }
        }
        if let Some(value) = new_time {
            if value.is_valid()
                && (value.numerator != time.numerator || value.denominator != time.denominator)
            {
                time = value;
                prefixes.push(format!("[M:{}/{}]", time.numerator, time.denominator));
            }
        }
        let clef_changed = new_clef.is_some_and(|value| Some(value.transposed) != clef);
        if let Some(value) = new_clef {
            clef = Some(value.transposed);
        }
        // Port de `xml2abc.py::doAttr` (lignes 1329-1331 :
        // `clefOct[n] = -clef-octave-change`) et `mkNote` (ligne 1154 :
        // `oct += clefOct[...]`) : `abc2midi` ignore le `-8`/`+8` des clefs
        // ABC, donc xml2abc décale les hauteurs écrites d'autant d'octaves.
        let clef_octave = clef.map_or(0, |value| i32::from(value.octave_change()));
        if key_changed || clef_changed {
            let mut field = format!("[K:{}", pitch::key_field(&key));
            if let Some(clef_value) =
                clef.and_then(|clef| pitch::clef_name(&scoretk_core::model::Clef::new(clef)))
            {
                field.push_str(&format!(" clef={clef_value}"));
            }
            field.push(']');
            prefixes.push(field);
        }

        if measure.repeat_start() {
            prefixes.push("|:".to_owned());
        }
        if let Some(volta) = volta_label(score, measure, staff) {
            prefixes.push(volta);
        }

        let mut accidental_state = pitch::AccidentalState::new(key.transposed_key.fifths());

        if range.is_empty() {
            let len = if measure.len.is_positive() {
                measure.len
            } else {
                measure.nominal_len
            };
            if len.is_positive() {
                line.push('Z');
            }
        } else {
            let mut cursor_tick = measure.start_tick;
            for (offset, item) in items[range.clone()].iter().enumerate() {
                if item.tick > cursor_tick {
                    push_fill_rest(&mut line, cursor_tick, item.tick, measure);
                }
                if let Some(id) = item.chord_rest.tuplet {
                    for tuplet_id in tuplet_chain(score, id) {
                        if !open_tuplets.contains(&tuplet_id) {
                            let (nested, notes) =
                                tuplet_counts.get(&tuplet_id).copied().unwrap_or((0, 0));
                            let token = tuplet_open(score, tuplet_id, nested, notes);
                            if !token.is_empty() {
                                line.push_str(&token);
                                open_tuplets.push(tuplet_id);
                            }
                        }
                    }
                }
                let transpose = transpose_at(score, staff, item.tick) + 12 * clef_octave;
                let mut grace = String::new();
                let token = render_item(
                    score,
                    measure,
                    item.chord_rest,
                    &mut accidental_state,
                    transpose,
                    &mut grace,
                );
                // § 4.20 de la norme ABC 2.1 : l'ordre d'un accord est
                // `<grace notes>`, `<chord symbols>`, `<annotations>/<decorations>`,
                // `<accidentals>`, `<note>`. Le groupe d'agréments précède donc
                // les décorations ; sinon `abc2midi` rejette la note comme
                // « Malformed note ». `xml2abc.py::mkNote` (ligne 1217) ouvre
                // le groupe avant que `doDirection` (lignes 1040-1054) n'ajoute
                // les décorations, et `doMaat` (ligne 656) les écrit dans cet
                // ordre.
                line.push_str(&grace);
                for token in &before[offset] {
                    line.push_str(token);
                }
                line.push_str(&token);
                for token in &after[offset] {
                    line.push_str(token);
                }
                cursor_tick = item.end_tick.max(item.tick);
            }
            if cursor_tick < measure.end_tick() {
                push_fill_rest(&mut line, cursor_tick, measure.end_tick(), measure);
            }
        }

        let bar = end_bar_line(measure, measure_index + 1 == score.measures.len());
        let mut full_line = prefixes;
        if !line.is_empty() {
            full_line.push(line);
        }
        if !bar.is_empty() {
            full_line.push(bar);
        }
        out.push_str(&full_line.join(" "));
        out.push('\n');

        for lyric_line in lyrics_lines(&items, &all_verses, range.clone(), &mut pending_word) {
            out.push_str(&lyric_line);
            out.push('\n');
        }
    }
}

/// En-tête `V:` d'une voix.
fn voice_header(score: &Score, spec: writer::VoiceSpec) -> String {
    let staff = spec.staff;
    let name = score
        .staves
        .get(staff)
        .and_then(|value| score.parts.get(value.part.index()))
        .map(|part| {
            if part.long_name.trim().is_empty() {
                part.name.clone()
            } else {
                part.long_name.clone()
            }
        })
        .unwrap_or_default();
    let mut header = format!("V:{}", spec.number);
    if !name.trim().is_empty() {
        header.push_str(&format!(" name=\"{}\"", text::escape_annotation(&name)));
    }
    if let Some(clef) = writer::initial_clef(score, staff) {
        if let Some(name) = pitch::clef_name(&scoretk_core::model::Clef::new(clef)) {
            header.push_str(&format!(" clef={name}"));
        }
    }
    // Port de `xml2abc.py::doAttr` (lignes 1278 et 1319-1332) : la
    // transposition chromatique, augmentée de l'`octave-change` que l'import
    // MusicXML a déjà repliée dans `Transpose::chromatic`, est écrite dans
    // l'en-tête `V:`. Sans elle `abc2midi` jouerait la hauteur notée au lieu
    // de la hauteur concert, comme le MIDI de référence de MuseScore.
    let debut = score
        .measures
        .first()
        .map_or(Tick::ZERO, |measure| measure.start_tick);
    // Seule la transposition de l'instrument : la compensation d'ottava est
    // déjà appliquée aux hauteurs écrites et ne doit pas être rejouée.
    let transposition = instrument_transpose(score, staff, debut);
    if transposition != 0 {
        header.push_str(&format!(" transpose={transposition}"));
    }
    header
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
            let length = if chord_rest.is_full_measure_rest() {
                measure.len
            } else {
                duration::chord_rest_length(chord_rest)
            };
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

/// Transposition chromatique de l'instrument seul, sans compensation d'ottava.
fn instrument_transpose(score: &Score, staff: usize, tick: Tick) -> i32 {
    let Some(staff_data) = score.staves.get(staff) else {
        return 0;
    };
    let part = score.part(staff_data.part);
    part.instrument_at(tick)
        .transpose
        .map_or(0, |transpose| transpose.chromatic)
}

/// Décalage à retrancher de la hauteur du modèle pour obtenir la hauteur écrite.
fn transpose_at(score: &Score, staff: usize, tick: Tick) -> i32 {
    let instrument = instrument_transpose(score, staff, tick);
    // `pitch2xml` ajoute le décalage d'ottava à l'octave écrite.
    let ottava = score
        .spanners
        .iter()
        .filter(|spanner| {
            spanner.start.track.staff().index() == staff
                && spanner.start.tick <= tick
                && tick < spanner.end.tick
        })
        .map(|spanner| match spanner.kind {
            SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveUp) => 1,
            SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveDown) => -1,
            SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthUp) => 2,
            SpannerType::Ottava(scoretk_core::model::OttavaType::FifteenthDown) => -2,
            _ => 0,
        })
        .sum::<i32>();
    instrument - 12 * ottava
}

/// Vrai si une voix de la portée contient des accords ou silences.
pub(crate) fn voice_has_content(score: &Score, staff: usize, voice: usize) -> bool {
    let track = Track::new(StaffId::from_index(staff), voice as u8);
    score.measures.iter().any(|measure| {
        measure
            .segments
            .iter()
            .any(|segment| segment.chord_rest(track).is_some())
    })
}

/// Jeton d'un silence couvrant toute une mesure.
///
/// Port de `xml2abc.py::doMaat` (ligne 674) et de la grammaire des mesures
/// (`abc_grammar`, lignes 428-436) : `Z` est le silence de **mesure entière**,
/// dont la durée est celle de l'indication `M:` courante. Une mesure écrite
/// plus courte (ou plus longue) que `M:` doit être remplie par un `z` explicite
/// avec son multiplicateur ; sinon `abc2midi` décalerait toutes les attaques
/// suivantes d'autant.
fn rest_token(len: Fraction, measure: &Measure) -> String {
    if len == measure.nominal_len {
        "Z".to_owned()
    } else {
        format!("z{}", duration::multiplier(len, duration::unit()))
    }
}

/// Remplit un trou par un silence de la bonne longueur.
fn push_fill_rest(line: &mut String, start: Tick, end: Tick, measure: &Measure) {
    let len = end - start;
    if !len.is_positive() {
        return;
    }
    line.push_str(&rest_token(len, measure));
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

/// Nombre de groupes imbriqués et de notes directes de chaque n-olet.
fn tuplet_counts(score: &Score, items: &[Item<'_>]) -> BTreeMap<TupletId, (usize, usize)> {
    let mut counts: BTreeMap<TupletId, (BTreeSet<TupletId>, usize)> = BTreeMap::new();
    for item in items {
        let Some(id) = item.chord_rest.tuplet else {
            continue;
        };
        let chain = tuplet_chain(score, id);
        for (index, tuplet_id) in chain.iter().enumerate() {
            let entry = counts.entry(*tuplet_id).or_default();
            if let Some(child) = chain.get(index + 1) {
                entry.0.insert(*child);
            } else {
                entry.1 += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(|(id, (nested, notes))| (id, (nested.len(), notes)))
        .collect()
}

/// Jeton d'ouverture d'un n-olet (`(3`, `(5:4`, `(3:2:3`…).
fn tuplet_open(score: &Score, id: TupletId, nested: usize, notes: usize) -> String {
    let tuplet = score.tuplet(id);
    let (actual, normal) = tuplet.displayed_numbers();
    let actual = if actual > 0 {
        actual
    } else {
        tuplet.ratio.numerator()
    };
    let normal = if normal > 0 {
        normal
    } else {
        tuplet.ratio.denominator()
    };
    if actual == normal {
        return String::new();
    }
    let total = nested + notes;
    if (nested > 0 || total as i64 != actual) && total > 0 {
        format!("({actual}:{normal}:{total}")
    } else if normal == 2 {
        format!("({actual}")
    } else {
        format!("({actual}:{normal}")
    }
}

/// Barre de fin de mesure.
fn end_bar_line(measure: &Measure, last: bool) -> String {
    use scoretk_core::model::BarLineType;
    match measure.end_bar_line {
        BarLineType::EndStartRepeat => "::".to_owned(),
        BarLineType::EndRepeat => ":|".to_owned(),
        BarLineType::End => "|]".to_owned(),
        BarLineType::Double | BarLineType::DoubleShort => "||".to_owned(),
        BarLineType::Dotted | BarLineType::DoubleDotted => ";".to_owned(),
        _ if last => "|]".to_owned(),
        _ => "|".to_owned(),
    }
}

/// Étiquette de volta qui commence à cette mesure.
fn volta_label(score: &Score, measure: &Measure, staff: usize) -> Option<String> {
    for spanner in &score.spanners {
        if !matches!(spanner.kind, SpannerType::Volta) || !spanner.visible {
            continue;
        }
        if spanner.start.tick != measure.start_tick || spanner.start.track.staff().index() != staff
        {
            continue;
        }
        let endings = spanner
            .volta
            .as_ref()
            .map(|volta| volta.endings.clone())
            .unwrap_or_default();
        if endings.is_empty() {
            continue;
        }
        let numbers: Vec<String> = endings.iter().map(u8::to_string).collect();
        return Some(format!("[{}", numbers.join(",")));
    }
    None
}

/// Ajoute les décorations propres aux accords (articulations, ornements…).
fn draw_chords(
    score: &Score,
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    before: &mut [Vec<String>],
    _after: &mut [Vec<String>],
) {
    for (offset, item) in items[range].iter().enumerate() {
        let Some(chord) = item.chord_rest.chord_ref() else {
            continue;
        };
        for articulation in &chord.articulations {
            if !articulation.visible {
                continue;
            }
            if let Some(token) = decorations::articulation_token(&articulation.kind) {
                before[offset].push(token.to_owned());
            }
        }
        for ornament in &chord.ornaments {
            if !ornament.visible {
                continue;
            }
            if let Some(token) = decorations::ornament_token(&ornament.kind) {
                before[offset].push(format!("!{token}!"));
            }
        }
        if chord
            .arpeggio
            .as_ref()
            .is_some_and(|arpeggio| arpeggio.visible)
        {
            before[offset].push("!arpeggio!".to_owned());
        }
        for note_id in &chord.notes {
            let note = score.note(*note_id);
            for fingering in &note.fingerings {
                if !fingering.visible {
                    continue;
                }
                // § 4.14 : seuls `!0!`-`!5!` (et les doigtés de cordes
                // `!p!`-`!c!`) sont des symboles définis ; un nom de symbole
                // ne peut pas contenir d'espace. MuseScore stocke parfois le
                // doigté avec des balises `<font …>` : on retire les balises
                // et on n'écrit que le symbole, sinon `abc2midi` avertit
                // « instruction !<font! ignored » sur chaque note.
                if let Some(token) = fingering_token(&fingering.text) {
                    before[offset].push(format!("!{token}!"));
                }
            }
        }
    }
}

/// Symbole de doigté admissible dans une décoration `!…!` (§ 4.14).
///
/// MuseScore 2.x enregistre par exemple le doigté `1` précédé de balises de
/// mise en forme : selon le lecteur, il reste `font size='9'/font … 1` ou
/// `<font …></font> … 1`. On retire les balises éventuelles et on ne garde
/// que le dernier mot, qui est le symbole ; un texte qui ne se réduit pas à
/// un seul symbole connu ne produit aucun jeton plutôt qu'un nom invalide.
fn fingering_token(value: &str) -> Option<char> {
    let mut plain = String::with_capacity(value.len());
    let mut in_markup = false;
    for character in value.chars() {
        match character {
            '<' => in_markup = true,
            '>' => in_markup = false,
            _ if !in_markup => plain.push(character),
            _ => {}
        }
    }
    let last = plain.split_whitespace().next_back()?;
    let mut chars = last.chars();
    let token = chars.next()?;
    (chars.next().is_none() && matches!(token, '0'..='5' | 'p' | 'i' | 'm' | 'a' | 'c'))
        .then_some(token)
}

/// Ajoute les annotations de segment aux notes les plus proches.
fn draw_annotations(
    measure: &Measure,
    staff: usize,
    voice: usize,
    score: &Score,
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    before: &mut [Vec<String>],
) {
    for segment in &measure.segments {
        for entry in &segment.annotations {
            if entry.track.staff().index() != staff {
                continue;
            }
            let declared = entry.track.voice() as usize;
            let target = if declared != 0 && voice_has_content(score, staff, declared) {
                declared
            } else {
                0
            };
            if target != voice {
                continue;
            }
            let system_level = matches!(
                &entry.annotation,
                Annotation::Tempo(_) | Annotation::RehearsalMark(_)
            );
            if system_level && voice != 0 {
                continue;
            }
            let Some(token) = annotation_token(&entry.annotation) else {
                continue;
            };
            let Some(offset) = item_at_or_after(items, range.clone(), segment.tick) else {
                continue;
            };
            before[offset].push(token);
        }
    }
}

/// Jeton ABC d'une annotation.
fn annotation_token(annotation: &Annotation) -> Option<String> {
    if !annotation.visible() {
        return None;
    }
    let token = match annotation {
        Annotation::Dynamic(dynamic) => {
            // Port de `xml2abc.py::dynamics_map` (lignes 59-71) : seules les
            // nuances de la table sont des `!nom!` ; `sf`, `fz`, `fp`… sont
            // écrites comme annotations et non comme décorations ignorées.
            if decorations::is_dynamic_token(&dynamic.text) {
                format!("!{}!", dynamic.text.trim().to_ascii_lowercase())
            } else {
                let value = text::escape_annotation(&dynamic.text);
                return (!value.is_empty()).then(|| format!("\"^{value}\""));
            }
        }
        Annotation::Fermata(_) => "!fermata!".to_owned(),
        Annotation::Harmony(harmony) => {
            let value = text::escape_annotation(&harmony.text);
            return (!value.is_empty()).then(|| format!("\"{value}\""));
        }
        // Le champ `Q:` de l'en-tête porte déjà le texte du tempo
        // (`writer.rs::tempo_field`). L'écrire aussi en annotation le
        // dupliquerait à chaque aller-retour : la relecture crée un
        // `StaffText` en plus du `Tempo`. Le modèle pivot fusionne ce que
        // `xml2abc.py::doDirection` écrit depuis `<metronome>` et `<words>`.
        Annotation::Tempo(_) => return None,
        Annotation::RehearsalMark(mark) => {
            let value = text::escape_annotation(mark.text.plain());
            return (!value.is_empty()).then(|| format!("\"^{value}\""));
        }
        Annotation::StaffText(value) | Annotation::Expression(value) => {
            let plain = text::escape_annotation(value.text.plain());
            if plain.is_empty() {
                return None;
            }
            let marker = match annotation.placement() {
                scoretk_core::model::Placement::Above => '^',
                scoretk_core::model::Placement::Below => '_',
            };
            return Some(format!("\"{marker}{plain}\""));
        }
        Annotation::SystemText(value) => {
            let value = text::escape_annotation(value.text.plain());
            return (!value.is_empty()).then(|| format!("\"^{value}\""));
        }
        Annotation::InstrumentChange(change) => {
            let value = text::escape_annotation(change.text.plain());
            return (!value.is_empty()).then(|| format!("\"^{value}\""));
        }
        Annotation::Other { text, .. } => {
            let value = text::escape_annotation(text);
            return (!value.is_empty()).then(|| format!("\"^{value}\""));
        }
        Annotation::FiguredBass(_) | Annotation::TremoloBar(_) => return None,
    };
    Some(token)
}

/// Ajoute les spanners (slurs et soufflets) rattachés à leurs notes.
#[allow(clippy::too_many_arguments)]
fn draw_spanners(
    score: &Score,
    measure: &Measure,
    staff: usize,
    voice: usize,
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    before: &mut [Vec<String>],
    after: &mut [Vec<String>],
    last_measure: bool,
) {
    for spanner in &score.spanners {
        if spanner.start.track.staff().index() != staff || !spanner.visible {
            continue;
        }
        let declared = spanner.start.track.voice() as usize;
        let target = if declared != 0 && voice_has_content(score, staff, declared) {
            declared
        } else {
            0
        };
        if target != voice {
            continue;
        }
        match spanner.kind {
            SpannerType::Slur => {
                if spanner.start.tick >= measure.start_tick
                    && spanner.start.tick < measure.end_tick()
                {
                    if let Some(offset) = item_at_or_after(items, range.clone(), spanner.start.tick)
                    {
                        before[offset].push("(".to_owned());
                    }
                }
                if spanner.end.tick > measure.start_tick && spanner.end.tick <= measure.end_tick() {
                    if let Some(offset) = item_at_or_before(items, range.clone(), spanner.end.tick)
                    {
                        after[offset].push(")".to_owned());
                    }
                }
            }
            SpannerType::Hairpin(kind) => {
                let (start, end) = match kind {
                    HairpinType::Crescendo => ("!<(!", "!<)!"),
                    HairpinType::Diminuendo => ("!>(!", "!>)!"),
                };
                if spanner.start.tick >= measure.start_tick
                    && spanner.start.tick < measure.end_tick()
                {
                    if let Some(offset) = item_at_or_after(items, range.clone(), spanner.start.tick)
                    {
                        before[offset].push(start.to_owned());
                    }
                }
                // Un soufflet qui se termine exactement au début de la mesure
                // suivante appartenait à cette mesure-là : l'écrire dans la
                // précédente collait le `!>)!` au `!>(!` de la première note
                // et rendait l'aller-retour instable. On vise donc la mesure
                // qui contient le tick de fin (sauf en fin de partition, où
                // le tick vaut la fin de la dernière mesure).
                if spanner.end.tick >= measure.start_tick
                    && (spanner.end.tick < measure.end_tick()
                        || (last_measure && spanner.end.tick <= measure.end_tick()))
                {
                    if let Some(offset) = item_at_or_after(items, range.clone(), spanner.end.tick) {
                        before[offset].push(end.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
}

/// Changements d'armure, de mesure et de clef en cours de mesure.
///
/// MuseScore écrit ces changements dans des `<attributes>` au milieu de la
/// mesure ; ABC les rend par des champs en ligne `[K:…]` et `[M:…]`.
#[allow(clippy::too_many_arguments)]
fn draw_context_changes(
    score: &Score,
    measure: &Measure,
    staff: usize,
    context_staff: usize,
    key: &mut KeySig,
    clef: &mut Option<scoretk_core::model::ClefType>,
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    before: &mut [Vec<String>],
) {
    let mut changes: Vec<ContextChange> = Vec::new();
    for segment in &measure.segments {
        if segment.tick <= measure.start_tick {
            continue;
        }
        for (track, item) in segment.iter_elements() {
            let kind = match item {
                scoretk_core::model::SegmentItem::KeySig(_) => 0u8,
                scoretk_core::model::SegmentItem::TimeSig(_) => 1,
                scoretk_core::model::SegmentItem::Clef(_) => 2,
                _ => continue,
            };
            if (kind == 2 && track.staff().index() != staff)
                || (kind != 2 && track.staff().index() != context_staff)
            {
                continue;
            }
            if changes
                .iter()
                .any(|(tick, existing, ..)| *tick == segment.tick && *existing == kind)
            {
                continue;
            }
            match item {
                scoretk_core::model::SegmentItem::KeySig(value) => {
                    changes.push((segment.tick, kind, Some(value.clone()), None, None));
                }
                scoretk_core::model::SegmentItem::TimeSig(value) => {
                    changes.push((segment.tick, kind, None, None, Some(value.clone())));
                }
                scoretk_core::model::SegmentItem::Clef(value) => {
                    changes.push((segment.tick, kind, None, Some(value.transposed), None));
                }
                _ => {}
            }
        }
    }
    changes.sort_by_key(|(tick, kind, ..)| (*tick, *kind));
    let _ = score;
    for (tick, kind, new_key, new_clef, new_time) in changes {
        let token = match kind {
            0 => {
                if let Some(value) = new_key {
                    *key = value;
                }
                format!("[K:{}]", pitch::key_field(key))
            }
            1 => {
                if let Some(value) = new_time {
                    format!("[M:{}/{}]", value.numerator, value.denominator)
                } else {
                    continue;
                }
            }
            _ => {
                if let Some(value) = new_clef {
                    *clef = Some(value);
                }
                let clef_name = clef
                    .and_then(|value| pitch::clef_name(&scoretk_core::model::Clef::new(value)))
                    .unwrap_or_else(|| "treble".to_owned());
                format!("[K:{} clef={clef_name}]", pitch::key_field(key))
            }
        };
        let Some(offset) = item_at_or_after(items, range.clone(), tick) else {
            continue;
        };
        before[offset].push(token);
    }
}

/// Indice du premier élément au tick donné ou après.
fn item_at_or_after(
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    tick: Tick,
) -> Option<usize> {
    let start = range.start;
    let end = range.end;
    let local = items[start..end].partition_point(|item| item.tick < tick);
    let offset = local.min(end.saturating_sub(start).saturating_sub(1));
    (start < end).then_some(offset)
}

/// Indice du dernier élément au tick donné ou avant.
fn item_at_or_before(
    items: &[Item<'_>],
    range: std::ops::Range<usize>,
    tick: Tick,
) -> Option<usize> {
    let start = range.start;
    let end = range.end;
    if start >= end {
        return None;
    }
    let local = items[start..end].partition_point(|item| item.tick <= tick);
    Some(local.saturating_sub(1).min(end - start - 1))
}

/// Rend un accord ou un silence en jeton ABC.
fn render_item(
    score: &Score,
    measure: &Measure,
    chord_rest: &ChordRest,
    accidental_state: &mut pitch::AccidentalState,
    transpose: i32,
    grace: &mut String,
) -> String {
    if let Some(rest) = chord_rest.rest_ref() {
        if chord_rest.is_full_measure_rest() {
            // Un `<Rest>` de mesure n'a pas de `ticks` propres dans le modèle :
            // sa longueur écrite est celle de la mesure (le `ticks` de
            // `ChordRest` peut être nul).
            return rest_token(measure.len, measure);
        }
        let head = if rest.visible { 'z' } else { 'x' };
        let length = duration::chord_rest_length(chord_rest);
        return format!("{head}{}", duration::multiplier(length, duration::unit()));
    }
    let Some(chord) = chord_rest.chord_ref() else {
        return String::new();
    };
    render_chord(score, chord_rest, chord, accidental_state, transpose, grace)
}

/// Rend un accord : agréments, hauteurs, durée, liaison.
fn render_chord(
    score: &Score,
    chord_rest: &ChordRest,
    chord: &Chord,
    accidental_state: &mut pitch::AccidentalState,
    transpose: i32,
    grace: &mut String,
) -> String {
    let mut token = String::new();
    let mut grace_group = String::new();
    for grace in &chord.grace_chords {
        let acciaccatura = matches!(
            grace.chord.grace,
            Some(scoretk_core::model::GraceType::Acciaccatura)
        );
        for note_id in &grace.chord.notes {
            let note = score.note(*note_id);
            if !note.visible {
                continue;
            }
            // Une seule paire d'accolades par accord : la norme interdit deux
            // groupes d'agréments consécutifs sans note intermédiaire. La
            // barre oblique, elle, peut précéder chaque note ; l'espace évite
            // qu'elle soit lue comme une fraction de la durée précédente
            // (`{g/a}` vaut `g`/2 puis `a`, sans acciaccatura).
            if acciaccatura {
                if !grace_group.is_empty() {
                    grace_group.push(' ');
                }
                grace_group.push('/');
            }
            let (letter, alter, octave) = pitch::note_spelling(note, transpose);
            grace_group.push_str(&pitch::abc_note(letter, alter, octave, accidental_state));
            grace_group.push_str(&duration::multiplier(
                grace.duration.ticks(),
                duration::unit(),
            ));
        }
    }
    if !grace_group.is_empty() {
        grace.push('{');
        grace.push_str(&grace_group);
        grace.push('}');
    }

    let notes: Vec<&Note> = chord
        .notes
        .iter()
        .map(|id| score.note(*id))
        .filter(|note| note.visible && !note.ghost)
        .collect();
    let notes: Vec<&Note> = if notes.is_empty() {
        chord.notes.iter().map(|id| score.note(*id)).collect()
    } else {
        notes
    };

    if notes.is_empty() {
        token.push('z');
    } else if notes.len() == 1 {
        let note = notes[0];
        let (letter, alter, octave) = pitch::note_spelling(note, transpose);
        token.push_str(&pitch::abc_note(letter, alter, octave, accidental_state));
    } else {
        token.push('[');
        for note in &notes {
            let (letter, alter, octave) = pitch::note_spelling(note, transpose);
            token.push_str(&pitch::abc_note(letter, alter, octave, accidental_state));
        }
        token.push(']');
    }

    let length = duration::chord_rest_length(chord_rest);
    token.push_str(&duration::multiplier(length, duration::unit()));

    let tied = notes.iter().any(|note| note.tie_forward.is_some());
    if tied {
        token.push('-');
    }
    token
}

/// Lignes `w:` de la mesure, une par couplet.
fn lyrics_lines(
    items: &[Item<'_>],
    verses: &BTreeSet<usize>,
    range: std::ops::Range<usize>,
    pending_word: &mut bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    for &verse in verses {
        let mut tokens: Vec<String> = Vec::new();
        let mut extend = Fraction::ZERO;
        for item in &items[range.clone()] {
            if !item.chord_rest.is_chord() {
                continue;
            }
            let lyrics = item
                .chord_rest
                .lyrics
                .iter()
                .find(|lyrics| lyrics.verse == verse);
            match lyrics {
                Some(lyrics) => {
                    extend = Fraction::ZERO;
                    let mut syllable = text::escape_lyric(&lyrics.text);
                    if matches!(
                        lyrics.syllabic,
                        LyricsSyllabic::Begin | LyricsSyllabic::Middle
                    ) {
                        syllable.push('-');
                        *pending_word = true;
                    } else if lyrics.syllabic == LyricsSyllabic::End {
                        *pending_word = false;
                    }
                    tokens.push(syllable);
                    if let Some(melisma) = lyrics.melisma {
                        extend = melisma;
                    }
                }
                None => {
                    if extend.is_positive() {
                        tokens.push("_".to_owned());
                        extend -= item.chord_rest.ticks();
                    } else {
                        tokens.push("*".to_owned());
                    }
                }
            }
        }
        if tokens.is_empty() {
            continue;
        }
        // § 5.1 : `*` est une syllabe blanche, et une ligne `w:` peut n'en
        // contenir que : les notes restantes restent sans paroles. `xml2abc.py`
        // (`writePart`, lignes 331-355) écrit un `w:` après chaque ligne de
        // musique chantée. Omettre la ligne terminale économise quelques
        // octets mais déclenche « Line of music without lyrics » sur une ligne
        // qui porte pourtant des notes ; on l'écrit donc aussi en fin de
        // couplet.
        lines.push(format!("w: {}", tokens.join(" ")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barres_de_mesure() {
        use scoretk_core::model::Fraction;
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        assert_eq!(end_bar_line(&measure, false), "|");
        assert_eq!(end_bar_line(&measure, true), "|]");
        measure.end_bar_line = scoretk_core::model::BarLineType::EndRepeat;
        assert_eq!(end_bar_line(&measure, false), ":|");
        measure.end_bar_line = scoretk_core::model::BarLineType::Double;
        assert_eq!(end_bar_line(&measure, false), "||");
    }

    #[test]
    fn remplissage_de_mesure() {
        use scoretk_core::model::Fraction;
        let measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        let mut line = String::new();
        push_fill_rest(&mut line, Fraction::ZERO, Fraction::ONE, &measure);
        assert_eq!(line, "Z");
        let mut line = String::new();
        push_fill_rest(&mut line, Fraction::ZERO, Fraction::QUARTER, &measure);
        assert_eq!(line, "z2");
    }
}
