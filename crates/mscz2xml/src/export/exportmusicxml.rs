//! Export MusicXML `partwise`.
//!
//! Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` et
//! des helpers de `internal/shared/`. La structure et les noms de méthodes
//! suivent le C++ (`write`, `write_parts`, `write_measure`,
//! `write_measure_staves`, `write_measure_tracks`, `chord`, `rest`,
//! `calc_divisions`, `move_to_tick`, …) pour que les itérations vers la
//! parité restent directes.
//!
//! # Ce qui n'est pas exporté
//!
//! Le modèle ne contient ni style, ni mise en page, ni géométrie : tout ce que
//! MuseScore tire de `MStyle` et des positions (`<defaults>`, `<print>`,
//! attributs `default-x`/`default-y`, largeur des mesures, `<measure-style>`,
//! sauts de page/système, hampes calculées) est absent de la sortie. Les
//! éléments correspondants portent un `// TODO(parité)`. Les `<credit>` sont
//! exportés pour leur texte, sans position ni police du style.
//!
//! Les éléments dont le modèle ne garde pas l'information (spanners, symboles,
//! doigtés, détails des changements d'instrument, voltas, marqueurs, pédales,
//! etc.) sont ignorés silencieusement.

use std::collections::BTreeSet;

use crate::model::chord::ChordRestKind;
use crate::model::{
    Accidental, AccidentalType, Annotation, BeamMode, Chord, ChordRest, Clef, ClefType, Duration,
    Fraction, GraceType, Harmony, Key, KeyMode, KeySig, Lyrics, LyricsSyllabic, Measure, Note,
    NoteHeadGroup, NoteHeadType, Part, Placement, Score, Segment, SegmentItem, SegmentType,
    StaffId, StemDirection, Tick, TimeSig, TimeSigType, Track, Tuplet, TupletId, TupletNumberType,
    VOICES,
};
use crate::Result;

use super::notations;
use super::nuances::{self, Nuances};
use super::pitch;
use super::repeats;
use super::text;
use super::writer::XmlWriter;

/// Logiciel annoncé dans `<encoding>`, identique à MuseScore 4.7.
pub const DEFAULT_SOFTWARE: &str = "MuseScore Studio 4.7.4";

/// Date d'encodage par défaut.
///
/// MuseScore écrit la date du jour ; la bibliothèque n'a ni horloge ni
/// entrées/sorties et doit rester déterministe, d'où cette constante figée
/// (surchargeable par [`ExportOptions::encoding_date`]).
pub const DEFAULT_ENCODING_DATE: &str = "2026-11-16";

/// Options d'export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Contenu de `<software>`.
    pub software: String,
    /// Contenu de `<encoding-date>`.
    pub encoding_date: String,
    /// Vrai pour exporter les hauteurs en sons réels (concert pitch).
    ///
    /// MuseScore exporte par défaut les hauteurs écrites (`false`), en
    /// utilisant `Note::tpc_transposed` et la transposition de l'instrument.
    pub concert_pitch: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            software: DEFAULT_SOFTWARE.to_owned(),
            encoding_date: DEFAULT_ENCODING_DATE.to_owned(),
            concert_pitch: false,
        }
    }
}

/// Exporte une partition en MusicXML `partwise` non compressé.
pub fn export_musicxml(score: &Score) -> Result<String> {
    export_musicxml_with(score, &ExportOptions::default())
}

/// Variante de [`export_musicxml`] avec options.
pub fn export_musicxml_with(score: &Score, options: &ExportOptions) -> Result<String> {
    let context = Context::new(score, options);
    let mut out = Out::new();
    let mut nuances = Nuances::default();
    context.write(&mut out, &mut nuances);
    Ok(out.xml.finish())
}

/// Calcule les divisions MusicXML, comme `ExportMusicXml::calcDivisions`.
///
/// Parcourt toutes les parties, toutes les voix et tous les segments pour
/// récolter les durées de déplacement (`backup`/`forward`) et les durées
/// réelles, puis prend le PPCM des dénominateurs divisé par quatre. La
/// constante de départ vaut quatre pour garantir des divisions non nulles.
pub fn calc_divisions(score: &Score) -> u32 {
    compute_divisions(&Context::new(score, &ExportOptions::default()))
}

/// État d'écriture : tampon, divisions, position courante et `attributes`.
pub(crate) struct Out {
    pub(crate) xml: XmlWriter,
    pub(crate) div: u32,
    pub(crate) tick: Tick,
    in_attributes: bool,
    /// État des notations à numéroter (liaisons, glissandos, arpèges).
    pub(crate) notations: notations::NotationsState,
}

impl Out {
    fn new() -> Self {
        Self {
            xml: XmlWriter::new(),
            div: 1,
            tick: Tick::ZERO,
            in_attributes: false,
            notations: notations::NotationsState::default(),
        }
    }

    /// Ouvre ou ferme `<attributes>` à la demande, comme `Attributes::doAttr`.
    pub(crate) fn attr(&mut self, open: bool) {
        if open && !self.in_attributes {
            self.xml.start_element("attributes", &[]);
            self.in_attributes = true;
        } else if !open && self.in_attributes {
            self.xml.end_element();
            self.in_attributes = false;
        }
    }

    /// Ferme `<attributes>` s'il est encore ouvert (`Attributes::stop`).
    fn attr_stop(&mut self) {
        self.attr(false);
    }

    /// Convertit une durée en divisions MusicXML.
    ///
    /// Reproduit `calculateTimeDeltaInDivisions` : le C++ réduit la fraction
    /// puis prend son numérateur, ce qui suppose que le PPCM des divisions
    /// rend le résultat entier.
    pub(crate) fn duration_in_divisions(&self, delta: Tick) -> i64 {
        (delta * Fraction::from_integer(i64::from(self.div) * 4)).numerator()
    }

    /// Déplace la position courante en émettant `<backup>` ou `<forward>`.
    ///
    /// Reproduit `ExportMusicXml::moveToTick`. Le paramètre `stretch` du C++
    /// n'a pas d'équivalent dans le modèle (pas d'étirement de portée).
    fn move_to_tick(&mut self, target: Tick) {
        if target < self.tick {
            self.attr(false);
            let duration = self.duration_in_divisions(self.tick - target);
            self.xml.start_element("backup", &[]);
            self.xml.tag("duration", &duration.to_string());
            self.xml.end_element();
        } else if target > self.tick {
            self.attr(false);
            let duration = self.duration_in_divisions(target - self.tick);
            self.xml.start_element("forward", &[]);
            self.xml.tag("duration", &duration.to_string());
            self.xml.end_element();
        }
        self.tick = target;
    }
}

/// Profondeur maximale d'imbrication de n-olets suivie par l'export.
///
/// MuseScore n'impose pas de limite, mais un modèle corrompu pourrait former
/// une chaîne cyclique : cette borne évite toute boucle infinie.
const MAX_TUPLET_DEPTH: usize = 64;

/// Élément enfant d'un n-olet, dans l'ordre de `Tuplet::elements()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TupletElement {
    /// Accord ou silence directement enfant du n-olet.
    Chord {
        /// Position du segment.
        tick: Tick,
        /// Piste (portée et voix).
        track: Track,
        /// Durée notée, utilisée par `determineTupletNormalTicks`.
        duration: Fraction,
    },
    /// N-olet imbriqué directement enfant de celui-ci.
    Tuplet(TupletId),
}

impl TupletElement {
    fn tick(self, score: &Score) -> Tick {
        match self {
            Self::Chord { tick, .. } => tick,
            Self::Tuplet(id) => score.tuplet(id).tick,
        }
    }

    fn duration(self, score: &Score) -> Fraction {
        match self {
            Self::Chord { duration, .. } => duration,
            Self::Tuplet(id) => score.tuplet(id).len,
        }
    }
}

/// Deux références désignent-elles le même enfant de n-olet ?
///
/// La durée stockée dans `TupletElement::Chord` ne fait pas partie de
/// l'identité : elle ne sert qu'à `determineTupletNormalTicks`.
fn same_element(left: TupletElement, right: TupletElement) -> bool {
    match (left, right) {
        (
            TupletElement::Chord {
                tick: left_tick,
                track: left_track,
                ..
            },
            TupletElement::Chord {
                tick: right_tick,
                track: right_track,
                ..
            },
        ) => left_tick == right_tick && left_track == right_track,
        (TupletElement::Tuplet(left), TupletElement::Tuplet(right)) => left == right,
        _ => false,
    }
}

/// Index des enfants de chaque n-olet, triés par position.
struct TupletIndex {
    elements: Vec<Vec<TupletElement>>,
}

impl TupletIndex {
    fn new(score: &Score) -> Self {
        let mut elements = vec![Vec::new(); score.tuplets.len()];

        for measure in &score.measures {
            for segment in &measure.segments {
                for (track, item) in segment.iter_elements() {
                    let SegmentItem::ChordRest(chord_rest) = item else {
                        continue;
                    };
                    let Some(id) = chord_rest.tuplet else {
                        continue;
                    };
                    if let Some(children) = elements.get_mut(id.index()) {
                        children.push(TupletElement::Chord {
                            tick: segment.tick,
                            track,
                            duration: chord_rest.duration.ticks(),
                        });
                    }
                }
            }
        }

        for (id, tuplet) in score.tuplets.iter().enumerate() {
            if let Some(parent) = tuplet.parent {
                if let Some(children) = elements.get_mut(parent.index()) {
                    children.push(TupletElement::Tuplet(TupletId::from_index(id)));
                }
            }
        }

        for children in &mut elements {
            children.sort_by(|left, right| {
                left.tick(score)
                    .cmp(&right.tick(score))
                    .then_with(|| rank(*left).cmp(&rank(*right)))
            });
        }

        Self { elements }
    }

