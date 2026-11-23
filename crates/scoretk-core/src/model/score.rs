// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/score.h` de MuseScore Studio, Copyright (C)
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

//! Partition complète et arènes d'objets.
//!
//! Portage de `Score` (`engraving/dom/score.h`). La partition possède les
//! parties, les portées, les mesures et les arènes de notes, n-olets et
//! spanners. Aucun objet ne pointe vers son parent : les relations se font par
//! identifiants typés, ce qui garde [`Score`] `Send + Sync` et facilite le
//! portage wasm.

use super::fraction::{Fraction, Tick};
use super::ids::{MeasureId, NoteId, PartId, SpannerId, StaffId, TupletId};
use super::measure::Measure;
use super::note::Note;
use super::part::{Part, Staff};
use super::playback::PlaybackTransposition;
use super::spanner::Spanner;
use super::style::StyleSettings;
use super::text::{Credit, TboxText};
use super::tuplet::Tuplet;

/// Nombre de divisions MusicXML par noire utilisé quand aucune valeur n'est
/// imposée. MuseScore choisit une valeur calculée par `calcDivisions()` dans
/// `exportmusicxml.cpp` ; 480 est la valeur classique des fichiers MuseScore.
pub const DEFAULT_DIVISION: u32 = 480;

/// Métadonnées d'œuvre.
///
/// Correspond aux champs de `Score` lus par l'export MusicXML
/// (`engraving/dom/score.h`, `exportmusicxml.cpp` pour `<work>`,
/// `<identification>` et `<credit>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreMetadata {
    /// Titre de l'œuvre.
    pub work_title: String,
    /// Titre du mouvement.
    pub movement_title: String,
    /// Numéro de mouvement.
    pub movement_number: String,
    /// Numéro d'œuvre.
    pub work_number: String,
    /// Compositeur.
    pub composer: String,
    /// Parolier (`metaTag` `lyricist`).
    pub lyricist: String,
    /// Poète (`metaTag` `poet`), distinct de `lyricist` dans MuseScore 4.
    pub poet: String,
    /// Traducteur (`metaTag` `translator`).
    pub translator: String,
    /// Arrangeur.
    pub arranger: String,
    /// Mention de copyright.
    pub copyright: String,
    /// Source.
    pub source: String,
    /// Date de création du fichier (`creationDate`), exportée dans
    /// `<miscellaneous>`.
    ///
    /// Vide quand le `.mscx` ne la porte pas ; l'export retombe alors sur la
    /// date d'encodage, comme `MasterScore` initialise les métadonnées à la
    /// date du jour.
    pub creation_date: String,
    /// Plateforme d'origine (`platform`), exportée dans `<miscellaneous>`.
    ///
    /// `MasterScore` la fixe à la plateforme courante ; la bibliothèque étant
    /// déterministe, la valeur par défaut est `Linux`, celle des références du
    /// corpus. Un `<metaTag>` présent, même vide, la remplace.
    pub platform: String,
    /// Sous-titre (`subtitle`), exporté dans `<miscellaneous>`.
    pub subtitle: String,
    /// Version du format source (`mscVersion`), exportée dans
    /// `<miscellaneous>` quand le fichier la porte.
    pub msc_version: String,
}

impl Default for ScoreMetadata {
    fn default() -> Self {
        Self {
            work_title: String::new(),
            movement_title: String::new(),
            movement_number: String::new(),
            work_number: String::new(),
            composer: String::new(),
            lyricist: String::new(),
            poet: String::new(),
            translator: String::new(),
            arranger: String::new(),
            copyright: String::new(),
            source: String::new(),
            creation_date: String::new(),
            platform: "Linux".to_owned(),
            subtitle: String::new(),
            msc_version: String::new(),
        }
    }
}

