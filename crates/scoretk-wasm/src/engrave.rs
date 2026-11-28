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

//! Surface WebAssembly de la gravure (feature `engrave`).
//!
//! `engrave(bytes, options)` garde le score une seule fois dans un handle
//! [`Engraving`] : `pageCount`, `renderPage`, `layout`, `pageOfElement` et
//! `relayout` travaillent tous sur le même `scoretk::Engraver`. Aucune option
//! de conversion n'est modifiée : cette API est strictement additive.

use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use crate::api;
use crate::options::{boolean, number, property, string};

use scoretk::EngraveOptions as RustEngraveOptions;

/// Options de gravure, objet JavaScript littéral (voir `TS_ENGRAVE` dans
/// `api.rs`). Elles acceptent aussi les noms utilisés par cantus/Verovio.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "EngraveOptions")]
    pub type EngraveOptions;
}

/// Verovio's page size in 100 % units (A4 portrait), used when a caller sends
/// only `pageWidth` or only `pageHeight`.
const VEROVIO_PAGE_WIDTH: u32 = 2100;
const VEROVIO_PAGE_HEIGHT: u32 = 2970;

/// Handle d'une partition gravée. `free()` est généré par wasm-bindgen.
#[wasm_bindgen(skip_typescript)]
pub struct Engraving {
    engraver: scoretk::Engraver,
    /// Options of the current layout, also used as fallback of a partial
    /// `relayout` call.
    options: RustEngraveOptions,
    /// Options of the first layout, restored by `relayout()` without argument
    /// (the equivalent of Verovio's `redoLayout()`).
    initial_options: RustEngraveOptions,
}

#[wasm_bindgen]
impl Engraving {
    /// Nombre de pages du rendu courant.
    #[wasm_bindgen(js_name = "pageCount", skip_typescript)]
    pub fn page_count(&self) -> usize {
        self.engraver.page_count()
    }

