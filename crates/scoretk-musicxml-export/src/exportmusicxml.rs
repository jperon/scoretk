// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` et
// `importexport/musicxml/internal/shared/` de MuseScore Studio, Copyright (C)
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

use crate::Result;
use scoretk_core::model::chord::ChordRestKind;
use scoretk_core::model::stable_ids::{
    stable_grace_note_id_with_source, stable_measure_id, stable_note_id, stable_note_id_with_source,
};
use scoretk_core::model::{
    Accidental, AccidentalType, Annotation, Chord, ChordRest, Clef, ClefType, Duration, Extension,
    Fraction, GraceChord, GraceType, Instrument, InstrumentChange, Key, KeyMode, KeySig, Lyrics,
    LyricsSyllabic, Measure, Note, NoteHeadGroup, NoteHeadType, Part, Placement, Score, Segment,
    SegmentItem, SegmentType, SpannerType, StaffId, StemDirection, Tempo, TempoKind, Tick, TimeSig,
    TimeSigType, Track, Tuplet, TupletId, TupletNumberType, VOICES,
};

use super::beams;
use super::chant;
use super::figured_bass;
use super::harmony;
use super::layout;
use super::notations;
use super::nuances::{self, Nuances};
use super::ottava;
use super::pedals;
use super::pitch;
use super::repeats;
use super::rest_dots;
use super::stems;
use super::systems;
use super::text;
use super::textlines;
use super::tuplet_bracket;
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
    /// Hauteurs en sons réels (concert pitch). `None` (défaut) suit le style
    /// enregistré dans la partition (`StyleSettings::concert_pitch`,
    /// `Sid::concertPitch`) ; `Some(v)` est une surcharge explicite de
    /// l'appelant, notamment l'option wasm `concertPitch`.
    ///
    /// Sans style et sans surcharge, MuseScore exporte les hauteurs écrites
    /// (`false`), en utilisant `Note::tpc_transposed` et la transposition.
    pub concert_pitch: Option<bool>,
    /// Vrai pour émettre un `id` sur `<measure>` et `<note>`.
    ///
    /// Les identifiants sont calculés par [`scoretk_core::model::stable_ids`] à partir des
    /// seuls indices du modèle et restent donc stables d'un export à l'autre.
    /// Désactivé par défaut pour ne pas diverger des références MuseScore
    /// 4.7.4, qui n'écrivent aucun `id` sur ces éléments.
    pub stable_ids: bool,
    /// Vue « moderne » du chant grégorien (option additive `gabcModernClef`) :
    /// clef de sol (avec `8vb` si le registre est bas) et portée à 5 lignes.
    /// Sans l'option, la clé d'ut et les 4 lignes du `.gabc` sont conservées.
    pub gabc_modern_clef: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            software: DEFAULT_SOFTWARE.to_owned(),
            encoding_date: DEFAULT_ENCODING_DATE.to_owned(),
            concert_pitch: None,
            stable_ids: false,
            gabc_modern_clef: false,
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
    div: u32,
    pub(crate) tick: Tick,
    in_attributes: bool,
    /// Vrai si le `<attributes>` ouvert contient déjà un `<staff-details>` :
    /// une clef `after-barline` ne peut plus y être écrite (XSD 4.0).
    pub(crate) attributes_has_details: bool,
    /// État des notations à numéroter (liaisons, glissandos, arpèges).
    pub(crate) notations: notations::NotationsState,
}

impl Out {
    pub(crate) fn new() -> Self {
        Self {
            xml: XmlWriter::new(),
            div: 1,
            tick: Tick::ZERO,
            in_attributes: false,
            attributes_has_details: false,
            notations: notations::NotationsState::default(),
        }
    }

    /// Ouvre ou ferme `<attributes>` à la demande, comme `Attributes::doAttr`.
    pub(crate) fn attr(&mut self, open: bool) {
        if open && !self.in_attributes {
            self.xml.start_element("attributes", &[]);
            self.in_attributes = true;
            self.attributes_has_details = false;
        } else if !open && self.in_attributes {
            self.xml.end_element();
            self.in_attributes = false;
            self.attributes_has_details = false;
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
    /// Valeur effective : surcharge de l'appelant ou style de la partition.
    concert_pitch: bool,
    tuplets: TupletIndex,
    /// Ligatures reconstituées, comme les `Beam` de la mise en page.
    beams: beams::Beams,
    /// Systèmes explicites : portées masquées et barres de courtoisie.
    systems: systems::Systems,
    /// Groupes de silences multi-mesures déduits des mesures vides
    /// (`derived_mmrest_runs`) : MuseScore les reconstruit à la mise en page
    /// et les écrit dans toutes les parties (`measureStyle`,
    /// `exportmusicxml.cpp`).
    mmrests: Vec<Option<u16>>,
}

/// État des spanners d'une partie (`spannersStopped` et compteur d'instruments
/// dans le C++).
#[derive(Default)]
struct PartSpanners {
    /// Spanners dont le début a déjà été écrit.
    started: Vec<usize>,
    /// Spanners dont la fin a déjà été écrite.
    stopped: Vec<usize>,
    /// Numéro du prochain instrument introduit (`I1` étant le principal).
    next_instrument: usize,
    /// Chiffrages figurés en cours de prolongation (`FigBassMap`).
    fig_bass: figured_bass::Pending,
}

/// Index nécessaires aux identifiants stables d'un `ChordRest`.
///
/// Le rang est calculé par [`Context::write_measure_tracks`], dans l'ordre
/// exact du parcours d'écriture (silences compris), et transmis jusqu'à
/// [`Context::chord`] et [`Context::rest`].
#[derive(Clone, Copy)]
struct StableIds {
    /// Index de la mesure dans `Score::measures`.
    measure_index: usize,
    /// Rang du `ChordRest` dans sa voix, silences compris.
    chord_rank: usize,
}

/// Accord d'agrément en cours d'écriture (`Chord::grace_chords`).
#[derive(Clone, Copy)]
struct GraceRef<'a> {
    /// Durée notée de l'ornement.
    duration: &'a Duration,
    /// Position dans `Chord::grace_chords`.
    index: usize,
    /// Vrai si l'ornement précède l'accord principal.
    before: bool,
    /// Décalage de lecture par rapport au curseur courant, en noires.
    ///
    /// Un ornement après l'accord (`GRACE8_AFTER`, `\afterGrace`) est écrit
    /// après la note principale dans le flux, mais LilyPond le joue à
    /// `afterGraceFraction` (3/4 par défaut) de la durée de cette note
    /// (`ly/music-functions-init.ly:86-115`). Le `<offset>` MusicXML porte ce
    /// décalage ; négatif après la note, il vaut −1/4 de sa durée.
    offset: Option<Tick>,
    /// Ligature calculée pour cet ornement.
    beam: Option<&'a beams::GraceBeam>,
    /// Vrai si l'import ABC demande un `<time-modification>` sur l'agrément
    /// (`GraceChord::in_tuplet`).
    in_tuplet: bool,
}

impl<'a> Context<'a> {
    fn new(score: &'a Score, options: &'a ExportOptions) -> Self {
        // `ExportMusicXml::write` (exportmusicxml.cpp:8944) suit le style
        // `Sid::concertPitch` de la partition ; `pitch2xml` (l. 1654) s'en sert
        // pour annuler la transposition. Une surcharge explicite gagne.
        let concert_pitch = options
            .concert_pitch
            .unwrap_or(score.engraving.style.concert_pitch);
        Self {
            score,
            options,
            concert_pitch,
            tuplets: TupletIndex::new(score),
            beams: beams::Beams::new(score, concert_pitch),
            systems: systems::Systems::new(score),
            mmrests: scoretk_core::model::derived_mmrest_runs(score),
        }
    }

    /// Vrai si l'export écrit les hauteurs en sons réels (`concertPitch`).
    pub(crate) fn concert_pitch(&self) -> bool {
        self.concert_pitch
    }

