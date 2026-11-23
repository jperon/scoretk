// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/read400.cpp`,
// `engraving/rw/read410/read410.cpp`, `engraving/rw/read460/read460.cpp` et
// `engraving/rw/read500/read500.cpp` de MuseScore Studio, Copyright (C)
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

//! Lecteur `.mscx` 4.x (versions 4.00 à 5.00).
//!
//! Portage de `engraving/rw/read400/read400.cpp`, `read410/read410.cpp`,
//! `read460/read460.cpp` et `read500/read500.cpp`, avec leurs satellites
//! `staffrw`/`staffread`, `measurerw`/`measureread` et `tread`. Les différences
//! entre versions sont branchées sur [`Reader::version`] là où le C++ diverge :
//!
//! - **4.00** : armure lue dans `<accidental>`, clé d'en-tête devinée par la
//!   position, `BeamMode` ancien (`begin32`/`begin64`).
//! - **4.10** : `concertKey`/`actualKey` acceptés en plus d'`<accidental>`,
//!   `<isHeader>` fait foi, `BeamMode` lu directement.
//! - **4.50+** : courtoisie par `<isCourtesy>` uniquement.
//! - **4.60+** : armure lue uniquement dans `concertKey`/`actualKey`.
//! - **5.00** : `<Part id>` et `<Staff>` positionnel — tolérés dans toutes les
//!   versions par le lecteur.
//!
//! Ce qui n'est pas du noyau musical est ignoré volontairement, avec une note
//! `TODO(parité)` quand l'export MusicXML en dépendra.

mod dom;
mod figures;
mod fret;
mod names;
mod notations;
mod nuances;
mod ottava;
mod pedals;
mod repeats;
mod style;
mod textlines;

use dom::Element;

use crate::container::MscVersion;
use crate::{Error, Result};
use scoretk_core::model::{ids::StaffId, tpc};
use scoretk_core::model::{
    text_style_defaults, Accidental, Annotation, Articulation, BarLine, BarLineType, BeamMode,
    Breath, Chord, ChordRest, ChordRestKind, Clef, ClefType, Credit, CreditJustify, CreditStyle,
    Duration, DurationType, Fraction, GraceChord, GraceType, Harmony, HarmonyDegree,
    HarmonyDegreeKind, Instrument, InstrumentChange, Key, KeyMode, KeySig, Lyrics, LyricsSyllabic,
    Measure, MidiArticulation, Note, NoteId, Part, Placement, RehearsalFrame, RehearsalMark,
    RichText, Score, Segment, SegmentItem, SegmentType, Staff, StaffText, StemDirection,
    SystemText, Tempo, Tick, TimeSig, TimeSigType, Track, Transpose, TremoloKind, Tuplet, TupletId,
    TupletNumberType, VeloType, VOICES,
};

/// Vrai si un cadre porte un saut de page (`LayoutBreak` `page`).
fn has_page_break(element: &Element) -> bool {
    element.children_named("LayoutBreak").any(|layout_break| {
        layout_break
            .child("subtype")
            .is_some_and(|subtype| subtype.text() == "page")
    })
}

/// Lit un `.mscx` 4.x/5.x.
pub fn read(mscx: &[u8], version: MscVersion) -> Result<Score> {
    let document = dom::parse(mscx)?;
    let score_element = if document.name == "Score" {
        &document
    } else {
        document.child("Score").ok_or(Error::MissingScore)?
    };

    let mut reader = Reader::new(version);
    reader.read_score(score_element)?;
    Ok(reader.score)
}

/// État d'une portée pendant la lecture.
#[derive(Clone, Debug, Default)]
struct StaffState {
    /// Étirement temporel courant (`Staff::timeStretch`).
    time_stretch: Option<Fraction>,
}

/// Extrémité `<location>` d'une moitié de liaison, relative au tick courant.
#[derive(Clone, Copy, Debug)]
struct TieLocation {
    measures: i64,
    fractions: Fraction,
    staves: i64,
    voices: i64,
}

impl TieLocation {
    /// Tick absolu de l'autre extrémité.
    ///
    /// `Location::toAbsolute` additionne les fractions à la position dans la
    /// mesure courante, puis les mesures déplacent le tout : la position
    /// relative dans la mesure cible est la même que dans la mesure de départ
    /// (`<measures>1</measures><fractions>-3/4</fractions>` vise le début de la
    /// mesure suivante depuis le troisième quart).
    fn absolute_tick(self, tick: Tick, measures: &[scoretk_core::model::Measure]) -> Tick {
        let Some(current) = measure_index_at(measures, tick) else {
            return tick + self.fractions;
        };
        let current_base = measures[current].start_tick;
        let target = current as i64 + self.measures;
        let base = usize::try_from(target)
            .ok()
            .and_then(|index| measures.get(index))
            .map_or(current_base, |measure| measure.start_tick);
        base + (tick - current_base) + self.fractions
    }

    fn absolute_track(self, track: Track) -> Track {
        let raw = i64::from(track.raw()) + self.voices + self.staves * VOICES as i64;
        Track::from_raw(raw.max(0) as u32)
    }
}

/// Index de la mesure contenant un tick.
/// Lit un `<Channel>` d'instrument (`TRead::read(InstrChannel*)`,
/// `read400/tread.cpp:926-1020`).
fn read_instrument_channel(node: &Element) -> scoretk_core::model::InstrumentChannel {
    use scoretk_core::model::InstrumentChannel;
    let mut channel = InstrumentChannel {
        name: node.attr("name").unwrap_or("").to_owned(),
        ..InstrumentChannel::default()
    };
    channel.program = node.child("program").and_then(|program| {
        program
            .attr_i64("value")
            .or_else(|| program.int())
            .and_then(|value| u8::try_from(value).ok())
    });
    // `CTRL_HBANK`/`CTRL_LBANK` (`tread.cpp:953-961`) : banque =
    // `(hbank << 7) + lbank`, dans cet ordre d'écriture.
    let mut bank = 0u16;
    let mut has_bank = false;
    for controller in node.children_named("controller") {
        let Some(value) = controller.attr_i64("value") else {
            continue;
        };
        match controller.attr_i64("ctrl") {
            Some(0) => {
                bank = (bank & 0x7f) | ((value as u16) << 7);
                has_bank = true;
            }
            Some(32) => {
                bank = (bank & !0x7f) | ((value as u16) & 0x7f);
                has_bank = true;
            }
            _ => {}
        }
    }
    channel.bank = has_bank.then_some(bank);
    channel.midi_port = node
        .child("midiPort")
        .and_then(Element::int)
        .and_then(|value| u8::try_from(value).ok());
    channel.midi_channel = node
        .child("midiChannel")
        .and_then(Element::int)
        .and_then(|value| u8::try_from(value).ok());
    channel.mute = node
        .child("mute")
        .and_then(Element::int)
        .map(|value| value as i32);
    channel.solo = node
        .child("solo")
        .and_then(Element::int)
        .map(|value| value as i32);
    channel
}

fn measure_index_at(measures: &[scoretk_core::model::Measure], tick: Tick) -> Option<usize> {
    let index = measures.partition_point(|measure| measure.start_tick <= tick);
    index.checked_sub(1).filter(|index| *index < measures.len())
}

/// Extrémité d'une liaison de prolongation en attente d'appariement.
#[derive(Clone, Debug)]
struct TieRecord {
    note: NoteId,
    track: Track,
    tick: Tick,
    pitch: u8,
    /// Vrai pour la moitié `<Tie><next>`, faux pour `<prev>`.
    is_start: bool,
    /// Position de l'autre extrémité, d'après `<location>`.
    other: TieLocation,
    /// Direction explicite lue dans `<Tie><up>` (`Pid::SLUR_DIRECTION`).
    up: Option<bool>,
    /// Vrai si la liaison est affichée (`<Tie><visible>`).
    visible: bool,
}

/// État complet du lecteur pour un fichier.
struct Reader {
    /// Version numérique (`400`, `410`, `460`, `500`).
    version: u32,
    score: Score,
    /// `Score::fileDivision`, en ticks par noire.
    file_division: u32,
    staves: Vec<StaffState>,
    /// Identifiants `id` des `<Staff>` de déclaration, indexés par portée.
    staff_ids: Vec<Option<u32>>,
    /// Prochaine portée pour les fichiers 5.0 dont `<Staff>` n'a pas d'`id`.
    next_music_staff: usize,
    /// Tick de création de la prochaine mesure (portée 0).
    global_tick: Tick,
    /// Dernière durée nominale de mesure (métrique héritée).
    last_nominal: Fraction,
    /// Liaisons lues mais pas encore appariées.
    ties: Vec<TieRecord>,
    /// Moitiés de voltas lues mais pas encore appariées.
    voltas: Vec<repeats::VoltaHalf>,
    /// Moitiés de soufflets lues mais pas encore appariées.
    spanner_halves: Vec<nuances::Half>,
    /// Moitiés de spanners de notation (liaison, trille, glissando).
    notation_halves: Vec<notations::Half>,
    /// Moitiés de lignes de texte lues mais pas encore appariées.
    text_lines: Vec<textlines::TextLineHalf>,
    /// Moitiés d'ottavas lues mais pas encore appariées.
    ottava_halves: Vec<ottava::Half>,
    pedal_halves: Vec<pedals::Half>,
    /// Mesures à répéter, développées en fin de lecture.
    measure_repeats: Vec<crate::read::measure_repeat::MeasureRepeat>,
    /// Vrai si le style active les silences multi-mesures
    /// (`<createMultiMeasureRests>`).
    create_multi_measure_rests: bool,
}

impl Reader {
    fn new(version: MscVersion) -> Self {
        let mut score = Score::new();
        score.style = scoretk_core::model::StyleSettings::for_version(version.major());
        Self {
            version: version.as_u32(),
            score,
            file_division: 480,
            staves: Vec::new(),
            staff_ids: Vec::new(),
            next_music_staff: 0,
            global_tick: Fraction::ZERO,
            last_nominal: Fraction::ONE,
            ties: Vec::new(),
            voltas: Vec::new(),
            spanner_halves: Vec::new(),
            notation_halves: Vec::new(),
            text_lines: Vec::new(),
            ottava_halves: Vec::new(),
            pedal_halves: Vec::new(),
            measure_repeats: Vec::new(),
            create_multi_measure_rests: false,
        }
    }

    fn stretch(&self, staff_index: usize) -> Fraction {
        self.staves
            .get(staff_index)
            .and_then(|state| state.time_stretch)
            .unwrap_or(Fraction::ONE)
    }

    //------------------------------------------------------------------
    // <Score>
    //------------------------------------------------------------------

