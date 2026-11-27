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

//! Oracle SMF indépendant de l'importeur, partagé par `midi_smf_capacity.rs`
//! et `layer_coverage.rs`.
//!
//! La règle de `docs/agents/model-modularisation.md` §5.3 est « 100 % = ce que
//! le format porte lui-même ». Un SMF ne porte pas les ticks *écrits* d'une
//! partition : il porte des messages Note On/Off datés (reprises déroulées,
//! durées de tenue, liaisons fusionnées, ornements décalés) et des
//! méta-événements. L'oracle est donc **le fichier MIDI produit**, lu avec
//! `midly`, sans passer par l'importeur de `scoretk`.
//!
//! Les deux vues (fichier et modèle relu) sont ramenées à la même forme :
//! `NoteKey` pour les attaques et `KindValues` pour les valeurs distinctes
//! des méta-évènements. `smf_inventory` transforme ces deux vues en
//! `LayerInventory` afin que la couverture soit mesurée avec le même
//! `compare_with_kinds` que les autres directions.
//!
//! Certains helpers ne servent qu'à l'un des deux binaires de test qui
//! incluent ce module (le diagnostic champ par champ contre les notes) ;
//! `dead_code` est donc désactivé localement.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use scoretk::model::{
    inventory::InventoryItem, ChordRestKind, Fraction, LayerInventory, Layers, Score, SegmentItem,
};

/// Ticks MIDI par noire dans les fichiers écrits par `scoretk-midi-export`.
pub const PPQ: i64 = 480;
/// Unités du modèle par ronde : `4 * PPQ`.
pub const UNITS_PER_WHOLE: u32 = PPQ as u32 * 4;

/// Attaque telle que le SMF la porte : `(hauteur, attaque, durée, vélocité,
/// canal)`. Les ticks sont ceux du fichier, pas ceux de la gravure.
pub type NoteKey = (u8, i64, i64, u8, u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub reference: usize,
    pub matched: usize,
}

impl Counts {
    pub fn add(&mut self, reference: usize, matched: usize) {
        self.reference += reference;
        self.matched += matched;
    }

    pub fn percent(self) -> f64 {
        if self.reference == 0 {
            100.0
        } else {
            self.matched as f64 * 100.0 / self.reference as f64
        }
    }
}

/// Extrait les attaques d'un SMF en appariant Note On / Note Off **par piste
/// et par (canal, hauteur)**, comme un synthétiseur et comme
/// `MidiTrack::mergeNoteOnOff` (`midishared/midifile.cpp`).
pub fn smf_notes(bytes: &[u8]) -> Vec<NoteKey> {
    let smf = Smf::parse(bytes).expect("un SMF valide");
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        let mut open: Vec<(u8, u8, i64, u8)> = Vec::new(); // canal, hauteur, attaque, vélocité
        for event in track {
            tick += i64::from(event.delta.as_int());
            let TrackEventKind::Midi {
                channel,
                ref message,
            } = event.kind
            else {
                continue;
            };
            let channel = channel.as_int();
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    open.push((channel, key.as_int(), tick, vel.as_int()))
                }

                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    let pitch = key.as_int();
                    if let Some(index) = open
                        .iter()
                        .position(|(c, p, _, _)| *c == channel && *p == pitch)
                    {
                        let (channel, pitch, onset, velocity) = open.remove(index);
                        // `exportmidi.cpp` écrit le Note Off à `fin - 1` : la
                        // fin de la performance est le tick du Note Off + 1.
                        // Un Note On de vélocité nulle sans note ouverte est
                        // un « restrike » de `fixupMIDI`, pas une attaque.
                        if tick > onset {
                            result.push((pitch, onset, tick - onset + 1, velocity, channel));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    result.sort();
    result
}

/// Extrait les attaques du modèle importé. La durée est la durée **jouée**
/// (`ChordRest::ticks`), sommée sur une chaîne de liaisons : le SMF n'a qu'une
/// attaque pour une note liée, même si l'import la découpe en fragments.
pub fn model_notes(score: &Score) -> Vec<NoteKey> {
    let mut by_id: HashMap<usize, (Fraction, Fraction, u8)> = HashMap::new(); // id → (tick, durée, canal)
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, element) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = element else {
                    continue;
                };
                let duration = chord_rest
                    .ticks
                    .unwrap_or_else(|| chord_rest.duration.ticks());
                let staff = track.staff().raw() as usize;
                let channel = score
                    .staves
                    .get(staff)
                    .and_then(|item| score.parts.get(item.part.index()))
                    .and_then(|part| part.instrument.midi_channel)
                    .unwrap_or(0);
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    by_id.insert(id.index(), (segment.tick, duration, channel));
                }
            }
        }
    }
    let mut result = Vec::new();
    for (id, note) in score.notes.iter().enumerate() {
        if note.tie_back.is_some() || !by_id.contains_key(&id) {
            continue;
        }
        let (tick, _, channel) = by_id[&id];
        let mut duration = by_id[&id].1;
        let mut next = note.tie_forward;
        while let Some(following) = next {
            let Some((_, length, _)) = by_id.get(&following.index()) else {
                break;
            };
            duration += *length;
            next = score.notes[following.index()].tie_forward;
        }
        result.push((
            note.pitch,
            tick.ticks(UNITS_PER_WHOLE),
            duration.ticks(UNITS_PER_WHOLE).max(1),
            note.playback.velocity.unwrap_or(0),
            // The channel status nibble is carried per Note On, not per part:
            // full-support import records it on the note itself.
            note.playback.midi_channel.unwrap_or(channel),
        ));
    }
    result.sort();
    result
}

