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

use wasm_bindgen::JsValue;

/// Options lues depuis l'objet `ConvertOptions` (propriétés absentes, nulles ou
/// vides ignorées).
pub(crate) struct ApiOptions {
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) encoding_date: Option<String>,
    pub(crate) software: Option<String>,
    pub(crate) concert_pitch: Option<bool>,
    pub(crate) expand_repeats: Option<bool>,
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
        }
    }
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
