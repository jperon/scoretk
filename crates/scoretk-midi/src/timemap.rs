// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier ne porte pas de code C++ : il ajoute à l'écriture MIDI de
// `scoretk` une carte temps ↔ évènements, construite à partir du rendu
// (`render.rs`) et du déroulement des reprises (`repeat_list.rs`), modifié
// (écrit en Rust) en 2026.
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

//! Carte des temps de la performance MIDI (`timemap`).
//!
//! Une carte de temps associe à chaque tick d'exécution du fichier MIDI produit
//! par [`super::write_midi_with_timemap`] son temps en microsecondes, et
//! recense les synchronisations utiles à un lecteur de partition : changements
//! de tempo, débuts de mesures (y compris les passages successifs dus aux
//! reprises), attaques de notes et pauses de section.
//!
//! La carte est un **sous-produit du rendu** : elle est remplie aux mêmes
//! endroits que les évènements MIDI (voir [`super::render`]), donc les ticks
//! qu'elle contient sont exactement ceux du fichier écrit, pauses de section
//! comprises. L'invariance à l'octet est vérifiée par
//! `tests/timemap_alignment.rs`.
//!
//! L'intégration du tempo est **entière** : chaque segment de tempo contribue
//! `delta_ticks × micros_per_quarter / DIVISION`, accumulé en [`i64`], sans
//! passer par un flottant. Le résultat est donc reproductible et ne dérive pas
//! avec la longueur du morceau.
//!
//! # Limites
//!
//! - **Pas de géométrie** : la carte ne contient ni abscisse, ni système, ni
//!   page ; elle ne peut pas servir à placer un curseur de lecture à l'écran
//!   sans un moteur de mise en page.
//! - **Pas de vélocité dans la carte** : le rendu MIDI applique les nuances, les
//!   soufflets, les multiplicateurs d'articulation puis, par note,
//!   `Note::customizeVelocity` (`Note::user_velocity` et `Note::velo_type`),
//!   mais `NoteSync` ne porte pas la vélocité résultante. La vélocité d'import
//!   MIDI (`Note::velocity`, absolue 0-127) n'intervient pas dans ce rendu ;
//!   elle sert à l'export MusicXML.
//! - **Les fermatas ne sont pas modélisées** : leur allongement n'apparaît ni
//!   dans les ticks ni dans les temps.
//! - **Une mesure peut apparaître plusieurs fois** (reprises, voltas, sauts) :
//!   [`MeasureSync::pass`] distingue les passages, [`MeasureSync::segment`]
//!   donne le segment de reprise et [`MeasureSync::measure`] l'indice de la
//!   mesure dans `Score::measures`.
//! - **Les trémolos répètent la même note** : plusieurs [`NoteSync`] peuvent
//!   partager le même identifiant ([`NoteSync::xml_id`]), comme les attaques
//!   qu'elles décrivent.
//!
//! Les formules d'identifiants MusicXML viennent de
//! `mscz2xml::model::xmlid` : la carte et l'export MusicXML produisent donc
//! exactement les mêmes chaînes, sans recopie. Voir `docs/agents/timemap.md`.

use mscz2xml::model::xmlid::{grace_note_xml_id, measure_xml_id, note_xml_id};
use mscz2xml::model::Score;

use super::pause_map::PauseMap;
use super::repeat_list::{RepeatList, DIVISION};

/// Changement de tempo de la performance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoChange {
    /// Tick d'exécution du changement.
    pub tick: i64,
    /// Temps du changement depuis le début, en microsecondes.
    pub time_us: i64,
    /// Tempo en microsecondes par noire.
    pub micros_per_quarter: u32,
}

/// Synchronisation d'une mesure jouée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureSync {
    /// Tick d'exécution du début de la mesure.
    pub tick: i64,
    /// Tick d'exécution de la fin de la mesure (exclu).
    pub end_tick: i64,
    /// Temps du début de la mesure, en microsecondes.
    pub time_us: i64,
    /// Temps de la fin de la mesure, en microsecondes.
    pub end_time_us: i64,
    /// Indice de la mesure dans `Score::measures`.
    pub measure: usize,
    /// Numéro de mesure du modèle (`Measure::number`, à partir de zéro).
    pub number: usize,
    /// Indice du segment de reprise ([`RepeatList::segments`]).
    pub segment: usize,
    /// Numéro de passage du segment (le premier vaut 1).
    pub pass: i32,
    /// Vrai quand une pause de section précède la mesure.
    pub pause_before: bool,
}

