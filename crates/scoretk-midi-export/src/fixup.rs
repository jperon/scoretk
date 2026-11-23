// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `EventsHolder::fixupMIDI` et de son usage dans
// `engraving/compat/midi/event.cpp` et
// `importexport/midi/internal/midiexport/exportmidi.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Unisson et chevauchement de même hauteur sur un canal.
//!
//! `EventsHolder::fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`)
//! compte, par (canal, hauteur), les notes en cours. Une attaque de même
//! hauteur lancée pendant qu'une autre sonne reçoit `discard = portée de la
//! précédente + 1` ; `exportmidi.cpp:304-323` écrit alors une extinction à son
//! tick sur la piste précédente (« restrike ») et l'extinction de la première
//! note est ignorée ou déplacée sur la piste de la dernière attaque. Un canal
//! MIDI n'a qu'un note-off par hauteur : sans cette fusion, MuseScore et les
//! synthétiseurs coupent la mauvaise note. Les notes d'orgue ou de piano
//! écrites en unisson sur deux portées d'une même partie sont le cas typique.

use std::collections::BTreeMap;

use super::render::{Event, EventKind, Performance};

/// Sort de chaque évènement de note après `fixupMIDI`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    /// Écrit sur sa propre piste.
    Keep,
    /// Extinction ignorée (une autre note de même hauteur la remplace).
    Drop,
    /// Extinction déplacée sur la piste de la dernière attaque.
    Hoist(usize),
    /// Attaque qui déclenche une extinction sur la piste `usize`.
    Restrike(usize),
}

