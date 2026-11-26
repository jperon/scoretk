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

Usage : gen-instruments-table.py <instruments.xml> <instruments-4.7.4.xml> \
        <importmidi_instrument.cpp> <dst.rs>

La table sert à la lecture MuseScore (reconnaissance des instruments) et à
l'import MIDI (choix du gabarit, transposition, clefs de concert) : c'est une
donnée de modèle, sans logique de format, donc elle vit dans `scoretk-core`.

Le second fichier XML est celui de **MuseScore 4.7.4**, la version qui produit
les références : seules les clefs en sont extraites (4 bits par portée dans un
`u16`, voir `CLEF_CODES`), car la copie 5.0 en diffère pour 13 instruments
exotiques. Les clefs codées sont jointes aux gabarits du premier XML par
identifiant, sans dépendre de l'ordre des instruments.
"""
import re
import sys
import xml.etree.ElementTree as ET

src_xml, ref_xml, src_cpp, dst = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
root = ET.parse(src_xml).getroot()

# `TConv::fromXml(..., ClefType::G)` et `InstrumentTemplate::read` :
# une clef inconnue ou absente reste `G`. Quatre bits suffisent pour les
# quinze valeurs présentes dans `instruments.xml` (4 portées maximum).
CLEF_CODES = {
    'G': 0, 'F': 1, 'G8vb': 2, 'G8va': 3, 'G15mb': 4, 'G15ma': 5,
    'C1': 6, 'C2': 7, 'C3': 8, 'C4': 9, 'C5': 10,
    'F8vb': 11, 'F8va': 12, 'F15mb': 13, 'F15ma': 14, 'PERC': 15,
}


def read_clefs(instr, by_id, seen):
    """Clefs de concert par portée, `init` résolu comme dans le C++."""
    values = {0: 0}           # défaut : G sur la première portée
    for child in instr:
        tag = child.tag
        if tag == 'init' and child.text and child.text not in seen:
            parent = by_id.get(child.text.strip())
            if parent is not None:
                values.update(read_clefs(parent, by_id, seen | {child.text}))
        elif tag == 'staves':
            try:
                count = max(1, min(4, int((child.text or '1').strip())))
            except ValueError:
                count = 1
            for index in range(count):
                values.setdefault(index, 0)
        elif tag in ('clef', 'concertClef'):
            try:
                index = max(0, min(3, int(child.get('staff', '1')) - 1))
            except ValueError:
                index = 0
            values[index] = CLEF_CODES.get((child.text or 'G').strip(), 0)
    return values


ref_root = ET.parse(ref_xml).getroot()
ref_by_id = {
    instr.get('id'): instr
    for group in ref_root.iter('InstrumentGroup')
    for instr in group.findall('Instrument')
}
ref_clefs = {
    instr_id: sum(code << (4 * index)
                  for index, code in read_clefs(instr, ref_by_id, {instr_id}).items())
    for instr_id, instr in ref_by_id.items()
}
# `InstrumentTemplate::staffCount` : défaut 1, lui aussi de la version 4.7.4.
ref_staves = {
    instr_id: max(1, min(4, int(instr.findtext('staves', '1') or '1')))
    for instr_id, instr in ref_by_id.items()
}


def _pitch_range(text, upper=False):
    """`'36-67'` → 36 ou 67 ; absente ou invalide → `MIN_PITCH`/`MAX_PITCH`."""
    defaut = 127 if upper else 0
    if not text or '-' not in text:
        return defaut
    try:
        parts = [int(part) for part in text.split('-')[:2]]
    except ValueError:
        return defaut
    return parts[1] if upper else parts[0]


rows = []
for group in root.iter('InstrumentGroup'):
    for instr in group.findall('Instrument'):
        # `InstrumentTemplate::read` : les programmes sont ceux des canaux,
        # dans l'ordre du fichier (`channel.program()`), et `useDrumset` vient
        # de `<drumset>`, pas de la présence des tambours.
        programs = []
        for chan in instr.findall('Channel'):
            for prog in chan.findall('program'):
                try:
                    programs.append(int(prog.get('value', '-1')))
                except ValueError:
                    pass
        # `drumset` n'est créé que sur un `<Drum>` explicite (ou
        # `<percussionPanelColumns>`), et `Drumset::name(i)` est vide sans
        # `<name>` : c'est ce que teste `findNotEmptyDrumPitches`.
        drum_mask = 0
        for drum in instr.findall('Drum'):
            try:
                pitch = int(drum.get('pitch', '-1'))
            except ValueError:
                continue
            if 0 <= pitch < 128 and (drum.findtext('name') or '').strip():
                drum_mask |= 1 << pitch
        rows.append({
            'id': instr.get('id', ''),
            'group': group.get('id', ''),
            'track_name': instr.findtext('trackName', ''),
            'long_name': instr.findtext('longName', ''),
            'short_name': instr.findtext('shortName', ''),
            'music_xml_id': instr.findtext('musicXMLid', ''),
            'programs': sorted(set(programs)),
            'channel_programs': programs,
            'transpose_diatonic': int(instr.findtext('transposeDiatonic', '0') or 0),
            'transpose_chromatic': int(instr.findtext('transposeChromatic', '0') or 0),
            'clef_codes': ref_clefs.get(instr.get('id', ''), 0),
            'staves': ref_staves.get(instr.get('id', ''), 1),
            'staff_group': instr.findtext('staffGroup', ''),
            'use_drumset': int(instr.findtext('drumset', '0') or 0) != 0,
            'has_drumset': bool(instr.findall('Drum'))
            or instr.find('percussionPanelColumns') is not None,
            'drum_pitch_mask': drum_mask,
            'genres': [g.text for g in instr.findall('genre') if g.text],
            'min_pitch': _pitch_range(instr.findtext('pPitchRange', '')),
            'max_pitch': _pitch_range(instr.findtext('pPitchRange', ''), upper=True),
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
//! `share/instruments/instruments.xml`, des **clefs de la version 4.7.4** et de
//! `GM1_SOUND_DATA` de `importmidi_instrument.cpp`. Ne pas éditer à la main.
//! Utilisé par la lecture MuseScore (reconnaissance) et par l'import MIDI
//! (gabarit, transposition, clefs de concert).''')
out.append('')
out.append('''/// Gabarit d'instrument de MuseScore (`InstrumentTemplate`).
#[derive(Clone, Copy, Debug)]
pub struct Template {
    /// Identifiant MuseScore (`Instrument::id`).
    pub id: &'static str,
    /// Groupe d'instruments (`InstrumentGroup::id`), dans l'ordre du fichier.
    pub group: &'static str,
    /// Nom de piste (`trackName`).
    pub track_name: &'static str,
    /// Nom long.
    pub long_name: &'static str,
    /// Nom court.
    pub short_name: &'static str,
    /// Identifiant MusicXML (`musicXMLid`).
    pub music_xml_id: &'static str,
    /// Programmes MIDI des canaux, dans l'ordre du fichier.
    pub channel_programs: &'static [u8],
    /// Masque des programmes MIDI des canaux (bit `n` = programme `n`).
    pub program_mask: u128,
    /// Genres du gabarit (`<genre>`), dans l'ordre du fichier.
    pub genres: &'static [&'static str],
    /// Transposition diatonique (`transposeDiatonic`).
    pub transpose_diatonic: i32,
    /// Transposition chromatique (`transposeChromatic`).
    pub transpose_chromatic: i32,
    /// Clefs de concert, 4 bits par portée (`TConv::fromXml`), de la version
    /// 4.7.4 : le codage est décrit dans `gen-instruments-table.py`.
    pub clef_codes: u16,
    /// Nombre de portées (`InstrumentTemplate::staffCount`, défaut 1).
    pub staves: u8,
    /// Groupe de portée (`staffGroup`), `"TAB"` pour les tablatures.
    pub staff_group: &'static str,
    /// Vrai si le gabarit utilise un jeu de percussion (`<drumset>`).
    pub use_drumset: bool,
    /// Vrai si le gabarit porte un jeu de percussion explicite (`drumset`).
    pub has_drumset: bool,
    /// Hauteurs non vides du jeu de percussion (`findNotEmptyDrumPitches`).
    pub drum_pitch_mask: u128,
    /// Hauteur minimale du gabarit (`minPitchP`), -1 si absente.
    pub min_pitch: i32,
    /// Hauteur maximale du gabarit (`maxPitchP`), -1 si absente.
    pub max_pitch: i32,
}''')
out.append('')
out.append('''/// Catalogue, dans l'ordre du fichier source (groupes puis instruments) :
/// c'est l'ordre de `instrumentGroups`, dont dépend le tri stable de
/// `sortInstrumentTemplates`.
pub const TEMPLATES: &[Template] = &[''')
for row in rows:
    mask = sum(1 << p for p in row['programs'] if 0 <= p < 128)
    out.append('    Template {')
    out.append(f"        id: \"{esc(row['id'])}\",")
    out.append(f"        group: \"{esc(row['group'])}\",")
    out.append(f"        track_name: \"{esc(row['track_name'])}\",")
    out.append(f"        long_name: \"{esc(row['long_name'])}\",")
    out.append(f"        short_name: \"{esc(row['short_name'])}\",")
    out.append(f"        music_xml_id: \"{esc(row['music_xml_id'])}\",")
    channel = ', '.join(str(p) for p in row['channel_programs'])
    out.append(f"        channel_programs: &[{channel}],")
    out.append(f'        program_mask: {mask},')
    # `rustfmt` éclate les tableaux de plus de 60 colonnes (`array_width`).
    genres = ', '.join(f"\"{esc(g)}\"" for g in row['genres'])
    if row['genres'] and len(genres) <= 60:
        out.append(f"        genres: &[{genres}],")
    elif row['genres']:
        out.append("        genres: &[")
        for genre in row['genres']:
            out.append(f"            \"{esc(genre)}\",")
        out.append("        ],")
    else:
        out.append("        genres: &[],")
    out.append(f"        transpose_diatonic: {row['transpose_diatonic']},")
    out.append(f"        transpose_chromatic: {row['transpose_chromatic']},")
    out.append(f"        clef_codes: 0x{row['clef_codes']:04x},")
    out.append(f"        staves: {row['staves']},")
    out.append(f"        staff_group: \"{esc(row['staff_group'])}\",")
    out.append(f"        use_drumset: {str(row['use_drumset']).lower()},")
    out.append(f"        has_drumset: {str(row['has_drumset']).lower()},")
    out.append(f"        drum_pitch_mask: {row['drum_pitch_mask']},")
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
    (0..128)
        .find(|program| template_has_program(template, *program as u8))
        .map(|program| program as i16)
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