    fn read_score(&mut self, element: &Element) -> Result<()> {
        // Première passe : parties et métadonnées, pour que les portées
        // existent avant la lecture de la musique.
        for child in &element.children {
            match child.name.as_str() {
                "Division" => {
                    if let Some(division) = child.int() {
                        if division > 0 {
                            self.file_division = division as u32;
                        }
                    }
                }
                "Part" => self.read_part(child),
                "Style" => {
                    self.score.style = style::read(child, MscVersion::from_raw(self.version));
                    self.create_multi_measure_rests = child
                        .child("createMultiMeasureRests")
                        .and_then(Element::int)
                        .is_some_and(|value| value != 0);
                }
                "metaTag" => self.read_meta_tag(child),
                "workTitle" => self.score.metadata.work_title = child.text().to_owned(),
                "workNumber" => self.score.metadata.work_number = child.text().to_owned(),
                "movementTitle" => self.score.metadata.movement_title = child.text().to_owned(),
                "copyright" | "rights" => {
                    self.score.metadata.copyright = child.text().to_owned();
                }
                "source" => self.score.metadata.source = child.text().to_owned(),
                _ => {}
            }
        }

        // Seconde passe : musique des portées, dans l'ordre du fichier.
        // TODO(parité): <Score> imbriqué (parties liées inline) ; pour l'export
        // de la partition complète, les ignorer est correct.
        for child in element.children_named("Staff") {
            self.read_staff(child)?;
        }

        self.assign_multiple_rests();
        self.resolve_ties();
        self.resolve_ties();
        repeats::resolve(self);
        // Les soufflets sont résolus avant les lignes de texte : à un même
        // tick, MuseScore ajoute les spanners dans l'ordre du fichier, où le
        // soufflet précède la ligne de texte dans le corpus.
        nuances::resolve(&mut self.score, &self.spanner_halves);
        textlines::resolve(self);
        pedals::resolve(&mut self.score, &self.pedal_halves);
        notations::resolve(self);
        ottava::resolve(&mut self.score, &self.ottava_halves);
        // Après les résolutions de spanners : la copie d'une mesure répétée
        // reprend ceux de la mesure source.
        for repeat in std::mem::take(&mut self.measure_repeats) {
            crate::read::measure_repeat::expand(&mut self.score, &repeat);
        }
        Ok(())
    }

    /// Lit un `<Harmony>` et l'ajoute comme annotation (`readMeasure206` :
    /// `read500/measureread.cpp:448-450` lit aussi celui d'un `FretDiagram`).
    fn read_harmony_annotation(&mut self, child: &Element) -> Harmony {
        let name = child
            .child("name")
            .map(|node| node.text().to_owned())
            .unwrap_or_default();
        let mut harmony = Harmony::new(name.clone());
        harmony.root_tpc = child
            .child("root")
            .and_then(Element::int)
            .and_then(|value| i8::try_from(value).ok())
            .unwrap_or(tpc::INVALID);
        harmony.bass_tpc = child
            .child("bass")
            .and_then(Element::int)
            .and_then(|value| i8::try_from(value).ok());
        harmony.name = name;
        harmony.function = child.child("function").map(|node| node.text().to_owned());
        harmony.placement = placement(child.child("placement"));
        harmony.play = child
            .child("play")
            .and_then(Element::int)
            .is_none_or(|value| value != 0);
        // `TRead::read(Harmony*)` : `<harmonyType>` conserve
        // l'écriture (standard, romaine, Nashville), dont dépend
        // toute la branche d'export MusicXML.
        if let Some(kind) = child.child("harmonyType").and_then(Element::int) {
            harmony.kind = match kind {
                1 => "roman",
                2 => "nashville",
                _ => "normal",
            }
            .to_owned();
        }
        for degree in child.children_named("degree") {
            if let Some(degree) = read_harmony_degree(degree) {
                harmony.degrees.push(degree);
            }
        }
        if let Some(text) = child.child("text") {
            harmony.text = text.text.clone();
        }
        harmony
    }

    fn read_meta_tag(&mut self, element: &Element) {
        let name = element.attr("name").unwrap_or_default();
        let value = element.text().to_owned();
        let metadata = &mut self.score.metadata;
        match name {
            "workTitle" => metadata.work_title = value,
            "movementTitle" => metadata.movement_title = value,
            "movementNumber" => metadata.movement_number = value,
            "workNumber" => metadata.work_number = value,
            "composer" => metadata.composer = value,
            "lyricist" => metadata.lyricist = value,
            "poet" => metadata.poet = value,
            "translator" => metadata.translator = value,
            "arranger" => metadata.arranger = value,
            "copyright" => metadata.copyright = value,
            "source" => metadata.source = value,
            "creationDate" => metadata.creation_date = value,
            "platform" => metadata.platform = value,
            "subtitle" => metadata.subtitle = value,
            "mscVersion" => metadata.msc_version = value,
            _ => {}
        }
    }

    //------------------------------------------------------------------
    // <Part>
    //------------------------------------------------------------------

    fn read_part(&mut self, element: &Element) {
        let part_id = self.score.add_part(Part::new("", ""));

        // L'instrument précède les portées : `Staff::init` prend sa clef par
        // défaut sur l'instrument, puis `<defaultClef>` peut la remplacer.
        let instrument_element = element.child("Instrument");
        let mut instrument = instrument_element
            .map(|child| self.read_instrument(child))
            .unwrap_or_default();
        let instrument_clefs = instrument_element
            .map(read_instrument_clefs)
            .unwrap_or_default();

        let mut declared = 0usize;
        for staff_element in element.children_named("Staff") {
            let mut staff = Staff::new(part_id);
            if let Some(staff_type) = staff_element.child("StaffType") {
                staff.lines = staff_lines(staff_type);
                // `Staff::mag()` : une portée `<small>` utilise `smallStaffMag`
                // (0.7, `styledef.cpp:529`) ; l'export en dérive `<staff-size>`.
                if let Some(small) = staff_type.child("small").and_then(Element::int) {
                    staff.small = small != 0;
                }
                if let Some(mag) = staff_type
                    .child("mag")
                    .and_then(|mag| mag.text().parse::<f64>().ok())
                {
                    staff.user_mag_percent = (mag * 100.0).round() as i32;
                }
            }
            // `TRead::read(Staff*)` (`read500/tread.cpp:4111`) :
            // `<isStaffVisible>` commande `Staff::setVisible`. MuseScore signale
            // ensuite la portée par `print-object="no"` dans `<staff-details>`.
            if let Some(value) = staff_element.child("isStaffVisible").and_then(Element::int) {
                staff.visible = value != 0;
            }
            if let Some(value) = staff_element.child("barLineSpan").and_then(Element::int) {
                staff.bar_line_span = value != 0;
            }
            for bracket in staff_element.children_named("bracket") {
                let kind = bracket.attr_i64("type").unwrap_or(-1);
                if kind < 0 {
                    continue;
                }
                staff.brackets.push(scoretk_core::model::StaffBracket {
                    kind: kind.clamp(0, 127) as i8,
                    span: bracket.attr_i64("span").unwrap_or(0).max(0) as usize,
                    visible: bracket.attr_i64("visible").unwrap_or(1) != 0,
                });
            }
            staff.default_clef = if has_default_clef(staff_element) {
                read_default_clef(staff_element)
            } else {
                instrument_clefs
                    .get(declared)
                    .copied()
                    .unwrap_or(Clef::new(ClefType::G))
            };
            self.score.add_staff_with(part_id, staff);
            self.staff_ids
                .push(staff_element.attr_i64("id").map(|id| id as u32));
            self.staves.push(StaffState::default());
            declared += 1;
        }
        if declared == 0 {
            // Une partie doit avoir au moins une portée (`Part::nstaves() > 0`).
            let mut staff = Staff::new(part_id);
            staff.default_clef = instrument_clefs
                .first()
                .copied()
                .unwrap_or(Clef::new(ClefType::G));
            self.score.add_staff_with(part_id, staff);
            self.staff_ids.push(None);
            self.staves.push(StaffState::default());
        }

        let part_track_name = element
            .child("trackName")
            .map(|child| child.text().to_owned())
            .unwrap_or_default();
        // `TRead::readLegacyStaffName` (`read500/tread.cpp:4200`) : les noms
        // d'instrument gardent leurs sauts de ligne `<br/>`.
        let part_long_name = element
            .child("name")
            .map(Element::legacy_staff_name)
            .unwrap_or_default();
        let part_short_name = element
            .child("shortName")
            .map(Element::legacy_staff_name)
            .unwrap_or_default();

        let long_name = if instrument.long_name.is_empty() {
            part_long_name
        } else {
            instrument.long_name.clone()
        };
        let short_name = if instrument.short_name.is_empty() {
            part_short_name
        } else {
            instrument.short_name.clone()
        };
        let name = match (part_track_name.is_empty(), instrument.long_name.is_empty()) {
            (false, _) => part_track_name,
            (true, false) => instrument.long_name.clone(),
            (true, true) => long_name.clone(),
        };
        if instrument.long_name.is_empty() {
            instrument.long_name = long_name.clone();
        }
        if instrument.short_name.is_empty() {
            instrument.short_name = short_name.clone();
        }

        let part = self.score.part_mut(part_id);
        part.name = name;
        part.long_name = long_name;
        part.short_name = short_name;
        part.instrument = instrument;
        // `TRead::read(Part*)` (`read500/tread.cpp`) : `<show>0</show>` masque
        // toute la partie ; `Staff::show()` en dépend (`staff.cpp:1420-1423`).
        part.visible = element.child("show").and_then(Element::int).unwrap_or(1) != 0;
        // TODO(parité): <preferSharpFlat>, <show>, <soloist>.
    }

    fn read_instrument(&self, element: &Element) -> Instrument {
        let mut instrument = Instrument {
            id: element
                .child("instrumentId")
                .map(|child| child.text().to_owned())
                .or_else(|| element.attr("id").map(str::to_owned))
                .unwrap_or_default(),
            long_name: element
                .child("longName")
                .or_else(|| {
                    // 5.0 déplace les noms dans <InstrumentLabel>.
                    element
                        .child("InstrumentLabel")
                        .and_then(|label| label.child("longName"))
                })
                .map(Element::legacy_staff_name)
                .unwrap_or_default(),
            short_name: element
                .child("shortName")
                .or_else(|| {
                    element
                        .child("InstrumentLabel")
                        .and_then(|label| label.child("shortName"))
                })
                .map(Element::legacy_staff_name)
                .unwrap_or_default(),
            // `Instrument::trackName()`, distinct du nom long : c'est lui que
            // `<instrument-name>` reprend.
            track_name: element
                .child("trackName")
                .or_else(|| {
                    element
                        .child("InstrumentLabel")
                        .and_then(|label| label.child("trackName"))
                })
                .map(|child| child.text().to_owned())
                .unwrap_or_default(),
            ..Instrument::default()
        };

        let chromatic = element
            .child("transposeChromatic")
            .and_then(Element::int)
            .unwrap_or(0) as i32;
        let diatonic = element
            .child("transposeDiatonic")
            .and_then(Element::int)
            .map_or_else(|| chromatic_to_diatonic(chromatic), |value| value as i32);
        if chromatic != 0 || diatonic != 0 {
            instrument.transpose = Some(Transpose::new(diatonic, chromatic));
        }

        // `rebuildMidiMapping` (`midimapping.cpp:286-310`) alloue un canal
        // MIDI à chaque `<Channel>` de tous les instruments, y compris les
        // canaux secondaires (mute, harmony…) qui ne sont pas exportés.
        instrument.channel_count = element.children_named("Channel").count().max(1);
        // Table complète des canaux (`TRead::read(InstrChannel*)`,
        // read400/tread.cpp:926-1020 ; read500/tread.cpp:1144-1238).
        instrument.channels = element
            .children_named("Channel")
            .map(read_instrument_channel)
            .collect();
        instrument.midi_program = element
            .child("Channel")
            .and_then(|channel| channel.child("program"))
            .and_then(|program| program.attr_i64("value"))
            .and_then(|value| u8::try_from(value).ok());

        // Réglages de canal : contrôleur 7 (volume) et 10 (panoramique).
        // `midiInstrument` en dérive `<volume>`/`<pan>` à l'export.
        if let Some(channel) = element.child("Channel") {
            for controller in channel.children_named("controller") {
                let Some(value) = controller
                    .attr_i64("value")
                    .and_then(|value| u8::try_from(value.clamp(0, 127)).ok())
                else {
                    continue;
                };
                match controller.attr_i64("ctrl") {
                    Some(7) => instrument.volume = Some(value),
                    Some(10) => instrument.pan = Some(value),
                    _ => {}
                }
            }
            // Canal MIDI du premier canal sonore (`<Channel><midiChannel>`,
            // base zéro), comme le lit `read302` et comme `TRead` le stocke.
            instrument.midi_channel = channel
                .child("midiChannel")
                .and_then(Element::int)
                .and_then(|value| u8::try_from(value.clamp(0, 127)).ok());
            // Port du canal (`Channel::port` dans le C++) : MuseScore l'écrit
            // dans `<midi-device port>` (exportmusicxml.cpp:7939-7947).
            instrument.midi_port = channel
                .child("midiPort")
                .and_then(Element::int)
                .and_then(|value| u8::try_from(value.clamp(0, 15)).ok());
        }

        // Table des articulations MIDI (`read(MidiArticulation*)`) : une
        // entrée sans attribut `name` donne la durée de tenue par défaut de
        // l'instrument, les autres modifient vélocité et tenue.
        for articulation in element.children_named("Articulation") {
            instrument.midi_articulations.push(MidiArticulation {
                name: articulation.attr("name").unwrap_or_default().to_owned(),
                velocity: articulation
                    .child("velocity")
                    .and_then(Element::int)
                    .unwrap_or(0) as i16,
                gate_time: articulation
                    .child("gateTime")
                    .and_then(Element::int)
                    .unwrap_or(0) as i16,
            });
        }

        // TODO(parité): <useDrumset>, <Drum>, <stringData>.
        instrument
    }