    fn children(&self, id: TupletId) -> &[TupletElement] {
        self.elements
            .get(id.index())
            .map_or(&[], |children| children.as_slice())
    }

    fn is_first(&self, id: TupletId, element: TupletElement) -> bool {
        self.children(id)
            .first()
            .is_some_and(|first| same_element(*first, element))
    }

    fn is_last(&self, id: TupletId, element: TupletElement) -> bool {
        self.children(id)
            .last()
            .is_some_and(|last| same_element(*last, element))
    }

    /// Vrai si le n-olet n'a que des accords ou silences pour enfants.
    fn is_simple(&self, score: &Score, id: TupletId) -> bool {
        if score.tuplet(id).parent.is_some() {
            return false;
        }
        self.children(id)
            .iter()
            .all(|element| matches!(element, TupletElement::Chord { .. }))
    }
}

fn rank(element: TupletElement) -> u8 {
    match element {
        TupletElement::Tuplet(_) => 0,
        TupletElement::Chord { .. } => 1,
    }
}

/// Contexte immuable de l'export.
pub(crate) struct Context<'a> {
    pub(crate) score: &'a Score,
    options: &'a ExportOptions,
    tuplets: TupletIndex,
}

impl<'a> Context<'a> {
    fn new(score: &'a Score, options: &'a ExportOptions) -> Self {
        Self {
            score,
            options,
            tuplets: TupletIndex::new(score),
        }
    }

    // ------------------------------------------------------------------
    // Haut niveau : write, work, identification, part-list, parties
    // ------------------------------------------------------------------

    fn write(&self, out: &mut Out, nuances: &mut Nuances) {
        out.div = compute_divisions(self);

        out.xml.start_document();
        out.xml.doctype(
            "score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \
             \"http://www.musicxml.org/dtds/partwise.dtd\"",
        );
        out.xml
            .start_element("score-partwise", &[("version", "4.0")]);

        self.work(out);
        self.identification(out);
        text::write_credits(out, self.score);
        // TODO(parité): <defaults> (style, polices, mise en page).
        self.part_list(out);
        self.write_parts(out, nuances);

        out.xml.end_element();
    }

    /// Écrit `<work>` et `<movement-title>`.
    fn work(&self, out: &mut Out) {
        let metadata = &self.score.metadata;
        if !metadata.work_title.is_empty() || !metadata.work_number.is_empty() {
            out.xml.start_element("work", &[]);
            if !metadata.work_number.is_empty() {
                out.xml.tag("work-number", &metadata.work_number);
            }
            if !metadata.work_title.is_empty() {
                out.xml.tag("work-title", &metadata.work_title);
            }
            out.xml.end_element();
        }
        if !metadata.movement_number.is_empty() {
            out.xml.tag("movement-number", &metadata.movement_number);
        }
        if !metadata.movement_title.is_empty() {
            out.xml.tag("movement-title", &metadata.movement_title);
        }
    }

    /// Écrit `<identification>`.
    ///
    /// MuseScore y range les créateurs par ordre alphabétique de type, puis
    /// `rights`, `encoding`, `source` et `miscellaneous`. Les champs
    /// `creationDate` et `platform` des fichiers MuseScore ne sont pas dans le
    /// modèle : `miscellaneous` est donc omis.
    fn identification(&self, out: &mut Out) {
        let metadata = &self.score.metadata;

        out.xml.start_element("identification", &[]);
        for (kind, value) in [
            ("arranger", &metadata.arranger),
            ("composer", &metadata.composer),
            ("lyricist", &metadata.lyricist),
        ] {
            if !value.is_empty() {
                out.xml.tag_with("creator", &[("type", kind)], value);
            }
        }
        if !metadata.copyright.is_empty() {
            out.xml.tag("rights", &metadata.copyright);
        }

        out.xml.start_element("encoding", &[]);
        out.xml.tag("software", &self.options.software);
        out.xml.tag("encoding-date", &self.options.encoding_date);
        out.xml
            .tag_empty("supports", &[("element", "accidental"), ("type", "yes")]);
        out.xml
            .tag_empty("supports", &[("element", "beam"), ("type", "yes")]);
        out.xml.tag_empty(
            "supports",
            &[
                ("element", "print"),
                ("attribute", "new-page"),
                ("type", "yes"),
                ("value", "yes"),
            ],
        );
        out.xml.tag_empty(
            "supports",
            &[
                ("element", "print"),
                ("attribute", "new-system"),
                ("type", "yes"),
                ("value", "yes"),
            ],
        );
        out.xml
            .tag_empty("supports", &[("element", "stem"), ("type", "yes")]);
        out.xml.end_element();

        if !metadata.source.is_empty() {
            out.xml.tag("source", &metadata.source);
        }
        // TODO(parité): <miscellaneous> (creationDate, platform, autres
        // métadonnées MuseScore absentes du modèle).
        out.xml.end_element();
    }

    /// Écrit `<part-list>` (`partList`).
    ///
    /// TODO(parité): `<part-group>` (accolades, crochets, portées liées), qui
    /// demande les informations de bracket absentes du modèle.
    fn part_list(&self, out: &mut Out) {
        let hidden_name = self.score.parts.len() == 1;

        out.xml.start_element("part-list", &[]);
        for (index, part) in self.score.parts.iter().enumerate() {
            let number = index + 1;
            let part_id = format!("P{number}");
            let instrument_id = format!("P{number}-I1");

            out.xml.start_element("score-part", &[("id", &part_id)]);

            let name = plain_part_name(part);
            let name_attributes: &[(&str, &str)] = if hidden_name {
                &[("print-object", "no")]
            } else {
                &[]
            };
            out.xml.tag_with("part-name", name_attributes, &name);
            // TODO(parité): <part-name-display> pour les noms contenant des
            // altérations (♭/♯), qui demandent les fragments de texte.

            if !part.short_name.is_empty() {
                let abbreviation = plain_text(&part.short_name);
                out.xml
                    .tag_with("part-abbreviation", name_attributes, &abbreviation);
            }

            out.xml
                .start_element("score-instrument", &[("id", &instrument_id)]);
            let instrument_name = part.instrument.track_name.clone();
            out.xml.tag("instrument-name", &instrument_name);
            if !part.instrument.id.is_empty() {
                out.xml.tag("instrument-sound", &part.instrument.id);
            }
            out.xml.end_element();

            // MuseScore écrit un élément vide non auto-fermant pour
            // <midi-device>. Le modèle n'a pas de port MIDI : celui par
            // défaut de MuseScore vaut 1.
            out.xml
                .tag_with("midi-device", &[("id", &instrument_id), ("port", "1")], "");
            out.xml
                .start_element("midi-instrument", &[("id", &instrument_id)]);
            // MuseScore n'écrit le canal que s'il tient dans 1..=16.
            if number <= 16 {
                out.xml.tag("midi-channel", &number.to_string());
            }
            if let Some(program) = part.instrument.midi_program {
                if u16::from(program) < 128 {
                    out.xml
                        .tag("midi-program", &(u16::from(program) + 1).to_string());
                }
            }
            // Valeurs par défaut de MuseScore (volume 100/127, pan centré) :
            // le modèle ne porte pas les réglages de canal.
            out.xml.tag("volume", "78.7402");
            out.xml.tag("pan", "0");
            out.xml.end_element();

            out.xml.end_element();
        }
        out.xml.end_element();
    }

    /// Écrit toutes les parties (`writeParts`).
    fn write_parts(&self, out: &mut Out, nuances: &mut Nuances) {
        for part_index in 0..self.score.parts.len() {
            let part_id = format!("P{}", part_index + 1);
            out.tick = Tick::ZERO;
            out.in_attributes = false;
            nuances.reset_part();
            out.notations.reset_part();
            out.xml.start_element("part", &[("id", &part_id)]);

            for measure_index in 0..self.score.measures.len() {
                self.write_measure(out, measure_index, part_index, nuances);
            }

            out.xml.end_element();
        }
    }

    // ------------------------------------------------------------------
    // Mesures
    // ------------------------------------------------------------------

    /// Écrit une mesure (`writeMeasure`).
    fn write_measure(
        &self,
        out: &mut Out,
        measure_index: usize,
        part_index: usize,
        nuances: &mut Nuances,
    ) {
        let measure = &self.score.measures[measure_index];
        let part = &self.score.parts[part_index];
        let staves = part.staves.len();
        let is_first = measure_index == 0;

        // TODO(parité): numérotation `implicit="yes"` des mesures d'anacrouse
        // (le modèle n'a pas l'équivalent de `excludeFromNumbering`).
        let number = (measure_index + 1).to_string();
        out.xml.start_element("measure", &[("number", &number)]);
        out.in_attributes = false;

        // TODO(parité): <print> (sauts de page/système, numéro de page) et
        // attribut width : la mise en page n'est pas dans le modèle.

        // La barre de gauche doit être le premier élément de la mesure.
        self.barline_left(out, measure, part_index);

        // Les attributs ne sont écrits que sur la première mesure réelle.
        if is_first {
            out.attr(true);
            out.xml.tag("divisions", &out.div.to_string());
        }
        self.keysig_timesig(out, measure, part);
        if is_first && staves > 1 {
            out.xml.tag("staves", &staves.to_string());
        }
        // TODO(parité): <instruments> pour les parties multi-instruments.
        self.write_measure_clefs(out, measure, measure_index, part);
        if is_first {
            self.write_staff_details(out, part);
            self.write_instrument_details(out, part);
        }
        // TODO(parité): <measure-style> (silences multi-mesures, reprises de
        // mesure).

        repeats::repeat_at_measure_start(self, out, measure, part_index);

        self.write_measure_staves(out, measure, part, nuances);
        self.annotations_without_note(out, measure, part);

        out.move_to_tick(measure.end_tick());
        repeats::repeat_at_measure_stop(self, out, measure, part_index);
        self.barline_right(out, measure, part_index, measure_index);
        out.attr_stop();

        out.xml.end_element();
    }