/// Partition en mémoire.
///
/// Correspond à `Score` dans `engraving/dom/score.h`. Les objets référencés à
/// plusieurs endroits vivent dans des arènes :
/// [`Score::notes`], [`Score::tuplets`] et [`Score::spanners`]. Les accords et
/// silences restent possédés par les segments, comme dans MuseScore.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    /// Métadonnées d'œuvre.
    pub metadata: ScoreMetadata,
    /// Réglages de style globaux (format de page, `lyricsPlacement`…).
    pub style: StyleSettings,
    /// Divisions MusicXML par noire.
    pub division: u32,
    /// Parties, dans l'ordre de la partition.
    pub parts: Vec<Part>,
    /// Portées, toutes parties confondues.
    pub staves: Vec<Staff>,
    /// Mesures, dans l'ordre chronologique.
    pub measures: Vec<Measure>,
    /// Textes de cadre de titre, exportés en `<credit>`.
    pub credits: Vec<Credit>,
    /// Textes de cadres `TBox`, exportés en `<words>` de direction.
    pub tbox_texts: Vec<TboxText>,
    /// Transposition de lecture trouvée dans un second `\score` MIDI LilyPond.
    ///
    /// `None` pour toutes les autres sources. Ignoré par tous les écrivains
    /// sauf l'écrivain MIDI : la partition gravée reste la référence du modèle.
    pub playback_transposition: Option<PlaybackTransposition>,
    /// La partition est enveloppée dans `\articulate` (`ly/articulate.ly`).
    ///
    /// LilyPond remplace alors chaque note par une note plus courte et un
    /// silence : facteur 7/8 par défaut, 1/2 pour un staccato, 1/4 pour un
    /// staccatissimo, 3/4 pour un portato ou un staccato sous liaison, 1 pour
    /// un tenuto. Seul l'écrivain MIDI s'en sert ; les autres écrivains
    /// gardent la partition gravée.
    pub playback_articulate: bool,
    /// La partition a été lue sous `\unfoldRepeats`.
    ///
    /// Drapeau de **lecture** : le lecteur LilyPond déplie alors les reprises
    /// dans le modèle. L'écrivain MIDI n'en dépend pas, il obéit à
    /// `MidiOptions::expand_repeats` ; l'oracle LilyPond passe
    /// `--no-expand-repeats` aux sources sans `\unfoldRepeats` pour jouer le
    /// fichier tel qu'écrit.
    pub playback_unfold_repeats: bool,
    /// Arène des notes.
    pub notes: Vec<Note>,
    /// Arène des n-olets.
    pub tuplets: Vec<Tuplet>,
    /// Arène des spanners.
    pub spanners: Vec<Spanner>,
}

impl Score {
    /// Construit une partition vide avec [`DEFAULT_DIVISION`].
    pub fn new() -> Self {
        Self::with_division(DEFAULT_DIVISION)
    }

    /// Construit une partition vide avec une résolution donnée.
    ///
    /// # Panics
    ///
    /// Panique si `division` vaut zéro.
    pub fn with_division(division: u32) -> Self {
        assert!(
            division > 0,
            "les divisions MusicXML doivent être non nulles"
        );
        Self {
            metadata: ScoreMetadata::default(),
            style: StyleSettings::default(),
            division,
            parts: Vec::new(),
            staves: Vec::new(),
            measures: Vec::new(),
            credits: Vec::new(),
            tbox_texts: Vec::new(),
            playback_transposition: None,
            playback_articulate: false,
            playback_unfold_repeats: true,
            notes: Vec::new(),
            tuplets: Vec::new(),
            spanners: Vec::new(),
        }
    }

    /// Ajoute une partie et lui attribue son identifiant.
    ///
    /// Les portées déjà présentes dans `part.staves` ne sont pas enregistrées
    /// automatiquement : ajouter chaque portée avec [`Score::add_staff`] ou
    /// [`Score::add_staff_with`].
    pub fn add_part(&mut self, mut part: Part) -> PartId {
        let id = PartId::from_index(self.parts.len());
        part.id = id;
        self.parts.push(part);
        id
    }

    /// Ajoute une portée standard de cinq lignes à une partie.
    pub fn add_staff(&mut self, part: PartId) -> StaffId {
        self.add_staff_with(part, Staff::new(part))
    }

    /// Ajoute une portée personnalisée à une partie.
    ///
    /// L'identifiant et la partie de `staff` sont écrasés pour rester cohérents
    /// avec les arènes.
    pub fn add_staff_with(&mut self, part: PartId, mut staff: Staff) -> StaffId {
        let id = StaffId::from_index(self.staves.len());
        staff.id = id;
        staff.part = part;
        self.staves.push(staff);
        self.parts[part.index()].staves.push(id);
        id
    }

    /// Ajoute une mesure.
    pub fn add_measure(&mut self, measure: Measure) -> MeasureId {
        let id = MeasureId::from_index(self.measures.len());
        self.measures.push(measure);
        id
    }

    /// Ajoute un texte de cadre de titre (`<credit>` à l'export).
    pub fn add_credit(&mut self, credit: Credit) -> usize {
        self.credits.push(credit);
        self.credits.len() - 1
    }