    //------------------------------------------------------------------
    // <Staff> (musique)
    //------------------------------------------------------------------

    fn read_staff(&mut self, element: &Element) -> Result<()> {
        let staff_index = match element.attr_i64("id") {
            Some(id) => {
                let id = id as u32;
                self.staff_ids
                    .iter()
                    .position(|declared| *declared == Some(id))
                    .unwrap_or_else(|| id.saturating_sub(1) as usize)
            }
            None => {
                let index = self.next_music_staff;
                self.next_music_staff += 1;
                index
            }
        };
        if staff_index >= self.score.staves.len() {
            // Portée sans déclaration `<Part>` : rien à remplir.
            return Ok(());
        }

        let mut measure_index = 0usize;
        // Numéro de page courant pour les textes de cadre, comme
        // `Score::pages()` : un `<LayoutBreak>` de type `page` place l'élément
        // qui le porte sur la page suivante.
        let mut page = 1u32;
        // Un saut de section porte sur la mesure suivante.
        let mut section_break = false;
        let mut section_keeps_numbering = false;
        let mut section_pause: Option<f64> = None;
        for child in &element.children {
            match child.name.as_str() {
                "Measure" => {
                    // Une mesure de multi-repos est un marqueur de mise en
                    // page : ce n'est pas une mesure musicale, elle ne compte
                    // donc ni dans la numérotation ni dans le tick.
                    if let Some(count) = child
                        .child("multiMeasureRest")
                        .and_then(Element::int)
                        .filter(|count| *count > 0)
                    {
                        // Le marqueur suit la première mesure du groupe qu'il
                        // remplace (`Measure::mmRestFirst`) : la mesure
                        // précédente ouvre le `<multiple-rest>`.
                        if measure_index > 0 {
                            let previous = measure_index - 1;
                            let count = u16::try_from(count).unwrap_or(u16::MAX);
                            // `createMMRest` (mmrestlayout.cpp:161-206) crée un
                            // `<multiple-rest>` pour la portée du `<Staff>` qui
                            // porte le marqueur, pas pour toute la mesure.
                            self.score.measures[previous]
                                .mmrests
                                .set(StaffId::from_index(staff_index), count);
                            if staff_index == 0 {
                                self.score.measures[previous].multi_measure_rest = Some(count);
                            }
                        }
                        continue;
                    }
                    self.read_measure(child, staff_index, measure_index)?;
                    if section_break {
                        let measure = &mut self.score.measures[measure_index];
                        measure.section_break_before = true;
                        measure.section_keeps_numbering = section_keeps_numbering;
                        measure.section_pause = section_pause;
                    }
                    section_break = Self::has_section_break(child);
                    if section_break {
                        section_keeps_numbering = !Self::section_break_starts_at_one(child);
                        section_pause = Self::section_break_pause(child);
                    }
                    measure_index += 1;
                }
                // Les cadres changent la numérotation affichée mais pas le
                // tick musical ; un saut de section y porte sur la mesure
                // suivante. MuseScore les écrit sous la première portée
                // (`StaffRead::readStaff`).
                "VBox" => {
                    if Self::has_section_break(child) {
                        section_break = true;
                        section_keeps_numbering = !Self::section_break_starts_at_one(child);
                        section_pause = Self::section_break_pause(child);
                    }
                    if staff_index == 0 {
                        self.read_vbox(child, page);
                    }
                }
                "TBox" => {
                    if Self::has_section_break(child) {
                        section_break = true;
                        section_keeps_numbering = !Self::section_break_starts_at_one(child);
                        section_pause = Self::section_break_pause(child);
                    }
                    // Texte de cadre exporté en `<words>` au début de la mesure
                    // qui suit (`tboxTextAsWords`) ; un cadre suivi d'un saut
                    // de page est au-dessous du système précédent.
                    if staff_index == 0 {
                        let below = measure_index > 0 && has_page_break(child);
                        if let Some(mut text) = textlines::read_tbox(child, measure_index, below) {
                            if below {
                                text.measure = crate::read::mmrest_group_start(
                                    &self.score.measures,
                                    text.measure,
                                );
                            }
                            self.score.add_tbox_text(text);
                        }
                    }
                }
                "HBox" | "FBox" => {
                    if Self::has_section_break(child) {
                        section_break = true;
                        section_keeps_numbering = !Self::section_break_starts_at_one(child);
                    }
                }
                "tick" => {}
                _ => {}
            }
            // Un saut de page s'applique *après* l'élément qui le porte :
            // le cadre ou la mesure reste sur la page courante, le suivant
            // commence la page d'après (mise en page de MuseScore).
            if staff_index == 0 && page_break(child) {
                page += 1;
                self.score.style.page_count = self.score.style.page_count.max(page);
            }
        }
        Ok(())
    }

    /// Vrai si un cadre ou une mesure porte un saut de section.
    /// Le `<LayoutBreak>` de section d'une mesure ou d'un cadre. Une mesure
    /// peut en porter plusieurs (saut de ligne + saut de section) :
    /// `MeasureBase::sectionBreakElement()` (`measurebase.cpp:741-752`) choisit
    /// celui dont le sous-type est `section`.
    fn section_layout_break(element: &Element) -> Option<&Element> {
        element.children_named("LayoutBreak").find(|layout_break| {
            layout_break
                .child("subtype")
                .is_some_and(|subtype| subtype.text() == "section")
        })
    }

    fn has_section_break(element: &Element) -> bool {
        Self::section_layout_break(element).is_some()
    }

    /// Durée de la pause du saut de section (`LayoutBreak::pause()`,
    /// `layoutbreak.cpp:138-140`) : `<pause>` en secondes, `None` si absent.
    fn section_break_pause(element: &Element) -> Option<f64> {
        Self::section_layout_break(element)
            .and_then(|layout_break| layout_break.child("pause"))
            .and_then(|pause| pause.text().parse().ok())
    }

    /// `startWithMeasureOne` du saut de section porté par l'élément.
    /// Absent, le corpus MuseScore 4.7.4 relance la numérotation à 1.
    fn section_break_starts_at_one(element: &Element) -> bool {
        Self::section_layout_break(element)
            .and_then(|layout_break| layout_break.child("startWithMeasureOne"))
            .and_then(Element::int)
            .unwrap_or(1)
            != 0
    }

    /// Lit les textes d'un cadre vertical (`VBox`), exportés en `<credit>`.
    ///
    /// `VBox::el()` (`engraving/dom/vbox.cpp`) contient des `Text` ; l'export
    /// les parcourt par page (`ExportMusicXml::credits`). `page` vient du
    /// dernier `<LayoutBreak>` rencontré.
    fn read_vbox(&mut self, element: &Element, page: u32) {
        let height = element
            .child("height")
            .map(Element::text)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(10.0);
        for text_element in element.children_named("Text") {
            if !visible(text_element) {
                continue;
            }
            let style = text_element
                .child("style")
                .map(Element::text)
                .unwrap_or_default();
            let markup = text_element
                .child("text")
                .map_or("", |node| node.raw_inner.as_str());
            let text = RichText::from_markup(markup);
            if text.is_empty() {
                continue;
            }
            let mut credit = Credit::new(text, CreditStyle::from_name(style));
            credit.page = page;
            credit.vbox_height = height;
            // `<align>` ne fixe que l'alignement horizontal (`Pid::POSITION`),
            // l'alignement vertical reste celui du style.
            if let Some(align) = text_element.child("align").map(Element::text) {
                if let Some(horizontal) = align.split(',').next() {
                    if let Some(justify) = CreditJustify::from_name(horizontal.trim()) {
                        credit.justify = justify;
                    }
                }
            }
            self.score.add_credit(credit);
        }
    }

    /// Complète les silences multi-mesures d'un seul tenant.
    ///
    /// Avec `<createMultiMeasureRests>`, MuseScore remplace aussi les mesures
    /// vides isolées par un multi-repos de un : le `.mscx` n'écrit alors aucun
    /// marqueur. Les groupes plus longs viennent des marqueurs déjà lus ; toute
    /// mesure vide non couverte reçoit `multiple-rest 1`.
    fn assign_multiple_rests(&mut self) {
        if !self.create_multi_measure_rests {
            return;
        }
        // Les mesures vides isolées ne deviennent un multi-repos de un que si
        // la partition entière en est faite (mise en page de MuseScore) ; une
        // partition avec du contenu ne les regroupe pas.
        if !self.score.measures.iter().all(is_empty_measure) {
            return;
        }
        let mut covered_until = 0usize;
        for index in 0..self.score.measures.len() {
            if index < covered_until {
                continue;
            }
            if let Some(count) = self.score.measures[index].multi_measure_rest {
                covered_until = index + usize::from(count);
                continue;
            }
            if is_empty_measure(&self.score.measures[index]) {
                self.score.measures[index].multi_measure_rest = Some(1);
                for staff in 0..self.score.staves.len() {
                    self.score.measures[index]
                        .mmrests
                        .set(StaffId::from_index(staff), 1);
                }
                covered_until = index + 1;
            }
        }
    }

    //------------------------------------------------------------------
    // <Measure>
    //------------------------------------------------------------------