    /// Écrit les portées d'une mesure pour une partie (`writeMeasureStaves`).
    fn write_measure_staves(
        &self,
        out: &mut Out,
        measure: &Measure,
        part: &Part,
        nuances: &mut Nuances,
    ) {
        let staves = part.staves.len();
        out.notations.start_measure();
        for (position, staff_id) in part.staves.iter().enumerate() {
            out.move_to_tick(measure.start_tick);
            let xml_staff = if staves > 1 { position + 1 } else { 0 };
            self.write_measure_tracks(out, measure, *staff_id, xml_staff, nuances);
            if position + 1 < staves {
                // Retour au début de la mesure pour la portée suivante.
                out.move_to_tick(measure.start_tick);
            }
        }
    }

    /// Écrit les quatre voix d'une portée (`writeMeasureTracks`).
    fn write_measure_tracks(
        &self,
        out: &mut Out,
        measure: &Measure,
        staff_id: StaffId,
        xml_staff: usize,
        nuances: &mut Nuances,
    ) {
        for voice in 0..VOICES {
            let track = Track::new(staff_id, voice as u8);
            for segment in &measure.segments {
                // TODO(parité): spanners et annotations des segments TimeTick.
                if segment.kind.is_time_tick_type() {
                    continue;
                }
                let Some(item) = segment.element(track) else {
                    continue;
                };
                if let SegmentItem::BarLine(barline) = item {
                    // Une reprise au début ne doit pas produire de
                    // backup/forward parasite.
                    if barline.kind.is_repeat_start() {
                        continue;
                    }
                }

                out.move_to_tick(segment.tick);

                if let SegmentItem::ChordRest(_) = item {
                    out.attr(false);
                    if annotation_track(segment, staff_id) == Some(track) {
                        self.harmonies(out, segment, staff_id);
                        self.annotations(out, segment, staff_id, xml_staff);
                    }
                    nuances.start(self.score, out, segment, staff_id, xml_staff, track);
                }
                let is_chord_rest = matches!(item, SegmentItem::ChordRest(_));

                self.write_element(out, segment, item, measure, xml_staff, track);

                if is_chord_rest {
                    nuances.stop(self.score, out, out.tick, staff_id, xml_staff);
                }
            }
        }
    }

    /// Écrit un élément de segment (`writeElement`).
    fn write_element(
        &self,
        out: &mut Out,
        segment: &Segment,
        item: &SegmentItem,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
    ) {
        match item {
            SegmentItem::Clef(clef) => {
                self.mid_measure_clef(out, segment, clef, measure, xml_staff);
            }
            SegmentItem::ChordRest(chord_rest) => match &chord_rest.kind {
                ChordRestKind::Chord(chord) => {
                    self.chord_entry(out, chord_rest, chord, measure, xml_staff, track);
                }
                ChordRestKind::Rest(rest) => {
                    if !rest.gap {
                        self.rest(out, chord_rest, measure, xml_staff, track);
                    }
                }
            },
            SegmentItem::BarLine(barline) => {
                if segment.tick != measure.start_tick && segment.tick != measure.end_tick() {
                    self.barline_middle(out, barline);
                }
            }
            // Armures, indications de mesure et respirations sont écrites
            // ailleurs (ou ignorées pour les respirations).
            SegmentItem::KeySig(_) | SegmentItem::TimeSig(_) | SegmentItem::Breath(_) => {}
        }
    }

    // ------------------------------------------------------------------
    // Divisions et durées
    // ------------------------------------------------------------------

    /// Durée notée d'un accord ou d'un silence, points compris.
    fn notated_ticks(&self, chord_rest: &ChordRest, measure: &Measure) -> Fraction {
        if chord_rest.is_full_measure_rest() {
            measure.len
        } else {
            chord_rest.duration.ticks()
        }
    }

    /// Durée réelle (`globalTicks`), n-olets appliqués.
    ///
    /// MuseScore divise la durée notée par le ratio de chaque n-olet parent.
    /// Le modèle n'a pas d'étirement de portée (`Staff::timeStretch`), le
    /// facteur correspondant vaut donc un. Quand `<duration>` diffère de la
    /// figure (trémolo, figure étirée), sa valeur fait foi.
    fn global_ticks(&self, chord_rest: &ChordRest, measure: &Measure) -> Fraction {
        let mut ticks = chord_rest
            .ticks
            .unwrap_or_else(|| self.notated_ticks(chord_rest, measure));
        let mut tuplet = chord_rest.tuplet;
        let mut depth = 0;
        while let Some(id) = tuplet {
            let Some(value) = self.score.tuplets.get(id.index()) else {
                break;
            };
            // Un ratio nul ou une chaîne cyclique (modèle corrompu) ne doit pas
            // faire boucler l'export.
            if value.ratio.is_zero() || depth >= MAX_TUPLET_DEPTH {
                break;
            }
            ticks /= value.ratio;
            tuplet = value.parent;
            depth += 1;
        }
        ticks
    }

    /// Durée consommée par l'élément (`DurationElement::actualTicks`).
    ///
    /// C'est la durée réelle divisée par l'étirement de la portée ; la valeur
    /// exportée dans `<duration>` reste [`Context::global_ticks`].
    fn actual_ticks(
        &self,
        chord_rest: &ChordRest,
        measure: &Measure,
        staff_id: StaffId,
    ) -> Fraction {
        let ticks = self.global_ticks(chord_rest, measure);
        let stretch = self.score.staff(staff_id).time_stretch;
        if stretch.is_positive() {
            ticks / stretch
        } else {
            ticks
        }
    }

    /// Transposition chromatique de la portée, en demi-tons.
    fn transpose_chromatic(&self, staff_id: StaffId) -> i32 {
        if self.options.concert_pitch {
            return 0;
        }
        let part = self.score.part(self.score.staff(staff_id).part);
        part.instrument
            .transpose
            .map_or(0, |transpose| transpose.chromatic)
    }
}