    /// SVG structuré d'une page (identifiants stables, `g.system`).
    #[wasm_bindgen(js_name = "renderPage", skip_typescript)]
    pub fn render_page(&self, page: usize) -> Result<String, JsError> {
        if page >= self.engraver.page_count() {
            return Err(JsError::new(&format!(
                "page {page} hors limites ({} pages)",
                self.engraver.page_count()
            )));
        }
        String::from_utf8(scoretk::engrave_page_svg(&self.engraver, page))
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Description JSON d'une page, ou tableau de toutes les pages quand
    /// `page` est absent.
    #[wasm_bindgen(skip_typescript)]
    pub fn layout(&self, page: Option<usize>) -> Result<String, JsError> {
        let count = self.engraver.page_count();
        if let Some(page) = page {
            if page >= count {
                return Err(JsError::new(&format!(
                    "page {page} hors limites ({count} pages)"
                )));
            }
            return String::from_utf8(scoretk::engrave_page_layout(&self.engraver, page))
                .map_err(|error| JsError::new(&error.to_string()));
        }
        let mut out = String::from("[");
        for page in 0..count {
            if page > 0 {
                out.push(',');
            }
            let json = String::from_utf8(scoretk::engrave_page_layout(&self.engraver, page))
                .map_err(|error| JsError::new(&error.to_string()))?;
            out.push_str(&json);
        }
        out.push(']');
        Ok(out)
    }

    /// Page (0-basée) contenant l'identifiant stable, ou `null`.
    #[wasm_bindgen(js_name = "pageOfElement", skip_typescript)]
    pub fn page_of_element(&self, id: &str) -> JsValue {
        scoretk::engrave_page_of_element(&self.engraver, id)
            .map_or(JsValue::NULL, |page| JsValue::from_f64(page as f64))
    }

    /// Refait la mise en page avec de nouvelles options, sans relire la
    /// partition. Sans options, les options courantes sont conservées.
    #[wasm_bindgen(skip_typescript)]
    pub fn relayout(&mut self, options: Option<EngraveOptions>) -> Result<(), JsError> {
        let value = options_value(options.as_ref());
        let options = if options.is_some() {
            engrave_options(&value, Some(&self.options))
        } else {
            self.initial_options.clone()
        };
        scoretk::relayout(&mut self.engraver, &options).map_err(api::js_error)?;
        self.options = options;
        Ok(())
    }
}

/// Grave une partition à partir de ses octets. Le score est déplacé dans le
/// handle, sans copie, et reste disponible pour `relayout`.
#[wasm_bindgen(skip_typescript)]
pub fn engrave(input: &[u8], options: Option<EngraveOptions>) -> Result<Engraving, JsError> {
    let value = options_value(options.as_ref());
    let score = api::read_input(input, &value)?;
    let options = engrave_options(&value, None);
    let engraver = scoretk::engrave(score, &options).map_err(api::js_error)?;
    Ok(Engraving {
        engraver,
        initial_options: options.clone(),
        options,
    })
}

/// Convertit l'objet opaque en `JsValue` (une valeur absente devient
/// `undefined`, comme pour les autres options du paquet).
fn options_value(options: Option<&EngraveOptions>) -> JsValue {
    match options {
        Some(options) => <EngraveOptions as AsRef<JsValue>>::as_ref(options).clone(),
        None => JsValue::UNDEFINED,
    }
}

/// Construit les options de gravure. Quand au moins un nom Verovio est
/// présent (`pageWidth`, `pageHeight`, `condense`, `systemMaxPerPage`,
/// `breaks`, `transpose`, ou un `scale` supérieur à 4), `EngraveOptions::from_verovio`
/// convertit les unités 100 % et le zoom en pourcentage ; les noms propres à
/// scoretk sont appliqués ensuite et peuvent surcharger chaque champ.
fn engrave_options(value: &JsValue, fallback: Option<&RustEngraveOptions>) -> RustEngraveOptions {
    let verovio_scale = u32_opt(value, "scale");
    let verovio = u32_opt(value, "pageWidth").is_some()
        || u32_opt(value, "pageHeight").is_some()
        || property(value, "condense").is_some()
        || property(value, "systemMaxPerPage").is_some()
        || property(value, "breaks").is_some()
        || property(value, "transpose").is_some()
        || verovio_scale.is_some_and(|scale| scale > 4);

    let mut options = if verovio {
        RustEngraveOptions::from_verovio(
            u32_opt(value, "pageWidth").unwrap_or(VEROVIO_PAGE_WIDTH),
            u32_opt(value, "pageHeight").unwrap_or(VEROVIO_PAGE_HEIGHT),
            verovio_scale.unwrap_or(scoretk::VEROVIO_DEFAULT_SCALE_PERCENT),
            number(value, "lyricSize").unwrap_or(scoretk::VEROVIO_LYRIC_SIZE),
            boolean(value, "adjustPageHeight").unwrap_or(false),
            condense(value).unwrap_or(false),
            u32_opt(value, "systemMaxPerPage").unwrap_or(0) as usize,
            i32_opt(value, "transpose").unwrap_or(0),
        )
    } else {
        fallback.cloned().unwrap_or_default()
    };

    if !verovio {
        if let Some(scale) = number(value, "scale") {
            options.scale = scale;
        }
    }
    if let Some(width) = u32_opt(value, "pageWidthPx") {
        options.page_width_px = nonzero(width);
    }
    if let Some(height) = u32_opt(value, "pageHeightPx") {
        options.page_height_px = nonzero(height);
    }
    if let Some(adjust) = boolean(value, "adjustPageHeight") {
        options.adjust_page_height = adjust;
    }
    if let Some(one) = boolean(value, "oneSystemPerPage") {
        options.one_system_per_page = one;
    }
    if let Some(hide) = boolean(value, "hideEmptyStaves") {
        options.hide_empty_staves = hide;
    }
    if let Some(hide) = condense(value) {
        // Verovio's `condense: "auto"` skips empty staves except on the first
        // system; `condenseFirstPage` is not meaningful in our layout.
        options.hide_empty_staves = hide;
    }
    if let Some(max) = u32_opt(value, "systemMaxPerPage") {
        // 0 is Verovio's "unlimited"; 1 is cantus' horizontal mode.
        options.one_system_per_page = max == 1;
        options.max_systems_per_page = max as usize;
    }
    if let Some(breaks) = string(value, "breaks") {
        match breaks.as_str() {
            // Verovio's default: ignore encoded breaks and fill to width.
            "auto" | "none" => options.ignore_encoded_breaks = true,
            "encoded" => options.ignore_encoded_breaks = false,
            _ => {}
        }
    }
    if let Some(lyric) = number(value, "lyricSize") {
        options.lyric_size = lyric;
    }
    if let Some(semitones) = i32_opt(value, "transposeSemitones") {
        options.transpose_semitones = semitones;
    }
    if let Some(semitones) = i32_opt(value, "transpose") {
        options.transpose_semitones = semitones;
    }
    if property(value, "partSizes").is_some() {
        options.part_mag_percent = part_sizes(value);
    }
    options
}

/// `condense: "auto"`/`true` is Verovio's hide-empty-staves option.
fn condense(value: &JsValue) -> Option<bool> {
    match property(value, "condense")? {
        value if value.as_bool().is_some() => value.as_bool(),
        value => value.as_string().map(|text| text == "auto"),
    }
}

fn nonzero(value: u32) -> Option<u32> {
    (value > 0).then_some(value)
}

fn u32_opt(value: &JsValue, name: &str) -> Option<u32> {
    let number = number(value, name)?;
    (number >= 0.0 && number <= u32::MAX as f64).then(|| number.round() as u32)
}

fn i32_opt(value: &JsValue, name: &str) -> Option<i32> {
    let number = number(value, name)?;
    (number >= i32::MIN as f64 && number <= i32::MAX as f64).then(|| number.round() as i32)
}

/// `partSizes` maps a part index (string key of a JS object) to a percentage.
/// A `BTreeMap` keeps the order independent of the caller's key order.
fn part_sizes(value: &JsValue) -> Vec<(usize, i32)> {
    let Some(object) =
        property(value, "partSizes").and_then(|value| value.dyn_into::<js_sys::Object>().ok())
    else {
        return Vec::new();
    };
    let mut sizes = BTreeMap::new();
    for key in js_sys::Object::keys(&object).iter() {
        let (Some(key), Ok(percent)) = (key.as_string(), js_sys::Reflect::get(&object, &key))
        else {
            continue;
        };
        let (Ok(index), Some(percent)) = (key.parse::<usize>(), percent.as_f64()) else {
            continue;
        };
        if percent.is_finite() {
            sizes.insert(index, percent.round() as i32);
        }
    }
    sizes.into_iter().collect()
}