    fn read_measure(
        &mut self,
        element: &Element,
        staff_index: usize,
        measure_index: usize,
    ) -> Result<()> {
        // Une mesure de multi-repos (marqueur) n'existe pas dans la partition :
        // `StaffRead::readStaff` ne l'ajoute pas et n'avance pas le tick.
        if element.has_child("multiMeasureRest") {
            // TODO(parité): conserver le regroupement de multi-repos.
            return Ok(());
        }

        let irregular = element.attr("len").is_some();
        let model_index = if staff_index == 0 {
            let mut measure = Measure::new(measure_index, self.global_tick, self.last_nominal);
            // Seul `<irregular>1</irregular>` exclut la mesure de la
            // numérotation (`Measure::excludeFromNumbering`) ; un simple
            // `@len` rend la mesure irrégulière mais la garde numérotée.
            measure.exclude_from_numbering = element
                .child("irregular")
                .and_then(Element::int)
                .is_some_and(|value| value != 0);
            if let Some(len) = element.attr_fraction("len") {
                if len.is_positive() {
                    measure.len = len;
                }
            }
            self.score.add_measure(measure).index()
        } else if measure_index < self.score.measures.len() {
            measure_index
        } else {
            // Mesure absente de la première portée : `readStaff` en crée une
            // vide (« missing measure! »).
            let start = self
                .score
                .measures
                .last()
                .map_or(Fraction::ZERO, Measure::end_tick);
            self.score
                .add_measure(Measure::new(measure_index, start, self.last_nominal))
                .index()
        };

        {
            let measure = &mut self.score.measures[model_index];
            if element.has_child("startRepeat") {
                measure.start_bar_line = BarLineType::StartRepeat;
            }
            if let Some(end_repeat) = element.child("endRepeat") {
                measure.end_bar_line = BarLineType::EndRepeat;
                measure.repeat_count = end_repeat
                    .int()
                    .and_then(|value| u8::try_from(value.clamp(0, 255)).ok())
                    .unwrap_or(0);
            }
        }

        let mut voice_index = 0u8;
        for child in &element.children {
            match child.name.as_str() {
                "voice" => {
                    self.read_voice(model_index, staff_index, voice_index, child, irregular)?;
                    // TODO(parité): plus de quatre voix par portée n'est pas
                    // représentable dans le modèle (`VOICES`).
                    voice_index = (voice_index + 1).min(VOICES as u8 - 1);
                }
                "Marker" => {
                    // `MeasureRead::readMeasure` : l'objet est ajouté à la
                    // mesure, pas à un segment.
                    let marker = repeats::read_marker(child, staff_index);
                    self.score.measures[model_index]
                        .repeat_marks
                        .push(scoretk_core::model::RepeatMark::Marker(marker));
                }
                "Jump" => {
                    let jump = repeats::read_jump(child, staff_index);
                    self.score.measures[model_index]
                        .repeat_marks
                        .push(scoretk_core::model::RepeatMark::Jump(jump));
                }
                // `<LayoutBreak><subtype>line|page</subtype>` : la mesure
                // finit un système (voir `line_break`).
                "LayoutBreak" => {
                    // Un saut de section porte sur la mesure suivante : il est
                    // traité par `has_section_break`/`section_keeps_numbering` plus
                    // haut, pas ici (sinon la mesure courante serait marquée
                    // deux fois).
                    if matches!(
                        child.child("subtype").map(Element::text),
                        Some("line" | "page")
                    ) {
                        self.score.measures[model_index].line_break = true;
                    }
                }
                // `Measure::setMeasureNumberOffset` (`measureread.cpp:121`) :
                // numérotation d'origine conservée par une partition extraite.
                "noOffset" => {
                    if let Some(offset) = child.int() {
                        self.score.measures[model_index].number_offset = offset as i32;
                    }
                }
                // TODO(parité): <stretch>, <measureRepeatCount>, <MMRestRange>,
                // <visible>.
                _ => {}
            }
        }

        if staff_index == 0 {
            let measure = &self.score.measures[model_index];
            self.last_nominal = measure.nominal_len;
            self.global_tick = measure.end_tick();
        }
        Ok(())
    }