/// Valeurs distinctes portées par chaque kind. Les méta-évènements étant
/// réécrits à chaque passage de reprise, comparer les occurrences pénaliserait
/// le format pour une répétition qu'il doit justement faire : on compare les
/// **valeurs** que le SMF porte, pas le nombre de fois où il les répète.
pub type KindValues = BTreeMap<&'static str, BTreeSet<String>>;

pub fn insert(values: &mut KindValues, kind: &'static str, value: impl Into<String>) {
    values.entry(kind).or_default().insert(value.into());
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_owned()
}

/// Décode une balise d'information RP-026 `{#NOM=valeur}` dans un `FF 01`.
/// Les quatre noms normés (`TITLE`, `COMPOSER`, `LYRICS`, `ARTIST`) et la
/// forme générique `{#}` sont des porteurs MMA RP-026, pas des préfixes
/// privés (`docs/agents/format-ref/midi.md` §"Lyrics: RP-017 and RP-026").
pub fn rp026_tag(content: &str) -> Option<(String, &str)> {
    let inner = content.strip_prefix("{#")?.strip_suffix('}')?;
    let (name, value) = inner.split_once('=')?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((name.to_ascii_lowercase(), value))
}

/// Retire les `FF 01` dont la charge utile commence par un préfixe privé
/// (`@ST:`, `@SY:`, `@TK:`). Un lecteur standard les affiche comme du simple
/// texte mais ne peut pas les rattacher à un champ du modèle : c'est la
/// mesure « sans extensions privées », sur le même fichier et le même oracle.
pub fn strip_private_text(bytes: &[u8]) -> Vec<u8> {
    let smf = Smf::parse(bytes).expect("un SMF valide");
    let mut stripped = Smf::new(midly::Header::new(smf.header.format, smf.header.timing));
    stripped.tracks = smf
        .tracks
        .iter()
        .map(|track| {
            track
                .iter()
                .cloned()
                .filter(|event| match event.kind {
                    TrackEventKind::Meta(MetaMessage::Text(data)) => {
                        let text = text(data);
                        !text.starts_with("@ST:")
                            && !text.starts_with("@SY:")
                            && !text.starts_with("@TK:")
                    }
                    _ => true,
                })
                .collect()
        })
        .collect();
    let mut result = Vec::new();
    stripped.write(&mut result).expect("rewrite SMF");
    result
}

