// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `EventsHolder::fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`)
// et de `exportmidi.cpp:304-323` de MuseScore Studio, modifié (réécrit en Rust)
// en 2026.

//! Extinctions de restrike sur les autres canaux d'une même partie.
//!
//! `fixupMIDI` compte les notes en cours par (canal interne, hauteur), mais
//! `exportmidi.cpp:304-323` écrit l'extinction du restrike sur la piste de
//! l'attaque précédente **avec le canal de cette piste**. Quand une partie
//! utilise deux canaux MIDI (notes et harmonies d'un orgue, par exemple), une
//! attaque sur l'un éteint donc aussi la note de même hauteur encore ouverte
//! sur l'autre : le synthétiseur n'a qu'un note-off par (canal, hauteur).

use std::collections::BTreeMap;

/// Canaux dont une note de `key` sonne encore dans `part`, avec `channel`
/// (canal de la nouvelle attaque) en premier.
pub(crate) fn restrike_channels(
    playing: &BTreeMap<(usize, u8, u8), (u32, usize)>,
    part: usize,
    channel: u8,
    key: u8,
) -> Vec<u8> {
    let mut channels = vec![channel];
    for ((other_part, other_channel, other_key), (count, _)) in playing {
        if *other_part == part
            && *other_key == key
            && *other_channel != channel
            && *count > 0
            && !channels.contains(other_channel)
        {
            channels.push(*other_channel);
        }
    }
    channels
}