    //------------------------------------------------------------------
    // <voice>
    //------------------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn read_voice(
        &mut self,
        measure_index: usize,
        staff_index: usize,
        voice: u8,
        element: &Element,
        irregular: bool,
    ) -> Result<()> {
        let track = Track::new(StaffId::from_index(staff_index), voice);
        let mut tick = self.score.measures[measure_index].start_tick;
        let mut tuplets: Vec<(TupletId, Fraction)> = Vec::new();
        let mut grace_chords: Vec<GraceChord> = Vec::new();
        let mut stretch = self.stretch(staff_index);
        // Premier accord d'un trémolo à deux notes : l'accord suivant de la
        // même voix fait partie du même trémolo (`Measure::connectTremolo`).
        let mut pending_tremolo: Option<u8> = None;
        // `<Beam><StemDirection>` s'applique au premier accord ou silence qui
        // suit (`MeasureRead::readVoice` : `startingBeam`).
        let mut pending_beam: Option<StemDirection> = None;

        for child in &element.children {
            match child.name.as_str() {
                // TODO(parité): <location> complet (mesures, portées, voix) ;
                // seule la fraction relative est appliquée ici, ce qui suffit
                // au corpus (aucune <location> au niveau d'une voix).
                "location" => {
                    if let Some(fractions) = child.child("fractions").and_then(Element::fraction) {
                        tick += fractions;
                    }
                }
                "tick" => {
                    if let Some(raw) = child.int() {
                        tick = self.file_tick(raw);
                    }
                }
                "Chord" => {
                    let parsed = self.read_chord(child, tick, track)?;
                    if let Some(grace) = parsed.grace {
                        let mut grace_chord = GraceChord::new(grace, parsed.duration);
                        grace_chord.chord = parsed.into_chord(Vec::new());
                        grace_chords.push(grace_chord);
                        // Un ornement ne consomme aucun tick.
                        continue;
                    }

                    let tuplet = tuplets.last().map(|(id, _)| *id);
                    let ticks = parsed.ticks;
                    let explicit_ticks = parsed.explicit_ticks;
                    let beam = parsed.beam;
                    let duration = parsed.duration;
                    let lyrics = parsed.lyrics.clone();
                    let mut chord = parsed.into_chord(std::mem::take(&mut grace_chords));
                    if let Some(count) = pending_tremolo.take() {
                        chord.tremolo_two_chord = true;
                        chord.tremolo = Some(notations::tremolo_stop(count));
                    } else {
                        pending_tremolo = chord
                            .tremolo
                            .filter(|tremolo| tremolo.kind == TremoloKind::Start)
                            .map(|tremolo| tremolo.count);
                    }

                    let mut chord_rest = ChordRest::chord(duration);
                    chord_rest.ticks = explicit_ticks;
                    chord_rest.tuplet = tuplet;
                    chord_rest.beam_mode = beam;
                    chord_rest.beam_direction = pending_beam.take();
                    chord_rest.lyrics = lyrics;
                    chord_rest.kind = ChordRestKind::Chord(chord);
                    self.insert_item(
                        measure_index,
                        SegmentType::ChordRest,
                        tick,
                        track,
                        SegmentItem::ChordRest(chord_rest),
                    );
                    tick += actual_ticks(ticks, &tuplets, stretch);
                }
                "Rest" => {
                    let parsed = self.read_rest(child, measure_index, stretch, tick, track);
                    let tuplet = tuplets.last().map(|(id, _)| *id);
                    let ticks = parsed.ticks;
                    let duration = parsed.duration;
                    let gap = parsed.gap;
                    let visible = parsed.visible;
                    let note_dots = parsed.note_dots;
                    let lyrics = parsed.lyrics;

                    let mut chord_rest = ChordRest::rest(duration);
                    chord_rest.ticks = parsed.explicit_ticks;
                    chord_rest.tuplet = tuplet;
                    chord_rest.lyrics = lyrics;
                    chord_rest.beam_direction = pending_beam.take();
                    if let Some(rest) = chord_rest.rest_mut() {
                        rest.gap = gap;
                        rest.visible = visible;
                        rest.note_dots = note_dots;
                    }
                    self.insert_item(
                        measure_index,
                        SegmentType::ChordRest,
                        tick,
                        track,
                        SegmentItem::ChordRest(chord_rest),
                    );
                    tick += actual_ticks(ticks, &tuplets, stretch);
                }
                "Beam" => {
                    pending_beam = child.child("StemDirection").map(|direction| {
                        match direction.text().trim() {
                            "up" => StemDirection::Up,
                            "down" => StemDirection::Down,
                            _ => StemDirection::Auto,
                        }
                    });
                }
                "Tuplet" => {
                    let (tuplet, explicit_bracket) = self.read_tuplet(child, tick);
                    if let Some((parent, _)) = tuplets.last() {
                        let parent = *parent;
                        self.score.tuplet_mut(tuplet).parent = Some(parent);
                        // Un n-olet imbriqué dont la durée de base tient dans une
                        // croche est groupé par les ligatures : MuseScore ne
                        // dessine alors pas de crochet (`Tuplet::calcHasBracket`).
                        // Un ratio non réduit de croches (`4:4`) reste affiché
                        // avec crochet. Un `<bracket>` explicite fait foi.
                        if !explicit_bracket {
                            let base = self.score.tuplet(tuplet).base_len;
                            let ratio = self.score.tuplet(tuplet).ratio;
                            if base < Fraction::new(1, 8)
                                || (base == Fraction::new(1, 8) && ratio != Fraction::ONE)
                            {
                                self.score.tuplet_mut(tuplet).bracket = false;
                            }
                        }
                    }
                    let ratio = self.score.tuplet(tuplet).ratio;
                    tuplets.push((tuplet, ratio));
                }
                "endTuplet" => {
                    // `<endTuplet/>` sans ouverture : ignoré, comme le C++.
                    tuplets.pop();
                }
                "TimeSig" => {
                    let mut time_sig = read_time_sig(child);
                    let courtesy = if self.version >= 450 {
                        flag(child, "isCourtesy")
                    } else {
                        tick > self.score.measures[measure_index].start_tick
                    };
                    time_sig.courtesy = courtesy;
                    let kind = if courtesy {
                        SegmentType::TimeSigAnnounce
                    } else {
                        SegmentType::TimeSig
                    };
                    // La métrique ne change la durée de la mesure que lue à son
                    // début (`currTick == measure->tick()` dans le C++).
                    if !courtesy && tick == self.score.measures[measure_index].start_tick {
                        stretch = if time_sig.stretch.is_positive() {
                            time_sig.stretch
                        } else {
                            Fraction::ONE
                        };
                        self.staves[staff_index].time_stretch = Some(stretch);
                        // L'étirement de portée sert aussi à l'export :
                        // `DurationElement::actualTicks` divise par lui.
                        self.score
                            .staff_mut(StaffId::from_index(staff_index))
                            .time_stretch = stretch;
                        if staff_index == 0 {
                            // `TimeSig::stretch` divise la durée réelle : une
                            // métrique 3/4 étirée 2/3 vaut 9/8.
                            let nominal = time_sig.measure_len() / stretch;
                            let measure = &mut self.score.measures[measure_index];
                            measure.nominal_len = nominal;
                            if !irregular {
                                measure.len = nominal;
                            }
                        }
                    }
                    self.insert_item(
                        measure_index,
                        kind,
                        tick,
                        track,
                        SegmentItem::TimeSig(time_sig),
                    );
                }
                "KeySig" => {
                    let measure_end = self.score.measures[measure_index].end_tick();
                    let mut key_sig = self.read_key_sig(child);
                    let courtesy = if self.version >= 450 {
                        flag(child, "isCourtesy")
                    } else {
                        tick == measure_end
                    };
                    key_sig.courtesy = courtesy;
                    let kind = if courtesy {
                        SegmentType::KeySigAnnounce
                    } else {
                        SegmentType::KeySig
                    };
                    self.insert_item(
                        measure_index,
                        kind,
                        tick,
                        track,
                        SegmentItem::KeySig(key_sig),
                    );
                }
                "Clef" => {
                    let clef = read_clef(child);
                    let header = if self.version >= 410 {
                        flag(child, "isHeader")
                    } else {
                        self.guess_header_clef(measure_index, tick)
                    };
                    let kind = if header {
                        SegmentType::HeaderClef
                    } else {
                        SegmentType::Clef
                    };
                    self.insert_item(measure_index, kind, tick, track, SegmentItem::Clef(clef));
                }
                "BarLine" => {
                    let bar_line = repeats::read_bar_line(child);
                    self.read_bar_line(measure_index, tick, track, bar_line);
                }
                "Breath" => {
                    // Symboles de `SymNames` (`engraving/types/symnames.cpp:798-822`).
                    let kind = crate::read::notation_names::breath_kind(
                        child.child("symbol").map(Element::text).unwrap_or_default(),
                    );
                    // `MeasureRead::readVoice` place la respiration au-dessus
                    // pour la voix 0 et au-dessous pour la voix 1, puis
                    // `TRead::read` applique `<placement>` s'il est présent.
                    let placement = match child.child("placement") {
                        Some(node) => placement(Some(node)),
                        None if track.voice() % 2 == 1 => Placement::Below,
                        None => Placement::Above,
                    };
                    let pause_ms = child
                        .child("pause")
                        .and_then(|node| node.text().parse::<f64>().ok())
                        .map(|seconds| (seconds.max(0.0) * 1000.0).round() as u32);
                    self.insert_item(
                        measure_index,
                        SegmentType::Breath,
                        tick,
                        track,
                        SegmentItem::Breath(Breath {
                            kind,
                            placement,
                            pause_ms,
                        }),
                    );
                }
                "Dynamic" => {
                    let dynamic = nuances::read_dynamic(child);
                    self.add_annotation(measure_index, tick, track, Annotation::Dynamic(dynamic));
                }
                "StaffText" => {
                    let text = read_text_base(child);
                    self.add_annotation(
                        measure_index,
                        tick,
                        track,
                        Annotation::StaffText(StaffText {
                            text: text.text,
                            placement: text.placement,
                            visible: text.visible,
                        }),
                    );
                }
                "Expression" => {
                    let text = read_text_base(child);
                    self.add_annotation(
                        measure_index,
                        tick,
                        track,
                        Annotation::Expression(StaffText {
                            text: text.text,
                            placement: text.placement,
                            visible: text.visible,
                        }),
                    );
                }
                "SystemText" => {
                    let text = read_text_base(child);
                    self.add_annotation(
                        measure_index,
                        tick,
                        track,
                        Annotation::SystemText(SystemText {
                            text: text.text,
                            placement: text.placement,
                            visible: text.visible,
                            swing: textlines::read_swing(child),
                        }),
                    );
                }
                "Tempo" => {
                    let text = read_text_base(child);
                    // `TempoText::TempoText` initialise le tempo à 2 battements
                    // par seconde (120 BPM) ; `<tempo>` l'écrase.
                    let mut tempo = Tempo {
                        text: text.text,
                        beats_per_second: Some(
                            child
                                .child("tempo")
                                .map(Element::text)
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(2.0),
                        ),
                        follow_text: child.has_child("followText"),
                        placement: text.placement,
                        visible: text.visible,
                        kind: textlines::read_tempo_kind(child),
                    };
                    // `TRead::read(TempoText*)` : un tempo sans texte reçoit le
                    // texte « ♩ = N » et devient invisible.
                    if tempo.text.is_empty() {
                        if let Some(beats_per_second) = tempo.beats_per_second {
                            let bpm = (beats_per_second * 60.0).round();
                            tempo.text =
                                RichText::new(format!("<sym>metNoteQuarterUp</sym> = {bpm:.0}"));
                        }
                        tempo.visible = false;
                    }
                    self.add_annotation(measure_index, tick, track, Annotation::Tempo(tempo));
                }
                "RehearsalMark" => {
                    let text = read_text_base(child);
                    let frame = match child.child("frameType").and_then(Element::int) {
                        Some(0) => RehearsalFrame::NoFrame,
                        Some(2) => RehearsalFrame::Circle,
                        // Le style MuseScore 4.7 encadre les repères : rectangle.
                        _ => RehearsalFrame::Rectangle,
                    };
                    self.add_annotation(
                        measure_index,
                        tick,
                        track,
                        Annotation::RehearsalMark(RehearsalMark {
                            text: text.text,
                            placement: text.placement,
                            frame,
                            visible: text.visible,
                        }),
                    );
                }
                "InstrumentChange" => {
                    let text = read_text_base(child);
                    let instrument = child.child("Instrument").map(|instrument| {
                        let mut instrument = self.read_instrument(instrument);
                        if instrument.id.is_empty() {
                            if let Some(id) = scoretk_core::model::recognize_musicxml_id(
                                &instrument.track_name,
                                instrument.midi_program,
                            ) {
                                instrument.id = id.to_owned();
                            }
                        }
                        instrument
                    });
                    // `Segment::add` enregistre l'instrument d'un changement au
                    // tick du segment (`part->setInstrument`) : la liste
                    // `Part::instruments()` de l'export en dépend (identifiants
                    // `P<n>-I<r>`, canaux MIDI).
                    if let Some(instrument) = &instrument {
                        let part_id = self.score.staves[staff_index].part;
                        self.score
                            .part_mut(part_id)
                            .set_instrument_at(tick, instrument.clone());
                    }
                    self.add_annotation(
                        measure_index,
                        tick,
                        track,
                        Annotation::InstrumentChange(InstrumentChange {
                            text: text.text,
                            placement: text.placement,
                            visible: text.visible,
                            instrument,
                        }),
                    );
                }
                "Harmony" => {
                    let harmony = self.read_harmony_annotation(child);
                    self.add_annotation(measure_index, tick, track, Annotation::Harmony(harmony));
                }
                // `readMeasure206` (`read500/measureread.cpp:448-450`) crée un
                // `FretDiagram` et lit son `<Harmony>` : c'est la seule source
                // de l'accord dans les partitions de guitare (23 accords de
                // `bransle-de-chevaux` ignorés jusqu'ici).
                "FretDiagram" => {
                    if let Some(node) = child.child("Harmony") {
                        let mut harmony = self.read_harmony_annotation(node);
                        harmony.fret_diagram = Some(fret::read_fret_diagram(child));
                        self.add_annotation(
                            measure_index,
                            tick,
                            track,
                            Annotation::Harmony(harmony),
                        );
                    }
                }
                "Fermata" => {
                    // `MeasureRead::readVoice` fixe le placement de la piste
                    // avant `TRead::read` (`rw/read410/measureread.cpp:535-538`) :
                    // voix impaire au-dessous, comme `Fermata::propertyDefault`.
                    let default_placement = if track.voice() % 2 == 1 {
                        Placement::Below
                    } else {
                        Placement::Above
                    };
                    let annotation = notations::read_fermata(child, default_placement);
                    self.add_annotation(measure_index, tick, track, annotation);
                }
                "FiguredBass" => {
                    let bass = figures::read_figured_bass(child);
                    self.add_annotation(measure_index, tick, track, Annotation::FiguredBass(bass));
                }
                "TremoloBar" => {
                    let bar = figures::read_tremolo_bar(child);
                    self.add_annotation(measure_index, tick, track, Annotation::TremoloBar(bar));
                }
                "MeasureRepeat" | "RepeatMeasure" => {
                    // `TRead::read(MeasureRepeat*)` : `<subtype>` porte le
                    // nombre de mesures répétées.
                    let count = child
                        .child("subtype")
                        .and_then(Element::int)
                        .and_then(|value| u16::try_from(value.clamp(1, i64::from(u16::MAX))).ok())
                        .unwrap_or(1);
                    self.measure_repeats
                        .push(crate::read::measure_repeat::MeasureRepeat {
                            measure: measure_index,
                            staff: staff_index,
                            count,
                        });
                    tick += self.score.measures[measure_index].len;
                }
                // TODO(parité): <Spanner> autres que Tie (Slur, HairPin, Ottava,
                // TextLine, Trill, Pedal, LetRing…) : à modéliser dans
                // `Score::spanners`. Les liaisons de prolongation sont
                // traitées dans <Note>.
                "Spanner" => {
                    repeats::read_spanner(self, child, tick, track, measure_index);
                    notations::read_spanner(self, child, tick, track);
                    textlines::read_spanner(self, child, tick, track, measure_index);
                    if let Some(half) = nuances::read_half(child, measure_index, tick, track) {
                        self.spanner_halves.push(half);
                    }
                    if let Some(half) = ottava::read_half(child, measure_index, tick, track) {
                        self.ottava_halves.push(half);
                    }
                    if let Some(half) = pedals::read_half(child, measure_index, tick, track) {
                        self.pedal_halves.push(half);
                    }
                }
                // TODO(parité): <Beam>, <Ambitus>, <Segment>, <MeasureRepeat>,
                // <Symbol>, <Image>, <StaffState>, <PlayTechAnnotation>,
                // <Sticking>, <HarpPedalDiagram>.
                _ => {}
            }
        }

        // TODO(parité): ornements en fin de voix sans accord principal : le C++
        // les laisse non rattachés (fichier corrompu) ; ils sont ignorés ici.
        Ok(())
    }

    //------------------------------------------------------------------
    // <Chord>, <Rest>, <Note>
    //------------------------------------------------------------------

    fn read_chord(&mut self, element: &Element, tick: Tick, track: Track) -> Result<ParsedChord> {
        let mut duration_type = DurationType::Quarter;
        let mut dots = 0u8;
        let mut explicit = None;
        let mut grace = None;
        let mut beam = BeamMode::Auto;
        let mut stem = StemDirection::Auto;
        let mut staff_move = 0i8;
        let mut small = false;
        let mut articulations = Vec::new();
        let mut ornaments = Vec::new();
        let mut tremolo = None;
        let mut arpeggio = None;
        let mut chord_line = None;
        let mut lyrics = Vec::new();
        let mut notes = Vec::new();
        let mut tremolo_two_chord = false;

        for child in &element.children {
            match child.name.as_str() {
                "durationType" => {
                    duration_type =
                        names::duration_type(child.text()).unwrap_or(DurationType::Quarter);
                }
                "dots" => {
                    dots = child
                        .int()
                        .and_then(|value| u8::try_from(value.clamp(0, 4)).ok())
                        .unwrap_or(0);
                }
                "duration" => explicit = child.fraction(),
                "BeamMode" => {
                    beam = names::beam_mode(child.text(), self.version).unwrap_or(BeamMode::Auto);
                }
                "StemDirection" => {
                    stem = match child.text() {
                        "up" => StemDirection::Up,
                        "down" => StemDirection::Down,
                        _ => StemDirection::Auto,
                    };
                }
                "staffMove" => {
                    staff_move = child
                        .int()
                        .unwrap_or(0)
                        .clamp(-4, 4)
                        .try_into()
                        .unwrap_or(0);
                }
                "small" => small = child.int().unwrap_or(0) != 0,
                "Note" => notes.push(self.read_note(child, tick, track, &mut chord_line)?),
                // Trémolo à deux notes : la sous-famille `c8`…`c256` (ou
                // l'élément 5.0 `<TremoloTwoChord>`) réduit la durée réelle et
                // ajoute le rapport 2:1 de `<time-modification>`.
                "Tremolo" => {
                    let subtype = child.child("subtype").map(Element::text).unwrap_or("");
                    if subtype.starts_with('c') {
                        tremolo_two_chord = true;
                    }
                    tremolo = notations::read_tremolo(child);
                }
                "TremoloSingleChord" => tremolo = notations::read_tremolo(child),
                "TremoloTwoChord" => {
                    tremolo_two_chord = true;
                    tremolo = notations::read_tremolo_two_chord(child);
                }
                "Articulation" => {
                    for articulation in notations::read_articulations(child) {
                        match crate::read::notation_names::ornament_from_articulation(&articulation)
                        {
                            Some(ornament) => ornaments.push(ornament),
                            None => articulations.push(articulation),
                        }
                    }
                }
                "Ornament" => {
                    // L'accord embarqué dans l'ornement n'est pas un accord de
                    // segment : il ne compte pas dans la durée.
                    if let Some(ornament) = notations::read_ornament(child) {
                        ornaments.push(ornament);
                    }
                }
                "Arpeggio" => arpeggio = Some(notations::read_arpeggio(child)),
                "ChordLine" => chord_line = notations::read_chord_line(child),
                "Lyrics" => lyrics.push(read_lyrics(child, self.score.style.lyrics_placement)),
                "Spanner" => notations::read_spanner(self, child, tick, track),
                // TODO(parité): <Stem>, <Hook>, <noStem>.
                _ => {}
            }
            if let Some(kind) = names::grace_type(child.name.as_str()) {
                grace = Some(kind);
            }
        }

        let duration = Duration::with_dots(duration_type, dots);
        let mut ticks = duration.ticks();
        if let Some(value) = explicit {
            ticks = value;
        }

        Ok(ParsedChord {
            duration,
            ticks,
            explicit_ticks: explicit,
            notes,
            grace,
            beam,
            stem,
            staff_move,
            small,
            tremolo_two_chord,
            articulations,
            ornaments,
            tremolo,
            arpeggio,
            chord_line,
            lyrics,
        })
    }

    fn read_rest(
        &mut self,
        element: &Element,
        measure_index: usize,
        stretch: Fraction,
        tick: Tick,
        track: Track,
    ) -> ParsedRest {
        let mut duration_type = DurationType::Measure;
        let mut dots = 0u8;
        let mut explicit = None;
        let mut gap = false;
        let mut visible = true;
        let mut lyrics = Vec::new();
        // Les points explicites du fichier survivent à une portée masquée
        // (`Rest::checkDots` n'est appelé que par le layout).
        let note_dots: Vec<bool> = element
            .children_named("NoteDot")
            .map(|dot| dot.child("visible").and_then(Element::int).unwrap_or(1) != 0)
            .collect();

        for child in &element.children {
            match child.name.as_str() {
                "durationType" => {
                    duration_type =
                        names::duration_type(child.text()).unwrap_or(DurationType::Measure);
                }
                "dots" => {
                    dots = child
                        .int()
                        .and_then(|value| u8::try_from(value.clamp(0, 4)).ok())
                        .unwrap_or(0);
                }
                "duration" => explicit = child.fraction(),
                "gap" => gap = child.int().unwrap_or(0) != 0,
                "visible" => visible = child.int().unwrap_or(1) != 0,
                "Lyrics" => lyrics.push(read_lyrics(child, self.score.style.lyrics_placement)),
                "Spanner" => notations::read_spanner(self, child, tick, track),
                _ => {}
            }
        }

        // `readVoice` pré-remplit les silences avec la durée de la mesure
        // divisée par l'étirement ; `<duration>` l'écrase ensuite.
        let mut ticks = if duration_type == DurationType::Measure {
            let nominal = self.score.measures[measure_index].nominal_len;
            if stretch.is_positive() {
                nominal / stretch
            } else {
                nominal
            }
        } else {
            Duration::with_dots(duration_type, dots).ticks()
        };
        if let Some(value) = explicit {
            ticks = value;
        }

        ParsedRest {
            duration: Duration::with_dots(duration_type, dots),
            ticks,
            explicit_ticks: explicit,
            gap,
            visible,
            note_dots,
            lyrics,
        }
    }

    fn read_note(
        &mut self,
        element: &Element,
        tick: Tick,
        track: Track,
        chord_line: &mut Option<scoretk_core::model::ChordLineType>,
    ) -> Result<NoteId> {
        let mut note = Note::new(0, tpc::INVALID);

        for child in &element.children {
            match child.name.as_str() {
                "pitch" => {
                    note.pitch = child
                        .int()
                        .map(|value| value.clamp(0, 127) as u8)
                        .unwrap_or(0);
                }
                "tpc" => {
                    if let Some(value) = child.int().and_then(|value| i8::try_from(value).ok()) {
                        note.tpc = value;
                        note.tpc_transposed = value;
                    }
                }
                "small" => note.small = child.int().unwrap_or(0) != 0,
                "tpc2" => {
                    if let Some(value) = child.int().and_then(|value| i8::try_from(value).ok()) {
                        note.tpc_transposed = value;
                    }
                }
                "Accidental" => {
                    if let Some(subtype) = child.child("subtype").map(Element::text) {
                        if let Some(kind) = names::accidental_type(subtype) {
                            let mut accidental = Accidental::new(kind);
                            accidental.bracket = child
                                .child("bracket")
                                .and_then(Element::int)
                                .is_some_and(|value| value != 0);
                            // `AccidentalRole::USER` vaut 1 ; le rôle par défaut
                            // est AUTO. Les fichiers 3.x écrivent `parens`, les
                            // 4.x `parentheses`.
                            accidental.user = child
                                .child("role")
                                .and_then(Element::int)
                                .is_some_and(|value| value == 1);
                            accidental.small = child
                                .child("small")
                                .and_then(Element::int)
                                .is_some_and(|value| value != 0);
                            accidental.parentheses = child
                                .child("parens")
                                .or_else(|| child.child("parentheses"))
                                .and_then(Element::int)
                                .is_some_and(|value| value != 0);
                            accidental.visible = !child
                                .child("visible")
                                .and_then(Element::int)
                                .is_some_and(|value| value == 0);
                            note.accidental = Some(accidental);
                        }
                        // TODO(parité): altérations microtonales.
                    }
                }
                "head" => {
                    if let Some(head) = names::note_head(child.text()) {
                        note.head_group = head;
                    }
                }
                "headType" => {
                    if let Some(head) = names::head_type(child.text()) {
                        note.head = head;
                    }
                }
                "visible" => note.visible = child.int().unwrap_or(1) != 0,
                "Symbol" => {
                    if let Some(name) = child.child("name") {
                        if name.text().starts_with("noteheadParenthesis") {
                            note.parentheses = true;
                        }
                    }
                }
                "ghost" | "dead" => note.ghost = child.int().unwrap_or(0) != 0,
                "play" => note.play = child.int().unwrap_or(1) != 0,
                "velocity" => {
                    // `rw/read410/tread.cpp:3174-3175` (identique en 4.60 et
                    // 5.00) : `n->setUserVelocity(e.readInt())`.
                    // `Pid::USER_VELOCITY` est un INT signé (`dom/property.cpp:87`) :
                    // en mode offset, une valeur négative réduit la vélocité.
                    note.user_velocity = child.int().and_then(|value| i32::try_from(value).ok());
                }
                "veloType" => {
                    // `rw/read410/tread.cpp:3190-3191` : `TRead::readProperty(n,
                    // e, ctx, Pid::VELO_TYPE)`. `TConv::fromXml`
                    // (`typesconv.cpp:2499-2500`) lit `offset`/`user` et retombe
                    // sur `OFFSET_VAL` pour une valeur inconnue ; la valeur par
                    // défaut de la propriété est `USER_VAL`
                    // (`dom/note.cpp:3316-3317`).
                    note.velo_type = Some(match child.text() {
                        "user" => VeloType::UserVal,
                        _ => VeloType::OffsetVal,
                    });
                }
                "Spanner" if child.attr("type") == Some("Tie") => {
                    self.register_tie(&note, child, tick, track);
                }
                "Spanner" => notations::read_spanner(self, child, tick, track),
                "ChordLine" => {
                    if let Some(kind) = notations::read_chord_line(child) {
                        *chord_line = Some(kind);
                    }
                }
                "Fingering" => {
                    if let Some(fingering) = notations::read_fingering(child) {
                        note.fingerings.push(fingering);
                    }
                }
                // TODO(parité): <NoteLine> et autres spanners ancrés sur note.
                _ => {}
            }
        }

        if note.tpc == tpc::INVALID {
            note.tpc = names::natural_tpc(note.pitch);
            note.tpc_transposed = note.tpc;
        }

        Ok(self.score.add_note(note))
    }

    /// Enregistre une moitié de liaison de prolongation (`<Spanner type="Tie">`).
    fn register_tie(&mut self, note: &Note, element: &Element, tick: Tick, track: Track) {
        // La note n'est ajoutée à l'arène qu'après ses enfants : son indice est
        // donc la taille actuelle de l'arène.
        let note_id = NoteId::from_index(self.score.notes.len());
        let up = tie_direction(element);
        // La visibilité appartient au `Tie` : elle n'est écrite que sur la
        // moitié de départ.
        let visible = element
            .child("Tie")
            .and_then(|tie| tie.child("visible"))
            .and_then(Element::int)
            .is_none_or(|value| value != 0);
        for child in &element.children {
            let is_start = match child.name.as_str() {
                "next" => true,
                "prev" => false,
                _ => continue,
            };
            let other = child
                .child("location")
                .map(read_tie_location)
                .unwrap_or(TieLocation {
                    measures: 0,
                    fractions: Fraction::ZERO,
                    staves: 0,
                    voices: 0,
                });
            self.ties.push(TieRecord {
                note: note_id,
                track,
                tick,
                pitch: note.pitch,
                is_start,
                other,
                up,
                visible,
            });
        }
    }

    /// Apparie les deux moitiés des liaisons de prolongation.
    ///
    /// `ConnectorInfoReader::readAddConnector(Note*)` apparie par type, piste et
    /// position ; deux notes de même hauteur au même tick sur la même piste sont
    /// les deux extrémités d'une même liaison.
    fn resolve_ties(&mut self) {
        let mut used = vec![false; self.ties.len()];
        let measures = self.score.measures.clone();
        for index in 0..self.ties.len() {
            if !self.ties[index].is_start || used[index] {
                continue;
            }
            let start = self.ties[index].clone();
            let start_other_tick = start.other.absolute_tick(start.tick, &measures);
            let start_other_track = start.other.absolute_track(start.track);
            let partner = (0..self.ties.len()).find(|&other| {
                let candidate = &self.ties[other];
                !used[other]
                    && !candidate.is_start
                    && candidate.track == start_other_track
                    && candidate.pitch == start.pitch
                    && candidate.tick == start_other_tick
                    && candidate.other.absolute_track(candidate.track) == start.track
                    && candidate.other.absolute_tick(candidate.tick, &measures) == start.tick
            });
            if let Some(partner) = partner {
                used[index] = true;
                used[partner] = true;
                let end = self.ties[partner].clone();
                let visible = start.visible;
                let start_note = &mut self.score.notes[start.note.index()];
                start_note.tie_forward = Some(end.note);
                start_note.tie_forward_visible = visible;
                start_note.tie_up = start.up;
                let end_note = &mut self.score.notes[end.note.index()];
                end_note.tie_back = Some(start.note);
                end_note.tie_back_visible = visible;
            } else {
                // TODO(parité): demi-liaison orpheline (fichier corrompu) : le
                // C++ la journalise et la laisse non appariée.
            }
        }
    }

    //------------------------------------------------------------------
    // <Tuplet>
    //------------------------------------------------------------------

    fn read_tuplet(&mut self, element: &Element, tick: Tick) -> (TupletId, bool) {
        let normal = element
            .child("normalNotes")
            .and_then(Element::int)
            .unwrap_or(2);
        let actual = element
            .child("actualNotes")
            .and_then(Element::int)
            .unwrap_or(3);
        let base_type = element
            .child("baseNote")
            .map(Element::text)
            .and_then(names::duration_type)
            .unwrap_or(DurationType::Eighth);
        let base_dots = element
            .child("baseDots")
            .and_then(Element::int)
            .and_then(|value| u8::try_from(value.clamp(0, 4)).ok())
            .unwrap_or(0);

        let ratio = Fraction::try_new(actual.max(1), normal.max(1)).unwrap_or(Fraction::ONE);
        let base_len = Duration::with_dots(base_type, base_dots).ticks();
        // `TRead::read(Tuplet*)` : ticks = baseLen * ratio.denominator(), avec
        // le ratio **non réduit** (`normalNotes`).
        let len = base_len * Fraction::from_integer(normal.max(1));

        let mut tuplet = Tuplet::new(ratio, base_len, tick, len);
        tuplet.placement =
            element
                .child("direction")
                .and_then(|direction| match direction.text() {
                    "up" => Some(scoretk_core::model::Placement::Above),
                    "down" => Some(scoretk_core::model::Placement::Below),
                    _ => None,
                });
        // MuseScore affiche les nombres bruts du fichier (`4:4` reste `4:4`
        // dans `<tuplet-actual>`, même si le ratio interne est réduit).
        tuplet.actual_notes = actual.max(1);
        tuplet.normal_notes = normal.max(1);
        // `TRead::readStyledProperty` (read460/tread.cpp:1530) relit
        // `Pid::NUMBER_TYPE` (`numberType`) en priorité ; les fichiers plus
        // anciens n'ont que `<Number><text>`, conservé en repli.
        match element.child("numberType").and_then(Element::int) {
            Some(1) => tuplet.number_type = TupletNumberType::Ratio,
            Some(2) => tuplet.number_type = TupletNumberType::None,
            Some(0 | 3) | None => {
                if let Some(number) = element.child("Number") {
                    let text = number.child("text").map(Element::text).unwrap_or_default();
                    tuplet.number_type = if text.is_empty() {
                        TupletNumberType::None
                    } else if text.contains(':') {
                        TupletNumberType::Ratio
                    } else {
                        TupletNumberType::Number
                    };
                }
            }
            Some(_) => {}
        }
        // `Tuplet::calcHasBracket` n'est qu'une mise en page ; faute de ligatures
        // modélisées, on applique la règle du corpus : un n-olet de croches ou
        // plus court, imbriqué, se passe de crochet. Un `<bracket>` explicite
        // (écrit par MuseScore 3, et par l'écrivain de scoretk) fait foi.
        // `TRead::readStyledProperty` relit `Pid::BRACKET_TYPE`
        // (`SHOW_BRACKET = 1`, `SHOW_NO_BRACKET = 2`, `types.h:1604-1607`) ;
        // `<bracket>` reste le repli des fichiers 3.x.
        let bracket_type = element.child("bracketType").and_then(Element::int);
        let explicit_bracket = element.child("bracket").is_some() || bracket_type.is_some();
        tuplet.bracket_explicit = explicit_bracket;
        tuplet.bracket = match bracket_type {
            Some(1) => true,
            Some(2) => false,
            _ => element
                .child("bracket")
                .and_then(Element::int)
                .is_none_or(|value| value != 0),
        };
        let id = self.score.add_tuplet(tuplet);
        (id, explicit_bracket)
    }

    //------------------------------------------------------------------
    // <KeySig>
    //------------------------------------------------------------------

    fn read_key_sig(&self, element: &Element) -> KeySig {
        let mut concert = 0i8;
        let mut written = None;

        if self.version < 460 {
            if let Some(accidental) = element.child("accidental").and_then(Element::int) {
                // Pas de borne : MuseScore autorise les armures personnalisées.
                concert = i8::try_from(accidental).unwrap_or(0);
            }
        }
        if self.version >= 410 {
            if let Some(value) = element.child("concertKey").and_then(Element::int) {
                concert = i8::try_from(value).unwrap_or(0);
            }
            if let Some(value) = element.child("actualKey").and_then(Element::int) {
                written = Some(i8::try_from(value).unwrap_or(0));
            }
        }

        let mut key_sig = KeySig::new(Key::new(concert));
        key_sig.visible = element
            .child("visible")
            .and_then(Element::int)
            .is_none_or(|value| value != 0);
        key_sig.transposed_key = Key::new(written.unwrap_or(concert));
        key_sig.mode = element
            .child("mode")
            .map(Element::text)
            .and_then(names::key_mode)
            .unwrap_or(KeyMode::Unknown);
        // TODO(parité): <subtype> (ancien encodage), armures personnalisées
        // (<CustDef>/<KeySym>), <showNaturals>, <forInstrumentChange>.
        key_sig
    }

    /// Devine une clé d'en-tête avant 4.1 (`MeasureRead::readVoice`).
    fn guess_header_clef(&self, measure_index: usize, tick: Tick) -> bool {
        let measure = &self.score.measures[measure_index];
        if tick != measure.start_tick {
            return false;
        }
        !measure.segments.iter().any(|segment| {
            segment.tick == measure.start_tick
                && (segment.kind.is_key_sig_type() || segment.kind.is_time_sig_type())
        })
    }

    fn read_bar_line(&mut self, measure_index: usize, tick: Tick, track: Track, bar_line: BarLine) {
        let kind = bar_line.kind;
        let (measure_start, measure_len) = {
            let measure = &self.score.measures[measure_index];
            (measure.start_tick, measure.len)
        };
        let offset = tick - measure_start;
        let segment_kind = if !offset.is_zero() && offset != measure_len {
            SegmentType::BarLine
        } else if kind == BarLineType::StartRepeat && offset.is_zero() {
            SegmentType::StartRepeatBarLine
        } else if kind == BarLineType::StartRepeat && offset == measure_len {
            // Ancienne version : barre de début écrite à la fin, ignorée.
            return;
        } else if offset.is_zero() && !self.has_segment_at(measure_index, measure_start) {
            SegmentType::BeginBarLine
        } else {
            SegmentType::EndBarLine
        };

        self.insert_item(
            measure_index,
            segment_kind,
            tick,
            track,
            SegmentItem::BarLine(bar_line),
        );
        if segment_kind == SegmentType::EndBarLine {
            let measure = &mut self.score.measures[measure_index];
            measure.end_bar_line = kind;
        }
    }

    fn has_segment_at(&self, measure_index: usize, tick: Tick) -> bool {
        self.score.measures[measure_index]
            .segments
            .iter()
            .any(|segment| segment.tick == tick)
    }

    //------------------------------------------------------------------
    // Insertion dans les segments
    //------------------------------------------------------------------

    fn segment_index(&mut self, measure_index: usize, kind: SegmentType, tick: Tick) -> usize {
        let staff_count = self.score.staves.len();
        let measure = &mut self.score.measures[measure_index];
        if let Some(index) = measure
            .segments
            .iter()
            .position(|segment| segment.kind == kind && segment.tick == tick)
        {
            return index;
        }
        measure.add_segment(Segment::new(kind, tick, staff_count));
        measure
            .segments
            .iter()
            .position(|segment| segment.kind == kind && segment.tick == tick)
            .expect("segment inséré")
    }

    fn insert_item(
        &mut self,
        measure_index: usize,
        kind: SegmentType,
        tick: Tick,
        track: Track,
        item: SegmentItem,
    ) {
        let index = self.segment_index(measure_index, kind, tick);
        self.score.measures[measure_index].segments[index].set_element(track, item);
    }

    fn add_annotation(
        &mut self,
        measure_index: usize,
        tick: Tick,
        track: Track,
        annotation: Annotation,
    ) {
        let index = self.segment_index(measure_index, SegmentType::ChordRest, tick);
        self.score.measures[measure_index].segments[index].add_annotation(track, annotation);
    }

    /// Convertit un `<tick>` absolu (ticks de fichier) en fraction de ronde.
    fn file_tick(&self, raw: i64) -> Tick {
        let division = i64::from(self.file_division.max(1));
        let internal = (raw * 480 + division / 2) / division;
        Fraction::try_from_ticks(internal, 1920).unwrap_or(Fraction::ZERO)
    }
}

