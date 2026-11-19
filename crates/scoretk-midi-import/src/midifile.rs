// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/midi/internal/midishared/midifile.cpp` et
// `midievent.h` de MuseScore Studio, Copyright (C) MuseScore Limited and
// others, modifié (réécrit en Rust) en 2026. La lecture des octets SMF utilise
// la crate `midly` (Unlicense) ; le découpage en pistes, la séparation des
// canaux et la fusion note-on/note-off suivent le C++.
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

//! Lecture d'un Standard MIDI File en pistes d'évènements datés.
//!
//! Portage de `MidiFile::read`, `MidiFile::separateChannel` et
//! `MidiTrack::mergeNoteOnOff` (`midishared/midifile.cpp`) : chaque évènement
//! est daté en ticks de fichier, les notes sont fusionnées en évènements
//! `ME_NOTE` (attaque + durée), et une piste contenant plusieurs canaux est
//! scindée en autant de pistes.

use scoretk_core::model::Fraction;

/// Division MuseScore : 480 ticks par noire (`Constants::DIVISION`).
pub const DIVISION: i64 = 480;

/// Unité d'un tick MuseScore : `DIVISION * 4` par ronde.
pub const UNITS_PER_WHOLE: i64 = DIVISION * 4;

/// Types de méta-évènements utilisés par l'import (`midicoreevent.h`).
pub mod meta {
    /// Texte (`META_TEXT`).
    pub const TEXT: u8 = 0x01;
    /// Copyright (`META_COPYRIGHT`).
    pub const COPYRIGHT: u8 = 0x02;
    /// Nom de piste (`META_TRACK_NAME`).
    pub const TRACK_NAME: u8 = 0x03;
    /// Nom d'instrument (`META_INSTRUMENT_NAME`).
    pub const INSTRUMENT_NAME: u8 = 0x04;
    /// Parole (`META_LYRIC`).
    pub const LYRIC: u8 = 0x05;
    /// Repère (`META_MARKER`).
    pub const MARKER: u8 = 0x06;
    /// Titre (extension MuseScore, `META_TITLE`).
    pub const TITLE: u8 = 0x10;
    /// Sous-titre (extension MuseScore).
    pub const SUBTITLE: u8 = 0x11;
    /// Compositeur (extension MuseScore).
    pub const COMPOSER: u8 = 0x12;
    /// Traducteur (extension MuseScore).
    pub const TRANSLATOR: u8 = 0x13;
    /// Poète / parolier (extension MuseScore).
    pub const POET: u8 = 0x14;
    /// Changement de port (`META_PORT_CHANGE`).
    pub const PORT_CHANGE: u8 = 0x21;
    /// Tempo (`META_TEMPO`), en microsecondes par noire.
    pub const TEMPO: u8 = 0x51;
    /// Indication de mesure (`META_TIME_SIGNATURE`).
    pub const TIME_SIGNATURE: u8 = 0x58;
    /// Armure (`META_KEY_SIGNATURE`).
    pub const KEY_SIGNATURE: u8 = 0x59;
}

/// Contrôleurs MIDI utilisés par l'import (`CntrType`).
pub mod ctrl {
    /// Volume de canal (`CTRL_VOLUME`).
    pub const VOLUME: u8 = 0x07;
    /// Panoramique (`CTRL_PANPOT`).
    pub const PANPOT: u8 = 0x0a;
}