// ----------------------------------------------------------------------
// N-olets
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Nombre de niveaux de n-olets au-dessus d'un élément (`tupletNesting`).
    fn tuplet_nesting(&self, chord_rest: &ChordRest) -> usize {
        let mut nesting = 0;
        let mut tuplet = chord_rest.tuplet;
        while let Some(id) = tuplet {
            if nesting >= MAX_TUPLET_DEPTH {
                break;
            }
            nesting += 1;
            tuplet = self.score.tuplet(id).parent;
        }
        nesting
    }

    /// N-olet de l'élément, ou celui de son ancêtre s'il s'agit d'un n-olet.
    fn element_tuplet(&self, element: TupletElement, chord_rest: &ChordRest) -> Option<TupletId> {
        match element {
            TupletElement::Chord { .. } => chord_rest.tuplet,
            TupletElement::Tuplet(id) => self.score.tuplet(id).parent,
        }
    }

    /// N-olet qui commence à ce niveau (`startTupletAtLevel`).
    fn start_tuplet_at_level(
        &self,
        chord_rest: &ChordRest,
        tick: Tick,
        track: Track,
        level: usize,
    ) -> Option<TupletId> {
        let mut element = TupletElement::Chord {
            tick,
            track,
            duration: chord_rest.duration.ticks(),
        };
        for _ in 0..level {
            let tuplet = self.element_tuplet(element, chord_rest)?;
            if !self.tuplets.is_first(tuplet, element) {
                return None;
            }
            element = TupletElement::Tuplet(tuplet);
        }
        match element {
            TupletElement::Tuplet(id) => Some(id),
            TupletElement::Chord { .. } => None,
        }
    }

    /// N-olet qui se termine à ce niveau (`stopTupletAtLevel`).
    fn stop_tuplet_at_level(
        &self,
        chord_rest: &ChordRest,
        tick: Tick,
        track: Track,
        level: usize,
    ) -> Option<TupletId> {
        let mut element = TupletElement::Chord {
            tick,
            track,
            duration: chord_rest.duration.ticks(),
        };
        for _ in 0..level {
            let tuplet = self.element_tuplet(element, chord_rest)?;
            if !self.tuplets.is_last(tuplet, element) {
                return None;
            }
            element = TupletElement::Tuplet(tuplet);
        }
        match element {
            TupletElement::Tuplet(id) => Some(id),
            TupletElement::Chord { .. } => None,
        }
    }

    /// Écrit les `<tuplet>` d'un accord ou d'un silence (`tupletStartStop`).
    fn tuplet_start_stop(
        &self,
        out: &mut Out,
        chord_rest: &ChordRest,
        tick: Tick,
        track: Track,
        notations: &mut Notations,
    ) {
        let nesting = self.tuplet_nesting(chord_rest);
        let need_actual_and_normal = nesting > 1;

        for level in (0..nesting).rev() {
            if let Some(tuplet) = self.start_tuplet_at_level(chord_rest, tick, track, level + 1) {
                self.tuplet_start(
                    out,
                    tuplet,
                    nesting - level,
                    need_actual_and_normal,
                    notations,
                );
            }
            if let Some(tuplet) = self.stop_tuplet_at_level(chord_rest, tick, track, level + 1) {
                self.tuplet_stop(out, tuplet, nesting - level, notations);
            }
        }
    }

    /// Écrit `<tuplet type="start">` avec ses éventuels actual/normal.
    fn tuplet_start(
        &self,
        out: &mut Out,
        id: TupletId,
        number: usize,
        need_actual_and_normal: bool,
        notations: &mut Notations,
    ) {
        let tuplet = self.score.tuplet(id);
        let simple = self.tuplets.is_simple(self.score, id);

        let mut attributes: Vec<(&str, String)> = vec![("type", "start".to_owned())];
        if !simple {
            attributes.push(("number", number.to_string()));
        }
        attributes.push((
            "bracket",
            if tuplet.bracket { "yes" } else { "no" }.to_owned(),
        ));
        match tuplet.number_type {
            TupletNumberType::Number => {}
            TupletNumberType::Ratio => attributes.push(("show-number", "both".to_owned())),
            TupletNumberType::None => attributes.push(("show-number", "none".to_owned())),
        }
        // TODO(parité): direction/placement du n-olet (absent du modèle).

        let borrowed = borrow_attributes(&attributes);
        notations.tag(&mut out.xml, None);
        if need_actual_and_normal {
            out.xml.start_element("tuplet", &borrowed);
            tuplet_actual_and_normal(out, tuplet);
            out.xml.end_element();
        } else {
            out.xml.tag_empty("tuplet", &borrowed);
        }
    }

    /// Écrit `<tuplet type="stop">`.
    fn tuplet_stop(&self, out: &mut Out, id: TupletId, number: usize, notations: &mut Notations) {
        let simple = self.tuplets.is_simple(self.score, id);
        let number = number.to_string();
        let mut attributes: Vec<(&str, &str)> = vec![("type", "stop")];
        if !simple {
            attributes.push(("number", &number));
        }
        notations.tag(&mut out.xml, None);
        out.xml.tag_empty("tuplet", &attributes);
    }

    /// Indique si MuseScore doit écrire `<normal-type>` (`determineTupletNormalTicks`).
    ///
    /// Un `<normal-type>` n'est nécessaire que si les enfants du n-olet ont
    /// des durées différentes ou si leur nombre ne correspond pas au
    /// numérateur du ratio.
    fn tuplet_normal_ticks(&self, id: TupletId) -> Option<Fraction> {
        let tuplet = self.score.tuplet(id);
        let children = self.tuplets.children(id);
        if children.len() > 1 {
            let first = children[0].duration(self.score);
            if children[1..]
                .iter()
                .any(|element| element.duration(self.score) != first)
            {
                return Some(tuplet.base_len);
            }
        }
        if children.len() as i64 != tuplet.ratio.numerator() {
            return Some(tuplet.base_len);
        }
        None
    }

    /// Écrit `<time-modification>` (`writeTimeModification`).
    fn write_time_modification(&self, out: &mut Out, chord_rest: &ChordRest) {
        // Un trémolo à deux notes vaut un rapport 2:1 (`tremoloCorrection`).
        let tremolo = chord_rest
            .chord_ref()
            .is_some_and(|chord| chord.tremolo_two_chord);
        let mut actual: i64 = if tremolo { 2 } else { 1 };
        let mut normal: i64 = 1;
        let mut tuplet = chord_rest.tuplet;
        let mut depth = 0;
        while let Some(id) = tuplet {
            if depth >= MAX_TUPLET_DEPTH {
                break;
            }
            let value = self.score.tuplet(id);
            actual *= value.ratio.numerator();
            normal *= value.ratio.denominator();
            tuplet = value.parent;
            depth += 1;
        }
        if actual == 1 && normal == 1 {
            return;
        }

        out.xml.start_element("time-modification", &[]);
        out.xml.tag("actual-notes", &actual.to_string());
        out.xml.tag("normal-notes", &normal.to_string());
        if let Some(normal_ticks) = chord_rest
            .tuplet
            .and_then(|id| self.tuplet_normal_ticks(id))
        {
            if let Some(base) = pitch::tick_to_xml(normal_ticks) {
                out.xml.tag("normal-type", base.0);
                for _ in 0..base.1 {
                    out.xml.tag_empty("normal-dot", &[]);
                }
            }
        }
        out.xml.end_element();
    }
}

// ----------------------------------------------------------------------
// Accords, silences, notes
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Écrit un accord principal et ses accords d'agrément (`writeElement`).
    fn chord_entry(
        &self,
        out: &mut Out,
        chord_rest: &ChordRest,
        chord: &Chord,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
    ) {
        for grace in &chord.grace_chords {
            let kind = grace.chord.grace.unwrap_or(GraceType::Appoggiatura);
            if !grace_is_after(kind) {
                self.chord(
                    out,
                    &grace.chord,
                    Some(&grace.duration),
                    chord_rest,
                    measure,
                    xml_staff,
                    track,
                    true,
                );
            }
        }
        self.chord(
            out, chord, None, chord_rest, measure, xml_staff, track, false,
        );
        for grace in &chord.grace_chords {
            let kind = grace.chord.grace.unwrap_or(GraceType::Appoggiatura);
            if grace_is_after(kind) {
                self.chord(
                    out,
                    &grace.chord,
                    Some(&grace.duration),
                    chord_rest,
                    measure,
                    xml_staff,
                    track,
                    true,
                );
            }
        }
    }

    /// Écrit un accord (`chord`).
    #[allow(clippy::too_many_arguments)]
    fn chord(
        &self,
        out: &mut Out,
        chord: &Chord,
        grace_duration: Option<&Duration>,
        chord_rest: &ChordRest,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
        grace: bool,
    ) {
        let segment_tick = out.tick;
        let global_ticks = self.global_ticks(chord_rest, measure);
        if !grace {
            out.tick += self.actual_ticks(chord_rest, measure, track.staff());
        }

        let duration = grace_duration.copied().unwrap_or(chord_rest.duration);
        let notated = grace_duration.map_or_else(
            || self.notated_ticks(chord_rest, measure),
            |duration| duration.ticks(),
        );
        let voice = voice_number(xml_staff, track.voice());
        let transpose = self.transpose_chromatic(track.staff());

        for (index, note_id) in chord.notes.iter().enumerate() {
            let note = self.score.note(*note_id);
            let first = index == 0;

            out.attr(false);
            out.xml.start_element("note", &[]);

            if grace {
                // MuseScore affiche la barre oblique pour une acciaccatura,
                // même quand le fichier ne stocke pas le drapeau.
                if chord.grace_slash || chord.grace == Some(GraceType::Acciaccatura) {
                    out.xml.tag_empty("grace", &[("slash", "yes")]);
                } else {
                    out.xml.tag_empty("grace", &[]);
                }
            }
            let cue = is_cue_note(chord, note);
            if cue {
                out.xml.tag_empty("cue", &[]);
            }
            if !first {
                out.xml.tag_empty("chord", &[]);
            }

            write_pitch(out, note, self.options.concert_pitch, transpose);

            if !grace {
                out.xml.tag(
                    "duration",
                    &out.duration_in_divisions(global_ticks).to_string(),
                );
            }

            if !cue {
                if note.tie_back.is_some() {
                    out.xml.tag_empty("tie", &[("type", "stop")]);
                }
                if note.tie_forward.is_some() {
                    out.xml.tag_empty("tie", &[("type", "start")]);
                }
            }

            // TODO(parité): <instrument> pour les parties multi-instruments.

            out.xml.tag("voice", &voice.to_string());
            write_type(out, notated, chord, note, grace);
            if let Some(accidental) = &note.accidental {
                write_accidental(out, accidental);
            }
            // Un accord d'agrément n'appartient à aucun n-olet.
            if !grace {
                self.write_time_modification(out, chord_rest);
            }

            // Pas de hampe pour les rondes et les valeurs plus longues.
            if duration.kind.has_stem() {
                match chord.stem {
                    StemDirection::Up => out.xml.tag("stem", "up"),
                    StemDirection::Down => out.xml.tag("stem", "down"),
                    // La direction automatique est calculée par la mise en
                    // page : le modèle ne la connaît pas.
                    StemDirection::Auto => {}
                }
            }

            write_notehead(out, note, chord, duration);

            if xml_staff > 0 {
                out.xml.tag("staff", &xml_staff.to_string());
            }

            if first && !grace {
                write_beam(out, chord_rest.beam_mode);
            }

            let mut notations = Notations::new();
            if note.tie_back.is_some() {
                notations.tag(&mut out.xml, None);
                out.xml.tag_empty("tied", &[("type", "stop")]);
            }
            if note.tie_forward.is_some() {
                notations.tag(&mut out.xml, None);
                out.xml.tag_empty("tied", &[("type", "start")]);
            }
            if first && !grace {
                self.tuplet_start_stop(out, chord_rest, segment_tick, track, &mut notations);
            }
            if first {
                self.chord_attributes(out, chord, measure, segment_tick, track, &mut notations);
            }
            notations::fingering(&mut out.xml, note, &mut notations);
            let last = index + 1 == chord.notes.len();
            notations::arpeggio(
                &mut out.xml,
                &mut out.notations,
                chord,
                segment_tick,
                track,
                first,
                last,
                &mut notations,
            );
            notations::glissandi(
                self.score,
                &mut out.xml,
                &mut out.notations,
                segment_tick,
                track,
                &mut notations,
            );
            notations.finish(&mut out.xml);

            if first && !grace {
                for lyric in &chord_rest.lyrics {
                    write_lyric(out, lyric);
                }
            }

            out.xml.end_element();
        }
    }

    /// Écrit un silence (`rest`).
    fn rest(
        &self,
        out: &mut Out,
        chord_rest: &ChordRest,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
    ) {
        let segment_tick = out.tick;
        out.attr(false);

        out.xml.start_element("note", &[]);

        if chord_rest.is_full_measure_rest() {
            out.xml.tag_empty("rest", &[("measure", "yes")]);
        } else {
            out.xml.tag_empty("rest", &[]);
        }

        let tick_len = self.global_ticks(chord_rest, measure);
        out.tick += self.actual_ticks(chord_rest, measure, track.staff());
        out.xml
            .tag("duration", &out.duration_in_divisions(tick_len).to_string());

        let voice = voice_number(xml_staff, track.voice());
        out.xml.tag("voice", &voice.to_string());

        // Pas de <type> pour un silence de mesure entière.
        if !chord_rest.duration.is_measure() {
            let name = pitch::duration_type_name(chord_rest.duration.kind);
            if !name.is_empty() {
                out.xml.tag("type", name);
            }
            for _ in 0..chord_rest.duration.dots {
                out.xml.tag_empty("dot", &[]);
            }
        }

        self.write_time_modification(out, chord_rest);

        if xml_staff > 0 {
            out.xml.tag("staff", &xml_staff.to_string());
        }

        write_beam(out, chord_rest.beam_mode);

        let mut notations = Notations::new();
        let segment = measure
            .segments
            .iter()
            .find(|segment| segment.tick == segment_tick && segment.chord_rest(track).is_some());
        notations::rest_notations(
            self.score,
            &mut out.xml,
            measure,
            &mut out.notations,
            segment,
            segment_tick,
            out.tick,
            track,
            &mut notations,
        );
        self.tuplet_start_stop(out, chord_rest, segment_tick, track, &mut notations);
        notations.finish(&mut out.xml);

        for lyric in &chord_rest.lyrics {
            write_lyric(out, lyric);
        }

        out.xml.end_element();
    }

    /// Écrit les articulations et ornements d'un accord (`chordAttributes`).
    fn chord_attributes(
        &self,
        out: &mut Out,
        chord: &Chord,
        measure: &Measure,
        segment_tick: Tick,
        track: Track,
        notations: &mut Notations,
    ) {
        let segment = measure
            .segments
            .iter()
            .find(|segment| segment.tick == segment_tick && segment.chord_rest(track).is_some());
        // `doSlurs` est appelé juste avant `chordAttributes`.
        notations::slurs(
            self.score,
            &mut out.xml,
            &mut out.notations,
            segment_tick,
            track,
            notations,
        );
        notations::chord_attributes(
            self.score,
            &mut out.xml,
            chord,
            measure,
            segment,
            segment_tick,
            track,
            out.tick,
            notations,
        );
    }
}