    /// Tempo en battements par seconde en vigueur à un tick
    /// (`Score::tempo(tick)`).
    ///
    /// Pour « a tempo » et « Tempo I », MuseScore relit la carte des tempos
    /// au lieu du tempo propre au texte. Le modèle n'a pas de carte : le
    /// dernier tempo normal au tick ou avant en tient lieu.
    pub(crate) fn tempo_at(&self, tick: Tick) -> Option<f64> {
        let mut found = None;
        for measure in &self.score.measures {
            for segment in &measure.segments {
                if segment.tick > tick {
                    continue;
                }
                for entry in &segment.annotations {
                    if let Annotation::Tempo(tempo) = &entry.annotation {
                        if tempo.kind == TempoKind::Normal {
                            if let Some(beats_per_second) = tempo.beats_per_second {
                                found = Some(beats_per_second);
                            }
                        }
                    }
                }
            }
        }
        found
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
        layout::write_defaults(out, &self.score.engraving.style, self.concert_pitch);
        text::write_credits(out, self.score);
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
        // `identification` (`exportmusicxml.cpp:7237-7244`) : les cinq types
        // de créateurs sont rangés dans un `std::set`, donc par ordre
        // alphabétique. `poet` et `translator` sont des metaTags distincts de
        // `lyricist` dans les fichiers 4.x.
        for (kind, value) in [
            ("arranger", &metadata.arranger),
            ("composer", &metadata.composer),
            ("lyricist", &metadata.lyricist),
            ("poet", &metadata.poet),
            ("translator", &metadata.translator),
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
        // MuseScore écrit toutes les métadonnées non standard, triées par nom ;
        // `creationDate` et `platform` ont toujours une valeur (celle du
        // fichier, sinon celle de l'export).
        let creation_date = if metadata.creation_date.is_empty() {
            self.options.encoding_date.as_str()
        } else {
            metadata.creation_date.as_str()
        };
        out.xml.start_element("miscellaneous", &[]);
        out.xml.tag_with(
            "miscellaneous-field",
            &[("name", "creationDate")],
            creation_date,
        );
        if !metadata.msc_version.is_empty() {
            out.xml.tag_with(
                "miscellaneous-field",
                &[("name", "mscVersion")],
                &metadata.msc_version,
            );
        }
        if !metadata.platform.is_empty() {
            out.xml.tag_with(
                "miscellaneous-field",
                &[("name", "platform")],
                &metadata.platform,
            );
        }
        if !metadata.subtitle.is_empty() {
            out.xml.tag_with(
                "miscellaneous-field",
                &[("name", "subtitle")],
                &metadata.subtitle,
            );
        }
        out.xml.end_element();
        out.xml.end_element();
    }

    /// Écrit `<part-list>` (`partList`).
    ///
    /// TODO(parité): `<part-group>` (accolades, crochets, portées liées), qui
    /// demande les informations de bracket absentes du modèle.
    fn part_list(&self, out: &mut Out) {
        let hidden_name = self.score.parts.len() == 1;
        // `rebuildMidiMapping` attribue les canaux libres dans l'ordre des
        // parties, instruments puis canaux. Les canaux écrits dans le fichier
        // sont réservés d'avance et priment. Le canal 10 (percussion) reste
        // libre pour les instruments mélodiques.
        let mut used: BTreeSet<usize> = self
            .score
            .parts
            .iter()
            .flat_map(Part::instruments)
            .filter_map(|instrument| instrument.midi_channel.map(usize::from))
            .collect();
        let mut next_channel = 0usize;
        let mut take_free = || {
            while used.contains(&next_channel) || next_channel == 9 {
                next_channel += 1;
            }
            let channel = next_channel;
            used.insert(channel);
            next_channel += 1;
            channel
        };

        out.xml.start_element("part-list", &[]);
        // `partList` : groupes de parties ouverts par les accolades du premier
        // personnel, fermés quand `staffCount` atteint leur fin.
        let mut part_group_end: Vec<Option<usize>> = vec![None; 32];
        let mut staff_count = 0usize;
        for (index, part) in self.score.parts.iter().enumerate() {
            let number = index + 1;
            let part_id = format!("P{number}");

            self.part_groups(out, index, part, &mut part_group_end, staff_count);

            out.xml.start_element("score-part", &[("id", &part_id)]);

            let name = plain_part_name(part);
            let name_attributes: &[(&str, &str)] = if hidden_name {
                &[("print-object", "no")]
            } else {
                &[]
            };
            out.xml.tag_with("part-name", name_attributes, &name);
            // MuseScore décompose les noms contenant des altérations en
            // fragments `<display-text>`/`<accidental-text>`.
            if let Some(display) = part_display_name(part) {
                out.xml.start_element("part-name-display", name_attributes);
                write_display_name(out, display);
                out.xml.end_element();
            }

            if !part.short_name.is_empty() {
                let abbreviation = plain_text(&part.short_name);
                out.xml
                    .tag_with("part-abbreviation", name_attributes, &abbreviation);
                if contains_accidental(&part.short_name) {
                    out.xml
                        .start_element("part-abbreviation-display", name_attributes);
                    write_display_name(out, &part.short_name);
                    out.xml.end_element();
                }
            }

            // `scoreInstrument` puis `midiInstrument` pour chaque instrument de
            // la partie, dans l'ordre de `Part::instruments()`.
            let instruments: Vec<&Instrument> = part.instruments().collect();
            for (instrument_index, instrument) in instruments.iter().enumerate() {
                let instrument_id = format!("P{number}-I{}", instrument_index + 1);
                out.xml
                    .start_element("score-instrument", &[("id", &instrument_id)]);
                out.xml.tag("instrument-name", &instrument.track_name);
                if !instrument.id.is_empty() {
                    out.xml.tag("instrument-sound", &instrument.id);
                }
                out.xml.end_element();
            }
            let has_harmony = self.part_has_harmony(part);
            for (instrument_index, instrument) in instruments.iter().enumerate() {
                let instrument_id = format!("P{number}-I{}", instrument_index + 1);
                // MuseScore écrit un élément vide non auto-fermant pour
                // <midi-device> et y met `channel()->port() + 1`
                // (exportmusicxml.cpp:7939-7947 : le port du premier canal de
                // l'instrument, ou `part->midiPort() + 1` sans canal). Le port
                // par défaut reste 1 quand le fichier ne le fixe pas.
                let port = instrument.midi_port.map_or(1, |port| u16::from(port) + 1);
                let port = port.to_string();
                out.xml.tag_with(
                    "midi-device",
                    &[("id", &instrument_id), ("port", &port)],
                    "",
                );
                out.xml
                    .start_element("midi-instrument", &[("id", &instrument_id)]);
                let midi_channel = match instrument.midi_channel {
                    Some(channel) => usize::from(channel),
                    None => {
                        let channel = take_free();
                        // Chaque `<Channel>` secondaire occupe un canal dans
                        // `rebuildMidiMapping` (`midimapping.cpp:286-310`).
                        for _ in 1..instrument.channel_count.max(1) {
                            let _ = take_free();
                        }
                        // Le canal « harmony » occupe aussi un canal, mais
                        // n'est pas exporté ; il est déjà compté parmi les
                        // canaux de l'instrument quand il est déclaré.
                        if instrument_index == 0 && has_harmony && instrument.channel_count <= 1 {
                            let _ = take_free();
                        }
                        channel
                    }
                };
                // MuseScore n'écrit le canal que s'il tient dans 1..=16.
                if midi_channel < 16 {
                    out.xml.tag("midi-channel", &(midi_channel + 1).to_string());
                }
                if let Some(program) = instrument.midi_program {
                    if u16::from(program) < 128 {
                        out.xml
                            .tag("midi-program", &(u16::from(program) + 1).to_string());
                    }
                }
                // `midiInstrument` : volume et panorama viennent des
                // contrôleurs du canal, avec les valeurs par défaut de
                // MuseScore (100/127 et 64).
                let volume = f64::from(instrument.volume.unwrap_or(100)) / 127.0 * 100.0;
                // `midiInstrument` (`exportmusicxml.cpp:7353`) passe un double
                // à `XmlWriter::tag`, qui écrit six chiffres significatifs.
                out.xml.tag("volume", &format_number_significant(volume, 6));
                let pan = f64::from(instrument.pan.unwrap_or(64));
                let pan = ((pan - 63.5) / 63.5 * 90.0) as i32;
                out.xml.tag("pan", &pan.to_string());
                out.xml.end_element();
            }

            out.xml.end_element();

            staff_count += part.staves.len();
            // `partList` (`exportmusicxml.cpp:7953-7960`) parcourt
            // `partGroupEnd` de MAX_PART_GROUPS-1 à 0 : les groupes imbriqués
            // se ferment en pile, du plus grand numéro au plus petit.
            for (group, end) in part_group_end.iter_mut().enumerate().rev() {
                if end.is_some_and(|end| staff_count >= end) {
                    let group = (group + 1).to_string();
                    out.xml
                        .tag_empty("part-group", &[("type", "stop"), ("number", &group)]);
                    *end = None;
                }
            }
        }
        out.xml.end_element();
    }

    /// Ouvre les `<part-group>` d'une partie (`partList`).
    fn part_groups(
        &self,
        out: &mut Out,
        index: usize,
        part: &Part,
        part_group_end: &mut [Option<usize>],
        staff_count: usize,
    ) {
        let Some(first_staff) = part.staves.first() else {
            return;
        };
        let staff = self.score.staff(*first_staff);
        let mut bracket_found = false;
        for bracket in &staff.brackets {
            if bracket.kind < 0 {
                continue;
            }
            bracket_found = true;
            // Une accolade qui couvre exactement la partie est implicite : ne
            // pas l'exporter (MuseScore filtre les accolades « brace »).
            if bracket.span == part.staves.len() && bracket.kind == 1 {
                continue;
            }
            if index + 1 >= self.score.parts.len() {
                continue;
            }
            if let Some(group) = part_group_end.iter().position(Option::is_none) {
                self.part_group_start(
                    out,
                    group + 1,
                    bracket_symbol(bracket.kind),
                    staff.bar_line_span,
                );
                part_group_end[group] = Some(staff_count + bracket.span);
            }
        }
        // Partie à plusieurs portées sans accolade : groupe sans symbole, pour
        // que les portées restent liées dans MusicXML.
        if !bracket_found && part.staves.len() > 1 {
            if let Some(group) = part_group_end.iter().position(Option::is_none) {
                self.part_group_start(out, group + 1, "none", false);
                part_group_end[group] = Some(staff_count + part.staves.len());
            }
        }
    }

    /// Écrit un `<part-group type="start">`.
    fn part_group_start(&self, out: &mut Out, number: usize, symbol: &str, barline: bool) {
        let number = number.to_string();
        out.xml
            .start_element("part-group", &[("type", "start"), ("number", &number)]);
        out.xml.tag("group-symbol", symbol);
        if barline {
            out.xml.tag("group-barline", "yes");
        }
        out.xml.end_element();
    }

    /// Vrai si la partie contient un accord chiffré.
    ///
    /// `Part::updateHarmonyChannels` ajoute un canal « harmony » à son premier
    /// instrument, ce qui décale les canaux MIDI des instruments suivants.
    fn part_has_harmony(&self, part: &Part) -> bool {
        part.staves.iter().any(|staff_id| {
            self.score.measures.iter().any(|measure| {
                measure.segments.iter().any(|segment| {
                    segment.annotations.iter().any(|entry| {
                        entry.track.staff() == *staff_id
                            && matches!(entry.annotation, Annotation::Harmony(_))
                    })
                })
            })
        })
    }

    /// Écrit toutes les parties (`writeParts`).
    fn write_parts(&self, out: &mut Out, nuances: &mut Nuances) {
        for part_index in 0..self.score.parts.len() {
            let part_id = format!("P{}", part_index + 1);
            out.tick = Tick::ZERO;
            out.in_attributes = false;
            nuances.reset_part();
            out.notations.reset_part();
            // I1 est l'instrument principal ; les changements d'instrument
            // prennent les numéros suivants (`initInstrMap`).
            let mut spanners = PartSpanners {
                started: Vec::new(),
                stopped: Vec::new(),
                next_instrument: 2,
                fig_bass: figured_bass::Pending::default(),
            };
            out.xml.start_element("part", &[("id", &part_id)]);

            for measure_index in 0..self.score.measures.len() {
                self.write_measure(out, measure_index, part_index, nuances, &mut spanners);
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
        spanners: &mut PartSpanners,
    ) {
        let measure = &self.score.measures[measure_index];
        let part = &self.score.parts[part_index];
        let staves = part.staves.len();
        // Un saut de section rouvre une section : la numérotation repart à 1
        // et les `<attributes>` de début de section sont réécrits
        // (`MeasureNumberStateHandler`). `isFirstActualMeasure`
        // (`exportmusicxml.cpp:8307-8310`) ne redevient vrai qu'après
        // `init()`, appelé seulement si le saut porte `startWithMeasureOne`
        // (`updateForMeasure`, `exportmusicxml.cpp:8281-8286`). Un saut qui
        // conserve la numérotation ne réécrit donc ni `<divisions>` ni
        // `<staves>` (« TecumPrincipium fredipi » : 2 blocs en trop).
        let is_first = measure_index == 0
            || (measure.section_break_before && !measure.section_keeps_numbering);

        let (number, implicit) = self.measure_number(measure_index);
        // `stable_ids` ajoute `id` et `xml:id` de mesure (même valeur),
        // uniques par partie ; sans lui, la sortie reste identique aux
        // références MuseScore 4.7.4.
        let measure_id = self
            .options
            .stable_ids
            .then(|| stable_measure_id(part_index, measure_index));
        let mut measure_attributes: Vec<(&str, &str)> = vec![("number", &number)];
        if implicit {
            measure_attributes.push(("implicit", "yes"));
        }
        if let Some(id) = &measure_id {
            measure_attributes.push(("id", id));
            measure_attributes.push(("xml:id", id));
        }
        out.xml.start_element("measure", &measure_attributes);
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
        if is_first && !part.instrument_changes.is_empty() {
            out.xml.tag(
                "instruments",
                &(part.instrument_changes.len() + 1).to_string(),
            );
        }
        self.write_measure_clefs(out, measure, measure_index, part);
        if is_first {
            self.write_staff_details(out, measure_index, part);
            self.write_instrument_details(out, part, measure.start_tick);
        } else {
            // Portées masquées du système (`hideEmptyStaves`) : MuseScore les
            // signale à la première mesure du système, une `<attributes>` par
            // portée (`writeMeasure`).
            for &hidden in self.systems.hidden_at(measure_index) {
                // La première portée du score n'est jamais signalée :
                // `print()` ne calcule les distances qu'à partir de la
                // deuxième portée de la première partie.
                if hidden == 0 {
                    continue;
                }
                let Some(position) = part
                    .staves
                    .iter()
                    .position(|staff_id| staff_id.raw() as usize == hidden)
                else {
                    continue;
                };
                let number = (position + 1).to_string();
                let mut attributes: Vec<(&str, &str)> = Vec::new();
                if staves > 1 {
                    attributes.push(("number", &number));
                }
                attributes.push(("print-object", "no"));
                out.attr(true);
                out.xml.tag_empty("staff-details", &attributes);
                out.attributes_has_details = true;
                out.attr(false);
            }
        }
        // `measureStyle` : un `<measure-style>` ouvre le groupe de silences
        // multi-mesures, sur la première mesure du groupe. Le regroupement est
        // global à la partition : `MeasureLayout::createMultiMeasureRestsIfNeed`
        // remplace une suite de mesures vides et `measureStyle`
        // (`exportmusicxml.cpp:6581-6601`) l'écrit pour chaque partie. Les
        // marqueurs `<multiMeasureRest>` lus restent prioritaires ; sinon le
        // groupe est déduit des mesures vides (`derived_mmrest_runs`).
        let count = part
            .staves
            .iter()
            .find_map(|staff| measure.mmrests.count_for(*staff))
            .or(measure.multi_measure_rest)
            .or_else(|| self.mmrests.get(measure_index).copied().flatten());
        if let Some(count) = count {
            out.attr(true);
            out.xml.start_element("measure-style", &[]);
            out.xml.tag("multiple-rest", &count.to_string());
            out.xml.end_element();
        }
        // `measureStyle` : une mesure répétée ouvre un groupe
        // `<measure-style><measure-repeat>`.
        repeats::measure_style(self, out, measure, measure_index, part_index);

        repeats::repeat_at_measure_start(self, out, measure, part_index);

        // Les textes de `TBox` ne sont écrits que par la première partie, au
        // début de la mesure qui suit le cadre (`tboxTextAsWords`).
        if part_index == 0 {
            // `write_tbox` ferme les attributs lui-même s'il écrit un texte.
            textlines::write_tboxes(out, self, measure_index);
        }

        // Chant grégorien : les faits que MusicXML sait porter sont écrits
        // comme des directions et notations structurées, jamais comme une
        // copie de la source `.gabc` (`musicxml-4.0.xsd` : other-direction,
        // other-notation, other-technical ; `Gabc.tex:228-343,801-870`).
        if part_index == 0 {
            if let Some(Extension::Gregorian(gregorian)) = &self.score.extensions.format {
                out.attr(false);
                if measure_index == 0 {
                    chant::write_header_directions(out, gregorian);
                    chant::write_all_syllable_directions(out, gregorian);
                }
            }
        }

        self.write_measure_staves(
            out,
            measure,
            part,
            nuances,
            part_index,
            measure_index,
            spanners,
        );
        self.annotations_without_note(out, measure, part);

        out.move_to_tick(measure.end_tick());
        repeats::repeat_at_measure_stop(self, out, measure, part_index);
        self.barline_right(out, measure, part_index, measure_index);
        out.attr_stop();

        out.xml.end_element();
    }

    /// Écrit les portées d'une mesure pour une partie (`writeMeasureStaves`).
    #[allow(clippy::too_many_arguments)]
    fn write_measure_staves(
        &self,
        out: &mut Out,
        measure: &Measure,
        part: &Part,
        nuances: &mut Nuances,
        part_index: usize,
        measure_index: usize,
        spanners: &mut PartSpanners,
    ) {
        let staves = part.staves.len();
        out.notations.start_measure();
        for (position, staff_id) in part.staves.iter().enumerate() {
            out.move_to_tick(measure.start_tick);
            let xml_staff = if staves > 1 { position + 1 } else { 0 };
            let mut started = vec![false; self.score.spanners.len()];
            let mut stopped = vec![false; self.score.spanners.len()];
            self.write_measure_tracks(
                out,
                measure,
                *staff_id,
                xml_staff,
                nuances,
                part_index,
                measure_index,
                position,
                spanners,
                &mut started,
                &mut stopped,
            );
            if position + 1 < staves {
                // Retour au début de la mesure pour la portée suivante.
                out.move_to_tick(measure.start_tick);
            }
        }
    }

    /// Écrit les quatre voix d'une portée (`writeMeasureTracks`).
    #[allow(clippy::too_many_arguments)]
    fn write_measure_tracks(
        &self,
        out: &mut Out,
        measure: &Measure,
        staff_id: StaffId,
        xml_staff: usize,
        nuances: &mut Nuances,
        part_index: usize,
        measure_index: usize,
        staff_position: usize,
        spanners: &mut PartSpanners,
        ottava_started: &mut [bool],
        ottava_stopped: &mut [bool],
    ) {
        for voice in 0..VOICES {
            let track = Track::new(staff_id, voice as u8);
            // Rang du `ChordRest` dans la voix (`ids.chord_rank`), silences
            // compris : c'est l'ordre même de ce parcours, comme
            // `writeMeasureTracks` dans le C++.
            let mut chord_rank = 0usize;
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

                if matches!(item, SegmentItem::ChordRest(_)) {
                    out.attr(false);
                    // Changement d'instrument : MuseScore l'écrit une fois, au
                    // début de la partie, avant les annotations du segment.
                    if staff_position == 0 && voice == 0 {
                        if let Some(change) = instrument_change(segment, staff_id) {
                            textlines::write_instrument_change(
                                out,
                                self,
                                change,
                                part_index,
                                spanners.next_instrument,
                            );
                            spanners.next_instrument += 1;
                        }
                    }
                    // `writeMeasureTracks` appelle `harmonies` pour **chaque**
                    // piste portant un élément, avec cette piste : une harmonie
                    // attachée à la voix 2 est écrite par la boucle de cette
                    // voix, pas par celle de la voix 1.
                    self.harmonies(out, measure, segment, staff_id, track);
                    if annotation_track(segment, staff_id) == Some(track) {
                        self.annotations(out, segment, staff_id, xml_staff, false);
                        if let SegmentItem::ChordRest(chord_rest) = item {
                            figured_bass::write(
                                self,
                                out,
                                measure,
                                segment,
                                chord_rest,
                                staff_id,
                                &mut spanners.fig_bass,
                            );
                        }
                    }
                    // `spannerStart` : les directions de spanner suivent
                    // l'ordre des spanners, comme la `spannerMap` du C++.
                    for (index, spanner) in self.score.spanners.iter().enumerate() {
                        if spanner.start.tick != segment.tick
                            || spanner.start.track.staff() != staff_id
                            || spanners.started.contains(&index)
                        {
                            continue;
                        }
                        match spanner.kind {
                            SpannerType::TextLine => textlines::start(out, spanner, xml_staff),
                            SpannerType::Hairpin(_) => {
                                nuances.start_one(self.score, out, index, xml_staff)
                            }
                            SpannerType::Ottava(_) => ottava::write_start(
                                self.score,
                                out,
                                index,
                                xml_staff,
                                ottava_started,
                            ),
                            SpannerType::Pedal => pedals::start(out, spanner, xml_staff),
                            _ => continue,
                        }
                        spanners.started.push(index);
                    }
                }
                let is_chord_rest = matches!(item, SegmentItem::ChordRest(_));
                let ids = StableIds {
                    measure_index,
                    chord_rank,
                };

                self.write_element(out, segment, item, measure, xml_staff, track, ids);

                if is_chord_rest {
                    // Le rang avance après chaque `ChordRest`, silences et
                    // silences de portée (`gap`) compris, pour rester aligné
                    // sur le parcours d'écriture.
                    chord_rank += 1;
                    nuances.stop(self.score, out, out.tick, staff_id, xml_staff);
                    // Fins de lignes de texte (`spannerStop`), une seule fois.
                    for (index, spanner) in self.score.spanners.iter().enumerate() {
                        if spanner.kind != SpannerType::TextLine
                            || spanner.end.tick != out.tick
                            || spanner.start.track.staff() != staff_id
                            || spanners.stopped.contains(&index)
                        {
                            continue;
                        }
                        textlines::stop(out, spanner, xml_staff);
                        spanners.stopped.push(index);
                    }
                    // `spannerStop` : pédales et ottavas finissant à ce tick.
                    for spanner in &self.score.spanners {
                        if spanner.kind == SpannerType::Pedal
                            && spanner.visible
                            && spanner.start.track.staff() == staff_id
                            && spanner.end.tick == out.tick
                        {
                            pedals::stop(out, spanner, xml_staff);
                        }
                    }
                    ottava::write_stops_at(
                        self.score,
                        out,
                        out.tick,
                        staff_id,
                        xml_staff,
                        ottava_stopped,
                    );
                }
            }
        }
    }

    /// Écrit un élément de segment (`writeElement`).
    #[allow(clippy::too_many_arguments)]
    fn write_element(
        &self,
        out: &mut Out,
        segment: &Segment,
        item: &SegmentItem,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
        ids: StableIds,
    ) {
        match item {
            SegmentItem::Clef(clef) => {
                self.mid_measure_clef(out, segment, clef, measure, xml_staff, track);
            }
            SegmentItem::ChordRest(chord_rest) => match &chord_rest.kind {
                ChordRestKind::Chord(chord) => {
                    self.chord_entry(out, chord_rest, chord, measure, xml_staff, track, ids);
                }
                ChordRestKind::Rest(rest) => {
                    if !rest.gap {
                        self.rest(out, chord_rest, measure, xml_staff, track, ids);
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

    /// Vrai pour une vue moderne dérivée d'un payload grégorien (L6).
    ///
    /// Les retenues progressives avant un quilisma sont des durées
    /// d'exécution (`ChordRest::ticks`, utilisées par le MIDI) et non des
    /// durées notées : la projection grégorienne garde une noire par lettre
    /// chantée (`docs/agents/gabc.md` §7). Les écrire dans `<duration>`
    /// décalerait toute la grille d'attaques et casserait l'aller-retour par
    /// du MusicXML standard, qui ne connaît ni le quilisma ni les mesures
    /// inventées du chant.
    fn derived_chant(&self) -> bool {
        matches!(
            &self.score.extensions.format,
            Some(Extension::Gregorian(gregorian)) if gregorian.derived_modern_view
        )
    }

    /// Durée réelle (`globalTicks`), n-olets appliqués.
    ///
    /// MuseScore divise la durée notée par le ratio de chaque n-olet parent.
    /// Le modèle n'a pas d'étirement de portée (`Staff::timeStretch`), le
    /// facteur correspondant vaut donc un. Quand `<duration>` diffère de la
    /// figure (trémolo, figure étirée), sa valeur fait foi.
    fn global_ticks(&self, chord_rest: &ChordRest, measure: &Measure) -> Fraction {
        let mut ticks = if self.derived_chant() {
            self.notated_ticks(chord_rest, measure)
        } else {
            chord_rest
                .ticks
                .unwrap_or_else(|| self.notated_ticks(chord_rest, measure))
        };
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
    pub(crate) fn actual_ticks(
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
    fn transpose_chromatic(&self, staff_id: StaffId, tick: Tick) -> i32 {
        if self.concert_pitch {
            return 0;
        }
        let part = self.score.part(self.score.staff(staff_id).part);
        part.instrument_at(tick)
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
        let bracket = if tuplet.bracket_explicit {
            tuplet.bracket
        } else {
            tuplet_bracket::has_bracket(self.score, id, self.concert_pitch(), &self.beams)
        };
        attributes.push(("bracket", if bracket { "yes" } else { "no" }.to_owned()));
        match tuplet.number_type {
            TupletNumberType::Number => {}
            TupletNumberType::Ratio => attributes.push(("show-number", "both".to_owned())),
            TupletNumberType::None => attributes.push(("show-number", "none".to_owned())),
        }
        if let Some(placement) = tuplet.placement {
            attributes.push((
                "placement",
                match placement {
                    Placement::Above => "above",
                    Placement::Below => "below",
                }
                .to_owned(),
            ));
        }

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
        // `determineTupletNormalTicks` (`exportmusicxml.cpp:3767`) compare
        // `elements().size()` au numérateur du ratio tel que le C++ le stocke,
        // c'est-à-dire **non réduit** (6 pour un 6:4). `Fraction::numerator()`
        // renvoie 3 ici et ferait écrire un `<normal-type>` superflu.
        if children.len() as i64 != tuplet.actual_notes.max(1) {
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
            // `timeModification` (`exportmusicxml.cpp:4199-4213`) multiplie
            // les nombres bruts : une Fraction réduite changerait un 6:4 en
            // 3:2. Le modèle conserve les nombres affichés du fichier.
            let (act, nrm) = value.displayed_numbers();
            actual *= act.max(1);
            normal *= nrm.max(1);
            tuplet = value.parent;
            depth += 1;
        }
        // `Fraction::operator==` compare par valeur : 4:4 == 1:1, donc le
        // C++ (`ratio != Fraction(1,1)`, `:4255`) n'écrit rien pour ces
        // n-olets, tout en conservant 6:4 (qui vaut 3:2 mais s'écrit brut).
        if actual == normal {
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
    #[allow(clippy::too_many_arguments)]
    fn chord_entry(
        &self,
        out: &mut Out,
        chord_rest: &ChordRest,
        chord: &Chord,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
        ids: StableIds,
    ) {
        // Un accord sans hauteur (silence déguisé, `\skip` portant une
        // annotation comme le tempo d'ouverture) n'écrit aucune note : sans
        // cette sortie, `chord()` avançait quand même le curseur et la
        // portée laissait un `<backup>` sans contenu, qui décalait la
        // lecture de toutes les portées suivantes (Symphony5_1/2, deux
        // mesures perdues à l'ouverture).
        if chord.notes.is_empty() && chord.grace_chords.is_empty() {
            return;
        }
        self.grace_entries(
            out,
            &chord.grace_chords,
            true,
            chord_rest,
            measure,
            xml_staff,
            track,
            ids,
        );
        self.chord(out, chord, None, chord_rest, measure, xml_staff, track, ids);
        self.grace_entries(
            out,
            &chord.grace_chords,
            false,
            chord_rest,
            measure,
            xml_staff,
            track,
            ids,
        );
    }

    /// Écrit un groupe d'agréments (avant ou après l'élément principal).
    /// `lily/grace-iterator.cc:24-42` les joue dans la partie « grâce » du
    /// moment courant, même quand cet élément est un silence. L'ordre des
    /// deux groupes est significatif pour la parité MusicXML.
    #[allow(clippy::too_many_arguments)]
    fn grace_entries(
        &self,
        out: &mut Out,
        grace_chords: &[GraceChord],
        before: bool,
        chord_rest: &ChordRest,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
        ids: StableIds,
    ) {
        let indices: Vec<usize> = (0..grace_chords.len())
            .filter(|index| {
                let kind = grace_chords[*index]
                    .chord
                    .grace
                    .unwrap_or(GraceType::Appoggiatura);
                grace_is_after(kind) != before
            })
            .collect();
        if indices.is_empty() {
            return;
        }
        // `BeamLayout::beamGraceNotes` : les ornements avant et après forment
        // deux groupes distincts.
        let multiple_voices = stems::has_other_voices(
            measure,
            track,
            out.tick,
            self.actual_ticks(chord_rest, measure, track.staff()),
        );
        let track_even = track.raw().is_multiple_of(2);
        let beams = beams::grace_beams(grace_chords, &indices, multiple_voices, track_even);

        for (position, index) in indices.iter().enumerate() {
            let grace = &grace_chords[*index];
            let main_ticks = self.actual_ticks(chord_rest, measure, track.staff());
            let grace_ref = GraceRef {
                duration: &grace.duration,
                index: *index,
                before,
                offset: if before {
                    None
                } else {
                    grace.played_offset.map(|offset| offset - main_ticks)
                },
                beam: beams[position].as_ref(),
                in_tuplet: grace.in_tuplet,
            };
            self.chord(
                out,
                &grace.chord,
                Some(&grace_ref),
                chord_rest,
                measure,
                xml_staff,
                track,
                ids,
            );
        }
    }

    /// Rang de la partie à laquelle appartient une portée.
    fn part_index_for_staff(&self, staff: StaffId) -> Option<usize> {
        let part_id = self.score.staff(staff).part;
        self.score.parts.iter().position(|part| part.id == part_id)
    }

    /// Écrit un accord (`chord`).
    #[allow(clippy::too_many_arguments)]
    fn chord(
        &self,
        out: &mut Out,
        chord: &Chord,
        grace: Option<&GraceRef<'_>>,
        chord_rest: &ChordRest,
        measure: &Measure,
        xml_staff: usize,
        track: Track,
        ids: StableIds,
    ) {
        let segment_tick = out.tick;
        let global_ticks = self.global_ticks(chord_rest, measure);
        if grace.is_none() {
            out.tick += self.actual_ticks(chord_rest, measure, track.staff());
        }

        let grace_duration = grace.map(|grace| grace.duration);
        let duration = grace_duration.copied().unwrap_or(chord_rest.duration);
        let notated = grace_duration.map_or_else(
            || self.notated_ticks(chord_rest, measure),
            |duration| duration.ticks(),
        );
        let voice = voice_number(xml_staff, track.voice());
        let transpose = self.transpose_chromatic(track.staff(), segment_tick);
        // Hauteur écrite : une ottava décale l'octave exportée.
        let ottava = ottava::shift(self.score, track.staff(), segment_tick);
        // `attack` (MusicXML 4.0, attribut de `<note>`) décale la lecture sans
        // déplacer le curseur logique ; `<offset>` n'est pas autorisé sur une
        // note (xsd, complexType note, attribut attack).
        let attack = grace
            .and_then(|grace| grace.offset)
            .map(|offset| out.duration_in_divisions(offset).to_string());

        for (index, note_id) in chord.notes.iter().enumerate() {
            let note = self.score.note(*note_id);
            let first = index == 0;

            out.attr(false);
            // Vélocité utilisateur (import MIDI) : MuseScore l'écrit en
            // `dynamics`, avec son échelle 90 = 100 % et rien pour zéro.
            let dynamics = note
                .playback
                .velocity
                .filter(|velocity| *velocity != 0)
                .map(|velocity| format_dynamics(f64::from(velocity) * 100.0 / 90.0));
            // `stable_ids` : identifiant de note calculé à partir du rang réel
            // du `ChordRest` dans la voix, du côté de l'ornement et de l'index
            // de la note dans l'accord.
            let note_id = self.options.stable_ids.then(|| match grace {
                Some(grace) => stable_grace_note_id_with_source(
                    ids.measure_index,
                    track.staff().raw(),
                    track.voice(),
                    ids.chord_rank,
                    grace.index,
                    index,
                    grace.before,
                    note.source,
                ),
                None => stable_note_id_with_source(
                    ids.measure_index,
                    track.staff().raw(),
                    track.voice(),
                    ids.chord_rank,
                    index,
                    note.source,
                ),
            });
            let mut note_attributes: Vec<(&str, &str)> = Vec::new();
            if let Some(value) = &dynamics {
                note_attributes.push(("dynamics", value));
            }
            if let Some(id) = &note_id {
                note_attributes.push(("id", id));
                note_attributes.push(("xml:id", id));
            }
            if let Some(attack) = &attack {
                note_attributes.push(("attack", attack));
            }
            out.xml.start_element("note", &note_attributes);

            if grace.is_some() {
                // MuseScore affiche la barre oblique pour une acciaccatura,
                // même quand le fichier ne stocke pas le drapeau.
                if chord.grace_slash || chord.grace == Some(GraceType::Acciaccatura) {
                    out.xml.tag_empty("grace", &[("slash", "yes")]);
                } else {
                    out.xml.tag_empty("grace", &[]);
                }
            }
            // `isCueNote` (`exportmusicxml.cpp:4190-4193`) est
            // `isSmallNote && !play` : MuseScore n'écrit **aucun** `<cue/>` pour
            // une note ordinaires portant `<play>0</play>` (vérifié dans les
            // références 4.7.4 Vivaldi et Angels_Carol). Émettre `<cue/>` dès
            // que `!play` était donc une divergence d'export.
            //
            // `Note::unperformed` garde les seules musiques gravées sans
            // performer MIDI (clusters/NullVoice de LilyPond), sans équivalent
            // MuseScore : `<cue/>` les rend identifiables à l'oracle LilyPond
            // sans toucher aux partitions MuseScore (XSD : la branche `cue`
            // exclut `tie`, musicxml-4.0.xsd:5182-5200).
            let cue = is_cue_note(chord, note) || note.playback.unperformed;
            if cue {
                out.xml.tag_empty("cue", &[]);
            }
            if !first {
                out.xml.tag_empty("chord", &[]);
            }

            write_pitch(out, note, self.concert_pitch, transpose, ottava);

            if grace.is_none() {
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

            // `<instrument>` désigne l'instrument en vigueur quand la partie
            // en change en cours de route (`m_instrMap.size() > 1`).
            if let Some(part_index) = self.part_index_for_staff(track.staff()) {
                let part = &self.score.parts[part_index];
                if !part.instrument_changes.is_empty() {
                    let index = part.instrument_index_at(segment_tick);
                    let id = format!("P{}-I{}", part_index + 1, index + 1);
                    out.xml.tag_empty("instrument", &[("id", &id)]);
                }
            }

            out.xml.tag("voice", &voice.to_string());
            write_type(out, notated, chord, note, grace.is_some());
            if let Some(accidental) = &note.accidental {
                write_accidental(out, accidental);
            }
            // `writeTimeModification` lit `chord()->tuplet()` : un accord
            // d'agrément n'appartient à aucun n-olet dans MuseScore, donc la
            // référence n'écrit rien. L'import ABC, lui, marque les agréments
            // du groupe `(3…)` (`abc2xml.py::mkNote`, `GraceChord::in_tuplet`).
            if grace.is_none() || grace.is_some_and(|grace| grace.in_tuplet) {
                self.write_time_modification(out, chord_rest);
            }

            // Pas de hampe pour les rondes et les valeurs plus longues.
            // MuseScore écrit toujours une hampe dès qu'il y en a une ; la
            // direction automatique se calcule comme `Chord::up()`.
            if duration.kind.has_stem() {
                // Le chant grégorien est sans hampe (`Gabc.tex:452-700`) :
                // XSD 4.0 `stem-value` autorise explicitement `none`. Ce
                // champ standard reste lisible par un tiers, sans changer
                // l'inventaire du modèle.
                if note.source.is_some()
                    && matches!(self.score.extensions.format, Some(Extension::Gregorian(_)))
                {
                    out.xml.tag("stem", "none");
                } else {
                    let actual = if grace.is_some() {
                        Fraction::ZERO
                    } else {
                        self.actual_ticks(chord_rest, measure, track.staff())
                    };
                    let up = stems::stem_up(
                        self.score,
                        chord,
                        measure,
                        track,
                        segment_tick,
                        actual,
                        self.concert_pitch,
                        transpose,
                        grace.and_then(|grace| grace.beam).map_or_else(
                            || self.beams.up(measure.number, track, segment_tick),
                            |beam| Some(beam.up),
                        ),
                    );
                    out.xml.tag("stem", if up { "up" } else { "down" });
                }
            }

            write_notehead(out, note, chord, duration);
            // Une alteration grégorienne `x`/`y`/`#` n'est **pas** une note
            // chantée (`Gabc.tex:481-489,512-540`) : la projection dérivée ne
            // lui crée aucun `Note`, donc aucun `<notehead>x</notehead>` ne
            // doit être émis ici. Son glyphe reste dans le champ privé L6.

            // `staff + note->chord()->staffMove()` : un accord croisé change
            // de portée sans changer de piste.
            let written_staff = xml_staff as i32 + i32::from(chord.staff_move);
            if written_staff > 0 {
                out.xml.tag("staff", &written_staff.to_string());
            }

            if first {
                if let Some(grace) = grace {
                    if let Some(beam) = grace.beam {
                        for (level, text) in beam.texts.iter().enumerate() {
                            let number = (level + 1).to_string();
                            out.xml.tag_with("beam", &[("number", &number)], text);
                        }
                    }
                } else {
                    self.write_beams(out, measure, track, segment_tick);
                }
            }

            let mut notations = Notations::new();
            if note.tie_back.is_some() {
                notations.tag_element(&mut out.xml, None, note.tie_back_visible);
                out.xml.tag_empty("tied", &[("type", "stop")]);
            }
            if note.tie_forward.is_some() {
                notations.tag_element(&mut out.xml, None, note.tie_forward_visible);
                // `slurTieLineStyle` : la direction explicite du `<Tie>` est
                // exportée en `orientation` et `placement`.
                match note.tie_up {
                    Some(up) => {
                        let (orientation, placement) = if up {
                            ("over", "above")
                        } else {
                            ("under", "below")
                        };
                        out.xml.tag_empty(
                            "tied",
                            &[
                                ("orientation", orientation),
                                ("placement", placement),
                                ("type", "start"),
                            ],
                        );
                    }
                    None => out.xml.tag_empty("tied", &[("type", "start")]),
                }
            }
            if first && grace.is_none() {
                self.tuplet_start_stop(out, chord_rest, segment_tick, track, &mut notations);
            }
            if first {
                self.chord_attributes(
                    out,
                    chord,
                    grace.is_some(),
                    measure,
                    segment_tick,
                    track,
                    &mut notations,
                );
            }
            notations::fingering(&mut out.xml, note, &mut notations);
            notations::tremolo_bar(
                &mut out.xml,
                measure,
                note,
                segment_tick,
                track,
                &mut notations,
            );
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

            if let Some(Extension::Gregorian(gregorian)) = &self.score.extensions.format {
                // La projection saute les alterations non chantées : le rang
                // source est porté par `Note::source`, pas par le rang
                // `ChordRest` de la mesure (qui peut diverger).
                if let Some(source) = note.source {
                    if let Some(gregorian_note) = gregorian
                        .syllables
                        .get(source.gregorian_syllable)
                        .and_then(|syllable| syllable.notes.get(source.gregorian_note))
                    {
                        chant::write_note_notation(
                            out,
                            gregorian_note,
                            source.gregorian_syllable,
                            source.gregorian_note,
                        );
                        // Une barre de séparation ferme la mesure sur la
                        // dernière note *chantée* du groupe source.
                        let breath = gregorian
                            .syllables
                            .get(source.gregorian_syllable)
                            .is_some_and(|syllable| {
                                !syllable.bars.is_empty()
                                    && syllable.notes.iter().skip(source.gregorian_note + 1).all(
                                        |note| {
                                            note.signs.iter().all(|sign| {
                                                sign.kind
                                                    != scoretk_core::model::SignKind::Alteration
                                            })
                                        },
                                    )
                            });
                        chant::write_note_signs(
                            out,
                            gregorian_note,
                            source.gregorian_syllable,
                            breath,
                        );
                        // Un neume est le groupe parenthesé source
                        // (`Gabc.tex:36-58`) : une liure par syllabe chantée,
                        // entre sa première et sa dernière note chantée.
                        let sung: Vec<usize> = gregorian
                            .syllables
                            .get(source.gregorian_syllable)
                            .map(|syllable| {
                                syllable
                                    .notes
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, note)| {
                                        note.signs.iter().all(|sign| {
                                            sign.kind != scoretk_core::model::SignKind::Alteration
                                        })
                                    })
                                    .map(|(index, _)| index)
                                    .collect()
                            })
                            .unwrap_or_default();
                        let position = sung
                            .iter()
                            .position(|&index| index == source.gregorian_note);
                        chant::write_note_slur(
                            out,
                            position == Some(0),
                            sung.len() > 1 && position == Some(sung.len() - 1),
                        );
                    }
                }
            }

            if first && grace.is_none() {
                for lyric in &chord_rest.lyrics {
                    write_lyric(out, lyric);
                }
            }

            out.xml.end_element();
        }
    }

    /// Écrit les `<beam>` d'un élément d'après les ligatures reconstituées.
    fn write_beams(&self, out: &mut Out, measure: &Measure, track: Track, tick: Fraction) {
        for (index, text) in self
            .beams
            .texts(measure.number, track, tick)
            .iter()
            .enumerate()
        {
            let number = (index + 1).to_string();
            out.xml.tag_with("beam", &[("number", &number)], text);
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
        ids: StableIds,
    ) {
        let segment_tick = out.tick;
        // Les grâces du moment sont jouées par LilyPond avant le silence
        // (`lily/grace-iterator.cc:24-42`) : les écrire ici, sinon l'oracle
        // voit une attaque de moins et décale la classification du fichier.
        if let Some(graces) = chord_rest.rest_ref().map(|rest| &rest.grace_chords) {
            self.grace_entries(
                out, graces, true, chord_rest, measure, xml_staff, track, ids,
            );
        }
        out.attr(false);

        let visible = chord_rest.rest_ref().is_none_or(|rest| rest.visible);
        // Un silence est une `<note>` : `stable_ids` lui donne le même schéma
        // d'identifiant, avec `n0` puisqu'il n'a pas de note.
        let note_id = self.options.stable_ids.then(|| {
            stable_note_id(
                ids.measure_index,
                track.staff().raw(),
                track.voice(),
                ids.chord_rank,
                0,
            )
        });
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        if !visible {
            attributes.push(("print-object", "no"));
        }
        if let Some(id) = &note_id {
            attributes.push(("id", id));
            attributes.push(("xml:id", id));
        }
        out.xml.start_element("note", &attributes);

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
            // `Rest::checkDots` n'est appelé que par la mise en page
            // (`restlayout.cpp:148`), qui ignore une portée masquée :
            // `Staff::show()` vaut `part()->show() && visible()`.
            let staff_shown = self.score.staff_shown(track.staff());
            let explicit_dots = chord_rest.rest_ref().map_or(0, |rest| rest.note_dots.len());
            for _ in 0..rest_dots::dot_count(chord_rest.duration.dots, staff_shown, explicit_dots) {
                out.xml.tag_empty("dot", &[]);
            }
        }

        self.write_time_modification(out, chord_rest);

        if xml_staff > 0 {
            out.xml.tag("staff", &xml_staff.to_string());
        }

        self.write_beams(out, measure, track, segment_tick);

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
        if let Some(graces) = chord_rest.rest_ref().map(|rest| &rest.grace_chords) {
            self.grace_entries(
                out, graces, false, chord_rest, measure, xml_staff, track, ids,
            );
        }
    }

    /// Écrit les articulations et ornements d'un accord (`chordAttributes`).
    #[allow(clippy::too_many_arguments)]
    fn chord_attributes(
        &self,
        out: &mut Out,
        chord: &Chord,
        grace: bool,
        measure: &Measure,
        segment_tick: Tick,
        track: Track,
        notations: &mut Notations,
    ) {
        let segment = measure
            .segments
            .iter()
            .find(|segment| segment.tick == segment_tick && segment.chord_rest(track).is_some());
        // `doSlurs` est appelé juste avant `chordAttributes`. Un accord
        // d'agrément partage le tick de son accord principal : sans identité
        // de note dans le modèle, la liaison ne doit être écrite qu'une fois.
        if !grace {
            notations::slurs(
                self.score,
                &mut out.xml,
                &mut out.notations,
                segment_tick,
                track,
                notations,
            );
        }
        notations::chord_attributes(
            self.score,
            &mut out.xml,
            chord,
            grace,
            measure,
            segment,
            segment_tick,
            track,
            out.tick,
            notations,
            &mut out.notations,
        );
    }
}

// ----------------------------------------------------------------------
// Attributs de mesure : armures, mesures, clefs, portées
// ----------------------------------------------------------------------

impl Context<'_> {
    /// Numéro affiché d'une mesure et drapeau `implicit`, remis à l'état
    /// initial après un saut de section (`MeasureNumberStateHandler`,
    /// exportmusicxml.cpp:8264-8299). Une mesure exclue de la numérotation
    /// (anacrouse) porte `number="0" implicit="yes"` si elle ouvre la section,
    /// puis `number="Xn" implicit="yes"`.
    fn measure_number(&self, index: usize) -> (String, bool) {
        let debut = self.score.measures[..=index]
            .iter()
            .rposition(|measure| measure.section_break_before && !measure.section_keeps_numbering)
            .unwrap_or(0);
        let mut numero = 1i64;
        let mut irregulier = 1i64;
        let mut resultat = ("1".to_owned(), false);
        for mesure in &self.score.measures[debut..=index] {
            // `MeasureNumberStateHandler::updateForMeasure`
            // (`exportmusicxml.cpp:8288-8300`) : le décalage de numérotation
            // est ajouté au compteur courant avant d'écrire le numéro.
            numero += i64::from(mesure.number_offset);
            if mesure.exclude_from_numbering {
                if irregulier + numero == 2 {
                    resultat = ("0".to_owned(), true);
                } else {
                    resultat = (format!("X{irregulier}"), true);
                    irregulier += 1;
                }
            } else {
                resultat = (numero.to_string(), false);
                numero += 1;
            }
        }
        resultat
    }

    /// Écrit les armures et indications de mesure (`keysigTimesig`).
    fn keysig_timesig(&self, out: &mut Out, measure: &Measure, part: &Part) {
        if measure.start_tick.is_zero()
            && matches!(self.score.extensions.format, Some(Extension::Gregorian(_)))
        {
            // Le chant grégorien n'a pas de mesure métrique (`Gabc.tex:801-845`).
            // MusicXML 4.0 l'exprime explicitement par `<senza-misura/>` dans
            // un `<time print-object="no">` (XSD 4.0, `complexType time`) :
            // aucune durée inventée n'est annoncée comme une signature.
            out.attr(true);
            out.xml.start_element("time", &[("print-object", "no")]);
            out.xml.tag_empty("senza-misura", &[]);
            out.xml.end_element();
        }
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
                self.keysig(out, 0, Key::C, KeyMode::Unknown, true);
            }
        } else {
            let first = self.active_key(keysigs[0].1);
            // `keysigTimesig` n'écrit une armure commune que si toutes les
            // portées en ont une, identique ; sinon chaque armure est numérotée.
            let single = keysigs.len() == part.staves.len()
                && keysigs
                    .iter()
                    .all(|(_, keysig)| self.active_key(keysig) == first);
            if single {
                let keysig = keysigs[0].1;
                self.keysig(out, 0, first, keysig.mode, keysig.visible);
            } else {
                for (position, keysig) in keysigs {
                    self.keysig(
                        out,
                        position + 1,
                        self.active_key(keysig),
                        keysig.mode,
                        keysig.visible,
                    );
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
        if self.concert_pitch {
            keysig.concert_key
        } else {
            keysig.transposed_key
        }
    }

    /// Écrit `<key>` (`keysig`).
    fn keysig(&self, out: &mut Out, staff: usize, key: Key, mode: KeyMode, visible: bool) {
        out.attr(true);
        let number = staff.to_string();
        let mut attributes: Vec<(&str, &str)> = Vec::new();
        if staff > 0 {
            attributes.push(("number", &number));
        }
        if !visible {
            attributes.push(("print-object", "no"));
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
        if !timesig.visible {
            attributes.push(("print-object", "no"));
        }
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
            let start_clef = segment
                .and_then(|segment| segment.element(track))
                .or_else(|| {
                    // `findAndExportClef` (`exportmusicxml.cpp:7693-7714`) retombe
                    // sur cs2, la clef posée au début de la mesure courante,
                    // quand cs1 (en-tête ou mesure précédente) est absent.
                    // Sans ce repli, une clef 8va posée sur une portée sans
                    // HeaderClef était remplacée par la clef par défaut et
                    // réécrite ensuite par `writeElement` (deux `<clef>` dans
                    // `Stella_maris_instruments - La`). Au-delà de la première
                    // mesure, c'est `mid_measure_clef` qui écrit cette clef de
                    // début avec `after-barline="yes"` (`writeElement`) : la
                    // reprendre ici la dupliquerait.
                    measure.start_tick.is_zero().then(|| {
                        find_segment(measure, SegmentType::Clef, measure.start_tick)
                            .and_then(|segment| segment.element(track))
                    })?
                });
            match start_clef {
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
        if self.modern_gabc()
            && matches!(
                clef.concert,
                ClefType::C1
                    | ClefType::C2
                    | ClefType::C3
                    | ClefType::C4
                    | ClefType::C5
                    | ClefType::F
                    | ClefType::FBaritone
                    | ClefType::FSubbass
            )
        {
            // Vue moderne optionnelle : clé de sol, avec `8vb` quand le
            // registre grégorien descend sous le do médian (XSD 4.0
            // `clef-octave-change`). Sans option, la clé d'ut reste celle du
            // `.gabc` (`Gabc.tex:847-870`).
            return if self.score.notes.iter().any(|note| note.pitch < 60) {
                ClefType::G8Vb
            } else {
                ClefType::G
            };
        }
        if self.concert_pitch {
            clef.concert
        } else if clef.transposed != ClefType::Invalid {
            clef.transposed
        } else {
            clef.concert
        }
    }

    /// Vrai si la vue moderne du chant est demandée par l'option additive.
    fn modern_gabc(&self) -> bool {
        self.options.gabc_modern_clef
            && matches!(self.score.extensions.format, Some(Extension::Gregorian(_)))
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
        track: Track,
    ) {
        // MuseScore n'écrit ni les clefs générées ni celles de l'en-tête.
        if segment.kind == SegmentType::HeaderClef {
            return;
        }
        if segment.tick != measure.start_tick && segment.tick != measure.end_tick() {
            self.clef(out, xml_staff, self.active_clef(clef), &[]);
        } else if segment.tick == measure.start_tick {
            // `findAndExportClef` (`exportmusicxml.cpp:7693-7714`) chooses cs1
            // (the previous measure's Clef, or the first measure's
            // HeaderClef) over cs2 (a Clef in this measure at the same tick)
            // and writes exactly one clef per staff. Without this guard a
            // measure carrying both segments emitted two <clef> elements
            // (4 spurious lines in Stella_maris_instruments - La).
            let header = measure.segments.iter().find(|other| {
                other.kind == SegmentType::HeaderClef
                    && other.tick == segment.tick
                    && other.element(track).is_some()
            });
            let already_written = match header {
                Some(header) => header.element(track).is_some_and(|item| {
                    matches!(item, SegmentItem::Clef(other_clef)
                        if other_clef.concert == clef.concert
                            && other_clef.transposed == clef.transposed)
                }),
                // No HeaderClef on this track at tick 0: `findAndExportClef`
                // falls back to cs2 (`exportmusicxml.cpp:7699-7714`) and, when
                // cs2 is absent too, to the staff's generated default clef
                // (`exportDefaultClef`). `write_measure_clefs` implements that
                // same cs2/default fallback for `measure.start_tick.is_zero()`,
                // so it has already emitted this clef and `writeElement`
                // (`exportmusicxml.cpp:8001-8003`) must not write it again.
                // For later measures cs1 comes from the *previous* measure;
                // a Clef at the current start tick is then a genuine new
                // system clef and `writeElement` writes it with
                // `after-barline="yes"` — so the skip stays limited to tick 0.
                None => measure.start_tick.is_zero(),
            };
            if already_written {
                return;
            }
            // `Attributes::doAttr` referme `<attributes>` avant d'écrire une
            // clef de reprise : la XSD 4.0 exige toutes les clefs avant les
            // `<staff-details>`/`<measure-style>` du bloc courant.
            if out.attributes_has_details {
                out.attr(false);
            }
            self.clef(
                out,
                xml_staff,
                self.active_clef(clef),
                &[("after-barline", "yes")],
            );
        }
        // Une clef en fin de mesure sera écrite au début de la suivante.
    }

    /// Écrit `<staff-details>` pour les portées non standard ou masquées
    /// (`writeStaffDetails`, `exportmusicxml.cpp:8059-8104`).
    ///
    /// Le C++ ajoute `print-object="no"` aux portées de la liste
    /// `hiddenStaves` (`hideEmptyStaves`) et écrit le bloc dès qu'une portée
    /// est masquée, même si elle a 5 lignes. Le portage n'écrivait que les
    /// portées non standard et ignorait les portées masquées.
    fn write_staff_details(&self, out: &mut Out, measure_index: usize, part: &Part) {
        let staves = part.staves.len();
        let hidden = self.systems.hidden_at(measure_index);
        for (position, staff_id) in part.staves.iter().enumerate() {
            let staff = self.score.staff(*staff_id);
            let hidden_staff = !staff.visible || hidden.contains(&(staff_id.raw() as usize));
            // `staffMag = (small ? smallStaffMag : 1) * userMag`
            // (`staff.cpp:806-809`) ; `smallStaffMag` vient du style
            // (défaut 0.7, mais un `.mscz` peut le fixer à 0.9).
            let mag = if staff.small {
                self.score.engraving.style.small_staff_mag
            } else {
                1.0
            } * f64::from(staff.user_mag_percent)
                / 100.0;
            let size = (mag * 100.0).round() as i64;
            // TODO(parité): tablature et couleurs/visibilité des lignes.
            let lines = if self.modern_gabc() { 5 } else { staff.lines };
            if lines == 5 && !hidden_staff && size == 100 {
                continue;
            }
            out.attr(true);
            let number = (position + 1).to_string();
            let mut attributes: Vec<(&str, &str)> = Vec::new();
            if staves > 1 {
                attributes.push(("number", &number));
            }
            if hidden_staff {
                attributes.push(("print-object", "no"));
            }
            out.xml.start_element("staff-details", &attributes);
            out.xml.tag("staff-lines", &lines.to_string());
            // `writeStaffDetails` (`exportmusicxml.cpp:8139-8144`) :
            // `<staff-size scaling="mag*100">mag*100</staff-size>`.
            if size != 100 {
                let scaling = size.to_string();
                out.xml
                    .tag_with("staff-size", &[("scaling", &scaling)], &scaling);
            }
            out.xml.end_element();
            out.attributes_has_details = true;
        }
    }

    /// Écrit `<transpose>` (`writeInstrumentDetails`).
    fn write_instrument_details(&self, out: &mut Out, part: &Part, tick: Tick) {
        // Un instrument peut changer en cours de partition (clarinette en la
        // puis en si bémol) : `<transpose>` suit l'instrument en vigueur à la
        // mesure, pas l'instrument initial de la partie.
        let Some(transpose) = part.instrument_at(tick).transpose else {
            return;
        };
        if transpose.chromatic == 0 {
            return;
        }

        out.attr(true);
        if self.concert_pitch {
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
        if self.concert_pitch {
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
    fn barline_middle(&self, out: &mut Out, bar_line: &scoretk_core::model::BarLine) {
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
    /// Écrit les accords chiffrés du segment (`segmentHarmonies`, `harmonies`).
    ///
    /// MuseScore range un accord sans note dans un segment `ChordRest` vide ;
    /// MusicXML impose de l'écrire avant la note suivante avec un `<offset>`.
    /// On explore donc les segments suivants jusqu'au prochain élément de la
    /// piste, comme `harmonies`.
    fn harmonies(
        &self,
        out: &mut Out,
        measure: &Measure,
        segment: &Segment,
        staff_id: StaffId,
        track: Track,
    ) {
        self.segment_harmonies(out, segment, staff_id, track, Tick::ZERO);
        for next in &measure.segments {
            if !next.kind.is_chord_rest_type() || next.tick <= segment.tick {
                continue;
            }
            if next.element(track).is_some() {
                break;
            }
            self.segment_harmonies(out, next, staff_id, track, next.tick - segment.tick);
        }
    }

    /// Écrit les accords d'un segment avec le décalage donné.
    ///
    /// `segmentHarmonies` cherche les annotations sur la piste exacte
    /// (`seg->findAnnotations(ElementType::HARMONY, track, track)`) : une
    /// harmonie attachée à une voix sans note n'est jamais écrite, car la
    /// boucle du C++ ne visite que les pistes portant un élément
    /// (`writeMeasureTracks`). Filtrer par portée seule les exportait à tort.
    fn segment_harmonies(
        &self,
        out: &mut Out,
        segment: &Segment,
        staff_id: StaffId,
        track: Track,
        offset: Tick,
    ) {
        for entry in &segment.annotations {
            if entry.track != track || entry.track.staff() != staff_id {
                continue;
            }
            if let Annotation::Harmony(harmony) = &entry.annotation {
                harmony::write(out, harmony, offset);
            }
        }
    }

    /// Écrit les annotations du segment (`annotations`).
    ///
    /// MuseScore rattache toutes les annotations d'une portée à la première
    /// voix occupée du segment (`findTrackForAnnotations`).
    fn annotations(
        &self,
        out: &mut Out,
        segment: &Segment,
        staff_id: StaffId,
        xml_staff: usize,
        skip_invisible_tempo: bool,
    ) {
        for entry in &segment.annotations {
            if entry.track.staff() != staff_id {
                continue;
            }
            if !entry.annotation.visible() {
                // `findTrackForAnnotations` (`exportmusicxml.cpp:6375-6392`)
                // renvoie `nidx` sur un segment sans accord ni silence : le C++
                // n'écrit alors aucun `<sound tempo>` d'un tempo invisible.
                if skip_invisible_tempo && matches!(entry.annotation, Annotation::Tempo(_)) {
                    continue;
                }
                // `commonAnnotations` : un tempo invisible n'exporte que son
                // `<sound tempo>` ; un texte système invisible exporte son
                // swing avec un offset.
                match &entry.annotation {
                    Annotation::Tempo(tempo) => {
                        out.attr(false);
                        let beats_per_second = effective_tempo(self, tempo, segment.tick);
                        text::write_tempo_sound_value(out, beats_per_second);
                    }
                    Annotation::SystemText(text) => {
                        if let Some(swing) = &text.swing {
                            let offset = out.duration_in_divisions(segment.tick - out.tick);
                            out.attr(false);
                            text::write_swing_sound(out, swing, Some(offset));
                        }
                    }
                    _ => {}
                }
                continue;
            }
            let offset = out.duration_in_divisions(segment.tick - out.tick);
            match &entry.annotation {
                Annotation::Harmony(_) => {}
                // Les chiffrages figurés (`figuredBass`) et les barres de
                // vibrato (`writeGuitarBend`) sont écrits avec l'accord, pas
                // avec les annotations.
                Annotation::FiguredBass(_) | Annotation::TremoloBar(_) => {}
                Annotation::Dynamic(dynamic) => nuances::write_dynamic(out, dynamic, xml_staff),
                Annotation::StaffText(text) => {
                    text::write_words(out, &text.text, text.placement, false, xml_staff, offset);
                }
                Annotation::Expression(text) => {
                    text::write_words(out, &text.text, text.placement, false, xml_staff, offset);
                }
                Annotation::SystemText(text) => {
                    text::write_system_text(out, text, xml_staff, offset);
                }
                Annotation::Tempo(tempo) => {
                    let beats_per_second = effective_tempo(self, tempo, segment.tick);
                    let tempo = Tempo {
                        beats_per_second,
                        ..tempo.clone()
                    };
                    text::write_tempo(out, &tempo, xml_staff, offset);
                }
                Annotation::RehearsalMark(mark) => {
                    text::write_rehearsal(out, mark, xml_staff, offset);
                }
                Annotation::InstrumentChange(change) => {
                    text::write_words_styled(
                        out,
                        &change.text,
                        change.placement,
                        false,
                        xml_staff,
                        offset,
                        &text::INSTRUMENT_CHANGE_STYLE,
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
                self.annotations(out, segment, *staff_id, xml_staff, true);
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

/// Tempo effectif d'une indication (`tempoSound`).
///
/// « a tempo » et « Tempo I » reprennent le tempo en vigueur au tick.
fn effective_tempo(context: &Context<'_>, tempo: &Tempo, tick: Tick) -> Option<f64> {
    if tempo.kind == TempoKind::Normal {
        tempo.beats_per_second
    } else {
        context.tempo_at(tick).or(tempo.beats_per_second)
    }
}

/// Premier changement d'instrument d'une portée dans un segment
/// (`Segment::findAnnotation`).
fn instrument_change(segment: &Segment, staff_id: StaffId) -> Option<&InstrumentChange> {
    segment.annotations.iter().find_map(|entry| {
        if entry.track.staff() != staff_id {
            return None;
        }
        match &entry.annotation {
            Annotation::InstrumentChange(change) => Some(change),
            _ => None,
        }
    })
}

/// Écrit `<pitch>` (`writePitch`).
fn write_pitch(
    out: &mut Out,
    note: &Note,
    concert_pitch: bool,
    transpose_chromatic: i32,
    ottava: i32,
) {
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
    // `pitch2xml` corrige l'octave des ottavas (`ppitch - pitch`).
    let octave = octave + ottava;

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
    // `isSmallNote` (`exportmusicxml.cpp:4181-4184`) : la note elle-même ou
    // son accord peut porter `<small>1</small>`.
    let small = chord.small || note.small;
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
    // MuseScore écrit les nombres bruts du fichier, non réduits : `4:4` reste
    // `4`/`4` dans `<tuplet-number>`.
    let (actual, normal) = tuplet.displayed_numbers();
    let actual = actual.to_string();
    let normal = normal.to_string();
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
    // `ExportMusicXml::lyrics` (exportmusicxml.cpp:6130) n'écrit `<extend>`
    // que si `ticks().isNotZero()` : un `<ticks_f>0/16</ticks_f>` attentive
    // une syllabe sans prolongement, pas un melisma.
    if lyric.melisma.is_some_and(|ticks| ticks != Fraction::ZERO) {
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
    if note.parentheses {
        attributes.push(("parentheses", "yes"));
    }
    match note.head {
        NoteHeadType::Quarter => attributes.push(("filled", "yes")),
        NoteHeadType::Half | NoteHeadType::Whole => attributes.push(("filled", "no")),
        _ => {}
    }

    let up = chord.engraving.stem != StemDirection::Down;
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
            if note.head == NoteHeadType::Auto && !note.parentheses {
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
    // `writeAccidental` : le rôle utilisateur rend l'altération d'avertissement.
    if accidental.bracket {
        if accidental.user {
            // MuseScore 4.7.4 : `bracket` + rôle utilisateur s'exporte en
            // `cautionary="yes" parentheses="yes"` (Ave Verum, Angels Carol).
            // Le C++ 5.0 (`exportmusicxml.cpp:2866-2870`) écrit encore
            // `editorial`/`bracket`, mais la référence 4.7.4 prévaut.
            attributes.push(("cautionary", "yes"));
            attributes.push(("parentheses", "yes"));
        } else {
            attributes.push(("bracket", "yes"));
        }
    } else if accidental.parentheses {
        if accidental.user {
            attributes.push(("cautionary", "yes"));
        }
        attributes.push(("parentheses", "yes"));
    } else if accidental.user {
        attributes.push(("cautionary", "yes"));
        attributes.push(("parentheses", "no"));
    }
    if accidental.small {
        attributes.push(("size", "cue"));
    }
    // TODO(parité): smufl pour les altérations exotiques, couleur.
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
    // `isCueNote` (`exportmusicxml.cpp:4191-4193`) : `isSmallNote && !play`.
    (chord.small || note.small) && !note.playback.play
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

/// Symbole MusicXML d'une accolade MuseScore (`partGroupStart`).
/// `BracketType` (src/engraving/types/types.h:1289) : `NORMAL`, `BRACE`,
/// `SQUARE`, `LINE`, `GROUP`. L'ordre est celui du fichier `.mscx`
/// (`<bracket type="…">`) ; le portage inversait SQUARE et LINE.
fn bracket_symbol(kind: i8) -> &'static str {
    match kind {
        0 => "bracket",
        1 => "brace",
        2 => "square",
        3 => "line",
        4 => "none",
        _ => "none",
    }
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

/// Nom de partie à décomposer en fragments, si MuseScore en écrit un.
///
/// `writePartList` ne produit `<part-name-display>` que si le nom contient un
/// bémol ou un dièse typographique.
fn part_display_name(part: &Part) -> Option<&str> {
    if contains_accidental(&part.long_name) {
        Some(&part.long_name)
    } else if part.long_name.is_empty() && contains_accidental(&part.name) {
        Some(&part.name)
    } else {
        None
    }
}

fn contains_accidental(text: &str) -> bool {
    text.contains('♭') || text.contains('♯')
}

/// Écrit les fragments d'un nom (`writeDisplayName`).
fn write_display_name(out: &mut Out, name: &str) {
    let mut display = String::new();
    for ch in name.chars() {
        if ch != '♭' && ch != '♯' {
            display.push(ch);
            continue;
        }
        if !display.is_empty() {
            out.xml.tag("display-text", &display);
        }
        out.xml
            .tag("accidental-text", if ch == '♭' { "flat" } else { "sharp" });
        display.clear();
    }
    if !display.is_empty() {
        out.xml.tag("display-text", &display);
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

/// Réel écrit avec `digits` chiffres significatifs, sans zéros de queue.
///
/// Reproduit `String::number` de MuseScore utilisé par `XmlWriter::tag` pour
/// les attributs et textes numériques (ici `<volume>`).
pub(crate) fn format_number_significant(value: f64, digits: usize) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let exponent = value.abs().log10().floor() as i32;
    let decimals = (digits as i32 - 1 - exponent).clamp(0, 12) as usize;
    let text = format!("{:.*}", decimals, value);
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Dynamique d'une note, comme `ExportMusicXml::chord`
/// (`importexport/musicxml/internal/export/exportmusicxml.cpp`) : l'attribut
/// `dynamics` vaut `velo * 100.0 / 90.0`, écrit avec deux décimales puis sans
/// zéros de queue (`dynamics="80"`, `dynamics="54.44"`). MuseScore n'écrit
/// rien quand la vélocité utilisateur vaut zéro.
fn format_dynamics(value: f64) -> String {
    let text = format!("{value:.2}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_owned()
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
                            // `\afterGrace` : l'attribut `attack` d'une grâce
                            // après vaut `played_offset - durée principale`,
                            // une fraction que les durées seules ne portent pas
                            // (3/4 × une blanche = 3/8). Sans elle, les
                            // divisions restaient à 1 par noire et l'attaque
                            // était arrondie (lyrics-after-grace : 1.0 au lieu
                            // de 1.5). `ly/music-functions-init.ly:86-112`.
                            if let Some(chord) = chord_rest.chord_ref() {
                                for grace in &chord.grace_chords {
                                    if let Some(offset) = grace.played_offset {
                                        if !offset.is_zero() {
                                            fractions.insert(offset.abs());
                                        }
                                    }
                                }
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