/// Nature d'un évènement MIDI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Note fusionnée : hauteur, vélocité et durée en ticks de fichier
    /// (`ME_NOTE`, résultat de `mergeNoteOnOff`).
    Note {
        /// Hauteur MIDI (0-127).
        pitch: u8,
        /// Vélocité d'attaque (1-127).
        velocity: u8,
        /// Durée en ticks de fichier.
        len: i64,
    },
    /// Note-on encore non fusionnée.
    NoteOn {
        /// Hauteur MIDI.
        pitch: u8,
        /// Vélocité.
        velocity: u8,
    },
    /// Note-off encore non fusionnée.
    NoteOff {
        /// Hauteur MIDI.
        pitch: u8,
    },
    /// Méta-évènement ; `data` porte les octets bruts.
    Meta {
        /// Type de méta-évènement.
        kind: u8,
        /// Octets de données.
        data: Vec<u8>,
    },
    /// Changement de programme (`ME_PROGRAM`).
    Program(u8),
    /// Contrôleur continu (`ME_CONTROLLER`).
    Controller {
        /// Numéro de contrôleur.
        controller: u8,
        /// Valeur (0-127).
        value: u8,
    },
}

impl EventKind {
    /// Vrai pour un évènement porteur d'un canal.
    pub fn is_channel_event(&self) -> bool {
        matches!(
            self,
            Self::Note { .. }
                | Self::NoteOn { .. }
                | Self::NoteOff { .. }
                | Self::Program(_)
                | Self::Controller { .. }
        )
    }

    /// Vrai pour un méta-évènement du type donné.
    pub fn meta_kind(&self, wanted: u8) -> bool {
        matches!(self, Self::Meta { kind, .. } if *kind == wanted)
    }

    /// Données d'un méta-évènement.
    pub fn meta_data(&self) -> Option<&[u8]> {
        match self {
            Self::Meta { data, .. } => Some(data),
            _ => None,
        }
    }

    /// Texte UTF-8 d'un méta-évènement (octets invalides remplacés).
    pub fn meta_text(&self) -> Option<String> {
        self.meta_data().map(decode_meta_text)
    }
}

/// Décode le texte d'un méta-évènement en une chaîne acceptable en XML 1.0.
///
/// Les caractères de contrôle C0 (hors tabulation, saut de ligne et retour
/// chariot), ainsi que `U+FFFE`/`U+FFFF`, sont interdits en XML 1.0 et ne
/// peuvent pas être échappés. De nombreux fichiers MIDI (dont le corpus Lakh)
/// terminent leurs noms de piste par un octet NUL : sans ce filtrage, le nom se
/// retrouve tel quel dans `<part-name>` et le MusicXML produit est mal formé.
/// On retire ces caractères à la source, comme le fait le nettoyage appliqué
/// par `XmlWriter` côté MuseScore.
pub fn decode_meta_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| is_xml_10_char(*character))
        .collect()
}

/// Caractère autorisé par la production `Char` de la recommandation XML 1.0.
fn is_xml_10_char(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&character)
        || ('\u{E000}'..='\u{FFFD}').contains(&character)
        || ('\u{10000}'..='\u{10FFFF}').contains(&character)
}

#[cfg(test)]
mod meta_text_tests {
    use super::{decode_meta_text, meta, EventKind};

    #[test]
    fn strips_trailing_nul_from_track_name() {
        let raw = b"Back To Your Heart'\x00";
        assert_eq!(decode_meta_text(raw), "Back To Your Heart'");
    }

    #[test]
    fn keeps_ordinary_accents_and_xml_allowed_whitespace() {
        let raw = "Percussion, \"Piano\"\t\n\r—é\n".as_bytes();
        assert_eq!(decode_meta_text(raw), "Percussion, \"Piano\"\t\n\r—é\n");
    }

    #[test]
    fn strips_control_characters_and_noncharacters() {
        let raw = [b'A', 0x00, 0x01, 0x08, 0x0B, 0x0C, 0x1F, b'B'];
        assert_eq!(decode_meta_text(&raw), "AB");
        let utf16 = ['A', '\u{FFFE}', '\u{FFFF}', 'B'];
        let encoded: String = utf16.iter().collect();
        assert_eq!(decode_meta_text(encoded.as_bytes()), "AB");
    }

    #[test]
    fn meta_text_uses_the_same_decoding() {
        let event = EventKind::Meta {
            kind: meta::TRACK_NAME,
            data: b"Violon\x00".to_vec(),
        };
        assert_eq!(event.meta_text().as_deref(), Some("Violon"));
    }
}

