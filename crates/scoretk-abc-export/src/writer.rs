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
//! - Les hauteurs sont écrites en **hauteur écrite** (`pitch - transpose`),
//!   comme l'export MusicXML (`pitch2xml`) : une partition pour instrument
//!   transpositeur garde ainsi la même armure et les mêmes doigtés qu'à
//!   l'écran. ABC n'ayant pas de champ de transposition standard, le lecteur
//!   la conserve comme hauteur concert : l'aller-retour ne restitue pas
//!   `<transpose>` (limite documentée).
//! - Une mesure par ligne, pour des instantanés stables et des diffs lisibles.
//! - Les altérations explicites ne sont écrites que lorsqu'elles diffèrent de
//!   l'armure ou de l'état de la mesure, ABC mémorisant les altérations
//!   jusqu'à la barre suivante.

use scoretk_core::model::{
    Annotation, Clef, ClefType, Key, KeySig, Score, SegmentItem, TimeSig, VOICES,
};

use crate::voice;
use scoretk_abc_common::pitch;
use scoretk_abc_common::{Error, Result};

/// Voix ABC : une voix du modèle, ou la voix 0 muette d'une portée vide.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VoiceSpec {
    /// Portée du modèle.
    pub staff: usize,
    /// Voix du modèle (0 à [`VOICES`] - 1).
    pub voice: usize,
    /// Numéro ABC (base un).
    pub number: usize,
}

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
        // ABC 2.1 §3.1.2 : un `T:` répété est un titre alternatif. C'est le
        // champ standard qui porte le sous-titre, sans directive privée.
        if !self.score.metadata.subtitle.trim().is_empty() {
            out.push_str(&format!(
                "T:{}\n",
                scoretk_abc_common::text::escape_annotation(
                    &self.score.metadata.subtitle.replace(['\n', '\r'], " ")
                )
            ));
        }
        if !self.score.metadata.composer.trim().is_empty() {
            out.push_str(&format!(
                "C:{}\n",
                scoretk_abc_common::text::escape_annotation(&self.score.metadata.composer)
            ));
        }
        // ABC 2.1 §3.1 : `W:` est le champ de texte imprimé (contrairement à
        // `w:`, qui aligne des paroles sur les notes). C'est la contrepartie
        // des `<credit>` du modèle ; un champ par crédit, dans l'ordre.
        for credit in &self.score.engraving.credits {
            let text = credit.text.plain().replace(['\n', '\r'], " ");
            out.push_str(&format!(
                "W:{}\n",
                scoretk_abc_common::text::escape_annotation(&text)
            ));
        }
        // Champs standard : `T:` (titre et sous-titre, §3.1.2), `C:` (§3.1.3),
        // `Z:abc-copyright` (convention explicite de §3.1.10) et `N:`
        // (§3.1.11, notes générales, seul champ libre pour un numéro
        // d'œuvre). La directive privée (ABC 2.1 §11) ne porte plus que les
        // métadonnées qui n'ont aucun équivalent standard.
        if !self.score.metadata.copyright.trim().is_empty() {
            out.push_str(&format!(
                "Z:abc-copyright {}\n",
                scoretk_abc_common::text::escape_annotation(
                    &self.score.metadata.copyright.replace(['\n', '\r'], " ")
                )
            ));
        }
        if !self.score.metadata.work_number.trim().is_empty() {
            out.push_str(&format!(
                "N:{}\n",
                scoretk_abc_common::text::escape_annotation(
                    &self.score.metadata.work_number.replace(['\n', '\r'], " ")
                )
            ));
        }
        // ABC 2.1 n'a pas de cadre (`tbox`) mais le champ `H:` (§3.1.13)
        // transporte textes historiques multi-lignes : c'est le support
        // standard le plus proche pour les textes de cadre du modèle.
        for (index, tbox) in self.score.engraving.tbox_texts.iter().enumerate() {
            let text = tbox.text.plain().replace(['\n', '\r'], " ");
            let text = scoretk_abc_common::text::escape_annotation(&text);
            if !text.is_empty() {
                out.push_str(&format!("H:{text}\n"));
            }
            let _ = index;
        }
        for (name, value) in [
            ("movement_title", &self.score.metadata.movement_title),
            ("movement_number", &self.score.metadata.movement_number),
            ("lyricist", &self.score.metadata.lyricist),
            ("poet", &self.score.metadata.poet),
            ("translator", &self.score.metadata.translator),
            ("arranger", &self.score.metadata.arranger),
            ("source", &self.score.metadata.source),
        ] {
            if value.trim().is_empty() {
                continue;
            }
            let text = value.replace(['\n', '\r'], " ");
            out.push_str(&format!(
                "%%scoretk meta {name} {}\n",
                scoretk_abc_common::text::escape_annotation(&text)
            ));
        }
        // Textes de cadre : ABC 2.1 §11, directive privée (ils n'ont pas de
        // champ dédié dans la norme, contrairement aux crédits `W:`).
        for (index, tbox) in self.score.engraving.tbox_texts.iter().enumerate() {
            let text = tbox.text.plain().replace(['\n', '\r'], " ");
            out.push_str(&format!(
                "%%scoretk tbox {index} {}\n",
                scoretk_abc_common::text::escape_annotation(&text)
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
        let voices = self.voices();
        let mut key_line = format!("K:{}", pitch::key_field(&key));
        if voices.len() == 1 {
            let spec = voices[0];
            if let Some(clef) = self
                .initial_clef(spec.staff)
                .and_then(|clef| pitch::clef_name(&Clef::new(clef)))
            {
                key_line.push_str(&format!(" clef={clef}"));
            }
        }
        out.push_str(&key_line);
        out.push('\n');

        // abc-standard 2.1 §6.2 / §11.2 : `%%MIDI program` est le numéro GM
        // (1–128) et `%%MIDI channel` le canal (1–16), tandis que le modèle
        // stocke des indices base zéro comme le wire MIDI.
        for part in &self.score.parts {
            if let Some(program) = part.instrument.midi_program {
                out.push_str(&format!("%%MIDI program {}\n", program.saturating_add(1)));
            }
            if let Some(channel) = part.instrument.midi_channel {
                out.push_str(&format!("%%MIDI channel {}\n", channel.saturating_add(1)));
            }
        }

        if voices.len() > 1 {
            if let Some(score_line) = self.score_line(&voices) {
                out.push_str(&score_line);
                out.push('\n');
            }
        }

        for spec in &voices {
            voice::render_voice(self.score, *spec, &key, &mut out);
        }

        Ok(out)
    }

    /// Voix ABC à écrire : les voix 0 à `n` de chaque portée qui ont du
    /// contenu, plus la voix 0 d'une portée vide.
    fn voices(&self) -> Vec<VoiceSpec> {
        let mut voices = Vec::new();
        let mut number = 1usize;
        for staff in 0..self.score.staves.len() {
            let has_content = |voice| voice::voice_has_content(self.score, staff, voice);
            let any = (0..VOICES).any(has_content);
            if !any {
                voices.push(VoiceSpec {
                    staff,
                    voice: 0,
                    number,
                });
                number += 1;
                continue;
            }
            for voice in 0..VOICES {
                if has_content(voice) {
                    voices.push(VoiceSpec {
                        staff,
                        voice,
                        number,
                    });
                    number += 1;
                }
            }
        }
        voices
    }

    /// Titre : titre d'œuvre, à défaut titre de mouvement.
    fn title(&self) -> String {
        let title = if self.score.metadata.work_title.trim().is_empty() {
            &self.score.metadata.movement_title
        } else {
            &self.score.metadata.work_title
        };
        scoretk_abc_common::text::escape_annotation(title)
    }

    /// Indication de mesure au début de la partition, ou 4/4.
    fn initial_time_signature(&self) -> TimeSig {
        initial_time(self.score)
    }

    /// Armure au début de la partition, ou do majeur.
    fn initial_key_signature(&self) -> KeySig {
        measure_context(self.score, 0, Some(0))
            .1
            .unwrap_or(KeySig::new(Key::C))
    }

    /// Clef d'une portée au début de la partition.
    fn initial_clef(&self, staff: usize) -> Option<ClefType> {
        initial_clef(self.score, staff)
    }

    /// `%%score` des regroupements de portées.
    ///
    /// La grammaire est celle d'`abc2xml` : `(1 2)` regroupe des voix sur une
    /// même portée, `{…}` encadre une partie à accolade et `[…]` une partie à
    /// crochet. Un numéro seul est une portée isolée.
    fn score_line(&self, voices: &[VoiceSpec]) -> Option<String> {
        if self.score.parts.is_empty() {
            return None;
        }
        let mut groups = Vec::new();
        for part in &self.score.parts {
            if part.staves.is_empty() {
                continue;
            }
            let mut staves = Vec::new();
            for staff in &part.staves {
                let numbers: Vec<String> = voices
                    .iter()
                    .filter(|spec| spec.staff == staff.index())
                    .map(|spec| spec.number.to_string())
                    .collect();
                if numbers.is_empty() {
                    continue;
                }
                staves.push(if numbers.len() == 1 {
                    numbers[0].clone()
                } else {
                    format!("({})", numbers.join(" "))
                });
            }
            if staves.is_empty() {
                continue;
            }
            if staves.len() == 1 {
                groups.push(staves[0].clone());
                continue;
            }
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
            let delimiters = match bracket {
                1 => Some(('{', '}')),
                0 | 3 => Some(('[', ']')),
                // Sans accolade ni crochet, écrire des espaces comme
                // délimiteurs produisait `%%score  1 2 ` (doubles espaces et
                // espace final) et rendait l'aller-retour instable.
                _ => None,
            };
            let joined = staves.join(" ");
            groups.push(match delimiters {
                Some((open, close)) => format!("{open}{joined}{close}"),
                None => joined,
            });
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
                        // `Q:` porte des battements par minute. Écrire le
                        // nombre exact (et non un entier arrondi) garde la
                        // précision du tempo du modèle : un tempo de 80 BPM
                        // enregistré 1,333333 bps doit rester 79,99998 BPM.
                        let bpm = format!("{:.6}", bps * 60.0);
                        let bpm = bpm.trim_end_matches('0').trim_end_matches('.');
                        if bpm.parse::<f64>().is_ok_and(|value| value <= 0.0) {
                            return None;
                        }
                        let text = scoretk_abc_common::text::escape_annotation(tempo.text.plain());
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

/// Indication de mesure au début de la partition, ou 4/4.
pub(crate) fn initial_time(score: &Score) -> TimeSig {
    measure_context(score, 0, Some(0))
        .2
        .filter(TimeSig::is_valid)
        .unwrap_or_else(|| TimeSig::new(4, 4))
}

/// Portée de référence d'une partie pour l'armure et la mesure.
///
/// ABC n'a qu'une armure et qu'une indication de mesure par morceau : les
/// indications locales de MuseScore sont ramenées à celles de la première
/// portée de la partie.
pub(crate) fn context_staff(score: &Score, staff: usize) -> usize {
    score
        .staves
        .get(staff)
        .and_then(|staff| score.parts.get(staff.part.index()))
        .and_then(|part| part.staves.first())
        .map(|staff| staff.index())
        .unwrap_or(0)
}

/// Clef d'une portée au début de la partition, pour les en-têtes `V:`.
pub(crate) fn initial_clef(score: &Score, staff: usize) -> Option<ClefType> {
    measure_context(score, 0, Some(staff))
        .0
        .map(|clef| clef.transposed)
        .or_else(|| {
            score
                .staves
                .get(staff)
                .map(|staff| staff.default_clef.transposed)
        })
}

/// Clef et armure portées par les segments au début d'une mesure.
///
/// Renvoie `(clef, armure, mesure)` : les trois changements de contexte que
/// l'écriture ABC place en tête de mesure.
pub(crate) fn measure_context(
    score: &Score,
    measure_index: usize,
    staff: Option<usize>,
) -> (Option<Clef>, Option<KeySig>, Option<TimeSig>) {
    let mut clef = None;
    let mut key = None;
    let mut time = None;
    let Some(measure) = score.measures.get(measure_index) else {
        return (None, None, None);
    };
    // MuseScore range les changements de contexte au tick de la barre
    // **finale** : ils sont donc dans les segments de la mesure précédente,
    // pas au début de celle où ils s'appliquent (`Sancta_Maria`, mesure 49).
    // On lit les deux emplacements, exactement comme `xml2abc.py::doAttr`
    // lit tout `<attributes>` avant les notes qu'il modifie (lignes 1279-1331).
    let mut segments: Vec<&scoretk_core::model::Segment> = measure.segments.iter().collect();
    if let Some(previous) = measure_index
        .checked_sub(1)
        .and_then(|i| score.measures.get(i))
    {
        segments.extend(
            previous
                .segments
                .iter()
                .filter(|segment| segment.tick == measure.start_tick),
        );
    }
    for segment in segments {
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
            if staff.is_some_and(|staff| track.staff().index() != staff) {
                continue;
            }
            match item {
                SegmentItem::Clef(value) => {
                    clef.get_or_insert(*value);
                }
                SegmentItem::KeySig(value) => {
                    key.get_or_insert_with(|| value.clone());
                }
                SegmentItem::TimeSig(value) => {
                    time.get_or_insert_with(|| value.clone());
                }
                _ => {}
            }
        }
    }
    (clef, key, time)
}
