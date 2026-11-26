// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_instrument.cpp` (`msInstrName`),
// `importmidi_clef.cpp` (`hasGFclefs`) et d'`instrtemplate.cpp`
// (`clefType`, `TConv::fromXml`) de MuseScore Studio, Copyright (C)
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

//! Noms et clefs du gabarit d'instrument retenu pour une piste MIDI.
//!
//! `MidiInstr::msInstrName` nomme la partie d'après le gabarit, et
//! `Part::initFromInstrTemplate` en prend les noms long/court, l'identifiant
//! MusicXML et les clefs de concert. Les clefs viennent de la table partagée
//! `scoretk_core::instruments`, codée sur 4 bits par portée (version 4.7.4).

use scoretk_core::instruments::Template;
use scoretk_core::model::ClefType;

use super::super::chord::MTrack;
use super::template_for_track;

/// Nom long du gabarit, ou son nom de piste à défaut (`msInstrName`).
pub fn ms_instr_name(template: &Template) -> &'static str {
    if !template.long_name.is_empty() {
        template.long_name
    } else {
        template.track_name
    }
}

/// Clef de concert d'une portée (`TConv::fromXml` du nom XML).
///
/// Le codage est celui de `scripts/gen-instruments-table.py` : 4 bits par
/// portée, `0` pour `G`. Une valeur inconnue retombe sur `G`, comme dans
/// `TConv::fromXml(text, ClefType::G)`.
pub fn clef_type_from_code(code: u16) -> ClefType {
    match code {
        0 => ClefType::G,
        1 => ClefType::F,
        2 => ClefType::G8Vb,
        3 => ClefType::G8Va,
        4 => ClefType::G15Mb,
        5 => ClefType::G15Ma,
        6 => ClefType::C1,
        7 => ClefType::C2,
        8 => ClefType::C3,
        9 => ClefType::C4,
        10 => ClefType::C5,
        11 => ClefType::F8Vb,
        12 => ClefType::F8Va,
        13 => ClefType::F15Mb,
        14 => ClefType::F15Ma,
        15 => ClefType::Perc,
        _ => ClefType::G,
    }
}

/// Clefs de concert des portées du gabarit (`InstrumentTemplate::clefType`).
pub fn concert_clefs(template: &Template) -> Vec<ClefType> {
    (0..template.staves.max(1))
        .map(|staff| clef_type_from_code((template.clef_codes >> (4 * staff as u16)) & 0xf))
        .collect()
}

/// Vrai si le gabarit alterne clef de sol et clef de fa (`hasGFclefs`).
pub fn has_gf_clefs(template: &Template) -> bool {
    let mut has_g = false;
    let mut has_f = false;
    for clef in concert_clefs(template) {
        match clef {
            ClefType::G => has_g = true,
            ClefType::F => has_f = true,
            _ => {}
        }
    }
    has_g && has_f
}

/// Retient le gabarit de chaque piste (`findInstrumentsForAllTracks`, avec
/// `msInstrIndex` à sa valeur par défaut 0).
pub fn assign_templates(tracks: &mut [MTrack]) {
    for track in tracks.iter_mut() {
        track.ms_instr = template_for_track(track);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::instruments::template_for_id;

    #[test]
    fn noms_et_abreviations_du_catalogue() {
        let piano = template_for_id("piano").expect("piano");
        assert_eq!(ms_instr_name(piano), "Piano");
        assert_eq!(piano.short_name, "Pno.");
        assert_eq!(piano.track_name, "Piano");
        assert_eq!(piano.music_xml_id, "keyboard.piano");
        let guitar = template_for_id("guitar-steel").expect("guitare");
        assert_eq!(ms_instr_name(guitar), "Acoustic Guitar");
        assert_eq!(guitar.short_name, "Guit.");
    }

    #[test]
    fn clefs_de_concert_4_7_4() {
        let guitar = template_for_id("guitar-steel").expect("guitare");
        assert_eq!(concert_clefs(guitar), vec![ClefType::G8Vb]);
        assert!(!has_gf_clefs(guitar));
        let piano = template_for_id("piano").expect("piano");
        assert_eq!(concert_clefs(piano), vec![ClefType::G, ClefType::F]);
        assert!(has_gf_clefs(piano));
        let contrabass = template_for_id("contrabass").expect("contrebasse");
        assert_eq!(concert_clefs(contrabass), vec![ClefType::F8Vb]);
    }
}
