// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read302/read302.cpp` et `engraving/rw/read400/` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Lecteur des fichiers `.mscx` MuseScore 3.x (versions 3.00 à 3.02).
//!
//! Portage de `Read302` (`engraving/rw/read302/read302.cpp`) et des parties de
//! `read400` que ce lecteur réutilise : `staffrw.cpp`, `measurerw.cpp`,
//! `tread.cpp` et `connectorinforeader.cpp`. Le fichier est d'abord analysé en
//! un arbre XML (`xml`), puis lu en deux passes : parties et instruments, puis
//! musique portée par portée.
//!
//! # Périmètre
//!
//! Sont lus : parties, portées, instruments (nom, transposition, programme
//! MIDI), mesures (y compris irrégulières), voix, accords, notes (hauteur,
//! `tpc`, altération, tête, note morte), silences (y compris de mesure entière),
//! durées et points, n-olets imbriqués, armures, chiffrages, clés, liaisons de
//! prolongation, paroles et accords d'agrément. Les annotations simples
//! (nuances, textes, tempos, repères, accords chiffrés, points d'orgue) et les
//! spanners (soufflets, liaisons d'expression, pédales, ottavas, voltas…)
//! remplissent le modèle quand c'est direct ; tout le reste est ignoré sans
//! erreur, avec un `TODO(parité)` à l'endroit concerné.
//!
//! # Temps
//!
//! Les ticks internes suivent le modèle : fractions de ronde exactes, `1/4`
//! pour une noire. Les `<tick>` bruts (ticks MuseScore, 480 par noire) sont
//! convertis avec `<Division>` comme `ReadContext::fileDivision`.

mod annotations;
mod chord;
mod credits;
mod measure;
mod notations;
mod nuances;
mod part;
mod repeats;
mod spanner;
mod tables;
mod textlines;
mod xml;

use crate::container::MscVersion;
use crate::model::{
    BarLineType, Clef, ClefType, Fraction, Measure, Score, SegmentItem,
    SegmentType, StyleSettings, Track, Transpose, TupletId, VOICES,
};
use crate::Result;

use xml::Node;

/// Point d'entrée du lecteur 3.x.
///
/// `version` est la version déclarée par `<museScore version="…">` (voir
/// [`crate::container::detect_version`]) ; elle ne sert qu'aux corrections de
/// compatibilité propres à 3.02.
pub fn read(mscx: &[u8], version: MscVersion) -> Result<Score> {
    let document = xml::parse(mscx)?;
    let mut reader = Reader::new(version);
    let score_node = match document.name() {
        "museScore" => document.child("Score"),
        "Score" => Some(&document),
        _ => None,
    };
    if let Some(node) = score_node {
        reader.read_score(node);
    }
    Ok(reader.finish())
}

/// État de lecture partagé par les sous-modules.
pub(crate) struct Reader {
    /// Version du format.
    pub(crate) version: MscVersion,
    /// Partition en construction.
    pub(crate) score: Score,
    /// `<Division>` du fichier, pour les `<tick>` en ticks bruts.
    pub(crate) file_division: u32,
    /// Moitiés de connecteurs en attente d'appariement.
    pub(crate) connectors: Vec<spanner::Connector>,
    /// Mesures à répéter, développées en fin de lecture.
    pub(crate) measure_repeats: Vec<super::measure_repeat::MeasureRepeat>,
    /// Clef initiale déclarée par chaque portée (`<defaultClef>`, groupe de
    /// `StaffType`, sinon `<clef>` de l'instrument), appliquée en fin de
    /// lecture.
    pub(crate) staff_clefs: Vec<Option<Clef>>,
    /// Vrai si le style active les silences multi-mesures.
    pub(crate) create_multi_measure_rests: bool,
    /// Vrai si le `<bracket>` du n-olet était explicite dans le fichier.
    pub(crate) explicit_tuplet_brackets: Vec<bool>,
}

