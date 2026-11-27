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

//! Conversion d'un [`scoretk::FormatInfo`] en objet JavaScript `FormatInfo`.

use wasm_bindgen::JsValue;

/// Décrit un format disponible pour JavaScript : `{ name, extensions, read, write }`.
pub(crate) fn to_js(info: scoretk::FormatInfo) -> JsValue {
    let object = js_sys::Object::new();
    set(&object, "name", JsValue::from_str(info.name()));
    let extensions = js_sys::Array::new();
    for extension in info.extensions() {
        extensions.push(&JsValue::from_str(extension));
    }
    set(&object, "extensions", extensions.into());
    set(&object, "read", JsValue::from_bool(info.read));
    set(&object, "write", JsValue::from_bool(info.write));
    object.into()
}

/// `Reflect::set` échoue seulement sur un objet non extensible : impossible ici.
fn set(object: &js_sys::Object, key: &str, value: JsValue) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(key), &value);
}