impl MeasureSync {
    /// Identifiant MusicXML de la mesure pour une partie : `p{part}-m{measure}`.
    ///
    /// Délègue à [`mscz2xml::model::measure_xml_id`] ; `measure` est l'indice
    /// de la mesure dans `Score::measures` (base zéro), pas le numéro affiché
    /// ni l'indice du passage.
    pub fn xml_id(&self, part: usize) -> String {
        measure_xml_id(part, self.measure)
    }
}

/// Synchronisation d'une attaque de note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSync {
    /// Tick d'exécution de l'attaque.
    pub tick: i64,
    /// Temps de l'attaque, en microsecondes.
    pub time_us: i64,
    /// Durée entre l'attaque et l'extinction, en ticks.
    pub duration_ticks: i64,
    /// Indice de la mesure dans `Score::measures` (base zéro), identique à
    /// [`MeasureSync::measure`].
    pub measure: usize,
    /// Indice de la portée dans `Score::staves`.
    pub staff: u32,
    /// Voix de la portée (0 à 3).
    pub voice: u8,
    /// Rang du `ChordRest` dans la voix et la mesure, silences compris.
    pub chord: usize,
    /// Rang de la note dans l'accord (ou l'ornement).
    pub note: usize,
    /// Hauteur MIDI de la note (0 à 127).
    pub pitch: u8,
    /// Numéro de passage du segment de reprise.
    pub pass: i32,
    /// Vrai pour une note d'ornement.
    pub grace: bool,
    /// Position de l'ornement dans `Chord::grace_chords` (`None` hors
    /// ornement) ; sert à l'identifiant exact.
    pub grace_index: Option<usize>,
    /// Vrai quand l'ornement précède l'accord principal.
    pub grace_before: bool,
    /// Vrai pour une attaque de trémolo.
    pub tremolo: bool,
}

impl NoteSync {
    /// Identifiant MusicXML de la note, identique à celui de l'export.
    ///
    /// Délègue à [`mscz2xml::model::note_xml_id`] pour une note ou un silence
    /// et à [`mscz2xml::model::grace_note_xml_id`] pour un ornement (position
    /// dans `Chord::grace_chords` et côté avant/après).
    pub fn xml_id(&self) -> String {
        match self.grace_index {
            Some(index) => grace_note_xml_id(
                self.measure,
                self.staff,
                self.voice,
                self.chord,
                index,
                self.note,
                self.grace_before,
            ),
            None => note_xml_id(self.measure, self.staff, self.voice, self.chord, self.note),
        }
    }
}

/// Synchronisation d'une pause de section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PauseSync {
    /// Tick d'exécution du début de la pause.
    pub tick: i64,
    /// Tick d'exécution de la fin de la pause (exclu).
    pub end_tick: i64,
    /// Temps du début de la pause, en microsecondes.
    pub time_us: i64,
    /// Temps de la fin de la pause, en microsecondes.
    pub end_time_us: i64,
    /// Durée de la pause, en secondes.
    pub seconds: f64,
}

/// Carte des temps de la performance MIDI.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timemap {
    /// Division MIDI : unités par noire ([`DIVISION`]).
    pub division: u32,
    /// Nombre total de ticks de la performance, pauses comprises.
    pub ticks: i64,
    /// Changements de tempo, triés par tick croissant.
    pub tempo: Vec<TempoChange>,
    /// Mesures jouées, dans l'ordre d'exécution.
    pub measures: Vec<MeasureSync>,
    /// Attaques de notes, dans l'ordre du rendu.
    pub notes: Vec<NoteSync>,
    /// Pauses de section insérées.
    pub pauses: Vec<PauseSync>,
}

impl Timemap {
    /// Temps en microsecondes d'un tick d'exécution.
    ///
    /// Intégration entière du tempo : `Σ delta_ticks × micros_per_quarter /
    /// DIVISION`, accumulée en [`i64`]. Au-delà du dernier changement de
    /// tempo, le dernier tempo en vigueur est prolongé.
    pub fn time_us_at(&self, tick: i64) -> i64 {
        time_us_at(&self.tempo, tick)
    }

    /// Mesure jouée qui contient un tick d'exécution.
    ///
    /// Renvoie `None` dans une pause de section (le tick tombe entre deux
    /// mesures) et hors de la performance.
    pub fn measure_at(&self, tick: i64) -> Option<&MeasureSync> {
        let index = self
            .measures
            .partition_point(|measure| measure.tick <= tick);
        let measure = self.measures.get(index.checked_sub(1)?)?;
        (tick < measure.end_tick).then_some(measure)
    }
}

