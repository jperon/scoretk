// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/volta.h`, `engraving/dom/measure.h` et de la
// lecture des barres de reprise par `ExportMusicXml::barlineRight` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026 : structure `\repeat volta` / `\alternative` à
// partir des barres de reprise et des volatas.
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

//! Structure des reprises : `\repeat volta` et `\alternative`.
//!
//! MuseScore marque une reprise par une barre de début (`<startRepeat>`) et
//! une barre de fin (`<endRepeat>N</endRepeat>`, N lectures), et décrit les
//! premier et second endings par des `Volta` (`engraving/dom/volta.h`).
//! LilyPond exprime la même chose par
//!
//! ```text
//! \repeat volta 2 { corps }
//! \alternative { { premier ending } { second ending } }
//! ```
//!
//! où le corps s'arrête au début du premier ending et où le second ending
//! commence après la barre de fin de reprise. [`build`] produit, pour chaque
//! mesure, les jetons de structure à écrire avant et après elle ; toutes les
//! voix d'une portée doivent les recevoir pour rester alignées.

use scoretk_core::model::{Measure, Score, SpannerType, Tick};

/// Jeton de structure placé avant ou après une mesure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StructToken {
    /// `\repeat volta n {`
    OpenRepeat(u8),
    /// `}` : fin du corps de la reprise.
    CloseRepeatBody,
    /// `\alternative {`
    OpenAlternative,
    /// `{` : début d'un ending.
    OpenEnding,
    /// `}` : fin d'un ending.
    CloseEnding,
    /// `}` : fin du bloc `\alternative`.
    CloseAlternative,
}

/// Structure de reprise d'une partition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Structure {
    /// Jetons à écrire avant chaque mesure.
    pub before: Vec<Vec<StructToken>>,
    /// Jetons à écrire après chaque mesure.
    pub after: Vec<Vec<StructToken>>,
}

impl Structure {
    /// Jetons avant une mesure, vide si l'indice sort de la partition.
    pub(crate) fn before_measure(&self, index: usize) -> &[StructToken] {
        self.before.get(index).map_or(&[], Vec::as_slice)
    }

    /// Jetons après une mesure, vide si l'indice sort de la partition.
    pub(crate) fn after_measure(&self, index: usize) -> &[StructToken] {
        self.after.get(index).map_or(&[], Vec::as_slice)
    }
}

/// Volta réduite à son étendue de mesures.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Volta {
    start: usize,
    end: usize,
}

/// Construit la structure de reprise d'une partition.
pub(crate) fn build(score: &Score) -> Structure {
    let count = score.measures.len();
    let mut structure = Structure {
        before: vec![Vec::new(); count],
        after: vec![Vec::new(); count],
    };
    if count == 0 {
        return structure;
    }

    let voltas = collect_voltas(score);
    // Une barre de fin de reprise (`:|`) sans barre de début (`|:`) n'est pas
    // une reprise : LilyPond joue la musique une seule fois (`lily/parser.yy`,
    // `bar_line`). Sans cette garde, `section_start` vaudrait 0 et le writer
    // inventerait un `\repeat volta 2` depuis la première mesure.
    let mut section_start: Option<usize> = None;
    let mut index = 0usize;

    while index < count {
        let measure = &score.measures[index];
        if measure.repeat_start() {
            section_start = Some(index);
        }
        if measure.repeat_end() {
            if let Some(start) = section_start {
                let end_repeat_starts = measure.start_bar_line.is_repeat_start();
                let next_index =
                    build_section(&mut structure, &score.measures, &voltas, start, index);
                index = next_index.max(index + 1);
                section_start = Some(if end_repeat_starts { index - 1 } else { index });
                continue;
            }
        }
        index += 1;
    }

    structure
}

/// Remplit les jetons d'une section de reprise `start..=end`.
///
/// Renvoie l'indice de la première mesure qui suit la section, alternatives
/// comprises.
fn build_section(
    structure: &mut Structure,
    measures: &[Measure],
    voltas: &[Volta],
    start: usize,
    end: usize,
) -> usize {
    // `<endRepeat>N</endRepeat>` compte les lectures ; deux par défaut. Une
    // reprise jouée une seule fois n'est pas une reprise : la musique est
    // écrite une fois, sans `\repeat`.
    let plays = match measures[end].repeat_count {
        0 => 2,
        1 => return end + 1,
        count => count,
    };

    // Dernier ending : la volta qui couvre la mesure de fin de reprise.
    let first_alt = voltas
        .iter()
        .find(|volta| volta.start <= end && volta.end >= end)
        .cloned();

    // Endings suivants : volatas qui commencent juste après la barre de fin.
    let mut alternatives: Vec<Volta> = Vec::new();
    let mut cursor = end + 1;
    while let Some(volta) = voltas.iter().find(|volta| volta.start == cursor) {
        alternatives.push(volta.clone());
        cursor = volta.end + 1;
    }

    let body_follows = first_alt.as_ref().is_some_and(|volta| volta.start > start);
    let mut endings: Vec<Volta> = Vec::new();
    if let Some(first) = &first_alt {
        if body_follows {
            endings.push(first.clone());
        }
    }
    endings.extend(alternatives);

    if endings.len() < 2 {
        // Aucun second ending : le corps couvre toute la section, les volatas
        // ne changent que l'affichage des barres.
        structure.before[start].push(StructToken::OpenRepeat(plays));
        structure.after[end].push(StructToken::CloseRepeatBody);
        return end + 1;
    }

    // Quand le premier ending occupe toute la section, il tient lieu de corps
    // et les endings restants suivent.
    let body_end = if body_follows {
        first_alt.as_ref().map_or(end, |volta| volta.start - 1)
    } else {
        end
    };

    structure.before[start].push(StructToken::OpenRepeat(plays));
    structure.after[body_end].push(StructToken::CloseRepeatBody);
    structure.after[body_end].push(StructToken::OpenAlternative);
    structure.after[body_end].push(StructToken::OpenEnding);

    let mut last_end = body_end;
    for (index, ending) in endings.iter().enumerate() {
        last_end = ending.end;
        if index + 1 < endings.len() {
            structure.after[last_end].push(StructToken::CloseEnding);
            structure.after[last_end].push(StructToken::OpenEnding);
        }
    }
    structure.after[last_end].push(StructToken::CloseEnding);
    structure.after[last_end].push(StructToken::CloseAlternative);

    last_end + 1
}