//----------------------------------------------------------------------
// Valeurs intermédiaires
//----------------------------------------------------------------------

/// Accord en cours de lecture, avant insertion dans un segment.
struct ParsedChord {
    duration: Duration,
    ticks: Fraction,
    /// `<duration>` explicite, s'il diffère de la figure.
    explicit_ticks: Option<Fraction>,
    notes: Vec<NoteId>,
    grace: Option<GraceType>,
    beam: BeamMode,
    stem: StemDirection,
    staff_move: i8,
    small: bool,
    tremolo_two_chord: bool,
    articulations: Vec<Articulation>,
    ornaments: Vec<scoretk_core::model::Ornament>,
    tremolo: Option<scoretk_core::model::Tremolo>,
    arpeggio: Option<scoretk_core::model::Arpeggio>,
    chord_line: Option<scoretk_core::model::ChordLineType>,
    lyrics: Vec<Lyrics>,
}

impl ParsedChord {
    fn into_chord(self, grace_chords: Vec<GraceChord>) -> Chord {
        Chord {
            notes: self.notes,
            grace_chords,
            stem: self.stem,
            staff_move: self.staff_move,
            grace: self.grace,
            grace_slash: false,
            articulations: self.articulations,
            small: self.small,
            tremolo_two_chord: self.tremolo_two_chord,
            ornaments: self.ornaments,
            tremolo: self.tremolo,
            arpeggio: self.arpeggio,
            chord_line: self.chord_line,
        }
    }
}