// ----------------------------------------------------------------------
// Attributs de mesure : armures, mesures, clefs, portées
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Écrit les armures et indications de mesure (`keysigTimesig`).
    fn keysig_timesig(&self, out: &mut Out, measure: &Measure, part: &Part) {
        let mut keysigs: Vec<(usize, &KeySig)> = Vec::new();
        let mut timesigs: Vec<(usize, &TimeSig)> = Vec::new();

        for (position, staff_id) in part.staves.iter().enumerate() {
            let track = Track::new(*staff_id, 0);
            for segment in &measure.segments {
                if segment.tick > measure.start_tick {
                    break;
                }
                match segment.element(track) {
                    Some(SegmentItem::KeySig(keysig)) => keysigs.push((position, keysig)),
                    Some(SegmentItem::TimeSig(timesig)) => timesigs.push((position, timesig)),
                    _ => {}
                }
            }
        }

        if keysigs.is_empty() {
            // MuseScore écrit toujours une armure au tick zéro.
            if measure.start_tick.is_zero() {
                // MuseScore crée une armure factice à l'armure inconnue : seul
                // <fifths> est écrit.
                self.keysig(out, 0, Key::C, KeyMode::Unknown);
            }
        } else {
            let first = self.active_key(keysigs[0].1);
            let single = keysigs
                .iter()
                .all(|(_, keysig)| self.active_key(keysig) == first);
            if single {
                let keysig = keysigs[0].1;
                self.keysig(out, 0, first, keysig.mode);
            } else {
                for (position, keysig) in keysigs {
                    self.keysig(out, position + 1, self.active_key(keysig), keysig.mode);
                }
            }
        }

        if !timesigs.is_empty() {
            let single = timesigs.iter().all(|(_, timesig)| {
                timesig.numerator == timesigs[0].1.numerator
                    && timesig.denominator == timesigs[0].1.denominator
            });
            if single {
                self.timesig(out, timesigs[0].1, 0);
            } else {
                for (position, timesig) in timesigs {
                    self.timesig(out, timesig, position + 1);
                }
            }
        }
    }

    /// Tonalité à afficher (concert ou transposée).
    fn active_key(&self, keysig: &KeySig) -> Key {
        if self.options.concert_pitch {
            keysig.concert_key
        } else {
            keysig.transposed_key
        }
    }

    /// Écrit `<key>` (`keysig`).
    fn keysig(&self, out: &mut Out, staff: usize, key: Key, mode: KeyMode) {
        out.attr(true);
        let number = staff.to_string();
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        if staff > 0 {
            attributes.push(("number", &number));
        }
        out.xml.start_element("key", &attributes);
        // TODO(parité): armures non traditionnelles (key-step/key-alter/
        // key-accidental), qui demandent la liste des symboles d'armure.
        out.xml.tag("fifths", &key.fifths().to_string());
        match mode {
            KeyMode::Major => out.xml.tag("mode", "major"),
            KeyMode::Minor => out.xml.tag("mode", "minor"),
            KeyMode::None => out.xml.tag("mode", "none"),
            KeyMode::Unknown => {}
        }
        out.xml.end_element();
    }

    /// Écrit `<time>` (`timesig`).
    fn timesig(&self, out: &mut Out, timesig: &TimeSig, staff: usize) {
        out.attr(true);
        let number = staff.to_string();
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        match timesig.kind {
            TimeSigType::FourFour => attributes.push(("symbol", "common")),
            TimeSigType::AllaBreve => attributes.push(("symbol", "cut")),
            _ => {}
        }
        if staff > 0 {
            attributes.push(("number", &number));
        }
        // TODO(parité): print-object="no" (mesure non visible ou non affichée
        // sur cette portée) : le modèle n'a pas ces propriétés.
        out.xml.start_element("time", &attributes);
        out.xml.tag("beats", &timesig.numerator.to_string());
        out.xml.tag("beat-type", &timesig.denominator.to_string());
        out.xml.end_element();
    }

    /// Écrit les clefs de début de mesure (`findAndExportClef`, `exportDefaultClef`).
    fn write_measure_clefs(
        &self,
        out: &mut Out,
        measure: &Measure,
        measure_index: usize,
        part: &Part,
    ) {
        let staves = part.staves.len();

        // MuseScore reprend la clef posée à la fin de la mesure précédente,
        // ou la clef d'en-tête de la première mesure. Une clef de type `Clef`
        // au début d'une mesure est écrite par `writeElement` avec
        // `after-barline="yes"`.
        let segment = if measure_index > 0 {
            let previous = &self.score.measures[measure_index - 1];
            find_segment(previous, SegmentType::Clef, measure.start_tick)
        } else {
            find_segment(measure, SegmentType::HeaderClef, measure.start_tick)
        };

        for (position, staff_id) in part.staves.iter().enumerate() {
            let track = Track::new(*staff_id, 0);
            let xml_staff = if staves > 1 { position + 1 } else { 0 };
            match segment.and_then(|segment| segment.element(track)) {
                Some(SegmentItem::Clef(clef)) => {
                    self.clef(out, xml_staff, self.active_clef(clef), &[]);
                }
                // Aucune clef n'ouvre la partition : la mise en page en génère
                // une du type par défaut de la portée (`Staff::clefType`), que
                // `findAndExportClef` exporte comme une clef ordinaire.
                _ if measure.start_tick.is_zero() => {
                    let clef = self.score.staff(*staff_id).default_clef;
                    self.clef(out, xml_staff, self.active_clef(&clef), &[]);
                }
                _ => {}
            }
        }
    }

    /// Clef à afficher (concert ou transposée).
    fn active_clef(&self, clef: &Clef) -> ClefType {
        if self.options.concert_pitch {
            clef.concert
        } else if clef.transposed != ClefType::Invalid {
            clef.transposed
        } else {
            clef.concert
        }
    }

    /// Écrit `<clef>` (`clef`).
    fn clef(&self, out: &mut Out, staff: usize, clef: ClefType, extra: &[(&str, &str)]) {
        out.attr(true);
        let number = staff.to_string();
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        if staff > 0 {
            attributes.push(("number", &number));
        }
        attributes.extend_from_slice(extra);
        out.xml.start_element("clef", &attributes);

        let sign = clef.sign().musicxml_name();
        out.xml.tag("sign", sign);
        if sign != "percussion" && sign != "TAB" && clef.line() != 0 {
            out.xml.tag("line", &clef.line().to_string());
        }
        if clef.octave_change() != 0 {
            out.xml
                .tag("clef-octave-change", &clef.octave_change().to_string());
        }
        out.xml.end_element();
    }

    /// Écrit une clef changée au milieu d'une mesure (`writeElement`).
    fn mid_measure_clef(
        &self,
        out: &mut Out,
        segment: &Segment,
        clef: &Clef,
        measure: &Measure,
        xml_staff: usize,
    ) {
        // MuseScore n'écrit ni les clefs générées ni celles de l'en-tête.
        if segment.kind == SegmentType::HeaderClef {
            return;
        }
        if segment.tick != measure.start_tick && segment.tick != measure.end_tick() {
            self.clef(out, xml_staff, self.active_clef(clef), &[]);
        } else if segment.tick == measure.start_tick {
            self.clef(
                out,
                xml_staff,
                self.active_clef(clef),
                &[("after-barline", "yes")],
            );
        }
        // Une clef en fin de mesure sera écrite au début de la suivante.
    }

    /// Écrit `<staff-details>` pour les portées non standard (`writeStaffDetails`).
    fn write_staff_details(&self, out: &mut Out, part: &Part) {
        let staves = part.staves.len();
        for (position, staff_id) in part.staves.iter().enumerate() {
            let staff = self.score.staff(*staff_id);
            // TODO(parité): portées en petit format (staff-size), tablature
            // et portées masquées.
            if staff.lines == 5 {
                continue;
            }
            out.attr(true);
            let number = (position + 1).to_string();
            let mut attributes: Vec<(&str, &str)> = Vec::new();
            if staves > 1 {
                attributes.push(("number", &number));
            }
            out.xml.start_element("staff-details", &attributes);
            out.xml.tag("staff-lines", &staff.lines.to_string());
            out.xml.end_element();
        }
    }

    /// Écrit `<transpose>` (`writeInstrumentDetails`).
    fn write_instrument_details(&self, out: &mut Out, part: &Part) {
        let Some(transpose) = part.instrument.transpose else {
            return;
        };
        if transpose.chromatic == 0 {
            return;
        }

        out.attr(true);
        if self.options.concert_pitch {
            out.xml.start_element("for-part", &[]);
            out.xml.start_element("part-transpose", &[]);
        } else {
            out.xml.start_element("transpose", &[]);
        }
        out.xml
            .tag("diatonic", &(transpose.diatonic % 7).to_string());
        out.xml
            .tag("chromatic", &(transpose.chromatic % 12).to_string());
        let octave_change = transpose.chromatic / 12;
        if octave_change != 0 {
            out.xml.tag("octave-change", &octave_change.to_string());
        }
        out.xml.end_element();
        if self.options.concert_pitch {
            out.xml.end_element();
        }
        out.attr(false);
    }
}