/// Voltas de la partition, réduites à leurs étendues de mesures.
fn collect_voltas(score: &Score) -> Vec<Volta> {
    let mut voltas: Vec<Volta> = score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Volta))
        .map(|spanner| Volta {
            start: measure_at(&score.measures, spanner.start.tick),
            end: measure_before(&score.measures, spanner.end.tick),
        })
        .filter(|volta| volta.start <= volta.end)
        .collect();
    voltas.sort_by_key(|volta| (volta.start, volta.end));
    voltas.dedup();
    voltas
}

/// Indice de la mesure qui contient un tick (dernière mesure commençant au
/// plus tard à ce tick).
pub(crate) fn measure_at(measures: &[Measure], tick: Tick) -> usize {
    measures
        .partition_point(|measure| measure.start_tick <= tick)
        .saturating_sub(1)
}

/// Indice de la dernière mesure commençant strictement avant un tick.
fn measure_before(measures: &[Measure], tick: Tick) -> usize {
    measures
        .partition_point(|measure| measure.start_tick < tick)
        .saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        BarLineType, Fraction, Part, SpannerAnchor, SpannerType, Track, VoltaInfo,
    };

    fn partition(mesures: usize, reprises: &[(usize, usize, u8)]) -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff(part);
        for index in 0..mesures {
            let mut measure =
                Measure::new(index, Fraction::from_integer(index as i64), Fraction::ONE);
            for (start, end, count) in reprises {
                if index == *start {
                    measure.start_bar_line = BarLineType::StartRepeat;
                }
                if index == *end {
                    measure.end_bar_line = BarLineType::EndRepeat;
                    measure.repeat_count = *count;
                }
            }
            score.add_measure(measure);
        }
        score
    }

    /// Une barre de fin de reprise sans barre de début n'est pas une
    /// reprise (`lily/parser.yy` `bar_line` : `:|` seul ne joue qu'une fois).
    /// Le writer ne doit pas inventer un `\repeat volta 2` depuis la mesure 1.
    #[test]
    fn fin_de_reprise_sans_debut_ignoree() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff(part);
        for index in 0..4 {
            let mut measure =
                Measure::new(index, Fraction::from_integer(index as i64), Fraction::ONE);
            if index == 3 {
                measure.end_bar_line = BarLineType::EndRepeat;
                measure.repeat_count = 2;
            }
            score.add_measure(measure);
        }
        let structure = build(&score);
        assert!(
            structure.before_measure(0).is_empty(),
            "aucune reprise ne doit être ouverte : {:?}",
            structure.before_measure(0)
        );
        assert!(structure.after_measure(3).is_empty());
    }

    #[test]
    fn reprise_simple() {
        let score = partition(4, &[(1, 3, 2)]);
        let structure = build(&score);
        assert_eq!(structure.before_measure(1), &[StructToken::OpenRepeat(2)]);
        assert_eq!(structure.after_measure(3), &[StructToken::CloseRepeatBody]);
        assert!(structure.before_measure(0).is_empty());
    }

    #[test]
    fn reprise_avec_deux_endings() {
        let mut score = partition(4, &[(0, 2, 2)]);
        let mut premiere = scoretk_core::model::Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(Fraction::from_integer(1), Track::from_raw(0)),
            SpannerAnchor::new(Fraction::from_integer(3), Track::from_raw(0)),
        );
        premiere.volta = Some(VoltaInfo {
            endings: vec![1],
            closed: true,
        });
        score.add_spanner(premiere);
        let mut seconde = scoretk_core::model::Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(Fraction::from_integer(3), Track::from_raw(0)),
            SpannerAnchor::new(Fraction::from_integer(4), Track::from_raw(0)),
        );
        seconde.volta = Some(VoltaInfo {
            endings: vec![2],
            closed: true,
        });
        score.add_spanner(seconde);
        let structure = build(&score);
        assert_eq!(structure.before_measure(0), &[StructToken::OpenRepeat(2)]);
        assert_eq!(
            structure.after_measure(0),
            &[
                StructToken::CloseRepeatBody,
                StructToken::OpenAlternative,
                StructToken::OpenEnding
            ]
        );
        assert_eq!(
            structure.after_measure(2),
            &[StructToken::CloseEnding, StructToken::OpenEnding]
        );
        assert_eq!(
            structure.after_measure(3),
            &[StructToken::CloseEnding, StructToken::CloseAlternative]
        );
    }
}