/// Évènement daté avec son canal.
#[derive(Clone, Debug)]
pub struct RawEvent {
    /// Nature de l'évènement.
    pub kind: EventKind,
    /// Canal MIDI de l'évènement.
    pub channel: u8,
}

/// Piste MIDI.
#[derive(Clone, Debug, Default)]
pub struct MidiTrack {
    /// Évènements triés par tick, dans l'ordre du fichier.
    pub events: Vec<(i64, RawEvent)>,
    /// Canal de sortie (`-1` tant que `separate_channel` ne l'a pas fixé).
    pub out_channel: i32,
    /// Port de sortie.
    pub out_port: i32,
    /// Vrai pour une piste de percussion (canal 10, ou marqueur Yamaha).
    pub drum_track: bool,
}

impl MidiTrack {
    /// Note l'évènement avec son canal.
    pub fn insert(&mut self, tick: i64, event: RawEvent) {
        self.events.push((tick, event));
    }
}

/// Fichier MIDI lu.
#[derive(Clone, Debug)]
pub struct MidiFile {
    /// Pistes, dans l'ordre du fichier.
    pub tracks: Vec<MidiTrack>,
    /// Division du fichier : ticks par noire, ou ticks par seconde en SMPTE.
    pub division: i64,
    /// Vrai si `division` compte des ticks par seconde (SMPTE).
    pub is_division_in_tps: bool,
    /// Format SMF (0, 1 ou 2).
    pub format: u16,
}

impl MidiFile {
    /// Lit un SMF depuis des octets en mémoire.
    ///
    /// Équivalent de `MidiFile::read` : la division est interprétée comme
    /// `midifile.cpp` (SMPTE → ticks par seconde, 29,97 images/s arrondi).
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        let smf = midly::Smf::parse(bytes).map_err(|error| error.to_string())?;

        let (division, is_division_in_tps) = match smf.header.timing {
            midly::Timing::Metrical(ticks) => (i64::from(ticks.as_int()), false),
            midly::Timing::Timecode(fps, subframe) => {
                let frames = match fps {
                    midly::Fps::Fps24 => 24.0,
                    midly::Fps::Fps25 => 25.0,
                    midly::Fps::Fps29 => 29.97,
                    midly::Fps::Fps30 => 30.0,
                };
                let division = (frames * f64::from(subframe)).round() as i64;
                (division, true)
            }
        };
        if division <= 0 {
            return Err("division MIDI nulle".to_owned());
        }