/// Silence en cours de lecture.
struct ParsedRest {
    duration: Duration,
    ticks: Fraction,
    /// `<duration>` explicite, s'il diffère de la figure.
    explicit_ticks: Option<Fraction>,
    gap: bool,
    visible: bool,
    /// Points explicites `<NoteDot>` et leur visibilité.
    note_dots: Vec<bool>,
    lyrics: Vec<Lyrics>,
}

/// Durée réelle d'un accord ou silence, une fois divisée par les n-olets
/// ouverts et par l'étirement de la portée (`DurationElement::actualTicks`).
fn actual_ticks(base: Fraction, tuplets: &[(TupletId, Fraction)], stretch: Fraction) -> Fraction {
    let mut ticks = base;
    for (_, ratio) in tuplets {
        if ratio.is_positive() {
            ticks /= *ratio;
        }
    }
    if stretch.is_positive() {
        ticks /= stretch;
    }
    ticks
}

/// Lecture d'une indication de mesure (indépendante de la version).
fn read_time_sig(element: &Element) -> TimeSig {
    let numerator = element.child("sigN").and_then(Element::int).unwrap_or(4) as i32;
    let denominator = element.child("sigD").and_then(Element::int).unwrap_or(4) as i32;
    let kind = element
        .child("subtype")
        .and_then(Element::int)
        .map_or(TimeSigType::Normal, names::time_sig_type);
    let stretch = match (
        element.child("stretchN").and_then(Element::int),
        element.child("stretchD").and_then(Element::int),
    ) {
        (Some(numerator), Some(denominator)) if numerator > 0 && denominator > 0 => {
            Fraction::try_new(numerator, denominator).unwrap_or(Fraction::ONE)
        }
        _ => Fraction::ONE,
    };
    // `<Groups>` est lu par `TRead::read(TimeSig*)` et influe sur les
    // ruptures de ligature (`Groups::beamMode`, `groups.cpp:266-307`).
    let groups: Vec<(i32, u16)> = element
        .child("Groups")
        .into_iter()
        .flat_map(|groups| groups.children_named("Node"))
        .filter_map(|node| {
            Some((
                i32::try_from(node.attr_i64("pos")?).ok()?,
                u16::try_from(node.attr_i64("action")?).ok()?,
            ))
        })
        .collect();
    // TODO(parité): <textN>/<textD>, <showCourtesySig>.
    TimeSig {
        numerator,
        denominator,
        kind,
        stretch,
        courtesy: false,
        visible: element
            .child("visible")
            .and_then(Element::int)
            .is_none_or(|value| value != 0),
        groups: (!groups.is_empty()).then_some(groups),
    }
}