/// Intégration entière du tempo sur une suite de changements triés.
fn time_us_at(tempo: &[TempoChange], tick: i64) -> i64 {
    let Some(first) = tempo.first() else {
        return 0;
    };
    let index = tempo.partition_point(|change| change.tick <= tick);
    if index == 0 {
        // Avant le premier changement : on prolonge le premier tempo en
        // arrière (cas théorique, `tempo_events` fournit toujours le tick 0).
        return first.time_us
            - (first.tick - tick) * i64::from(first.micros_per_quarter) / i64::from(DIVISION);
    }
    let change = &tempo[index - 1];
    change.time_us
        + (tick - change.tick) * i64::from(change.micros_per_quarter) / i64::from(DIVISION)
}

/// Attaque de note à enregistrer dans la carte.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Attack {
    /// Tick d'exécution de l'attaque.
    pub tick: i64,
    /// Durée entre l'attaque et l'extinction, en ticks.
    pub duration_ticks: i64,
    /// Indice de la mesure dans `Score::measures`.
    pub measure: usize,
    /// Indice de la portée.
    pub staff: u32,
    /// Voix de la portée.
    pub voice: u8,
    /// Rang du `ChordRest` dans la voix et la mesure.
    pub chord: usize,
    /// Rang de la note dans l'accord.
    pub note: usize,
    /// Hauteur MIDI.
    pub pitch: u8,
    /// Numéro de passage du segment.
    pub pass: i32,
    /// Vrai pour une note d'ornement.
    pub grace: bool,
    /// Position de l'ornement dans `Chord::grace_chords` (`None` hors
    /// ornement) ; sert à l'identifiant exact.
    pub grace_index: Option<usize>,
    /// Vrai quand l'ornement précède l'accord principal.
    pub grace_before: bool,
    /// Vrai pour une attaque de trémolo.
    pub tremolo: bool,
}

/// Mesure déjà datée lors du rendu, en attente de ses temps définitifs.
#[derive(Clone, Copy, Debug)]
struct PendingMeasure {
    tick: i64,
    end_tick: i64,
    measure: usize,
    segment: usize,
    pass: i32,
    pause_before: bool,
}

/// Constructeur de la carte, rempli au fil du rendu.
///
/// Les attaques sont enregistrées avec leur tick d'exécution (déjà décalé par
/// les pauses) ; les temps sont calculés en une fois dans [`Builder::finish`],
/// quand la table des tempos est complète.
pub(crate) struct Builder<'a> {
    score: &'a Score,
    repeat_list: &'a RepeatList,
    pause_map: &'a PauseMap,
    tempo: Vec<TempoChange>,
    pending_measures: Vec<PendingMeasure>,
    notes: Vec<NoteSync>,
    /// Faux quand l'appelant ne veut que les octets MIDI : ni mesure ni
    /// attaque n'est alors enregistrée, `finish` renvoie une carte vide.
    capture: bool,
}

impl<'a> Builder<'a> {
    /// Crée le constructeur avec la table des tempos de la performance
    /// (`(tick d'exécution, microsecondes par noire)`), triée par tick.
    pub(crate) fn new(
        score: &'a Score,
        repeat_list: &'a RepeatList,
        pause_map: &'a PauseMap,
        tempo: &[(i64, u32)],
    ) -> Self {
        let mut changes = Vec::with_capacity(tempo.len());
        let mut time_us = 0i64;
        let mut previous_tick = 0i64;
        let mut previous_micros = 500_000u32;
        for (tick, micros) in tempo {
            if *tick > previous_tick {
                time_us +=
                    (*tick - previous_tick) * i64::from(previous_micros) / i64::from(DIVISION);
            }
            changes.push(TempoChange {
                tick: *tick,
                time_us,
                micros_per_quarter: *micros,
            });
            previous_tick = *tick;
            previous_micros = *micros;
        }
        Self {
            score,
            repeat_list,
            pause_map,
            tempo: changes,
            pending_measures: Vec::new(),
            notes: Vec::new(),
            capture: true,
        }
    }

    /// Constructeur inactif : la carte n'est pas construite (`write_midi`).
    pub(crate) fn disabled(
        score: &'a Score,
        repeat_list: &'a RepeatList,
        pause_map: &'a PauseMap,
    ) -> Self {
        Self {
            score,
            repeat_list,
            pause_map,
            tempo: Vec::new(),
            pending_measures: Vec::new(),
            notes: Vec::new(),
            capture: false,
        }
    }