        let mut tracks = Vec::with_capacity(smf.tracks.len());
        for track in &smf.tracks {
            let mut events = Vec::new();
            let mut tick: i64 = 0;
            for event in track {
                tick += i64::from(event.delta.as_int());
                let (channel, kind) = match event.kind {
                    midly::TrackEventKind::Midi { channel, message } => {
                        let channel = channel.as_int();
                        let kind = match message {
                            midly::MidiMessage::NoteOn { key, vel } => EventKind::NoteOn {
                                pitch: key.as_int(),
                                velocity: vel.as_int(),
                            },
                            midly::MidiMessage::NoteOff { key, .. } => EventKind::NoteOff {
                                pitch: key.as_int(),
                            },
                            midly::MidiMessage::ProgramChange { program } => {
                                EventKind::Program(program.as_int())
                            }
                            midly::MidiMessage::Controller { controller, value } => {
                                EventKind::Controller {
                                    controller: controller.as_int(),
                                    value: value.as_int(),
                                }
                            }
                            _ => continue,
                        };
                        (channel, kind)
                    }
                    midly::TrackEventKind::Meta(message) => {
                        let (kind, data) = match message {
                            midly::MetaMessage::TrackName(data) => {
                                (meta::TRACK_NAME, data.to_vec())
                            }
                            midly::MetaMessage::InstrumentName(data) => {
                                (meta::INSTRUMENT_NAME, data.to_vec())
                            }
                            midly::MetaMessage::Text(data) => (meta::TEXT, data.to_vec()),
                            midly::MetaMessage::Copyright(data) => (meta::COPYRIGHT, data.to_vec()),
                            midly::MetaMessage::Lyric(data) => (meta::LYRIC, data.to_vec()),
                            midly::MetaMessage::Marker(data) => (meta::MARKER, data.to_vec()),
                            midly::MetaMessage::Tempo(tempo) => {
                                let value = tempo.as_int();
                                (
                                    meta::TEMPO,
                                    vec![
                                        ((value >> 16) & 0xff) as u8,
                                        ((value >> 8) & 0xff) as u8,
                                        (value & 0xff) as u8,
                                    ],
                                )
                            }
                            midly::MetaMessage::TimeSignature(
                                numerator,
                                denominator,
                                clocks,
                                thirty_seconds,
                            ) => (
                                meta::TIME_SIGNATURE,
                                vec![numerator, denominator, clocks, thirty_seconds],
                            ),
                            midly::MetaMessage::KeySignature(sharps, minor) => {
                                (meta::KEY_SIGNATURE, vec![sharps as u8, u8::from(minor)])
                            }
                            _ => continue,
                        };
                        (0, EventKind::Meta { kind, data })
                    }
                    _ => continue,
                };
                events.push((tick, RawEvent { kind, channel }));
            }
            tracks.push(MidiTrack {
                events,
                out_channel: -1,
                out_port: 0,
                drum_track: false,
            });
        }

        // Format 2 : chaque piste est un motif indépendant joué à sa suite,
        // et non une voix simultanée (SMF §Format 0,1,2). MuseScore rejette ce
        // format (`midifile.cpp` lève « format not implemented ») ; le portage
        // le prend en charge en décalant chaque piste de la fin de la précédente.
        if smf.header.format == midly::Format::Sequential {
            let mut offset = 0i64;
            for track in &mut tracks {
                for (tick, _) in &mut track.events {
                    *tick += offset;
                }
                offset = track.events.last().map_or(offset, |(tick, _)| *tick);
            }
        }

