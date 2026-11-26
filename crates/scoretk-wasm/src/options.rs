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

//! Lecture des options JavaScript (`ConvertOptions`) vers une forme Rust.

use std::collections::BTreeMap;

use wasm_bindgen::{JsCast, JsValue};

/// Options lues depuis l'objet `ConvertOptions` (propriétés absentes, nulles ou
/// vides ignorées).
pub(crate) struct ApiOptions {
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) encoding_date: Option<String>,
    pub(crate) software: Option<String>,
    pub(crate) concert_pitch: Option<bool>,
    pub(crate) expand_repeats: Option<bool>,
    /// Vue moderne du chant grégorien (clé de sol, 5 lignes).
    pub(crate) gabc_modern_clef: Option<bool>,
    /// Porteurs SMuFL des neumes grégoriens dans le MusicXML.
    pub(crate) gabc_neume_smufl: Option<bool>,
    /// Porteurs MEI 5.1 au-delà de l'exporteur 4.7.4.
    pub(crate) mei_full_support: Option<bool>,
    /// Porteurs XSD 4.0 du MusicXML au-delà de l'exporteur 4.7.4.
    pub(crate) xml_full_support: Option<bool>,
    /// Préservation des valeurs du modèle dans l'export MusicXML.
    pub(crate) xml_lossless_round_trip: Option<bool>,
    pub(crate) midi_exact_ticks: Option<bool>,
    pub(crate) midi_text: Option<bool>,
    /// Porteurs SMF standard (aucun préfixe privé), lecture et écriture.
    pub(crate) midi_full_support: Option<bool>,
    /// Fichiers d'un dossier, indexés par leur chemin relatif à sa racine.
    /// Utilisés uniquement pour résoudre les `\include` LilyPond.
    pub(crate) includes: Option<BTreeMap<String, Vec<u8>>>,
}

impl ApiOptions {
    pub(crate) fn from_js(options: Option<&super::api::ConvertOptions>) -> Self {
        let value: JsValue = match options {
            Some(options) => {
                <super::api::ConvertOptions as AsRef<JsValue>>::as_ref(options).clone()
            }
            None => JsValue::UNDEFINED,
        };
        Self {
            from: string(&value, "from"),
            to: string(&value, "to"),
            encoding_date: string(&value, "encodingDate"),
            software: string(&value, "software"),
            concert_pitch: boolean(&value, "concertPitch"),
            expand_repeats: boolean(&value, "expandRepeats"),
            gabc_modern_clef: boolean(&value, "gabcModernClef"),
            gabc_neume_smufl: boolean(&value, "gabcNeumeSmufl"),
            mei_full_support: boolean(&value, "meiFullSupport"),
            xml_full_support: boolean(&value, "xmlFullSupport"),
            xml_lossless_round_trip: boolean(&value, "xmlLosslessRoundTrip"),
            midi_exact_ticks: boolean(&value, "midiExactTicks"),
            midi_text: boolean(&value, "midiText"),
            midi_full_support: boolean(&value, "midiFullSupport"),
            includes: includes(&value),
        }
    }
}

/// Lit `includes` comme une table chemin relatif → octets.
///
/// Seules les valeurs `Uint8Array` sont retenues : les autres entrées sont
/// ignorées, comme le sont les `\include` introuvables côté lecteur.
fn includes(value: &JsValue) -> Option<BTreeMap<String, Vec<u8>>> {
    let object = property(value, "includes")?
        .dyn_into::<js_sys::Object>()
        .ok()?;
    let mut files = BTreeMap::new();
    for key in js_sys::Object::keys(&object).iter() {
        let Some(path) = key.as_string() else {
            continue;
        };
        if let Ok(bytes) = js_sys::Reflect::get(&object, &key) {
            if let Ok(bytes) = bytes.dyn_into::<js_sys::Uint8Array>() {
                files.insert(path, bytes.to_vec());
            }
        }
    }
    Some(files)
}

fn property(value: &JsValue, name: &str) -> Option<JsValue> {
    let property = js_sys::Reflect::get(value, &JsValue::from_str(name)).ok()?;
    (!property.is_undefined() && !property.is_null()).then_some(property)
}

fn string(value: &JsValue, name: &str) -> Option<String> {
    let text = property(value, name)?.as_string()?;
    (!text.is_empty()).then_some(text)
}

fn boolean(value: &JsValue, name: &str) -> Option<bool> {
    property(value, name)?.as_bool()
}