/// Vrai si la portée déclare elle-même sa clef par défaut.
fn has_default_clef(element: &Element) -> bool {
    element.children.iter().any(|child| {
        matches!(
            child.name.as_str(),
            "defaultClef" | "defaultConcertClef" | "defaultTransposingClef"
        )
    })
}

/// Lit un degré de chiffrage (`<degree>` d'un `<Harmony>`).
///
/// Reproduit les bornes de `TRead::read(Harmony*)` : un degré hors de `1..=13`,
/// une altération hors de `-2..=2` ou un type inconnu est ignoré.
fn read_harmony_degree(element: &Element) -> Option<HarmonyDegree> {
    let value = element
        .child("degree-value")
        .and_then(Element::int)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| (1..=13).contains(value))?;
    let alter = element
        .child("degree-alter")
        .and_then(Element::int)
        .and_then(|value| i8::try_from(value).ok())
        .filter(|value| (-2..=2).contains(value))
        .unwrap_or(0);
    let kind = match element.child("degree-type").map(Element::text) {
        Some("add") => HarmonyDegreeKind::Add,
        Some("alter") => HarmonyDegreeKind::Alter,
        Some("subtract") => HarmonyDegreeKind::Subtract,
        _ => return None,
    };
    Some(HarmonyDegree { value, alter, kind })
}

/// Moitié de `ClefTypeList` modifiée par une balise de clef d'instrument.
#[derive(Clone, Copy)]
enum ClefChange {
    Both,
    Concert,
    Transposed,
}

/// Clefs par portée d'un `<Instrument>` (`<clef>`, `<concertClef>`,
/// `<transposingClef>`, attribut `staff` à partir de 1).
///
/// `Instrument::setClefType` crée les entrées manquantes avec la paire sol/sol ;
/// `<concertClef>` et `<transposingClef>` ne changent qu'une moitié.
fn read_instrument_clefs(element: &Element) -> Vec<Clef> {
    let mut clefs: Vec<Clef> = Vec::new();
    for child in &element.children {
        let change = match child.name.as_str() {
            "clef" => ClefChange::Both,
            "concertClef" => ClefChange::Concert,
            "transposingClef" => ClefChange::Transposed,
            _ => continue,
        };
        let index = child
            .attr_i64("staff")
            .filter(|value| *value >= 1)
            .map_or(0, |value| (value - 1) as usize);
        let Some(kind) = names::clef_type(child.text()) else {
            continue;
        };
        while clefs.len() <= index {
            clefs.push(Clef::new(ClefType::G));
        }
        match change {
            ClefChange::Both => clefs[index] = Clef::new(kind),
            ClefChange::Concert => clefs[index].concert = kind,
            ClefChange::Transposed => clefs[index].transposed = kind,
        }
    }
    clefs
}

/// Clef par défaut d'une portée (`TRead::readProperties(Staff*)`).
///
/// `<defaultClef>` fixe les deux clefs, `<defaultConcertClef>` et
/// `<defaultTransposingClef>` une seule ; sans élément, la paire vaut
/// sol/sol comme `ClefTypeList`.
fn read_default_clef(element: &Element) -> Clef {
    let mut clef = Clef::new(ClefType::G);
    for child in &element.children {
        let value = child.text();
        match child.name.as_str() {
            "defaultClef" => {
                let kind = names::clef_type(value).unwrap_or(ClefType::G);
                clef = Clef::new(kind);
            }
            "defaultConcertClef" => {
                clef.concert = names::clef_type(value).unwrap_or(ClefType::G);
            }
            "defaultTransposingClef" => {
                clef.transposed = names::clef_type(value).unwrap_or(ClefType::G);
            }
            _ => {}
        }
    }
    clef
}

/// Lecture d'une clef.
fn read_clef(element: &Element) -> Clef {
    let concert = element
        .child("concertClefType")
        .map(Element::text)
        .and_then(names::clef_type);
    let transposed = element
        .child("transposingClefType")
        .map(Element::text)
        .and_then(names::clef_type);
    let concert = concert.unwrap_or(ClefType::G);
    Clef::pair(concert, transposed.unwrap_or(concert))
}

/// Lecture des `<Lyrics>` d'un accord.
///
/// `default_placement` est `lyricsPlacement` (`Lyrics::propertyDefault` pour
/// `Pid::PLACEMENT`, « below » par défaut) : les fichiers n'écrivent
/// `<placement>` que si la valeur diffère du style.
fn read_lyrics(element: &Element, default_placement: Placement) -> Lyrics {
    let verse = element
        .child("no")
        .and_then(Element::int)
        .and_then(|value| usize::try_from(value.max(0)).ok())
        .unwrap_or(0);
    let syllabic = element
        .child("syllabic")
        .map(Element::text)
        .map_or(LyricsSyllabic::Single, names::syllabic);
    // `TRead::read(Lyrics*)` lit `<text>` avec `readXml()` : le texte peut être
    // dans un enfant (`<text><i>…</i></text>`, 129 paroles de Veni Creator).
    // `Element::text` ne garderait que le texte direct et perdrait la syllabe.
    let text = element
        .child("text")
        .map(|node| node.plain_text())
        .unwrap_or_default();
    let mut lyrics = Lyrics::new(verse, syllabic, text);
    lyrics.melisma = element
        .child("ticks_f")
        .or_else(|| element.child("ticks"))
        .and_then(Element::fraction);
    lyrics.placement = element
        .child("placement")
        .map_or(default_placement, |node| placement(Some(node)));
    lyrics
}

/// Placement au-dessus ou au-dessous de la portée.
fn placement(element: Option<&Element>) -> Placement {
    match element.map(Element::text) {
        Some("below") => Placement::Below,
        _ => Placement::Above,
    }
}

/// Propriétés communes des textes MuseScore (`TextBase`).
struct ParsedTextBase {
    text: RichText,
    placement: Placement,
    visible: bool,
}

/// Lit un texte MuseScore : balisage, placement et visibilité.
///
/// Le balisage est lu dans `Element::raw_inner`, qui re-sérialise le contenu
/// mixte de `<text>` comme `XmlReader::readXml` le fait pour le lecteur C++.
fn read_text_base(element: &Element) -> ParsedTextBase {
    let markup = element
        .child("text")
        .map_or("", |node| node.raw_inner.as_str());
    // Le style fixe le placement et la police de départ (`TextBase`), puis les
    // propriétés `italic`/`bold`/`underline`/`strike` du texte les écrasent.
    let (style_placement, mut format) = text_style_defaults(
        element
            .child("style")
            .map(Element::text)
            .unwrap_or_default(),
    );
    for (name, value) in [
        ("bold", &mut format.bold),
        ("italic", &mut format.italic),
        ("underline", &mut format.underline),
        ("strike", &mut format.strike),
    ] {
        if let Some(flag) = element.child(name).and_then(Element::int) {
            *value = flag != 0;
        }
    }
    if let Some(size) = element
        .child("size")
        .and_then(|node| node.text().parse::<f64>().ok())
    {
        format.font_size = Some(size);
    }
    if let Some(family) = element.child("family").map(Element::text) {
        if !family.is_empty() {
            format.font_family = Some(family.to_owned());
        }
    }
    ParsedTextBase {
        text: RichText::from_markup_with(markup, &format),
        placement: element
            .child("placement")
            .map_or(style_placement, |node| placement(Some(node))),
        visible: visible(element),
    }
}

/// Vrai si l'élément porte un saut de page (`<LayoutBreak>` `page`).
fn page_break(element: &Element) -> bool {
    element.child("LayoutBreak").is_some_and(|layout_break| {
        layout_break
            .child("subtype")
            .is_some_and(|subtype| subtype.text() == "page")
    })
}

/// Vrai si l'élément n'est pas marqué `<visible>0</visible>`.
fn visible(element: &Element) -> bool {
    !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0)
}

/// Lit un indicateur booléen (`<isCourtesy>1</isCourtesy>`).
fn flag(element: &Element, name: &str) -> bool {
    element
        .child(name)
        .and_then(Element::int)
        .is_some_and(|value| value != 0)
}

/// Nombre de lignes d'une portée d'après `<StaffType>`.
fn staff_lines(element: &Element) -> u8 {
    match element.child("name").map(Element::text) {
        Some("perc1Line") => 1,
        Some("perc2Line") => 2,
        Some("perc3Line") => 3,
        Some("perc4Line") => 4,
        _ => 5,
    }
    // TODO(parité): tablatures, portées sans ligne, <lines> explicite.
}

/// Lit la direction explicite d'une liaison (`<Tie><up>up|down</up>`).
fn tie_direction(element: &Element) -> Option<bool> {
    element
        .child("Tie")
        .and_then(|tie| tie.child("up"))
        .and_then(|up| match up.text() {
            "up" => Some(true),
            "down" => Some(false),
            _ => None,
        })
}

/// Lit une `<location>` de liaison.
fn read_tie_location(element: &Element) -> TieLocation {
    TieLocation {
        measures: element
            .child("measures")
            .and_then(Element::int)
            .unwrap_or(0),
        fractions: element
            .child("fractions")
            .and_then(Element::fraction)
            .unwrap_or(Fraction::ZERO),
        staves: element.child("staves").and_then(Element::int).unwrap_or(0),
        voices: element.child("voices").and_then(Element::int).unwrap_or(0),
    }
}

/// Vrai si la mesure ne contient qu'un silence de mesure entière.
///
/// Sert à reconnaître les mesures qu'un multi-repos d'une mesure remplace
/// (`Measure::isMMRest` implicite). Les annotations et spanners n'empêchent
/// pas le regroupement, mais un accord, une barre spéciale ou une reprise le
/// font.
fn is_empty_measure(measure: &Measure) -> bool {
    if !measure.repeat_marks.is_empty() {
        return false;
    }
    if measure.start_bar_line != BarLineType::Normal || measure.end_bar_line != BarLineType::Normal
    {
        return false;
    }
    let mut rest = false;
    for segment in &measure.segments {
        for (_, item) in segment.iter_elements() {
            match item {
                SegmentItem::ChordRest(chord_rest) if chord_rest.is_full_measure_rest() => {
                    rest = true;
                }
                SegmentItem::ChordRest(_) => return false,
                // Clefs, armures et indications de mesure n'empêchent pas le
                // regroupement : MuseScore les conserve dans la mesure qui ouvre
                // le multi-repos.
                _ => {}
            }
        }
    }
    rest
}

/// Intervalle diatonique approché d'un intervalle chromatique.
/// Utilisé seulement quand `<transposeDiatonic>` manque : `Interval::chromatic2diatonic`
/// dans MuseScore est plus fin pour les intervalles composés.
fn chromatic_to_diatonic(chromatic: i32) -> i32 {
    let table = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6, 7];
    match usize::try_from(chromatic) {
        Ok(index) if index < table.len() => table[index],
        _ => chromatic,
    }
}
