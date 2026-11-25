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

//! Conversion de [`scoretk_midi_export::Timemap`] vers les objets JavaScript de
//! `convertWithTimeline`.
//!
//! Les noms Rust restent en `snake_case` (`time_us`, `end_tick`…) ; la
//! surface JavaScript est en `camelCase` (`timeUs`, `endTick`…), comme le
//! reste de l'API. Aucune dépendance de sérialisation n'est ajoutée : les
//! objets sont construits avec `js_sys`.

use js_sys::{Array, Object, Reflect};
use scoretk::{MeasureSync, NoteSync, PauseSync, TempoChange, Timemap};
use wasm_bindgen::JsValue;

/// Convertit une carte des temps en objet JavaScript `Timeline`.
pub(crate) fn timeline_to_js(timemap: &Timemap) -> JsValue {
    let timeline = Object::new();
    set(
        &timeline,
        "division",
        JsValue::from_f64(f64::from(timemap.division)),
    );
    set(&timeline, "ticks", JsValue::from_f64(timemap.ticks as f64));

    let tempo = Array::new();
    for change in &timemap.tempo {
        tempo.push(&tempo_to_js(change));
    }
    set(&timeline, "tempo", tempo.into());

    let measures = Array::new();
    for measure in &timemap.measures {
        measures.push(&measure_to_js(measure));
    }
    set(&timeline, "measures", measures.into());

    let notes = Array::new();
    for note in &timemap.notes {
        notes.push(&note_to_js(note));
    }
    set(&timeline, "notes", notes.into());

    let pauses = Array::new();
    for pause in &timemap.pauses {
        pauses.push(&pause_to_js(pause));
    }
    set(&timeline, "pauses", pauses.into());

    timeline.into()
}

fn tempo_to_js(change: &TempoChange) -> JsValue {
    let object = Object::new();
    set(&object, "tick", JsValue::from_f64(change.tick as f64));
    set(&object, "timeUs", JsValue::from_f64(change.time_us as f64));
    set(
        &object,
        "microsPerQuarter",
        JsValue::from_f64(f64::from(change.micros_per_quarter)),
    );
    object.into()
}

fn measure_to_js(measure: &MeasureSync) -> JsValue {
    let object = Object::new();
    set(
        &object,
        "measure",
        JsValue::from_f64(measure.measure as f64),
    );
    // Seule clé d'ancrage à utiliser : indice 0-basé dans la partition
    // complète, identique à la position de la `<measure>` dans chaque
    // `<part>` du MusicXML. `number` (0-basé côté modèle, remis à 1 par
    // section côté MusicXML) n'est pas exposé.
    set(
        &object,
        "ordinal",
        JsValue::from_f64(measure.measure as f64),
    );
    set(
        &object,
        "segment",
        JsValue::from_f64(measure.segment as f64),
    );
    set(
        &object,
        "passage",
        JsValue::from_f64(f64::from(measure.pass)),
    );
    // Le MusicXML partwise répète les mesures par partie : la carte ne donne
    // qu'un identifiant, celui de la partie 0 (voir `docs/agents/timemap.md`).
    set(&object, "xmlId", JsValue::from_str(&measure.xml_id(0)));
    set(&object, "tick", JsValue::from_f64(measure.tick as f64));
    set(
        &object,
        "endTick",
        JsValue::from_f64(measure.end_tick as f64),
    );
    set(&object, "timeUs", JsValue::from_f64(measure.time_us as f64));
    set(
        &object,
        "endTimeUs",
        JsValue::from_f64(measure.end_time_us as f64),
    );
    set(
        &object,
        "timeMs",
        JsValue::from_f64(measure.time_us as f64 / 1000.0),
    );
    object.into()
}

fn note_to_js(note: &NoteSync) -> JsValue {
    let object = Object::new();
    set(&object, "tick", JsValue::from_f64(note.tick as f64));
    set(&object, "timeUs", JsValue::from_f64(note.time_us as f64));
    set(
        &object,
        "durationTicks",
        JsValue::from_f64(note.duration_ticks as f64),
    );
    set(&object, "measure", JsValue::from_f64(note.measure as f64));
    set(&object, "staff", JsValue::from_f64(f64::from(note.staff)));
    set(&object, "voice", JsValue::from_f64(f64::from(note.voice)));
    set(&object, "chord", JsValue::from_f64(note.chord as f64));
    set(&object, "note", JsValue::from_f64(note.note as f64));
    set(&object, "pitch", JsValue::from_f64(f64::from(note.pitch)));
    set(&object, "passage", JsValue::from_f64(f64::from(note.pass)));
    set(&object, "grace", JsValue::from_bool(note.grace));
    set(&object, "tremolo", JsValue::from_bool(note.tremolo));
    set(&object, "xmlId", JsValue::from_str(&note.xml_id()));
    // Champs additifs : `null` pour une note sans source textuelle (MuseScore,
    // MIDI, …), sinon indices UTF-16 depuis le début du fichier.
    set(
        &object,
        "sourceOffset",
        note.source_offset
            .map_or(JsValue::NULL, |offset| JsValue::from_f64(offset as f64)),
    );
    set(
        &object,
        "sourceEnd",
        note.source_end
            .map_or(JsValue::NULL, |end| JsValue::from_f64(end as f64)),
    );
    object.into()
}

fn pause_to_js(pause: &PauseSync) -> JsValue {
    let object = Object::new();
    set(&object, "tick", JsValue::from_f64(pause.tick as f64));
    set(&object, "endTick", JsValue::from_f64(pause.end_tick as f64));
    set(&object, "timeUs", JsValue::from_f64(pause.time_us as f64));
    set(
        &object,
        "endTimeUs",
        JsValue::from_f64(pause.end_time_us as f64),
    );
    set(&object, "seconds", JsValue::from_f64(pause.seconds));
    object.into()
}

fn set(object: &Object, name: &str, value: JsValue) {
    // `Reflect::set` ne peut échouer que sur un objet gelé ou un proxy : les
    // objets construits ici sont extensibles.
    let _ = Reflect::set(object, &JsValue::from_str(name), &value);
}