/// Méta-évènements et messages de canal tels qu'ils sont dans le fichier.
pub fn smf_kind_values(bytes: &[u8]) -> KindValues {
    let smf = Smf::parse(bytes).expect("un SMF valide");
    let mut values = KindValues::new();
    let mut sequence_names: Vec<String> = Vec::new();
    for (track_index, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, exponent, _, _)) => {
                    insert(
                        &mut values,
                        "time-sig",
                        format!("{numerator}:{}", 1u32 << exponent),
                    );
                }
                TrackEventKind::Meta(MetaMessage::KeySignature(fifths, minor)) => insert(
                    &mut values,
                    "key-sig",
                    format!("{fifths}:{}", if minor { "Minor" } else { "Major" }),
                ),
                TrackEventKind::Meta(MetaMessage::Tempo(micros)) => insert(
                    &mut values,
                    "tempo",
                    // `FF 51` stocke des microsecondes entières : la valeur
                    // relue en battements/s n'a pas la même précision.
                    format!("{:.3}", 1_000_000.0 / f64::from(micros.as_int())),
                ),
                TrackEventKind::Meta(MetaMessage::Lyric(data)) => {
                    insert(&mut values, "lyric", text(data).trim_end().to_owned());
                }
                TrackEventKind::Meta(MetaMessage::Copyright(data)) => {
                    insert(&mut values, "title", format!("copyright:{}", text(data)));
                }
                TrackEventKind::Meta(MetaMessage::TrackName(data)) => {
                    // Le premier `FF 03` de la piste 0 est le nom de séquence
                    // ([SMF §Meta-Events]), les suivants nomment les parties.
                    if tick == 0 && track_index == 0 {
                        sequence_names.push(text(data));
                    }
                }
                TrackEventKind::Meta(MetaMessage::InstrumentName(data)) => {
                    insert(&mut values, "instrument", text(data));
                }
                // `[SMF §Meta-Events]` définit `FF 06` comme repère et
                // `FF 07` comme cue point. Le mode `midi_full_support` écrit
                // les textes système dans le cue point (un texte au niveau du
                // système, pas d'une portée) et les repères dans `FF 06`.
                TrackEventKind::Meta(MetaMessage::Marker(data)) => {
                    insert(&mut values, "rehearsal-mark", text(data));
                }
                TrackEventKind::Meta(MetaMessage::CuePoint(data)) => {
                    insert(&mut values, "system-text", text(data));
                }
                TrackEventKind::Meta(MetaMessage::Text(data)) => {
                    let content = text(data);
                    if let Some(field) = content.strip_prefix("@TK:") {
                        if let Some((name, value)) = field.split_once(':') {
                            insert(&mut values, "title", format!("{name}:{value}"));
                        }
                    } else if let Some(value) = content.strip_prefix("@SY:") {
                        insert(&mut values, "system-text", value.to_owned());
                    } else if let Some(value) = content.strip_prefix("@ST:") {
                        insert(&mut values, "staff-text", value.to_owned());
                    } else if let Some(field) = rp026_tag(&content) {
                        // `[RP-026]` `{#TITLE=}`, `{#COMPOSER=}`, `{#LYRICS=}`,
                        // `{#ARTIST=}` et la forme générique `{#}` : porteurs
                        // standard des métadonnées, contrairement aux `@TK:`.
                        let (name, value) = field;
                        insert(&mut values, "title", format!("{name}:{value}"));
                    } else {
                        insert(&mut values, "staff-text", content);
                    }
                }
                TrackEventKind::Midi {
                    channel,
                    ref message,
                } => {
                    insert(&mut values, "channel", channel.as_int().to_string());
                    match message {
                        MidiMessage::ProgramChange { program } => {
                            insert(&mut values, "program", program.as_int().to_string());
                        }
                        MidiMessage::Controller { controller, value } => {
                            match controller.as_int() {
                                7 => insert(&mut values, "volume", value.as_int().to_string()),
                                10 => insert(&mut values, "pan", value.as_int().to_string()),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    // Un seul `FF 03` au tick 0 est un nom de piste. Le nom de **séquence**
    // n'est distinguishable que par l'ordre : l'écrivain `smf_text` le place
    // avant le nom de partie.
    if let Some(work_title) = sequence_names.first().filter(|_| sequence_names.len() >= 2) {
        insert(&mut values, "title", format!("work_title:{work_title}"));
    }
    values
}

/// Les mêmes valeurs dans le modèle relu, ramenées à ce que le SMF porte.
pub fn model_kind_values(score: &Score) -> KindValues {
    use scoretk::model::Annotation;

    let mut values = KindValues::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, element) in segment.iter_elements() {
                match element {
                    SegmentItem::TimeSig(time) => insert(
                        &mut values,
                        "time-sig",
                        format!("{}:{}", time.numerator, time.denominator),
                    ),
                    SegmentItem::KeySig(key) => insert(
                        &mut values,
                        "key-sig",
                        format!("{}:{:?}", key.concert_key.fifths(), key.mode),
                    ),
                    SegmentItem::ChordRest(chord_rest) => {
                        for lyric in &chord_rest.lyrics {
                            insert(&mut values, "lyric", lyric.text.trim_end().to_owned());
                        }
                        // Per-note channel from the Note On status nibble.
                        if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                            for id in &chord.notes {
                                if let Some(channel) = score.notes[id.index()].playback.midi_channel
                                {
                                    insert(&mut values, "channel", channel.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            for annotation in &segment.annotations {
                match &annotation.annotation {
                    Annotation::Tempo(tempo) => {
                        if let Some(bps) = tempo.beats_per_second {
                            insert(&mut values, "tempo", format!("{bps:.3}"));
                        }
                    }
                    Annotation::StaffText(value) | Annotation::Expression(value) => {
                        insert(&mut values, "staff-text", value.text.plain().to_owned());
                    }
                    Annotation::SystemText(value) => {
                        insert(&mut values, "system-text", value.text.plain().to_owned());
                    }
                    Annotation::RehearsalMark(value) => {
                        insert(&mut values, "rehearsal-mark", value.text.plain().to_owned());
                    }
                    _ => {}
                }
            }
        }
    }
    for (name, value) in [
        ("work_title", &score.metadata.work_title),
        ("subtitle", &score.metadata.subtitle),
        ("composer", &score.metadata.composer),
        ("lyricist", &score.metadata.lyricist),
        ("poet", &score.metadata.poet),
        ("translator", &score.metadata.translator),
        ("arranger", &score.metadata.arranger),
        ("source", &score.metadata.source),
        ("movement_title", &score.metadata.movement_title),
        ("movement_number", &score.metadata.movement_number),
        ("work_number", &score.metadata.work_number),
        ("copyright", &score.metadata.copyright),
    ] {
        if !value.is_empty() {
            insert(&mut values, "title", format!("{name}:{value}"));
        }
    }
    for part in &score.parts {
        insert(
            &mut values,
            "instrument",
            part.instrument.track_name.clone(),
        );
        if let Some(program) = part.instrument.midi_program {
            insert(&mut values, "program", program.to_string());
        }
        if let Some(channel) = part.instrument.midi_channel {
            insert(&mut values, "channel", channel.to_string());
        }
        if let Some(volume) = part.instrument.volume {
            insert(&mut values, "volume", volume.to_string());
        }
        if let Some(pan) = part.instrument.pan {
            insert(&mut values, "pan", pan.to_string());
        }
    }
    values
}

#[derive(Clone, Default)]
pub struct FieldCounts {
    pub combined: Counts,
    pub pitch: Counts,
    pub onset: Counts,
    pub duration: Counts,
    pub velocity: Counts,
    pub channel: Counts,
}

/// Compare deux listes d'attaques comme des multi-ensembles. Chaque champ est
/// aussi compté séparément : un canal erroné ne masque pas une bonne hauteur.
pub fn count_notes(expected: &[NoteKey], actual: &[NoteKey]) -> FieldCounts {
    fn multiset(values: impl Iterator<Item = u64>) -> BTreeMap<u64, usize> {
        let mut result = BTreeMap::new();
        for value in values {
            *result.entry(value).or_insert(0) += 1;
        }
        result
    }
    fn matches(expected: BTreeMap<u64, usize>, actual: &mut BTreeMap<u64, usize>) -> usize {
        expected
            .into_iter()
            .map(|(key, wanted)| {
                let found = actual.get_mut(&key).map_or(0, |count| {
                    let taken = (*count).min(wanted);
                    *count -= taken;
                    taken
                });
                found
            })
            .sum()
    }

    // Un SMF ne dit pas si le Note Off est à la fin incluse ou exclue :
    // `exportmidi.cpp` écrit `start + duration - 1` pour les notes écrites,
    // mais pas pour les accords réalisés. La durée jouée est donc comparée à
    // un tick près, les autres champs à l'identique.
    fn close_durations(expected: &mut [i64], actual: &mut [i64]) -> usize {
        expected.sort_unstable();
        actual.sort_unstable();
        let mut matched = 0;
        let mut index = 0;
        for duration in expected.iter() {
            while index < actual.len() && actual[index] < *duration - 1 {
                index += 1;
            }
            if index < actual.len() && (actual[index] - *duration).abs() <= 1 {
                matched += 1;
                index += 1;
            }
        }
        matched
    }

    let mut result = FieldCounts::default();
    let mut expected_groups: BTreeMap<(u8, i64, u8, u8), Vec<i64>> = BTreeMap::new();
    let mut actual_groups: BTreeMap<(u8, i64, u8, u8), Vec<i64>> = BTreeMap::new();
    for (pitch, onset, duration, velocity, channel) in expected {
        expected_groups
            .entry((*pitch, *onset, *velocity, *channel))
            .or_default()
            .push(*duration);
    }
    for (pitch, onset, duration, velocity, channel) in actual {
        actual_groups
            .entry((*pitch, *onset, *velocity, *channel))
            .or_default()
            .push(*duration);
    }
    let mut combined = 0;
    for (key, mut durations) in expected_groups {
        let mut others = actual_groups.remove(&key).unwrap_or_default();
        combined += close_durations(&mut durations, &mut others);
    }
    result.combined.add(expected.len(), combined);

    let by_pitch: fn(&NoteKey) -> u64 = |n| u64::from(n.0);
    let by_onset: fn(&NoteKey) -> u64 = |n| n.1.max(0) as u64;
    let by_velocity: fn(&NoteKey) -> u64 = |n| u64::from(n.3);
    let by_channel: fn(&NoteKey) -> u64 = |n| u64::from(n.4);
    for (field, counts) in [
        (by_pitch, &mut result.pitch),
        (by_onset, &mut result.onset),
        (by_velocity, &mut result.velocity),
        (by_channel, &mut result.channel),
    ] {
        counts.add(
            expected.len(),
            matches(
                multiset(expected.iter().map(field)),
                &mut multiset(actual.iter().map(field)),
            ),
        );
    }
    let mut expected_durations: Vec<i64> = expected.iter().map(|note| note.2).collect();
    let mut actual_durations: Vec<i64> = actual.iter().map(|note| note.2).collect();
    result.duration.add(
        expected.len(),
        close_durations(&mut expected_durations, &mut actual_durations),
    );
    result
}

/// Layer of a declared SMF item kind, as in the layer inventory of the model.
fn kind_layer(kind: &str) -> Layers {
    match kind {
        "time-sig" | "key-sig" => Layers::METRIC,
        "note" => Layers::EVENTS,
        "instrument" | "program" | "channel" | "volume" | "pan" | "velocity" => Layers::PLAYBACK,
        _ => Layers::TEXT,
    }
}

/// Projects one SMF view (attacks + distinct meta values) into the keyed
/// inventory used by `compare_with_kinds`. Both sides use the same projection,
/// so the comparison is a genuine multiset match and no item kind is dropped.
///
/// The note value carries pitch, played duration, velocity and channel; the
/// `end-1` convention is normalised by comparing `duration - 1` on both sides
/// (`exportmidi.cpp` writes Note Off at `start + duration - 1`). Meta values
/// are counted once per distinct value, as in `midi_smf_capacity.rs`: a meta
/// repeated by repeat unfolding is one carried fact, not several.
pub fn smf_inventory(notes: &[NoteKey], kinds: &KindValues) -> LayerInventory {
    let mut items =
        Vec::with_capacity(notes.len() + kinds.values().map(BTreeSet::len).sum::<usize>());
    for (pitch, onset, duration, velocity, channel) in notes {
        items.push(InventoryItem {
            layer: Layers::EVENTS,
            tick: Some(Fraction::from_ticks(*onset, UNITS_PER_WHOLE)),
            staff: None,
            kind: "note",
            value: format!("{}:{}:{}:{}", pitch, duration - 1, velocity, channel),
        });
    }
    for (kind, values) in kinds {
        let layer = kind_layer(kind);
        for value in values {
            items.push(InventoryItem {
                layer,
                tick: None,
                staff: None,
                kind,
                value: value.clone(),
            });
        }
    }
    LayerInventory { items }
}
