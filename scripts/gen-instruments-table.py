#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Données de `share/instruments/instruments.xml` et de
# `src/importexport/midi/internal/midiimport/importmidi_instrument.cpp`
# de MuseScore Studio, Copyright (C) MuseScore Limited and others,
# converties en Rust (fichier généré) en 2026.
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
# FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
# more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see the LICENSE file at the root of this repository or
# <https://www.gnu.org/licenses/gpl-3.0.html>.

"""Génère la table partagée des instruments dans `scoretk-core`.

Usage : gen-instruments-table.py <instruments.xml> <importmidi_instrument.cpp> <dst.rs>

La table sert à la lecture MuseScore (reconnaissance des instruments) et à
l'import MIDI (choix du gabarit et transposition) : c'est une donnée de
modèle, sans logique de format, donc elle vit dans `scoretk-core`.
"""
import re
import sys
import xml.etree.ElementTree as ET

src_xml, src_cpp, dst = sys.argv[1], sys.argv[2], sys.argv[3]
root = ET.parse(src_xml).getroot()


def _pitch_range(text, upper=False):
    """`'36-67'` → 36 ou 67."""
    if not text or '-' not in text:
        return -1
    try:
        parts = [int(part) for part in text.split('-')[:2]]
    except ValueError:
        return -1
    return parts[1] if upper else parts[0]


rows = []
for group in root.iter('InstrumentGroup'):
    for instr in group.findall('Instrument'):
        programs = []
        for chan in instr.findall('Channel'):
            for prog in chan.findall('program'):
                try:
                    programs.append(int(prog.get('value', '-1')))
                except ValueError:
                    pass
        rows.append({
            'id': instr.get('id', ''),
            'track_name': instr.findtext('trackName', ''),
            'long_name': instr.findtext('longName', ''),
            'short_name': instr.findtext('shortName', ''),
            'music_xml_id': instr.findtext('musicXMLid', ''),
            'programs': sorted(set(programs)),
            'transpose_diatonic': int(instr.findtext('transposeDiatonic', '0') or 0),
            'transpose_chromatic': int(instr.findtext('transposeChromatic', '0') or 0),
            'clef': instr.findtext('clef', ''),
            'staff_group': instr.findtext('staffGroup', ''),
            'use_drumset': instr.find('Drumset') is not None,
            'min_pitch': _pitch_range(instr.findtext('pPitchRange', '')),
            'max_pitch': _pitch_range(instr.findtext('pPitchRange', ''), upper=True),
            'common_genre': 'common' in [g.text for g in instr.findall('genre')],
        })

# GM1_SOUND_DATA de importmidi_instrument.cpp : l'ordre du tableau est le
# numéro de programme GM (0-127).
cpp = open(src_cpp, encoding='utf-8').read()
preferred = re.findall(r'GM1ProgramData\{\s*GM1Program::\w+,\s*"([^"]*)"sv\s*\}', cpp)
assert len(preferred) == 128, f'128 entrées attendues dans GM1_SOUND_DATA, obtenu {len(preferred)}'


def esc(value):
    return value.replace('\\', '\\\\').replace('"', '\\"')