/// Position de lecture dans une voix.
pub(crate) struct VoiceCtx {
    /// Tick absolu courant.
    pub(crate) tick: Fraction,
    /// Mesure courante.
    pub(crate) measure: usize,
    /// Piste courante (`portée * VOICES + voix`).
    pub(crate) track: Track,
    /// Portée courante.
    pub(crate) staff: usize,
    /// N-olets ouverts, du plus externe au plus interne.
    pub(crate) tuplets: Vec<TupletId>,
}

impl Reader {
    fn new(version: MscVersion) -> Self {
        let mut score = Score::new();
        score.style = StyleSettings::for_version(version.major());
        Self {
            version,
            score,
            file_division: crate::model::DEFAULT_DIVISION,
            connectors: Vec::new(),
            measure_repeats: Vec::new(),
            staff_clefs: Vec::new(),
            create_multi_measure_rests: false,
            explicit_tuplet_brackets: Vec::new(),
        }
    }

    fn finish(mut self) -> Score {
        self.apply_default_clefs();
        self.assign_multiple_rests();
        self.adjust_tuplet_brackets();
        spanner::resolve(&mut self);
        // Après `spanner::resolve` : la copie d'une mesure répétée reprend les
        // spanners (glissandos…) de la mesure source.
        self.expand_measure_repeats();
        self.score
    }

    /// Lit un `<Score>` : métadonnées et parties, puis musique des portées.
    fn read_score(&mut self, node: &Node) {
        for child in node.children() {
            match child.name() {
                "Division" => {
                    if let Some(value) = child.int_text() {
                        self.file_division = value.max(1) as u32;
                    }
                }
                "metaTag" => self.read_meta_tag(child),
                "work-title" => self.score.metadata.work_title = child.trimmed_text(),
                "movement-title" => self.score.metadata.movement_title = child.trimmed_text(),
                "work-number" => self.score.metadata.work_number = child.trimmed_text(),
                "copyright" | "rights" => {
                    self.score.metadata.copyright = child.text().trim().to_owned();
                }
                "source" => self.score.metadata.source = child.trimmed_text(),
                "Part" => part::read_part(self, child),
                "Style" => {
                    self.score.style = credits::read_style(child, self.version);
                    self.create_multi_measure_rests = child
                        .child("createMultiMeasureRests")
                        .and_then(Node::int_text)
                        .is_some_and(|value| value != 0);
                }
                // TODO(parité): `<Score>` imbriqué (parties liées),
                // `<Order>`, `<SystemObjects>`, `<PageLocks>`, `<SystemLocks>`,
                // `<SpannerMap>` (5.0)… ne sont pas lus. Les `<Spanner>`
                // complets écrits au niveau du `<Score>` (fichiers antérieurs à
                // 3.01) ne le sont pas non plus.
                _ => {}
            }
        }

        // La musique vient après les déclarations de parties ; les portées sont
        // numérotées par l'attribut `id` (1-based), comme `StaffRead::readStaff`.
        for child in node.children() {
            if child.name() == "Staff" {
                self.read_staff(child);
            }
        }
    }

    fn read_meta_tag(&mut self, node: &Node) {
        let Some(name) = node.attr("name") else {
            return;
        };
        // `Score::setMetaTag` conserve le texte tel quel (retours à la ligne
        // compris).
        let text = node.text();
        let metadata = &mut self.score.metadata;
        match name {
            "workTitle" => metadata.work_title = text,
            "movementTitle" => metadata.movement_title = text,
            "workNumber" => metadata.work_number = text,
            "composer" => metadata.composer = text,
            "lyricist" => metadata.lyricist = text,
            "arranger" => metadata.arranger = text,
            "copyright" => metadata.copyright = text,
            "source" => metadata.source = text,
            "creationDate" => metadata.creation_date = text,
            "platform" => metadata.platform = text,
            "subtitle" => metadata.subtitle = text,
            "mscVersion" => metadata.msc_version = text,
            _ => {}
        }
    }