    /// Enregistre une mesure jouée, avec ses bornes déjà calculées au rendu.
    ///
    /// `tick` et `end_tick` viennent du moment où la mesure est parcourue
    /// (décalage de segment et pauses compris) : `finish` ne les recalcule
    /// pas, il se contente de les vérifier en debug contre `RepeatList` et
    /// `PauseMap`.
    pub(crate) fn measure(
        &mut self,
        tick: i64,
        end_tick: i64,
        measure: usize,
        segment: usize,
        pass: i32,
        pause_before: bool,
    ) {
        if !self.capture {
            return;
        }
        self.pending_measures.push(PendingMeasure {
            tick,
            end_tick,
            measure,
            segment,
            pass,
            pause_before,
        });
    }

    /// Enregistre une attaque de note, au même endroit que le `NoteOn`.
    pub(crate) fn attack(&mut self, attack: Attack) {
        if !self.capture {
            return;
        }
        self.notes.push(NoteSync {
            tick: attack.tick,
            time_us: 0,
            // Une attaque d'ornement placée avant le tout début peut avoir une
            // fin antérieure à son début ; la carte ne porte jamais de durée
            // négative (le MIDI, lui, garde `NoteOff >= NoteOn + 1`).
            duration_ticks: attack.duration_ticks.max(0),
            measure: attack.measure,
            staff: attack.staff,
            voice: attack.voice,
            chord: attack.chord,
            note: attack.note,
            pitch: attack.pitch,
            pass: attack.pass,
            grace: attack.grace,
            grace_index: attack.grace_index,
            grace_before: attack.grace_before,
            tremolo: attack.tremolo,
        });
    }

    /// Convertit les mesures et les temps, et produit la carte définitive.
    pub(crate) fn finish(self) -> Timemap {
        if !self.capture {
            return Timemap::default();
        }
        let mut measures = Vec::with_capacity(self.pending_measures.len());
        for pending in &self.pending_measures {
            // Les bornes ont été figées au rendu ; on vérifie en debug que le
            // recalcul depuis `RepeatList`/`PauseMap` donne la même chose.
            #[cfg(debug_assertions)]
            {
                let segment = &self.repeat_list.segments[pending.segment];
                let offset = segment.utick - segment.tick;
                let start_utick = self.repeat_list.measure_start(pending.measure) + offset;
                let end_utick = self.repeat_list.measure_end(pending.measure) + offset;
                debug_assert_eq!(
                    pending.tick,
                    self.pause_map.tick_with_pauses(start_utick),
                    "borne de début de mesure incohérente : {:?}",
                    pending
                );
                debug_assert_eq!(
                    pending.end_tick,
                    self.pause_map.tick_with_pauses_before(end_utick),
                    "borne de fin de mesure incohérente : {:?}",
                    pending
                );
            }
            measures.push(MeasureSync {
                tick: pending.tick,
                end_tick: pending.end_tick,
                time_us: 0,
                end_time_us: 0,
                measure: pending.measure,
                number: self.score.measures[pending.measure].number,
                segment: pending.segment,
                pass: pending.pass,
                pause_before: pending.pause_before,
            });
        }

        let inserted: i64 = self
            .pause_map
            .spans()
            .iter()
            .map(|span| span.inserted)
            .sum();
        let pauses: Vec<PauseSync> = self
            .pause_map
            .spans()
            .iter()
            .map(|span| PauseSync {
                tick: span.paused_utick,
                end_tick: span.paused_utick + span.inserted,
                time_us: 0,
                end_time_us: 0,
                seconds: span.seconds,
            })
            .collect();

        // Ordre déterministe : par tick, puis par clé stable. Le tri est fait
        // ici (et non côté JS) pour que la carte Rust et la surface JS aient
        // le même ordre, et que `tick` soit non décroissant.
        let mut notes = self.notes;
        notes.sort_by_key(|note| {
            (
                note.tick,
                note.measure,
                note.staff,
                note.voice,
                note.chord,
                note.note,
                note.pitch,
            )
        });
        let mut timemap = Timemap {
            division: DIVISION,
            ticks: self.repeat_list.ticks() + inserted,
            tempo: self.tempo,
            measures,
            notes,
            pauses,
        };
        let tempo = &timemap.tempo;
        for measure in &mut timemap.measures {
            measure.time_us = time_us_at(tempo, measure.tick);
            measure.end_time_us = time_us_at(tempo, measure.end_tick);
        }
        for note in &mut timemap.notes {
            note.time_us = time_us_at(tempo, note.tick);
        }
        for pause in &mut timemap.pauses {
            pause.time_us = time_us_at(tempo, pause.tick);
            pause.end_time_us = time_us_at(tempo, pause.end_tick);
        }
        timemap
    }
}
