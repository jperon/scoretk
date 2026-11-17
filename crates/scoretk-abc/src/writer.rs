// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// L'écriture suit la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) et le comportement de
// `xml2abc.py` de Willem Vree (LGPL) ; aucune ligne de ce script n'est copiée.
// La disposition des voix et le choix de `L:` sont propres à ce portage.
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

//! Écriture d'une partition ABC.
//!
//! La partition est écrite en une voix ABC par portée (`V:`), chaque mesure
//! sur une ligne. Les champs d'en-tête reprennent les métadonnées du modèle
//! (`X:`, `T:`, `C:`, `M:`, `L:`, `Q:`, `K:`) et `%%score` conserve les
//! regroupements de portées. Le corps de chaque voix est produit par
//! [`crate::voice`].
//!
//! # Choix d'écriture
//!
//! - `L:1/8` est l'unité fixe : les multiplicateurs sont des entiers pour les
//!   valeurs courantes (`2`, `4`, `3`) et des fractions pour les autres
//!   (`3/2`, `2/3`), ce qui accepte exactement toute durée du modèle.
//! - Les hauteurs sont écrites en **hauteur concert** : ABC n'a pas de champ
//!   de transposition standard, et une partition transposée pour un instrument
//!   garderait sinon une armure fausse.
//! - Une mesure par ligne, pour des instantanés stables et des diffs lisibles.
//! - Les altérations explicites ne sont écrites que lorsqu'elles diffèrent de
//!   l'armure ou de l'état de la mesure, ABC mémorisant les altérations
//!   jusqu'à la barre suivante.

use mscz2xml::model::{
    Annotation, Clef, ClefType, Key, KeySig, Score, SegmentItem, TimeSig,
};

use crate::error::{Error, Result};
use crate::pitch;
use crate::voice;

/// Écrivain ABC.
pub(crate) struct Writer<'a> {
    score: &'a Score,
}

impl<'a> Writer<'a> {
    /// Construit l'écrivain.
    pub(crate) fn new(score: &'a Score) -> Self {
        Self { score }
    }

    /// Produit le document ABC complet.
    pub(crate) fn render(&self) -> Result<String> {
        if self.score.staves.is_empty() {
            return Err(Error::PartitionVide);
        }

        let mut out = String::new();
        out.push_str("%abc-2.1\n");
        out.push_str("X:1\n");
        out.push_str(&format!("T:{}\n", self.title()));
        if !self.score.metadata.composer.trim().is_empty() {
            out.push_str(&format!(
                "C:{}\n",
                crate::text::escape_annotation(&self.score.metadata.composer)
            ));
        }
        let time = self.initial_time_signature();
        out.push_str(&format!("M:{}/{}\n", time.numerator, time.denominator));
        out.push_str("L:1/8\n");
        if let Some(tempo) = self.tempo_field() {
            out.push_str(&tempo);
            out.push('\n');
        }
        let key = self.initial_key_signature();
        let mut key_line = format!("K:{}", pitch::key_field(&key));
        if self.score.staves.len() == 1 {
            if let Some(clef) = self
                .initial_clef(0)
                .and_then(|clef| pitch::clef_name(&Clef::new(clef)))
            {
                key_line.push_str(&format!(" clef={clef}"));
            }
        }
        out.push_str(&key_line);
        out.push('\n');

        if self.score.staves.len() > 1 {
            if let Some(score_line) = self.score_line() {
                out.push_str(&score_line);
                out.push('\n');
            }
        }

        for (index, _) in self.score.staves.iter().enumerate() {
            voice::render_voice(self.score, index, &key, &mut out);
        }

        Ok(out)
    }

    /// Titre : titre d'œuvre, à défaut titre de mouvement.
    fn title(&self) -> String {
        let title = if self.score.metadata.work_title.trim().is_empty() {
            &self.score.metadata.movement_title
        } else {
            &self.score.metadata.work_title
        };
        crate::text::escape_annotation(title)
    }

    /// Première indication de mesure de la partition, ou 4/4.
    fn initial_time_signature(&self) -> TimeSig {
        initial_time(self.score)
    }

    /// Première armure de la partition, ou do majeur.
    fn initial_key_signature(&self) -> KeySig {
        for measure in &self.score.measures {
            for segment in &measure.segments {
                if !segment.kind.is_key_sig_type() {
                    continue;
                }
                for (_, item) in segment.iter_elements() {
                    if let SegmentItem::KeySig(key) = item {
                        return key.clone();
                    }
                }
            }
        }
        KeySig::new(Key::C)
    }