out = []
out.append('''// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Table dérivée de `share/instruments/instruments.xml` et de
// `src/importexport/midi/internal/midiimport/importmidi_instrument.cpp`
// de MuseScore Studio, Copyright (C) MuseScore Limited and others,
// convertie en Rust (fichier généré) en 2026.
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

//! Catalogue des instruments de MuseScore (données partagées).
//!
//! Fichier **généré** par `scripts/gen-instruments-table.py` à partir de
//! `share/instruments/instruments.xml` et de `GM1_SOUND_DATA` de
//! `importmidi_instrument.cpp`. Ne pas éditer à la main. Utilisé par la
//! lecture MuseScore (reconnaissance) et par l'import MIDI (gabarit,
//! transposition, clefs).''')
out.append('')
out.append('''/// Gabarit d'instrument de MuseScore (`InstrumentTemplate`).
#[derive(Clone, Copy, Debug)]
pub struct Template {
    /// Identifiant MuseScore (`Instrument::id`).
    pub id: &'static str,
    /// Nom de piste (`trackName`).
    pub track_name: &'static str,
    /// Nom long.
    pub long_name: &'static str,
    /// Nom court.
    pub short_name: &'static str,
    /// Identifiant MusicXML (`musicXMLid`).
    pub music_xml_id: &'static str,
    /// Masque des programmes MIDI des canaux (bit `n` = programme `n`).
    pub program_mask: u128,
    /// Transposition diatonique (`transposeDiatonic`).
    pub transpose_diatonic: i32,
    /// Transposition chromatique (`transposeChromatic`).
    pub transpose_chromatic: i32,
    /// Groupe de portée (`staffGroup`), `"TAB"` pour les tablatures.
    pub staff_group: &'static str,
    /// Vrai si le gabarit utilise un jeu de percussion.
    pub use_drumset: bool,
    /// Hauteur minimale du gabarit (`minPitchP`), -1 si absente.
    pub min_pitch: i32,
    /// Hauteur maximale du gabarit (`maxPitchP`), -1 si absente.
    pub max_pitch: i32,
}''')
out.append('')
out.append('''/// Catalogue, dans l'ordre du fichier source : à force égale, le premier
/// instrument l'emporte (`searchTemplateForInstrNameList`).
pub const TEMPLATES: &[Template] = &[''')
for row in rows:
    mask = sum(1 << p for p in row['programs'] if 0 <= p < 128)
    out.append('    Template {')
    out.append(f"        id: \"{esc(row['id'])}\",")
    out.append(f"        track_name: \"{esc(row['track_name'])}\",")
    out.append(f"        long_name: \"{esc(row['long_name'])}\",")
    out.append(f"        short_name: \"{esc(row['short_name'])}\",")
    out.append(f"        music_xml_id: \"{esc(row['music_xml_id'])}\",")
    out.append(f'        program_mask: {mask},')
    out.append(f"        transpose_diatonic: {row['transpose_diatonic']},")
    out.append(f"        transpose_chromatic: {row['transpose_chromatic']},")
    out.append(f"        staff_group: \"{esc(row['staff_group'])}\",")
    out.append(f"        use_drumset: {str(row['use_drumset']).lower()},")
    out.append(f"        min_pitch: {row['min_pitch']},")
    out.append(f"        max_pitch: {row['max_pitch']},")
    out.append('    },')
out.append('];')
out.append('')
out.append('''/// Identifiant du gabarit préféré pour chaque programme GM 0-127
/// (`GM1_SOUND_DATA` de `importmidi_instrument.cpp:60-190`).
pub const GM1_PREFERRED_IDS: [&str; 128] = [''')
for preferred_id in preferred:
    out.append(f'    "{esc(preferred_id)}",')
out.append('];')
out.append('')
out.append('''/// Vrai si le gabarit porte le programme MIDI donné.
pub fn template_has_program(template: &Template, program: u8) -> bool {
    template.program_mask & (1u128 << u32::from(program)) != 0
}

/// Premier programme MIDI du gabarit, pour la lecture des partitions 3.x.
pub fn template_first_program(template: &Template) -> Option<i16> {
    (0..128).find(|program| template_has_program(template, *program as u8)).map(|program| program as i16)
}

/// Premier gabarit portant l'identifiant MuseScore donné.
pub fn template_for_id(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|template| template.id == id)
}

/// Gabarit préféré pour un programme GM, comme `getPreferredInstrument`.
pub fn preferred_template_for_program(program: u8) -> Option<&'static Template> {
    let id = GM1_PREFERRED_IDS[usize::from(program)];
    if id.is_empty() {
        return None;
    }
    template_for_id(id)
}''')
open(dst, 'w', encoding='utf-8').write('\n'.join(out) + '\n')