        Ok(Self {
            tracks,
            division,
            is_division_in_tps,
            format: u16::from(smf.header.format as u8),
        })
    }

    /// Sépare les canaux d'une même piste en pistes distinctes.
    ///
    /// Portage de `MidiFile::separateChannel` : une piste utilisant plusieurs
    /// canaux est scindée, les pistes ajoutées suivent immédiatement la piste
    /// d'origine. Le canal de sortie est le premier canal rencontré.
    pub fn separate_channel(&mut self) {
        let mut index = 0;
        while index < self.tracks.len() {
            let mut channels: Vec<u8> = Vec::new();
            for (_, event) in &self.tracks[index].events {
                if event.kind.is_channel_event() && !channels.contains(&event.channel) {
                    channels.push(event.channel);
                }
            }
            self.tracks[index].out_channel = i32::from(channels.first().copied().unwrap_or(0));
            if channels.len() <= 1 {
                index += 1;
                continue;
            }
            channels.sort_unstable();
            for channel in channels.iter().skip(1) {
                self.tracks.insert(
                    index
                        + channels
                            .iter()
                            .position(|candidate| candidate == channel)
                            .unwrap_or(0),
                    MidiTrack {
                        events: Vec::new(),
                        out_channel: i32::from(*channel),
                        out_port: 0,
                        drum_track: *channel == 9,
                    },
                );
            }
            // Répartit les évènements de canal dans les pistes ; les autres
            // évènements restent sur la piste de plus petit canal.
            let old_events = std::mem::take(&mut self.tracks[index].events);
            let mut buckets: Vec<Vec<(i64, RawEvent)>> = vec![Vec::new(); channels.len()];
            for (tick, event) in old_events {
                let bucket = if event.kind.is_channel_event() {
                    channels
                        .iter()
                        .position(|channel| *channel == event.channel)
                        .unwrap_or(0)
                } else {
                    0
                };
                buckets[bucket].push((tick, event));
            }
            for (offset, events) in buckets.into_iter().enumerate() {
                self.tracks[index + offset].events = events;
            }
            index += channels.len();
        }
        for track in &mut self.tracks {
            if track.out_channel == 9 {
                track.drum_track = true;
            }
            track.events.sort_by_key(|(tick, _)| *tick);
        }
    }

    /// Fusionne les couples note-on/note-off en évènements `ME_NOTE`.
    ///
    /// Portage de `MidiTrack::mergeNoteOnOff` : la durée est le delta jusqu'au
    /// premier note-off (ou note-on de vélocité nulle) de même hauteur ; une
    /// durée nulle ou négative vaut un tick, une note sans note-off aussi.
    pub fn merge_note_on_off(&mut self) {
        for track in &mut self.tracks {
            let events = std::mem::take(&mut track.events);
            let mut invalid = vec![false; events.len()];
            let mut result: Vec<(i64, RawEvent)> = Vec::with_capacity(events.len());

            for (index, (tick, event)) in events.iter().enumerate() {
                if invalid[index] {
                    continue;
                }
                match event.kind {
                    EventKind::NoteOn {
                        pitch: _,
                        velocity: 0,
                    } => {}
                    EventKind::NoteOn { pitch, velocity } => {
                        let mut len = 1;
                        for (next, (next_tick, next_event)) in
                            events.iter().enumerate().skip(index + 1)
                        {
                            if invalid[next] {
                                continue;
                            }
                            let closes = match next_event.kind {
                                EventKind::NoteOff { pitch: off } => off == pitch,
                                EventKind::NoteOn {
                                    pitch: off,
                                    velocity: 0,
                                } => off == pitch,
                                _ => false,
                            };
                            if closes {
                                invalid[next] = true;
                                len = (next_tick - tick).max(1);
                                break;
                            }
                        }
                        result.push((
                            *tick,
                            RawEvent {
                                kind: EventKind::Note {
                                    pitch,
                                    velocity,
                                    len,
                                },
                                channel: event.channel,
                            },
                        ));
                    }
                    EventKind::NoteOff { .. } => {}
                    _ => result.push((*tick, event.clone())),
                }
            }
            result.sort_by_key(|(tick, _)| *tick);
            track.events = result;
        }
    }
}

/// Convertit un tick de fichier en fraction de ronde MuseScore.
///
/// Portage de `toMuseScoreTicks` (`importmidi_fraction.cpp`) : en division
/// métrique, le reste de la division est arrondi à la demi-unité près. En SMPTE,
/// `tick / old_division` est un temps en secondes ; comme MuseScore ne consulte
/// pas la carte des tempos ici, on utilise l'équivalence fixe à 120 BPM (deux
/// noires par seconde, soit `2 * DIVISION` ticks MuseScore par seconde).
pub fn to_muse_score_ticks(tick: i64, old_division: i64, is_division_in_tps: bool) -> Fraction {
    if is_division_in_tps {
        let division = old_division.max(1);
        let integral = tick.div_euclid(division);
        let remainder = tick.rem_euclid(division);
        let per_second = DIVISION * 2;
        let converted = integral * per_second
            + (remainder * per_second + division / 2) / division;
        return Fraction::from_ticks(converted, UNITS_PER_WHOLE as u32);
    }
    let old_division = old_division.max(1);
    let integral = tick.div_euclid(old_division);
    let remainder = tick.rem_euclid(old_division);
    let tmp = remainder * DIVISION + old_division / 2;
    let ms_ticks = integral * DIVISION + tmp / old_division;
    Fraction::from_ticks(ms_ticks, UNITS_PER_WHOLE as u32)
}