    /// Première clef d'une portée.
    fn initial_clef(&self, staff: usize) -> Option<ClefType> {
        for measure in &self.score.measures {
            for segment in &measure.segments {
                if !segment.kind.is_clef_type() {
                    continue;
                }
                for (track, item) in segment.iter_elements() {
                    if track.staff().index() != staff {
                        continue;
                    }
                    if let SegmentItem::Clef(clef) = item {
                        return Some(clef.transposed);
                    }
                }
            }
        }
        self.score
            .staves
            .get(staff)
            .map(|staff| staff.default_clef.transposed)
    }

    /// `%%score` des regroupements de portées.
    fn score_line(&self) -> Option<String> {
        if self.score.parts.is_empty() {
            return None;
        }
        let mut groups = Vec::new();
        for part in &self.score.parts {
            if part.staves.is_empty() {
                continue;
            }
            let numbers: Vec<String> = part
                .staves
                .iter()
                .map(|staff| (staff.index() + 1).to_string())
                .collect();
            let line = if numbers.len() == 1 {
                numbers[0].clone()
            } else {
                let bracket = part
                    .staves
                    .iter()
                    .find_map(|staff| {
                        self.score
                            .staves
                            .get(staff.index())
                            .and_then(|staff| staff.brackets.first())
                    })
                    .map_or(-1, |bracket| i32::from(bracket.kind));
                let (open, close) = match bracket {
                    1 => ('(', ')'),
                    0 | 3 => ('[', ']'),
                    _ => (' ', ' '),
                };
                format!("{open}{}{close}", numbers.join(" "))
            };
            groups.push(line);
        }
        (!groups.is_empty()).then(|| format!("%%score {}", groups.join(" ")))
    }

    /// Champ `Q:` du premier tempo numérique.
    fn tempo_field(&self) -> Option<String> {
        for measure in &self.score.measures {
            for segment in &measure.segments {
                for annotation in &segment.annotations {
                    if let Annotation::Tempo(tempo) = &annotation.annotation {
                        let bps = tempo.beats_per_second?;
                        let bpm = (bps * 60.0).round() as i64;
                        if bpm <= 0 {
                            return None;
                        }
                        let text = crate::text::escape_annotation(tempo.text.plain());
                        return Some(if text.is_empty() {
                            format!("Q:1/4={bpm}")
                        } else {
                            format!("Q:\"{text}\" 1/4={bpm}")
                        });
                    }
                }
            }
        }
        None
    }
}

/// Première indication de mesure de la partition, ou 4/4.
pub(crate) fn initial_time(score: &Score) -> TimeSig {
    for measure in &score.measures {
        for segment in &measure.segments {
            if !segment.kind.is_time_sig_type() {
                continue;
            }
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::TimeSig(time) = item {
                    if time.is_valid() {
                        return time.clone();
                    }
                }
            }
        }
    }
    TimeSig::new(4, 4)
}

/// Clef initiale d'une portée, pour les en-têtes `V:`.
pub(crate) fn staff_clef(score: &Score, staff: usize) -> Option<Clef> {
    for measure in &score.measures {
        for segment in &measure.segments {
            if !segment.kind.is_clef_type() {
                continue;
            }
            for (track, item) in segment.iter_elements() {
                if track.staff().index() != staff {
                    continue;
                }
                if let SegmentItem::Clef(clef) = item {
                    return Some(*clef);
                }
            }
        }
    }
    score.staves.get(staff).map(|staff| staff.default_clef)
}

/// Clef et armure portées par les segments au début d'une mesure.
///
/// Renvoie `(clef, armure, mesure)` : les trois changements de contexte que
/// l'écriture ABC place en tête de mesure.
pub(crate) fn measure_context(
    score: &Score,
    measure_index: usize,
    staff: usize,
) -> (Option<Clef>, Option<KeySig>, Option<TimeSig>) {
    let mut clef = None;
    let mut key = None;
    let mut time = None;
    let Some(measure) = score.measures.get(measure_index) else {
        return (None, None, None);
    };
    for segment in &measure.segments {
        if segment.tick != measure.start_tick {
            continue;
        }
        if !(segment.kind.is_clef_type()
            || segment.kind.is_key_sig_type()
            || segment.kind.is_time_sig_type())
        {
            continue;
        }
        for (track, item) in segment.iter_elements() {
            if track.staff().index() != staff {
                continue;
            }
            match item {
                SegmentItem::Clef(value) => clef = Some(*value),
                SegmentItem::KeySig(value) => key = Some(value.clone()),
                SegmentItem::TimeSig(value) => time = Some(value.clone()),
                _ => {}
            }
        }
    }
    (clef, key, time)
}
