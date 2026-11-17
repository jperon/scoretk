// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
//
// Portage de `engraving/dom/part.h`, `engraving/dom/staff.h` et
// `engraving/dom/instrument.h` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Parties, portées et instruments.
//!
//! Portage de `Part` (`engraving/dom/part.h`), `Staff`
//! (`engraving/dom/staff.h`) et `Instrument` (`engraving/dom/instrument.h`).
//! Une partie possède une ou plusieurs portées ; la liste `staves` et le champ
//! [`Staff::part`] sont maintenus cohérents par [`super::Score::add_staff`].

use super::fraction::Fraction;
use super::ids::{PartId, StaffId};
use super::segment::{Clef, ClefType};

/// Instrument d'une partie.
///
/// Version minimale de `Instrument` dans `engraving/dom/instrument.h`, limitée
/// à ce que l'export MusicXML utilise pour `<score-part>` et `<transpose>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Instrument {
    /// Identifiant MusicXML / identifiant MuseScore de l'instrument, par
    /// exemple `"keyboard.piano"`.
    pub id: String,
    /// Nom long de l'instrument.
    pub long_name: String,
    /// Nom court de l'instrument.
    pub short_name: String,
    /// Nom de piste (`Instrument::trackName`) : c'est lui que MuseScore écrit
    /// dans `<instrument-name>`.
    pub track_name: String,
    /// Transposition écrite, si l'instrument est transpositeur.
    pub transpose: Option<Transpose>,
    /// Numéro de programme MIDI (0-127), si connu.
    pub midi_program: Option<u8>,
    /// Canal MIDI assigné au premier canal de l'instrument (`<midiChannel>`,
    /// base zéro), tel que lu dans le fichier.
    ///
    /// Absent quand le fichier ne fixe pas de canal ; MuseScore attribue alors
    /// un canal libre dans l'ordre des instruments (`rebuildMidiMapping`).
    pub midi_channel: Option<u8>,
    /// Volume du canal MIDI (`Channel::controller` 7), 0-127.
    ///
    /// Absent quand le fichier ne règle pas le contrôleur 7 ; MuseScore retient
    /// alors 100, valeur par défaut de son canal.
    pub volume: Option<u8>,
    /// Panoramique du canal MIDI (`Channel::controller` 10), 0-127.
    pub pan: Option<u8>,
}

impl Instrument {
    /// Construit un instrument à partir de ses identifiants et de ses noms.
    pub fn new(
        id: impl Into<String>,
        long_name: impl Into<String>,
        short_name: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let long_name = long_name.into();
        let short_name = short_name.into();
        Self {
            id,
            track_name: long_name.clone(),
            long_name,
            short_name,
            transpose: None,
            midi_program: None,
            midi_channel: None,
            volume: None,
            pan: None,
        }
    }
}

/// Transposition d'un instrument.
///
/// Correspond à `Interval` / aux valeurs de transposition de `Instrument`
/// dans `engraving/dom/instrument.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transpose {
    /// Intervalle diatonique en degrés.
    pub diatonic: i32,
    /// Intervalle chromatique en demi-tons.
    pub chromatic: i32,
}

impl Transpose {
    /// Construit une transposition.
    pub const fn new(diatonic: i32, chromatic: i32) -> Self {
        Self {
            diatonic,
            chromatic,
        }
    }
}