    /// Lit le contenu d'une `<Staff id="…">`.
    fn read_staff(&mut self, node: &Node) {
        let staff = node.attr_i32("id").map_or(0, |id| id.max(1) as usize - 1);
        if staff >= self.score.staves.len() {
            return;
        }

        let mut tick = Fraction::ZERO;
        let mut index = 0usize;
        // Numéro de page courant pour les textes de cadre : un `<LayoutBreak>`
        // de type `page` place l'élément qui le porte sur la page suivante.
        let mut page = 1u32;
        // Un saut de section porte sur la mesure suivante : MuseScore y
        // recommence la numérotation à 1 (et les `<attributes>` MusicXML).
        let mut section_break = false;
        for child in node.children() {
            match child.name() {
                "Measure" => {
                    // Une mesure de multi-repos est un marqueur de mise en
                    // page : `StaffRead::readStaff` ne l'ajoute pas à la liste
                    // des mesures et n'avance pas le tick (elle est rattachée à
                    // la mesure précédente par `setMMRest`).
                    if let Some(count) = child
                        .child("multiMeasureRest")
                        .and_then(Node::int_text)
                        .filter(|count| *count > 0)
                    {
                        if staff == 0 && index > 0 {
                            self.score.measures[index - 1].multi_measure_rest =
                                Some(u16::try_from(count).unwrap_or(u16::MAX));
                        }
                        continue;
                    }
                    let measure = self.ensure_measure(index, tick);
                    self.score.measures[measure].section_break_before |= section_break;
                    section_break = Self::has_section_break(child);
                    tick = measure::read_measure(self, measure, child, staff);
                    index += 1;
                }
                "tick" => {
                    if let Some(value) = child.int_text() {
                        tick = self.file_tick(value);
                    }
                }
                // Texte de cadre exporté en `<words>` au début de la mesure
                // (`tboxTextAsWords`). Un cadre suivi d'un saut de page est
                // au-dessous du système précédent.
                "TBox" => {
                    section_break |= Self::has_section_break(child);
                    let below = index > 0 && credits::page_break(child);
                    if let Some(mut text) = textlines::read_tbox(child, index, below) {
                        if below {
                            text.measure = super::mmrest_group_start(&self.score.measures, text.measure);
                        }
                        self.score.add_tbox_text(text);
                    }
                }
                "VBox" => {
                    section_break |= Self::has_section_break(child);
                    // Les textes d'un `VBox` deviennent des `<credit>` ; les
                    // autres cadres n'en portent pas.
                    if staff == 0 {
                        credits::read_vbox(&mut self.score, child, page);
                    }
                }
                // TODO(parité): `<HBox>` et `<FBox>` ne portent pas de texte ;
                // seuls leurs sauts de page et de section comptent.
                "HBox" | "FBox" => {
                    section_break |= Self::has_section_break(child);
                }
                _ => {}
            }
            // Un saut de page s'applique après l'élément qui le porte.
            if staff == 0 && credits::page_break(child) {
                page += 1;
                self.score.style.page_count = self.score.style.page_count.max(page);
            }
        }
    }

    /// Vrai si un cadre ou une mesure porte un saut de section.
    fn has_section_break(node: &Node) -> bool {
        node.child("LayoutBreak")
            .and_then(|layout_break| layout_break.child("subtype"))
            .is_some_and(|subtype| subtype.trimmed_text() == "section")
    }