/// Applique `EventsHolder::fixupMIDI` à la performance rendue.
///
/// `part_of_track` donne la partie de chaque portée : le C++ indexe
/// `EventsHolder` par **canal interne** de la partie, alors que l'écriture
/// peut envoyer deux parties sur le même canal MIDI de sortie. Deux portées
/// d'une même partie (orgue, piano) fusionnent donc leurs unissons, deux
/// parties distinctes non.
pub(crate) fn fixup_midi(
    performance: &mut Performance,
    part_of_track: &[usize],
    part_channels: &[Vec<u8>],
) {
    let tracks = &performance.tracks;
    // Ordre d'insertion du `std::multimap` du C++ : par tick, puis par ordre
    // de collecte. Les portées sont rendues dans l'ordre, donc
    // `(tick, piste, rang dans la piste)` reproduit cet ordre.
    let mut order: Vec<(i64, usize, usize)> = Vec::new();
    for (track_index, track) in tracks.iter().enumerate() {
        for (seq, event) in track.events.iter().enumerate() {
            match &event.kind {
                EventKind::NoteOn { velocity, .. } if *velocity > 0 => {
                    order.push((event.tick, track_index, seq));
                }
                EventKind::NoteOff { .. } => order.push((event.tick, track_index, seq)),
                _ => {}
            }
        }
    }
    order.sort_by_key(|(tick, track, seq)| (*tick, *track, *seq));

    let mut decisions: Vec<Vec<Decision>> = tracks
        .iter()
        .map(|track| vec![Decision::Keep; track.events.len()])
        .collect();
    // (partie, canal midi, hauteur) → (notes en cours, piste de la dernière
    // attaque). Le canal distingue la réalisation des accords chiffrés
    // (`Part::harmonyChannel`) des notes écrites de la même partie, tout en
    // gardant deux parties qui partageraient un canal de sortie séparées.
    let mut playing: BTreeMap<(usize, u8, u8), (u32, usize)> = BTreeMap::new();

    for (_, track_index, seq) in order {
        let event = &tracks[track_index].events[seq];
        match &event.kind {
            EventKind::NoteOn {
                channel,
                key,
                velocity,
            } if *velocity > 0 => {
                let part = part_of_track
                    .get(track_index)
                    .copied()
                    .unwrap_or(track_index);
                let state = playing
                    .entry((part, *channel, *key))
                    .or_insert((0, track_index));
                if state.0 >= 1 {
                    // `event.setDiscard(info[channel].event[pitch]->getOriginatingStaff()+1)`
                    decisions[track_index][seq] = Decision::Restrike(state.1);
                }
                state.0 += 1;
                state.1 = track_index;
            }
            EventKind::NoteOff { channel, key } => {
                let part = part_of_track
                    .get(track_index)
                    .copied()
                    .unwrap_or(track_index);
                let state = playing
                    .entry((part, *channel, *key))
                    .or_insert((0, track_index));
                if state.0 == 0 || state.0 - 1 > 0 {
                    // « already off or still playing » : extinction ignorée.
                    decisions[track_index][seq] = Decision::Drop;
                    state.0 = state.0.saturating_sub(1);
                } else {
                    // Dernière note de la hauteur : son extinction est écrite
                    // sur la piste de la dernière attaque.
                    decisions[track_index][seq] = Decision::Hoist(state.1);
                    state.0 = 0;
                }
            }
            _ => {}
        }
    }

    let mut rebuilt: Vec<Vec<Event>> = vec![Vec::new(); tracks.len()];
    let mut restrikes: Vec<Vec<Event>> = vec![Vec::new(); tracks.len()];
    for (track_index, track) in tracks.iter().enumerate() {
        for (seq, event) in track.events.iter().enumerate() {
            match decisions[track_index][seq] {
                Decision::Keep => rebuilt[track_index].push(event.clone()),
                Decision::Drop => {}
                Decision::Hoist(target) => {
                    let destination = if target < rebuilt.len() {
                        target
                    } else {
                        track_index
                    };
                    rebuilt[destination].push(event.clone());
                }
                Decision::Restrike(target) => {
                    rebuilt[track_index].push(event.clone());
                    if let (
                        Some(destination),
                        EventKind::NoteOn {
                            channel: source_channel,
                            key,
                            ..
                        },
                    ) = (restrikes.get_mut(target), &event.kind)
                    {
                        // `exportmidi.cpp:297-308` : l'extinction du restrike
                        // est insérée pour chaque canal de la partie avant le
                        // filtre de canal des lignes 331-334. La piste `target`
                        // est celle de la note précédente : on prend ses
                        // canaux de sortie (notes + harmonies).
                        let part = part_of_track.get(target).copied().unwrap_or(track_index);
                        let configured = part_channels.get(part).map(Vec::as_slice).unwrap_or(&[]);
                        // Repli : sans canal configuré pour la partie, on
                        // écrit l'extinction sur le canal de l'évènement,
                        // comme avant l'ajout du port par canal.
                        let fallback = [*source_channel];
                        let channels = if configured.is_empty() {
                            &fallback[..]
                        } else {
                            configured
                        };
                        for channel in channels.iter().copied() {
                            destination.push(Event {
                                tick: event.tick,
                                kind: EventKind::NoteOffRestrike { channel, key: *key },
                            });
                        }
                    }
                }
            }
        }
    }
    for (track, extra) in rebuilt.iter_mut().zip(restrikes) {
        track.extend(extra);
    }
    for (track, events) in performance.tracks.iter_mut().zip(rebuilt) {
        track.events = events;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::TrackData;

    fn on(tick: i64, channel: u8, key: u8) -> Event {
        Event {
            tick,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity: 80,
            },
        }
    }
    fn off(tick: i64, channel: u8, key: u8) -> Event {
        Event {
            tick,
            kind: EventKind::NoteOff { channel, key },
        }
    }

    /// `Veni veni Emmanuel`, tick 22 080 : la référence écrit sur la piste de
    /// la note précédente un restrike sur **chaque canal de la partie**
    /// (`exportmidi.cpp:297-308` place l'insertion avant le filtre de canal
    /// des lignes 331-334). Notre écrivain n'en met qu'un.
    #[test]
    fn restrike_ecrit_un_off_par_canal_de_la_partie() {
        let mut performance = Performance {
            tracks: vec![
                TrackData {
                    events: vec![on(0, 0, 67), on(100, 0, 67), off(200, 0, 67)],
                },
                TrackData {
                    events: vec![on(50, 1, 67), off(150, 1, 67)],
                },
            ],
        };
        fixup_midi(&mut performance, &[0, 0], &[vec![0, 1]]);
        let restrikes: Vec<(i64, u8)> = performance
            .tracks
            .iter()
            .flat_map(|t| t.events.iter())
            .filter_map(|e| match e.kind {
                EventKind::NoteOffRestrike { channel, key: 67 } => Some((e.tick, channel)),
                _ => None,
            })
            .collect();
        assert_eq!(restrikes, vec![(100, 0), (100, 1)], "un off par canal");
    }
    /// Sans table de canaux (partie inconnue), le restrike reste écrit sur
    /// le canal de l'évènement, comme avant le port par canal.
    #[test]
    fn restrike_sans_canal_configure_garde_le_canal_de_levenement() {
        let mut performance = Performance {
            tracks: vec![TrackData {
                events: vec![on(0, 3, 67), on(100, 3, 67), off(200, 3, 67)],
            }],
        };
        fixup_midi(&mut performance, &[0], &[]);
        let restrikes: Vec<(i64, u8)> = performance
            .tracks
            .iter()
            .flat_map(|t| t.events.iter())
            .filter_map(|e| match e.kind {
                EventKind::NoteOffRestrike { channel, key: 67 } => Some((e.tick, channel)),
                _ => None,
            })
            .collect();
        assert_eq!(
            restrikes,
            vec![(100, 3)],
            "repli sur le canal de l'évènement"
        );
    }
}