/// Identifiant MusicXML approché d'un instrument (`Instrument::recognizeMusicXmlId`).
///
/// MuseScore cherche l'instrument dans son catalogue par nom puis par
/// programme MIDI. Le modèle n'embarque pas le catalogue : cette table couvre
/// les instruments les plus courants, ce qui suffit à retrouver le
/// `<instrument-sound>` d'un changement d'instrument ancien (3.x) qui ne
/// stocke pas `instrumentId`.
pub fn recognize_musicxml_id(track_name: &str, program: Option<u8>) -> Option<&'static str> {
    let name = track_name.to_lowercase();
    let by_name = [
        ("flute", "wind.flutes.flute"),
        ("piccolo", "wind.flutes.piccolo"),
        ("oboe", "wind.reed.oboe"),
        ("bassoon", "wind.reed.bassoon"),
        ("clarinet", "wind.reed.clarinet.bflat"),
        ("sax", "wind.reed.saxophone"),
        ("trumpet", "brass.trumpet"),
        ("trombone", "brass.trombone"),
        ("tuba", "brass.tuba"),
        ("horn", "brass.french-horn"),
        ("violin", "strings.violin"),
        ("viola", "strings.viola"),
        ("cello", "strings.cello"),
        ("contrabass", "strings.contrabass"),
        ("guitar", "pluck.guitar"),
        ("harp", "pluck.harp"),
        ("piano", "keyboard.piano"),
        ("organ", "keyboard.organ"),
        ("harpsichord", "keyboard.harpsichord"),
        ("timpani", "percussion.timpani"),
        ("voice", "voice"),
    ];
    if let Some((_, id)) = by_name.iter().find(|(needle, _)| name.contains(needle)) {
        return Some(id);
    }
    match program? {
        0 => Some("keyboard.piano"),
        6 => Some("keyboard.harpsichord"),
        19 => Some("keyboard.organ"),
        24 => Some("pluck.guitar"),
        40 => Some("strings.violin"),
        41 => Some("strings.viola"),
        42 => Some("strings.cello"),
        43 => Some("strings.contrabass"),
        46 => Some("pluck.harp"),
        47 => Some("percussion.timpani"),
        56 => Some("brass.trumpet"),
        57 => Some("brass.trombone"),
        58 => Some("brass.tuba"),
        60 => Some("brass.french-horn"),
        68 => Some("wind.reed.oboe"),
        70 => Some("wind.reed.bassoon"),
        71 => Some("wind.reed.clarinet.bflat"),
        73 => Some("wind.flutes.flute"),
        _ => None,
    }
}

/// Partie de la partition.
///
/// Correspond à `Part` dans `engraving/dom/part.h`. Le champ `name` correspond
/// à `partName()` (nom de mixer), `long_name` et `short_name` aux noms affichés.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    /// Identifiant dans [`super::Score::parts`], attribué par
    /// [`super::Score::add_part`].
    pub id: PartId,
    /// Nom de la partie (mixer, identifiant MusicXML).
    pub name: String,
    /// Nom long affiché au début de la portée.
    pub long_name: String,
    /// Nom court affiché sur les portées suivantes.
    pub short_name: String,
    /// Instrument principal de la partie.
    pub instrument: Instrument,
    /// Changements d'instrument, triés par tick.
    ///
    /// Correspond aux entrées supplémentaires de `Part::instruments()` dans
    /// MuseScore : chaque `<InstrumentChange>` y ajoute son instrument au tick
    /// du segment (`Segment::add`). L'export en tire `<score-instrument>`,
    /// `<instruments>` et `<instrument>` par note.
    pub instrument_changes: Vec<(Fraction, Instrument)>,
    /// Portées appartenant à la partie, dans l'ordre.
    pub staves: Vec<StaffId>,
}

impl Part {
    /// Construit une partie avec un nom long et un nom court.
    ///
    /// Le nom de mixer reprend le nom long et l'instrument reprend les deux
    /// noms ; l'appelant peut les remplacer ensuite.
    pub fn new(long_name: impl Into<String>, short_name: impl Into<String>) -> Self {
        let long_name = long_name.into();
        let short_name = short_name.into();
        Self {
            id: PartId::from_raw(0),
            name: long_name.clone(),
            instrument: Instrument {
                long_name: long_name.clone(),
                short_name: short_name.clone(),
                track_name: long_name.clone(),
                ..Instrument::default()
            },
            long_name,
            short_name,
            instrument_changes: Vec::new(),
            staves: Vec::new(),
        }
    }