    /// Complète les silences multi-mesures d'un seul tenant.
    ///
    /// Avec `<createMultiMeasureRests>`, MuseScore remplace aussi les mesures
    /// vides isolées par un multi-repos de un, sans écrire de marqueur dans le
    /// `.mscx`. Les groupes plus longs viennent des marqueurs lus ; toute
    /// mesure vide non couverte reçoit `multiple-rest 1`.
    fn assign_multiple_rests(&mut self) {
        if !self.create_multi_measure_rests {
            return;
        }
        // Les mesures vides isolées ne deviennent un multi-repos de un que si
        // la partition entière en est faite (mise en page de MuseScore).
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
                covered_until = index + 1;
            }
        }
    }

    /// Ajuste le crochet des n-olets une fois leur imbrication connue.
    ///
    /// Un n-olet sans enfant ne porte pas de crochet (notes groupées par
    /// ligature) ; un n-olet qui en contient un en porte un. Le `<bracket>`
    /// explicite du fichier reste prioritaire.
    fn adjust_tuplet_brackets(&mut self) {
        let mut has_child = vec![false; self.score.tuplets.len()];
        for tuplet in &self.score.tuplets {
            if let Some(parent) = tuplet.parent {
                if let Some(entry) = has_child.get_mut(parent.index()) {
                    *entry = true;
                }
            }
        }
        for index in 0..self.score.tuplets.len() {
            if self
                .explicit_tuplet_brackets
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            if has_child[index] {
                self.score.tuplets[index].bracket = true;
            }
        }
    }

    /// Crée les mesures manquantes jusqu'à `index` et renvoie son index.
    fn ensure_measure(&mut self, index: usize, tick: Fraction) -> usize {
        while self.score.measures.len() <= index {
            let number = self.score.measures.len();
            let (start, nominal_len) = match self.score.measures.last() {
                Some(last) => (last.end_tick(), last.nominal_len),
                None => (tick, Fraction::new(4, 4)),
            };
            self.score
                .add_measure(Measure::new(number, start, nominal_len));
        }
        index
    }

    /// Convertit un tick de fichier (480 par noire) en fraction de ronde.
    ///
    /// `ReadContext::fileDivision` : met à l'échelle par
    /// `Constants::DIVISION` quand `<Division>` diffère de 480.
    fn file_tick(&self, ticks: i32) -> Fraction {
        let division = i64::from(self.file_division.max(1));
        let scaled = (i64::from(ticks) * i64::from(crate::model::DEFAULT_DIVISION) + division / 2)
            / division;
        Fraction::from_ticks(scaled, xml::TICKS_PER_WHOLE)
    }

    /// Donne une clef explicite aux portées dont la première mesure n'en a pas.
    ///
    /// MuseScore n'écrit ni `<Clef>` dans la première mesure ni l'information
    /// équivalente dans le modèle : la clef par défaut de la portée (déclarée
    /// par `<defaultClef>` ou le groupe de `StaffType`, sinon sol) est celle que
    /// l'export MusicXML écrit dans `<attributes>`. On la matérialise par un
    /// segment `HeaderClef`, seul endroit où le modèle sait ranger une clef.
    fn apply_default_clefs(&mut self) {
        if self.score.measures.is_empty() {
            return;
        }
        let start = self.score.measures[0].start_tick;
        for staff in 0..self.score.staves.len() {
            let clef = self
                .staff_clefs
                .get(staff)
                .copied()
                .flatten()
                .unwrap_or(Clef::new(ClefType::G));
            let track = Track::from_raw((staff * VOICES) as u32);
            let has_clef = self.score.measures[0].segments.iter().any(|segment| {
                segment.kind.is_clef_type()
                    && segment.tick == start
                    && segment.element(track).is_some()
            });
            if has_clef {
                continue;
            }
            measure::segment_mut(&mut self.score.measures[0], SegmentType::HeaderClef, start)
                .set_element(track, SegmentItem::Clef(clef));
        }
    }

    /// Développe les mesures `<RepeatMeasure>`.
    ///
    /// Le contenu de la mesure répétée est recopié depuis la précédente, comme
    /// le fait l'export de MuseScore en repointant la mesure ; voir
    /// [`super::measure_repeat::expand`].
    fn expand_measure_repeats(&mut self) {
        for repeat in std::mem::take(&mut self.measure_repeats) {
            super::measure_repeat::expand(&mut self.score, &repeat);
        }
    }
}

/// Vrai si la mesure ne contient qu'un silence de mesure entière.
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
                _ => {}
            }
        }
    }
    rest
}

/// Transposition de l'instrument porté par une portée.
pub(crate) fn instrument_transpose(score: &Score, staff: usize) -> Transpose {
    score
        .staves
        .get(staff)
        .map(|staff| staff.part.index())
        .and_then(|part| score.parts.get(part))
        .and_then(|part| part.instrument.transpose)
        .unwrap_or_default()
}