// ----------------------------------------------------------------------
// Barres de mesure
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Écrit une reprise ou une volta au début de la mesure (`barlineLeft`).
    fn barline_left(&self, out: &mut Out, measure: &Measure, part_index: usize) {
        repeats::barline_left(self, out, measure, part_index);
    }

    /// Écrit une barre de milieu de mesure (`barlineMiddle`).
    fn barline_middle(&self, out: &mut Out, bar_line: &crate::model::BarLine) {
        repeats::barline_middle(out, bar_line);
    }

    /// Écrit la barre de fin de mesure (`barlineRight`).
    fn barline_right(
        &self,
        out: &mut Out,
        measure: &Measure,
        part_index: usize,
        measure_index: usize,
    ) {
        repeats::barline_right(self, out, measure, part_index, measure_index);
    }
}

// ----------------------------------------------------------------------
// Annotations : harmonies, nuances, textes
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Écrit les accords chiffrés du segment (`harmonies`).
    fn harmonies(&self, out: &mut Out, segment: &Segment, staff_id: StaffId) {
        for entry in &segment.annotations {
            if entry.track.staff() != staff_id {
                continue;
            }
            if let Annotation::Harmony(harmony) = &entry.annotation {
                write_harmony(out, harmony);
            }
        }
    }

    /// Écrit les annotations du segment (`annotations`).
    ///
    /// MuseScore rattache toutes les annotations d'une portée à la première
    /// voix occupée du segment (`findTrackForAnnotations`).
    fn annotations(&self, out: &mut Out, segment: &Segment, staff_id: StaffId, xml_staff: usize) {
        for entry in &segment.annotations {
            if entry.track.staff() != staff_id {
                continue;
            }
            if !entry.annotation.visible() {
                // `commonAnnotations` : un tempo invisible n'exporte que son
                // `<sound tempo>`.
                if let Annotation::Tempo(tempo) = &entry.annotation {
                    out.attr(false);
                    text::write_tempo_sound(out, tempo);
                }
                continue;
            }
            let offset = out.duration_in_divisions(segment.tick - out.tick);
            match &entry.annotation {
                Annotation::Harmony(_) => {}
                Annotation::Dynamic(dynamic) => nuances::write_dynamic(out, dynamic, xml_staff),
                Annotation::StaffText(text) => {
                    text::write_words(out, &text.text, text.placement, false, xml_staff, offset);
                }
                Annotation::Expression(text) => {
                    text::write_words(out, &text.text, text.placement, false, xml_staff, offset);
                }
                Annotation::SystemText(text) => {
                    text::write_words(out, &text.text, text.placement, true, xml_staff, offset);
                }
                Annotation::Tempo(tempo) => text::write_tempo(out, tempo, xml_staff, offset),
                Annotation::RehearsalMark(mark) => {
                    text::write_rehearsal(out, mark, xml_staff, offset);
                }
                Annotation::InstrumentChange(change) => {
                    text::write_words(
                        out,
                        &change.text,
                        change.placement,
                        false,
                        xml_staff,
                        offset,
                    );
                }
                // Le point d'orgue est écrit dans les `<notations>` de
                // l'accord ou du silence de la même piste.
                Annotation::Fermata(_) => {}
                // TODO(parité): annotations non modélisées (doigtés,
                // symboles, pédales…).
                Annotation::Other { .. } => {}
            }
        }
    }

    /// Écrit les annotations qui ne sont attachées à aucune note
    /// (`annotationsWithoutNote`).
    fn annotations_without_note(&self, out: &mut Out, measure: &Measure, part: &Part) {
        let staves = part.staves.len();
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            for (position, staff_id) in part.staves.iter().enumerate() {
                if annotation_track(segment, *staff_id).is_some() {
                    continue;
                }
                let xml_staff = if staves > 1 { position + 1 } else { 0 };
                self.annotations(out, segment, *staff_id, xml_staff);
            }
        }
    }
}

// ----------------------------------------------------------------------
// Helpers d'écriture
// ----------------------------------------------------------------------

/// Gestion de `<notations>` (`Notations` dans le C++).
///
/// Le C++ rouvre `<notations>` quand la visibilité de l'élément change : une
/// articulation invisible ouvre `<notations print-object="no">`, et l'élément
/// visible suivant rouvre un `<notations>` normal.
pub(crate) struct Notations {
    printed: bool,
    container: Option<&'static str>,
    visible: bool,
}

impl Notations {
    pub(crate) fn new() -> Self {
        Self {
            printed: false,
            container: None,
            visible: true,
        }
    }

    /// Ouvre `<notations>` puis le conteneur demandé (`Notations::tag`).
    pub(crate) fn tag(&mut self, xml: &mut XmlWriter, container: Option<&'static str>) {
        self.tag_element(xml, container, true);
    }

    /// Variante de [`Notations::tag`] qui tient compte de la visibilité de
    /// l'élément écrit (`print-object="no"`).
    pub(crate) fn tag_element(
        &mut self,
        xml: &mut XmlWriter,
        container: Option<&'static str>,
        visible: bool,
    ) {
        if self.printed && self.visible != visible {
            self.finish(xml);
        }
        if !self.printed {
            if visible {
                xml.start_element("notations", &[]);
            } else {
                xml.start_element("notations", &[("print-object", "no")]);
            }
            // Le C++ rouvre le conteneur courant quand il appartient encore
            // au nouveau `<notations>`.
            if let Some(name) = container {
                if self.container == Some(name) {
                    xml.start_element(name, &[]);
                }
            }
        }
        if self.container != container {
            if self.container.is_some() {
                xml.end_element();
            }
            if let Some(name) = container {
                xml.start_element(name, &[]);
            }
        }
        self.container = container;
        self.printed = true;
        self.visible = visible;
    }

    /// Ferme le conteneur courant, sans fermer `<notations>`.
    pub(crate) fn close_container(&mut self, xml: &mut XmlWriter) {
        if self.container.is_some() {
            xml.end_element();
            self.container = None;
        }
    }

    /// Ferme tout ce qui est ouvert (`etag`).
    pub(crate) fn finish(&mut self, xml: &mut XmlWriter) {
        self.close_container(xml);
        if self.printed {
            xml.end_element();
            self.printed = false;
        }
    }
}

/// Écrit `<pitch>` (`writePitch`).
fn write_pitch(out: &mut Out, note: &Note, concert_pitch: bool, transpose_chromatic: i32) {
    // TODO(parité): percussions (unpitched), micro-intervalles
    // (centOffset/tuning) absents du modèle.
    let (step, alter, octave) = if !pitch::tpc_is_valid(note.tpc) {
        pitch::midi_pitch(note.pitch)
    } else {
        let used_tpc = if concert_pitch {
            note.tpc
        } else if pitch::tpc_is_valid(note.tpc_transposed) {
            note.tpc_transposed
        } else {
            note.tpc
        };
        let transpose = if concert_pitch {
            0
        } else {
            transpose_chromatic
        };
        pitch::written_pitch(note.pitch, used_tpc, transpose)
    };

    out.xml.start_element("pitch", &[]);
    out.xml.tag("step", &step.to_string());
    if alter != 0 {
        out.xml.tag("alter", &alter.to_string());
    }
    out.xml.tag("octave", &octave.to_string());
    out.xml.end_element();
}

