#!/usr/bin/env python3
"""Génère la table Rust des instruments à partir de share/instruments/instruments.xml."""
import sys
import xml.etree.ElementTree as ET

src, dst = sys.argv[1], sys.argv[2]
tree = ET.parse(src)
root = tree.getroot()

rows = []
for group in root.iter('InstrumentGroup'):
    for instr in group.findall('Instrument'):
        iid = instr.get('id', '')
        track = instr.findtext('trackName', '')
        long_name = instr.findtext('longName', '')
        short = instr.findtext('shortName', '')
        mxml = instr.findtext('musicXMLid', '')
        program = None
        chan = instr.find('Channel')
        if chan is not None:
            p = chan.find('program')
            if p is not None:
                program = int(p.get('value', '-1'))
        rows.append((iid, track, long_name, short, mxml, program))

def esc(s):
    return s.replace('\\', '\\\\').replace('"', '\\"')

out = []
out.append('//! Table des instruments de MuseScore.')
out.append('//!')
out.append('//! Fichier **généré** à partir de `share/instruments/instruments.xml` de')
out.append('//! MuseScore 4.4 (`scripts/gen-instruments-table.py`). Les partitions 3.x')
out.append('//! antérieures à 3.6 ne stockent ni identifiant MusicXML ni programme MIDI :')
out.append('//! `Instrument::updateInstrumentId` les retrouve par le nom de piste puis par')
out.append('//! le programme (`engraving/dom/instrument.cpp`).')
out.append('')
out.append('/// Instrument du catalogue de MuseScore.')
out.append('#[derive(Clone, Copy, Debug)]')
out.append('pub(crate) struct Template {')
out.append('    /// Identifiant MuseScore (`Instrument::id`).')
out.append('    pub id: &\'static str,')
out.append('    /// Nom de piste (`trackName`), premier critère de reconnaissance.')
out.append('    pub track_name: &\'static str,')
out.append('    /// Nom long affiché.')
out.append('    pub long_name: &\'static str,')
out.append('    /// Nom court affiché.')
out.append('    pub short_name: &\'static str,')
out.append('    /// Identifiant MusicXML (`instrument-sound`).')
out.append('    pub music_xml_id: &\'static str,')
out.append('    /// Programme MIDI du premier canal, si connu.')
out.append('    pub program: Option<i16>,')
out.append('}')
out.append('')
out.append('/// Catalogue, dans l\'ordre du fichier source : à force égale, le premier')
out.append('/// instrument l\'emporte, comme `searchTemplateForInstrNameList`.')
out.append('pub(crate) const TEMPLATES: &[Template] = &[')
for iid, track, long_name, short, mxml, program in rows:
    prog = 'None' if program is None or program < 0 else f'Some({program})'
    out.append('    Template {')
    out.append(f'        id: "{esc(iid)}",')
    out.append(f'        track_name: "{esc(track)}",')
    out.append(f'        long_name: "{esc(long_name)}",')
    out.append(f'        short_name: "{esc(short)}",')
    out.append(f'        music_xml_id: "{esc(mxml)}",')
    out.append(f'        program: {prog},')
    out.append('    },')
out.append('];')
out.append('')

with open(dst, 'w') as f:
    f.write('\n'.join(out))
print(f'{len(rows)} instruments, {len(out)} lignes')