    /// Enregistre un instrument au tick d'un changement d'instrument.
    ///
    /// Comme `InstrumentList::setInstrument`, une entrée existante au même tick
    /// est remplacée ; les changements restent triés par tick.
    pub fn set_instrument_at(&mut self, tick: Fraction, instrument: Instrument) {
        match self
            .instrument_changes
            .binary_search_by(|(existing, _)| existing.cmp(&tick))
        {
            Ok(index) => self.instrument_changes[index].1 = instrument,
            Err(index) => self.instrument_changes.insert(index, (tick, instrument)),
        }
    }

    /// Instrument en vigueur à un tick donné (`Part::instrument`).
    ///
    /// C'est le dernier changement dont le tick ne dépasse pas `tick`, ou
    /// l'instrument principal.
    pub fn instrument_at(&self, tick: Fraction) -> &Instrument {
        self.instrument_changes
            .iter()
            .rev()
            .find(|(change, _)| *change <= tick)
            .map_or(&self.instrument, |(_, instrument)| instrument)
    }

    /// Instruments de la partie, principal puis changements dans l'ordre des
    /// ticks (`Part::instruments()`).
    pub fn instruments(&self) -> impl Iterator<Item = &Instrument> {
        std::iter::once(&self.instrument).chain(
            self.instrument_changes
                .iter()
                .map(|(_, instrument)| instrument),
        )
    }

    /// Rang d'un instrument dans [`Self::instruments`], pour son identifiant
    /// `P<n>-I<rang+1>`.
    pub fn instrument_index_at(&self, tick: Fraction) -> usize {
        self.instrument_changes
            .iter()
            .filter(|(change, _)| *change <= tick)
            .count()
    }
}

/// Accolade ou crochet d'une portée (`<bracket>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaffBracket {
    /// Type MuseScore : 0 crochet, 1 accolade, 2 trait, 3 crochet carré.
    pub kind: i8,
    /// Nombre de portées couvertes.
    pub span: usize,
    /// Vrai si l'accolade est visible.
    pub visible: bool,
}

/// Portée d'une partie.
///
/// Correspond à `Staff` dans `engraving/dom/staff.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staff {
    /// Identifiant dans [`super::Score::staves`], attribué par
    /// [`super::Score::add_staff`].
    pub id: StaffId,
    /// Partie à laquelle la portée appartient.
    pub part: PartId,
    /// Nombre de lignes de la portée (5 pour une portée standard).
    pub lines: u8,
    /// Vrai si la portée est en petit format.
    pub small: bool,
    /// Étirement temporel de la portée (`Staff::timeStretch`, indication de
    /// mesure locale). Vaut un hors indication locale.
    pub time_stretch: Fraction,
    /// Barre de mesure commune avec la portée suivante de la partie
    /// (`Staff::barLineSpan()`), pour `group-barline`.
    pub bar_line_span: bool,
    /// Accolades et crochets de la portée (`<bracket>`), dans l'ordre des
    /// colonnes.
    pub brackets: Vec<StaffBracket>,
    /// Clef par défaut de la portée (`Staff::defaultClefType()`).
    ///
    /// MuseScore s'en sert quand aucun `<Clef>` n'ouvre la partition : la mise
    /// en page génère alors une clef de ce type au début de la première mesure.
    /// Par défaut, la paire vaut sol/sol comme `ClefTypeList`.
    pub default_clef: Clef,
}

impl Staff {
    /// Construit une portée standard de cinq lignes pour une partie.
    pub const fn new(part: PartId) -> Self {
        Self {
            id: StaffId::from_raw(0),
            part,
            lines: 5,
            small: false,
            bar_line_span: false,
            brackets: Vec::new(),
            time_stretch: Fraction::ONE,
            default_clef: Clef::new(ClefType::G),
        }
    }
}
