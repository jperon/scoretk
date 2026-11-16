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
mod frames;
mod measure;
mod nuances;
mod part;
mod repeats;
mod spanner;
mod tables;
mod textlines;
mod xml;

use crate::container::MscVersion;
use crate::model::{
    ChordRestKind, Clef, ClefType, Fraction, Measure, NoteId, Score, SegmentItem, SegmentType,
    Track, Transpose, TupletId, VOICES,
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
    pub(crate) measure_repeats: Vec<MeasureRepeat>,
    /// Clef initiale déclarée par chaque portée (`<defaultClef>`, groupe de
    /// `StaffType`, sinon `<clef>` de l'instrument), appliquée en fin de
    /// lecture.
    pub(crate) staff_clefs: Vec<Option<Clef>>,
}

/// Mesure marquée `<RepeatMeasure>` : son contenu est celui de la précédente.
pub(crate) struct MeasureRepeat {
    /// Index de la mesure qui répète.
    pub(crate) measure: usize,
    /// Portée concernée.
    pub(crate) staff: usize,
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
        Self {
            version,
            score: Score::new(),
            file_division: crate::model::DEFAULT_DIVISION,
            connectors: Vec::new(),
            measure_repeats: Vec::new(),
            staff_clefs: Vec::new(),
        }
    }

    fn finish(mut self) -> Score {
        self.apply_default_clefs();
        self.expand_measure_repeats();
        spanner::resolve(&mut self);
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
                // TODO(parité): `<Score>` imbriqué (parties liées), `<Style>`,
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
        let mut page = 1u32;
        for child in node.children() {
            match child.name() {
                "Measure" => {
                    // Une mesure de multi-repos est un marqueur de mise en
                    // page : `StaffRead::readStaff` ne l'ajoute pas à la liste
                    // des mesures et n'avance pas le tick (elle est rattachée à
                    // la mesure précédente par `setMMRest`).
                    if child.child("multiMeasureRest").is_some() {
                        continue;
                    }
                    let measure = self.ensure_measure(index, tick);
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
                    let below = index > 0 && frames::page_break(child);
                    if let Some(text) = textlines::read_tbox(child, index, below) {
                        self.score.add_tbox_text(text);
                    }
                }
                "VBox" => {
                    if staff == 0 {
                        for credit in frames::read_vbox(child, page) {
                            self.score.add_credit(credit);
                        }
                    }
                }
                // TODO(parité): `<HBox>` et `<FBox>` ne portent pas de texte ;
                // seuls leurs sauts de page comptent (voir plus bas).
                _ => {}
            }
            if frames::page_break(child) {
                page += 1;
            }
        }
        if staff == 0 {
            self.score.page_count = page;
            // Style « rights » de MuseScore 3.x : 8 points.
            self.score.copyright_font_size = 8.0;
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
    /// Le modèle n'a pas d'élément « mesure répétée » : comme l'export
    /// MusicXML de MuseScore, on recopie le contenu de la mesure précédente
    /// pour la portée concernée.
    fn expand_measure_repeats(&mut self) {
        for repeat in std::mem::take(&mut self.measure_repeats) {
            if repeat.measure == 0 || repeat.measure >= self.score.measures.len() {
                continue;
            }
            let source = repeat.measure - 1;
            let mut copies: Vec<(Track, SegmentItem, Fraction)> = Vec::new();
            {
                let previous = &self.score.measures[source];
                for segment in &previous.segments {
                    for (track, item) in segment.iter_elements() {
                        if track.staff().index() != repeat.staff {
                            continue;
                        }
                        if !matches!(item, SegmentItem::ChordRest(_)) {
                            continue;
                        }
                        copies.push((track, item.clone(), segment.tick - previous.start_tick));
                    }
                }
            }
            let target_start = self.score.measures[repeat.measure].start_tick;
            for (track, item, offset) in copies {
                let item = self.duplicate_chord_rest(item);
                let measure = &mut self.score.measures[repeat.measure];
                measure::segment_mut(measure, SegmentType::ChordRest, target_start + offset)
                    .set_element(track, item);
            }
        }
    }

    /// Copie un accord ou un silence, notes comprises, pour une mesure répétée.
    fn duplicate_chord_rest(&mut self, item: SegmentItem) -> SegmentItem {
        match item {
            SegmentItem::ChordRest(mut chord_rest) => {
                if let ChordRestKind::Chord(chord) = &mut chord_rest.kind {
                    chord.notes = self.duplicate_notes(&chord.notes);
                    for grace in &mut chord.grace_chords {
                        grace.chord.notes = self.duplicate_notes(&grace.chord.notes);
                    }
                }
                SegmentItem::ChordRest(chord_rest)
            }
            other => other,
        }
    }

    fn duplicate_notes(&mut self, notes: &[NoteId]) -> Vec<NoteId> {
        let mut copies = Vec::with_capacity(notes.len());
        for &note in notes {
            let mut copy = self.score.note(note).clone();
            // TODO(parité): les liaisons de la mesure répétée ne sont pas
            // dupliquées ; MuseScore les recalcule à l'export.
            copy.tie_forward = None;
            copy.tie_back = None;
            copies.push(self.score.add_note(copy));
        }
        copies
    }
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