/// Écrit `<type>` et les `<dot>` (`writeType`, `tick2xml`).
fn write_type(out: &mut Out, notated: Fraction, chord: &Chord, note: &Note, grace: bool) {
    let small = chord.small;
    let cue = is_cue_note(chord, note);
    let Some((name, dots)) = pitch::tick_to_xml(notated) else {
        return;
    };
    if name == "measure" {
        return;
    }
    if small && !cue && !grace {
        out.xml.tag_with("type", &[("size", "cue")], name);
    } else if small && !cue && grace {
        out.xml.tag_with("type", &[("size", "grace-cue")], name);
    } else {
        out.xml.tag("type", name);
    }
    for _ in 0..dots {
        out.xml.tag_empty("dot", &[]);
    }
}

/// Écrit `<tuplet-actual>` et `<tuplet-normal>`.
fn tuplet_actual_and_normal(out: &mut Out, tuplet: &Tuplet) {
    let actual = tuplet.ratio.numerator().to_string();
    let normal = tuplet.ratio.denominator().to_string();
    let base = pitch::tick_to_xml(tuplet.base_len);

    out.xml.start_element("tuplet-actual", &[]);
    out.xml.tag("tuplet-number", &actual);
    write_tuplet_type_and_dots(out, base);
    out.xml.end_element();

    out.xml.start_element("tuplet-normal", &[]);
    out.xml.tag("tuplet-number", &normal);
    write_tuplet_type_and_dots(out, base);
    out.xml.end_element();
}

/// Écrit `<tuplet-type>` et ses points.
fn write_tuplet_type_and_dots(out: &mut Out, base: Option<(&'static str, u8)>) {
    if let Some((name, dots)) = base {
        out.xml.tag("tuplet-type", name);
        for _ in 0..dots {
            out.xml.tag_empty("tuplet-dot", &[]);
        }
    }
}

/// Écrit `<beam>` (`writeBeam`).
///
/// MuseScore déduit les niveaux de ligature de l'objet `Beam` ; le modèle ne
/// garde que le mode de l'élément, d'où une sortie limitée au premier niveau.
fn write_beam(out: &mut Out, mode: BeamMode) {
    let name = match mode {
        BeamMode::Auto | BeamMode::None => return,
        BeamMode::Begin => "begin",
        BeamMode::Mid => "continue",
        BeamMode::End => "end",
        BeamMode::ForwardHook => "forward hook",
        BeamMode::BackwardHook => "backward hook",
    };
    out.xml.tag_with("beam", &[("number", "1")], name);
}

/// Écrit une syllabe (`<lyric>`).
fn write_lyric(out: &mut Out, lyric: &Lyrics) {
    if lyric.text.is_empty() {
        return;
    }
    let number = lyric.number().to_string();
    let mut attributes: Vec<(&str, String)> = vec![("number", number)];
    if lyric.placement == Placement::Above {
        attributes.push(("placement", "above".to_owned()));
    }
    let borrowed = borrow_attributes(&attributes);
    out.xml.start_element("lyric", &borrowed);
    out.xml.tag("syllabic", syllabic_name(lyric.syllabic));
    out.xml.tag("text", &lyric.text);
    if lyric.melisma.is_some() {
        out.xml.tag_empty("extend", &[]);
    }
    out.xml.end_element();
}

/// Écrit `<notehead>` (`writeNotehead`).
///
/// Le groupe (`<head>`) et le type (`<headType>`) sont distincts : le type
/// donne l'attribut `filled`, le groupe la valeur MusicXML, ou son nom SMuFL
/// pour les têtes sans équivalent.
fn write_notehead(out: &mut Out, note: &Note, chord: &Chord, duration: Duration) {
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    match note.head {
        NoteHeadType::Quarter => attributes.push(("filled", "yes")),
        NoteHeadType::Half | NoteHeadType::Whole => attributes.push(("filled", "no")),
        _ => {}
    }

    let up = chord.stem != StemDirection::Down;
    let head_type = if note.head == NoteHeadType::Auto {
        duration.kind.head_type()
    } else {
        note.head
    };

    // Note invisible : la tête seule disparaît (`writeNotehead`).
    if !note.visible {
        out.xml.tag_with("notehead", &attributes, "none");
        return;
    }

    let value = match &note.head_group {
        NoteHeadGroup::Normal => {
            if note.head == NoteHeadType::Auto {
                None
            } else {
                Some("normal")
            }
        }
        NoteHeadGroup::Slash => Some("slash"),
        NoteHeadGroup::TriangleUp => Some("triangle"),
        NoteHeadGroup::Diamond => Some("diamond"),
        NoteHeadGroup::Plus => Some("cross"),
        NoteHeadGroup::Cross => Some("x"),
        NoteHeadGroup::Circled => Some("circled"),
        NoteHeadGroup::XCircle => Some("circle-x"),
        NoteHeadGroup::TriangleDown => Some("inverted triangle"),
        NoteHeadGroup::Slashed1 => Some("slashed"),
        NoteHeadGroup::Slashed2 => Some("back slashed"),
        NoteHeadGroup::Do => Some("do"),
        NoteHeadGroup::Re => Some("re"),
        NoteHeadGroup::Mi => Some("mi"),
        NoteHeadGroup::Fa => Some(if up { "fa up" } else { "fa" }),
        NoteHeadGroup::La => Some("la"),
        NoteHeadGroup::Ti => Some("ti"),
        NoteHeadGroup::Sol => Some("so"),
        NoteHeadGroup::WithX
        | NoteHeadGroup::DiamondOld
        | NoteHeadGroup::CircledLarge
        | NoteHeadGroup::LargeArrow
        | NoteHeadGroup::BrevisAlt
        | NoteHeadGroup::LargeDiamond
        | NoteHeadGroup::HeavyCross
        | NoteHeadGroup::HeavyCrossHat => {
            // Groupe sans équivalent MusicXML : MuseScore écrit son nom SMuFL.
            if let Some(name) = smufl_notehead(&note.head_group, head_type, up) {
                attributes.push(("smufl", name));
                out.xml.tag_with("notehead", &attributes, "other");
            }
            return;
        }
        // TODO(parité): groupes MuseScore hors du corpus, dont le symbole
        // SMuFL dépend du type de tête et de la direction de hampe.
        NoteHeadGroup::Other(_) => return,
    };

    if let Some(value) = value {
        out.xml.tag_with("notehead", &attributes, value);
    }
}

/// Nom SMuFL de la tête (`SymNames::nameForSymId(note->noteHead())`).
///
/// Reproduit la table `noteHeads` de `engraving/dom/note.cpp` pour les groupes
/// sans valeur MusicXML ; la hampe ne change le symbole que pour les grandes
/// flèches et les grands losanges.
fn smufl_notehead(
    group: &NoteHeadGroup,
    head_type: NoteHeadType,
    up: bool,
) -> Option<&'static str> {
    use NoteHeadType::{Breve, Half, Whole};
    let name = match (group, head_type) {
        (NoteHeadGroup::WithX, Whole) => "noteheadWholeWithX",
        (NoteHeadGroup::WithX, Half) => "noteheadHalfWithX",
        (NoteHeadGroup::WithX, Breve) => "noteheadDoubleWholeWithX",
        (NoteHeadGroup::WithX, _) => "noteheadVoidWithX",

        (NoteHeadGroup::DiamondOld, Whole) => "noteheadDiamondWholeOld",
        (NoteHeadGroup::DiamondOld, Half) => "noteheadDiamondHalfOld",
        (NoteHeadGroup::DiamondOld, Breve) => "noteheadDiamondDoubleWholeOld",
        (NoteHeadGroup::DiamondOld, _) => "noteheadDiamondBlackOld",

        (NoteHeadGroup::CircledLarge, Whole) => "noteheadCircledWholeLarge",
        (NoteHeadGroup::CircledLarge, Half) => "noteheadCircledHalfLarge",
        (NoteHeadGroup::CircledLarge, Breve) => "noteheadCircledDoubleWholeLarge",
        (NoteHeadGroup::CircledLarge, _) => "noteheadCircledBlackLarge",

        (NoteHeadGroup::LargeArrow, _) => arrow_name(up, head_type),

        (NoteHeadGroup::BrevisAlt, Whole) => "noteheadWhole",
        (NoteHeadGroup::BrevisAlt, Half) => "noteheadHalf",
        (NoteHeadGroup::BrevisAlt, Breve) => "noteheadDoubleWholeSquare",
        (NoteHeadGroup::BrevisAlt, _) => "noteheadBlack",

        (NoteHeadGroup::LargeDiamond, Whole) => "noteheadSlashWhiteWhole",
        (NoteHeadGroup::LargeDiamond, Half) => "noteheadSlashDiamondWhite",
        (NoteHeadGroup::LargeDiamond, Breve) => {
            if up {
                "noteheadSlashWhiteDoubleWhole"
            } else {
                "noteheadSlashWhiteWhole"
            }
        }
        (NoteHeadGroup::LargeDiamond, _) => "noteheadSlashHorizontalEnds",

        (NoteHeadGroup::HeavyCross, _) => "noteheadHeavyX",
        (NoteHeadGroup::HeavyCrossHat, _) => "noteheadHeavyXHat",
        _ => return None,
    };
    Some(name)
}

/// Nom de la grande flèche : le sens du symbole est inverse de la hampe.
fn arrow_name(up: bool, head_type: NoteHeadType) -> &'static str {
    use NoteHeadType::{Breve, Half, Whole};
    match (up, head_type) {
        (true, Whole) => "noteheadLargeArrowDownWhole",
        (true, Half) => "noteheadLargeArrowDownHalf",
        (true, Breve) => "noteheadLargeArrowDownDoubleWhole",
        (true, _) => "noteheadLargeArrowDownBlack",
        (false, Whole) => "noteheadLargeArrowUpWhole",
        (false, Half) => "noteheadLargeArrowUpHalf",
        (false, Breve) => "noteheadLargeArrowUpDoubleWhole",
        (false, _) => "noteheadLargeArrowUpBlack",
    }
}

