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
use super::spanner::Spanner;
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// Parolier.
    pub lyricist: String,
    /// Arrangeur.
    pub arranger: String,
    /// Mention de copyright.
    pub copyright: String,
    /// Source.
    pub source: String,
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
            division,
            parts: Vec::new(),
            staves: Vec::new(),
            measures: Vec::new(),
            credits: Vec::new(),
            tbox_texts: Vec::new(),
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