    /// Ajoute un texte de cadre `TBox` (`<words>` à l'export).
    pub fn add_tbox_text(&mut self, text: TboxText) -> usize {
        self.tbox_texts.push(text);
        self.tbox_texts.len() - 1
    }

    /// Ajoute une note dans l'arène.
    pub fn add_note(&mut self, note: Note) -> NoteId {
        let id = NoteId::from_index(self.notes.len());
        self.notes.push(note);
        id
    }

    /// Ajoute un n-olet dans l'arène.
    pub fn add_tuplet(&mut self, tuplet: Tuplet) -> TupletId {
        let id = TupletId::from_index(self.tuplets.len());
        self.tuplets.push(tuplet);
        id
    }

    /// Ajoute un spanner dans l'arène.
    pub fn add_spanner(&mut self, spanner: Spanner) -> SpannerId {
        let id = SpannerId::from_index(self.spanners.len());
        self.spanners.push(spanner);
        id
    }

    /// Accès à une partie.
    pub fn part(&self, id: PartId) -> &Part {
        &self.parts[id.index()]
    }

    /// Accès mutable à une partie.
    pub fn part_mut(&mut self, id: PartId) -> &mut Part {
        &mut self.parts[id.index()]
    }

    /// Accès à une portée.
    pub fn staff(&self, id: StaffId) -> &Staff {
        &self.staves[id.index()]
    }

    /// Accès mutable à une portée.
    pub fn staff_mut(&mut self, id: StaffId) -> &mut Staff {
        &mut self.staves[id.index()]
    }

    /// Vrai si la portée est affichée, au sens de `Staff::show()`
    /// (`engraving/dom/staff.cpp:1420-1423`) : la partie et la portée doivent
    /// l'être. La mise en page ignore les portées masquées, notamment pour les
    /// points de silence (`RestLayout`) et le regroupement des multi-repos.
    pub fn staff_shown(&self, id: StaffId) -> bool {
        let staff = self.staff(id);
        staff.visible && self.part(staff.part).visible
    }

    /// Accès à une mesure.
    pub fn measure(&self, id: MeasureId) -> &Measure {
        &self.measures[id.index()]
    }

    /// Accès mutable à une mesure.
    pub fn measure_mut(&mut self, id: MeasureId) -> &mut Measure {
        &mut self.measures[id.index()]
    }

    /// Accès à une note.
    pub fn note(&self, id: NoteId) -> &Note {
        &self.notes[id.index()]
    }

    /// Accès mutable à une note.
    pub fn note_mut(&mut self, id: NoteId) -> &mut Note {
        &mut self.notes[id.index()]
    }

    /// Accès à un n-olet.
    pub fn tuplet(&self, id: TupletId) -> &Tuplet {
        &self.tuplets[id.index()]
    }

    /// Accès mutable à un n-olet.
    pub fn tuplet_mut(&mut self, id: TupletId) -> &mut Tuplet {
        &mut self.tuplets[id.index()]
    }

    /// Accès à un spanner.
    pub fn spanner(&self, id: SpannerId) -> &Spanner {
        &self.spanners[id.index()]
    }

    /// Accès mutable à un spanner.
    pub fn spanner_mut(&mut self, id: SpannerId) -> &mut Spanner {
        &mut self.spanners[id.index()]
    }

    /// Tick de fin de la partition (fin de la dernière mesure).
    pub fn end_tick(&self) -> Tick {
        self.measures
            .last()
            .map_or(Fraction::ZERO, Measure::end_tick)
    }

    /// Convertit un tick interne (ronde = `1/1`) en divisions MusicXML.
    ///
    /// MusicXML compte les divisions par noire, d'où le facteur quatre.
    pub fn tick_to_divisions(&self, tick: Tick) -> i64 {
        let units_per_whole = self
            .division
            .checked_mul(4)
            .expect("divisions MusicXML trop grandes");
        tick.ticks(units_per_whole)
    }

    /// Convertit des divisions MusicXML en tick interne.
    pub fn divisions_to_tick(&self, divisions: i64) -> Tick {
        let units_per_whole = self
            .division
            .checked_mul(4)
            .expect("divisions MusicXML trop grandes");
        Fraction::from_ticks(divisions, units_per_whole)
    }
}

impl Default for Score {
    fn default() -> Self {
        Self::new()
    }
}