/// Écrit une altération (`writeAccidental`).
fn write_accidental(out: &mut Out, accidental: &Accidental) {
    if !accidental.visible {
        return;
    }
    let Some(name) = accidental_name(accidental.kind) else {
        return;
    };
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    if accidental.bracket {
        attributes.push(("bracket", "yes"));
    } else if accidental.parentheses {
        attributes.push(("parentheses", "yes"));
    }
    // TODO(parité): smufl pour les altérations exotiques, taille, couleur.
    out.xml.tag_with("accidental", &attributes, name);
}

/// Nom MusicXML d'une altération (`accidentalType2MusicXmlString`).
pub(crate) fn accidental_name(kind: AccidentalType) -> Option<&'static str> {
    match kind {
        AccidentalType::Flat => Some("flat"),
        AccidentalType::Natural => Some("natural"),
        AccidentalType::Sharp => Some("sharp"),
        AccidentalType::DoubleFlat => Some("flat-flat"),
        AccidentalType::DoubleSharp => Some("double-sharp"),
        AccidentalType::NaturalFlat => Some("natural-flat"),
        AccidentalType::NaturalSharp => Some("natural-sharp"),
        AccidentalType::TripleFlat => Some("triple-flat"),
        AccidentalType::TripleSharp => Some("triple-sharp"),
        AccidentalType::None | AccidentalType::Other(_) => None,
    }
}

/// Écrit un accord chiffré (`harmony`).
fn write_harmony(out: &mut Out, harmony: &Harmony) {
    out.attr(false);
    let mut attributes: Vec<(&str, String)> = Vec::new();
    if harmony.placement == Placement::Below {
        attributes.push(("placement", "below".to_owned()));
    }
    attributes.push(("print-frame", "no".to_owned()));
    let borrowed = borrow_attributes(&attributes);

    out.xml.start_element("harmony", &borrowed);

    let text = if !harmony.text.is_empty() {
        harmony.text.as_str()
    } else {
        harmony.kind.as_str()
    };
    let kind = musicxml_kind(&harmony.kind);

    if pitch::tpc_is_valid(harmony.root_tpc) {
        harmony_root(out, "root", harmony.root_tpc);
        // TODO(parité): table complète `harmonyXmlKind` de MuseScore. Le
        // modèle ne garde que le nom interne, sans correspondance MusicXML ;
        // on retombe sur le texte comme MuseScore pour un accord non reconnu.
        match kind {
            Some(kind) if !text.is_empty() => {
                out.xml.tag_with("kind", &[("text", text)], kind);
            }
            Some(kind) => out.xml.tag("kind", kind),
            None if !text.is_empty() => {
                out.xml.tag_with("kind", &[("text", text)], "");
            }
            None => out.xml.tag("kind", "none"),
        }
        if let Some(bass) = harmony.bass_tpc {
            if pitch::tpc_is_valid(bass) {
                harmony_root(out, "bass", bass);
            }
        }
    } else if !text.is_empty() {
        out.xml.tag_with("kind", &[("text", text)], "");
    } else {
        out.xml.tag("kind", "none");
    }

    out.xml.end_element();
}

/// Écrit `<root>` ou `<bass>` d'un accord chiffré.
fn harmony_root(out: &mut Out, name: &str, tpc: i8) {
    let alter = pitch::tpc_alter(tpc);
    let step = pitch::tpc_step_name(tpc);
    out.xml.start_element(name, &[]);
    out.xml.tag(&format!("{name}-step"), &step.to_string());
    if alter != 0 {
        out.xml.tag(&format!("{name}-alter"), &alter.to_string());
    }
    out.xml.end_element();
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

/// Emprunte des attributs construits dynamiquement.
fn borrow_attributes<'a>(attributes: &'a [(&'a str, String)]) -> Vec<(&'a str, &'a str)> {
    attributes
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

/// Première valeur occupée d'une portée dans un segment.
pub(super) fn annotation_track(segment: &Segment, staff_id: StaffId) -> Option<Track> {
    (0..VOICES)
        .map(|voice| Track::new(staff_id, voice as u8))
        .find(|track| segment.element(*track).is_some())
}

/// Numéro de voix MusicXML (`voice` dans `ExportMusicXml::chord`).
fn voice_number(xml_staff: usize, voice: u8) -> i32 {
    if xml_staff == 0 {
        i32::from(voice) + 1
    } else {
        (xml_staff as i32 - 1) * VOICES as i32 + i32::from(voice) + 1
    }
}

/// Vrai si l'accord d'agrément vient après l'accord principal.
fn grace_is_after(kind: GraceType) -> bool {
    matches!(
        kind,
        GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After
    )
}

/// Vrai pour une petite note jouée comme repère (`isCueNote`).
fn is_cue_note(chord: &Chord, note: &Note) -> bool {
    chord.small && !note.play
}

fn syllabic_name(syllabic: LyricsSyllabic) -> &'static str {
    match syllabic {
        LyricsSyllabic::Single => "single",
        LyricsSyllabic::Begin => "begin",
        LyricsSyllabic::Middle => "middle",
        LyricsSyllabic::End => "end",
    }
}

pub(crate) fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

/// Correspondance minimale entre nom interne d'accord et valeur MusicXML.
///
/// Le modèle ne conserve pas la sémantique du chiffrage : seuls les noms qui
/// sont déjà des valeurs MusicXML sont reconnus.
fn musicxml_kind(kind: &str) -> Option<&'static str> {
    const KINDS: [&str; 39] = [
        "major",
        "minor",
        "augmented",
        "diminished",
        "dominant",
        "major-seventh",
        "minor-seventh",
        "diminished-seventh",
        "augmented-seventh",
        "half-diminished",
        "major-minor",
        "major-sixth",
        "minor-sixth",
        "dominant-ninth",
        "major-ninth",
        "minor-ninth",
        "dominant-11th",
        "major-11th",
        "minor-11th",
        "dominant-13th",
        "major-13th",
        "minor-13th",
        "suspended-second",
        "suspended-fourth",
        "Neapolitan",
        "Italian",
        "French",
        "German",
        "pedal",
        "power",
        "Tristan",
        "other",
        "none",
        "major-seventh-flat-five",
        "major-seventh-sharp-five",
        "major-seventh-sharp-eleventh",
        "minor-seventh-flat-five",
        "minor-seventh-sharp-five",
        "minor-major",
    ];
    KINDS
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(kind))
        .copied()
}

/// Texte brut d'un nom de partie, altérations remplacées comme MuseScore.
fn plain_part_name(part: &Part) -> String {
    if !part.long_name.is_empty() {
        plain_text(&part.long_name)
    } else if !part.name.is_empty() {
        plain_text(&part.name)
    } else {
        String::new()
    }
}

/// Remplace les bémols et dièses typographiques par leur équivalent ASCII.
fn plain_text(text: &str) -> String {
    text.replace('♭', "b").replace('♯', "#")
}

/// Nombre formaté comme MuseScore : entier si possible, sinon arrondi.
pub(crate) fn format_number(value: f64, decimals: usize) -> String {
    let factor = 10_f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    if rounded.fract() == 0.0 && rounded.abs() < 1e15 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

/// Cherche un segment d'un type et d'un tick donnés.
fn find_segment(measure: &Measure, kind: SegmentType, tick: Tick) -> Option<&Segment> {
    measure
        .segments
        .iter()
        .find(|segment| segment.kind == kind && segment.tick == tick)
}

/// Calcule les divisions (`calcDivisions`).
fn compute_divisions(context: &Context<'_>) -> u32 {
    let mut fractions: BTreeSet<Fraction> = BTreeSet::new();

    for part in &context.score.parts {
        let mut tick = Tick::ZERO;
        for measure in &context.score.measures {
            for staff_id in &part.staves {
                for voice in 0..VOICES {
                    let track = Track::new(*staff_id, voice as u8);
                    for segment in &measure.segments {
                        let Some(item) = segment.element(track) else {
                            continue;
                        };
                        if let SegmentItem::BarLine(barline) = item {
                            if barline.kind.is_repeat_start() {
                                continue;
                            }
                        }
                        if tick != segment.tick {
                            fractions.insert((segment.tick - tick).abs());
                            tick = segment.tick;
                        }
                        if let SegmentItem::ChordRest(chord_rest) = item {
                            let length = {
                                let mut length = context.global_ticks(chord_rest, measure);
                                // `calcDivisions` compte un trémolo à deux
                                // notes pour sa moitié.
                                if chord_rest
                                    .chord_ref()
                                    .is_some_and(|chord| chord.tremolo_two_chord)
                                {
                                    length /= Fraction::from_integer(2);
                                }
                                length
                            };
                            if !length.is_zero() {
                                fractions.insert(length.abs());
                            }
                            tick += length;
                        }
                    }
                }
            }
            let end = measure.end_tick();
            if end != tick {
                fractions.insert((end - tick).abs());
            }
            tick = end;
        }
    }

    let mut divisions: u128 = 4;
    for fraction in &fractions {
        divisions = lcm(divisions, u128::from(fraction.denominator().unsigned_abs()));
    }
    let result = divisions / 4;
    u32::try_from(result).unwrap_or(u32::MAX)
}

fn lcm(left: u128, right: u128) -> u128 {
    if left == 0 || right == 0 {
        return 0;
    }
    left / gcd(left, right) * right
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
