// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/dom/segment.h`, `engraving/dom/keysig.h`,
// `engraving/dom/timesig.h`, `engraving/dom/clef.h` et
// `engraving/dom/barline.h` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Segments, armures, mesures, clefs et barres de mesure.
//!
//! Portage des objets verticaux de MuseScore : `Segment` et `SegmentType`
//! (`engraving/dom/segment.h`), `KeySig` (`engraving/dom/keysig.h`),
//! `TimeSig` (`engraving/dom/timesig.h`), `Clef` (`engraving/dom/clef.h`) et
//! `BarLine` (`engraving/dom/barline.h`).
//!
//! Un segment contient un élément au plus par piste. La taille du vecteur est
//! `nb_portées * VOICES`, et l'index est [`Track::raw`], exactement comme
//! `Segment::_elist` dans MuseScore.

use super::annotation::{Annotation, Placement};
use super::chord::ChordRest;
use super::fraction::{Fraction, Tick};
use super::ids::{Track, VOICES};

/// Tonalité exprimée en nombre de dièses (positif) ou de bémols (négatif).
///
/// Correspond à `Key` dans `engraving/dom/key.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(i8);

impl Key {
    /// Do majeur / la mineur (aucune altération).
    pub const C: Self = Self(0);
    /// Sol majeur / mi mineur (un dièse).
    pub const G: Self = Self(1);
    /// Ré majeur / si mineur (deux dièses).
    pub const D: Self = Self(2);
    /// La majeur / fa dièse mineur (trois dièses).
    pub const A: Self = Self(3);
    /// Mi majeur / do dièse mineur (quatre dièses).
    pub const E: Self = Self(4);
    /// Si majeur / sol dièse mineur (cinq dièses).
    pub const B: Self = Self(5);
    /// Fa dièse majeur / ré dièse mineur (six dièses).
    pub const F_SHARP: Self = Self(6);
    /// Do dièse majeur / la dièse mineur (sept dièses).
    pub const C_SHARP: Self = Self(7);
    /// Fa majeur / ré mineur (un bémol).
    pub const F: Self = Self(-1);
    /// Si bémol majeur / sol mineur (deux bémols).
    pub const B_FLAT: Self = Self(-2);
    /// Mi bémol majeur / do mineur (trois bémols).
    pub const E_FLAT: Self = Self(-3);
    /// La bémol majeur / fa mineur (quatre bémols).
    pub const A_FLAT: Self = Self(-4);
    /// Ré bémol majeur / si bémol mineur (cinq bémols).
    pub const D_FLAT: Self = Self(-5);
    /// Sol bémol majeur / mi bémol mineur (six bémols).
    pub const G_FLAT: Self = Self(-6);
    /// Do bémol majeur / la bémol mineur (sept bémols).
    pub const C_FLAT: Self = Self(-7);

    /// Construit une tonalité à partir du nombre de quintes.
    ///
    /// MuseScore autorise les armures personnalisées au-delà de `-7..=7` ;
    /// aucune plage n'est donc imposée ici.
    pub const fn new(fifths: i8) -> Self {
        Self(fifths)
    }

    /// Nombre de dièses (positif) ou de bémols (négatif).
    pub const fn fifths(self) -> i8 {
        self.0
    }
}

impl Default for Key {
    fn default() -> Self {
        Self::C
    }
}

/// Mode d'une armure.
///
/// Correspond à `KeyMode` dans `engraving/dom/key.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyMode {
    /// Majeur.
    #[default]
    Major,
    /// Mineur.
    Minor,
    /// Aucun mode (armure atonale).
    None,
    /// Mode inconnu.
    Unknown,
}

/// Armure.
///
/// Correspond à `KeySig` dans `engraving/dom/keysig.h` et à `KeySigEvent`
/// (`engraving/dom/keysig.h` / `engraving/dom/key.h`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySig {
    /// Tonalité concert.
    pub concert_key: Key,
    /// Tonalité transposée (pour les instruments transpositeurs).
    pub transposed_key: Key,
    /// Mode.
    pub mode: KeyMode,
    /// Vrai pour une armure de courtoisie.
    pub courtesy: bool,
    /// Vrai si l'armure est visible (`print-object="no"` sinon).
    pub visible: bool,
}

impl KeySig {
    /// Construit une armure avec la même tonalité concert et transposée.
    pub const fn new(concert_key: Key) -> Self {
        Self {
            concert_key,
            transposed_key: concert_key,
            mode: KeyMode::Major,
            courtesy: false,
            visible: true,
        }
    }
}

/// Type d'affichage d'une mesure.
///
/// Correspond à `TimeSigType` dans `engraving/dom/timesig.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TimeSigType {
    /// Numérateur / dénominateur normaux.
    #[default]
    Normal,
    /// `C` (4/4).
    FourFour,
    /// `C` barré (2/2).
    AllaBreve,
    /// `C` barré à la Bach.
    CutBach,
    /// `C` barré triple (9/8).
    CutTriple,
}

/// Mesure (indication de mesure).
///
/// Correspond à `TimeSig` dans `engraving/dom/timesig.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeSig {
    /// Numérateur.
    pub numerator: i32,
    /// Dénominateur.
    pub denominator: i32,
    /// Type d'affichage.
    pub kind: TimeSigType,
    /// Étirement local (`localSig / globalSig`).
    pub stretch: Fraction,
    /// Vrai pour une mesure de courtoisie.
    pub courtesy: bool,
    /// Vrai si l'indication de mesure est visible.
    pub visible: bool,
}

impl TimeSig {
    /// Construit une mesure normale non étirée.
    pub fn new(numerator: i32, denominator: i32) -> Self {
        Self {
            numerator,
            denominator,
            kind: TimeSigType::Normal,
            stretch: Fraction::ONE,
            courtesy: false,
            visible: true,
        }
    }

    /// Longueur nominale de la mesure en fractions de ronde.
    ///
    /// Une mesure 4/4 vaut `1/1`, une mesure 6/8 vaut `3/4`. Un dénominateur
    /// nul (fichier corrompu) donne zéro.
    pub fn measure_len(&self) -> Fraction {
        Fraction::try_new(i64::from(self.numerator), i64::from(self.denominator))
            .unwrap_or(Fraction::ZERO)
    }

    /// Vrai si le numérateur et le dénominateur sont exploitables.
    pub const fn is_valid(&self) -> bool {
        self.denominator > 0 && self.numerator >= 0
    }
}

/// Signe de clef.
///
/// Correspond à la partie « sign » de `ClefType` dans
/// `engraving/dom/clef.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClefSign {
    /// Clef de sol.
    G,
    /// Clef de fa.
    F,
    /// Clef d'ut.
    C,
    /// Clef de percussion.
    Percussion,
    /// Clef de tablature.
    Tab,
    /// Aucun signe.
    None,
}

impl ClefSign {
    /// Nom du signe tel qu'écrit en MusicXML.
    pub const fn musicxml_name(self) -> &'static str {
        match self {
            Self::G => "G",
            Self::F => "F",
            Self::C => "C",
            Self::Percussion => "percussion",
            Self::Tab => "TAB",
            Self::None => "none",
        }
    }
}

/// Type de clef.
///
/// Correspond à `ClefType` dans `engraving/dom/clef.h`. Les variantes
/// exotiques (clefs anciennes, clefs de tablature à quatre cordes, etc.) sont
/// ajoutées au besoin ; les lecteurs peuvent les rapprocher d'une variante
/// voisine sans changer la structure du modèle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClefType {
    /// Clef invalide.
    #[default]
    Invalid,
    /// Clef de sol, deuxième ligne.
    G,
    /// Clef de sol octaviée en dessous.
    G8Vb,
    /// Clef de sol octaviée au-dessus.
    G8Va,
    /// Clef de sol quinziémée en dessous.
    G15Mb,
    /// Clef de sol quinziémée au-dessus.
    G15Ma,
    /// Clef de sol première ligne.
    G1,
    /// Clef d'ut première ligne.
    C1,
    /// Clef d'ut deuxième ligne.
    C2,
    /// Clef d'ut troisième ligne.
    C3,
    /// Clef d'ut quatrième ligne.
    C4,
    /// Clef d'ut cinquième ligne.
    C5,
    /// Clef de fa, quatrième ligne.
    F,
    /// Clef de fa octaviée en dessous.
    F8Vb,
    /// Clef de fa octaviée au-dessus.
    F8Va,
    /// Clef de fa quinziémée en dessous.
    F15Mb,
    /// Clef de fa quinziémée au-dessus.
    F15Ma,
    /// Clef de fa troisième ligne (baryton, `F_B`).
    FBaritone,
    /// Clef de fa cinquième ligne (sous-basse, `F_C`).
    FSubbass,
    /// Clef de percussion.
    Perc,
    /// Clef de percussion alternative.
    Perc2,
    /// Clef de tablature à six cordes.
    Tab,
    /// Clef de tablature à quatre cordes.
    Tab4,
    /// Clef de tablature à six cordes avec empattements.
    TabSerif,
    /// Clef de tablature à quatre cordes avec empattements.
    Tab4Serif,
}

impl ClefType {
    /// Signe MusicXML.
    pub const fn sign(self) -> ClefSign {
        match self {
            Self::G | Self::G8Vb | Self::G8Va | Self::G15Mb | Self::G15Ma | Self::G1 => ClefSign::G,
            Self::F
            | Self::F8Vb
            | Self::F8Va
            | Self::F15Mb
            | Self::F15Ma
            | Self::FBaritone
            | Self::FSubbass => ClefSign::F,
            Self::C1 | Self::C2 | Self::C3 | Self::C4 | Self::C5 => ClefSign::C,
            Self::Perc | Self::Perc2 => ClefSign::Percussion,
            Self::Tab | Self::Tab4 | Self::TabSerif | Self::Tab4Serif => ClefSign::Tab,
            Self::Invalid => ClefSign::None,
        }
    }

    /// Ligne de la clef (1-5), 0 si inconnue.
    pub const fn line(self) -> u8 {
        match self {
            Self::G
            | Self::G8Vb
            | Self::G8Va
            | Self::G15Mb
            | Self::G15Ma
            | Self::Perc
            | Self::Perc2 => 2,
            Self::G1 => 1,
            Self::C1 => 1,
            Self::C2 => 2,
            Self::C3 => 3,
            Self::C4 => 4,
            Self::C5 => 5,
            Self::F | Self::F8Vb | Self::F8Va | Self::F15Mb | Self::F15Ma => 4,
            Self::FBaritone => 3,
            Self::FSubbass => 5,
            Self::Tab | Self::Tab4 | Self::TabSerif | Self::Tab4Serif => 5,
            Self::Invalid => 0,
        }
    }

    /// Changement d'octave en octaves (`octave-change` MusicXML).
    pub const fn octave_change(self) -> i8 {
        match self {
            Self::G8Vb | Self::F8Vb => -1,
            Self::G8Va | Self::F8Va => 1,
            Self::G15Mb | Self::F15Mb => -2,
            Self::G15Ma | Self::F15Ma => 2,
            _ => 0,
        }
    }

    /// Vrai pour une clef de tablature.
    pub const fn is_tab(self) -> bool {
        matches!(
            self,
            Self::Tab | Self::Tab4 | Self::TabSerif | Self::Tab4Serif
        )
    }

    /// Vrai pour une clef de percussion.
    pub const fn is_percussion(self) -> bool {
        matches!(self, Self::Perc | Self::Perc2)
    }
}

/// Clef d'un segment.
///
/// Correspond à `Clef` et à `ClefTypeList` dans `engraving/dom/clef.h` : la
/// clef concert et la clef transposée sont conservées séparément.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Clef {
    /// Clef concert.
    pub concert: ClefType,
    /// Clef transposée.
    pub transposed: ClefType,
}

impl Clef {
    /// Construit une clef identique en concert et en transposé.
    pub const fn new(clef: ClefType) -> Self {
        Self {
            concert: clef,
            transposed: clef,
        }
    }

    /// Construit une paire de clefs.
    pub const fn pair(concert: ClefType, transposed: ClefType) -> Self {
        Self {
            concert,
            transposed,
        }
    }
}

impl Default for Clef {
    fn default() -> Self {
        Self::new(ClefType::Invalid)
    }
}

/// Type de barre de mesure.
///
/// Correspond à `BarLineType` dans `engraving/types/types.h`, utilisé par
/// `BarLine` dans `engraving/dom/barline.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BarLineType {
    /// Barre simple.
    #[default]
    Normal,
    /// Barre double.
    Double,
    /// Début de reprise.
    StartRepeat,
    /// Fin de reprise.
    EndRepeat,
    /// Fin et début de reprise.
    EndStartRepeat,
    /// Barre brisée.
    Broken,
    /// Barre finale.
    End,
    /// Barre pointée.
    Dotted,
    /// Barre double pointée.
    DoubleDotted,
    /// Barre en pointillés.
    Dashed,
    /// Barre courte.
    Short,
    /// Barre double courte.
    DoubleShort,
    /// Barre en tick.
    Tick,
}

impl BarLineType {
    /// Vrai si la barre marque un début de reprise.
    pub const fn is_repeat_start(self) -> bool {
        matches!(self, Self::StartRepeat | Self::EndStartRepeat)
    }

    /// Vrai si la barre marque une fin de reprise.
    pub const fn is_repeat_end(self) -> bool {
        matches!(self, Self::EndRepeat | Self::EndStartRepeat)
    }
}

/// Barre de mesure.
///
/// Correspond à `BarLine` dans `engraving/dom/barline.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarLine {
    /// Type de barre.
    pub kind: BarLineType,
    /// Vrai si la barre traverse toutes les portées du système.
    pub span_staff: bool,
    /// Extension vers le haut, en espaces.
    pub span_from: i8,
    /// Extension vers le bas, en espaces.
    pub span_to: i8,
}

impl BarLine {
    /// Construit une barre simple qui ne traverse qu'une portée.
    pub const fn new(kind: BarLineType) -> Self {
        Self {
            kind,
            span_staff: false,
            span_from: 0,
            span_to: 0,
        }
    }
}

/// Type de respiration.
///
/// Correspond à `BreathType` dans `engraving/dom/breath.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BreathType {
    /// Virgule.
    #[default]
    Comma,
    /// Césure normale.
    Caesura,
    /// Césure courbe (`caesuraCurved`).
    CaesuraCurved,
    /// Césure courte (`caesuraShort`).
    CaesuraShort,
    /// Césure épaisse (`caesuraThick`).
    CaesuraThick,
    /// Césure simple trait (`caesuraSingleStroke`).
    CaesuraSingle,
    /// Respiration en tick.
    Tick,
    /// Coup d'archet.
    Upbow,
}

/// Respiration.
///
/// Correspond à `Breath` dans `engraving/dom/breath.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Breath {
    /// Type de respiration.
    pub kind: BreathType,
    /// Placement vertical (`ChordRest::hasBreathMark` : la voix 0 est
    /// au-dessus, la voix 1 au-dessous).
    pub placement: Placement,
}

/// Type de segment.
///
/// Correspond à `SegmentType` dans `engraving/dom/segment.h`. L'ordre de
/// déclaration reproduit celui de MuseScore, ce qui permet de trier les
/// segments d'une mesure par `(tick, type)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SegmentType {
    /// Segment invalide.
    Invalid,
    /// Barre de mesure au début de la mesure.
    BeginBarLine,
    /// Clef d'en-tête de mesure.
    HeaderClef,
    /// Armure.
    KeySig,
    /// Ambitus.
    Ambitus,
    /// Respiration.
    Breath,
    /// Indication de mesure.
    TimeSig,
    /// Barre de début de reprise.
    StartRepeatBarLine,
    /// Clef de début de reprise (annonce).
    ClefStartRepeatAnnounce,
    /// Armure de début de reprise (annonce).
    KeySigStartRepeatAnnounce,
    /// Indication de mesure de début de reprise (annonce).
    TimeSigStartRepeatAnnounce,
    /// Clef.
    Clef,
    /// Barre de mesure.
    BarLine,
    /// Tick de temps (segment sans élément visible).
    TimeTick,
    /// Accord ou silence.
    ChordRest,
    /// Clef répétée (annonce de courtoisie).
    ClefRepeatAnnounce,
    /// Armure répétée (annonce de courtoisie).
    KeySigRepeatAnnounce,
    /// Indication de mesure répétée (annonce de courtoisie).
    TimeSigRepeatAnnounce,
    /// Barre de fin de mesure.
    EndBarLine,
    /// Armure de courtoisie.
    KeySigAnnounce,
    /// Indication de mesure de courtoisie.
    TimeSigAnnounce,
}

impl SegmentType {
    /// Vrai pour un segment porteur d'accords ou de silences.
    pub const fn is_chord_rest_type(self) -> bool {
        matches!(self, Self::ChordRest)
    }

    /// Vrai pour un segment de tick de temps.
    pub const fn is_time_tick_type(self) -> bool {
        matches!(self, Self::TimeTick)
    }

    /// Vrai pour un segment porteur d'une durée.
    pub const fn has_duration(self) -> bool {
        matches!(self, Self::ChordRest | Self::TimeTick)
    }

    /// Vrai pour une barre de mesure.
    pub const fn is_bar_line_type(self) -> bool {
        matches!(
            self,
            Self::BeginBarLine | Self::StartRepeatBarLine | Self::BarLine | Self::EndBarLine
        )
    }

    /// Vrai pour une clef (y compris les annonces de reprise et de courtoisie).
    pub const fn is_clef_type(self) -> bool {
        matches!(
            self,
            Self::HeaderClef
                | Self::Clef
                | Self::ClefStartRepeatAnnounce
                | Self::ClefRepeatAnnounce
        )
    }

    /// Vrai pour une armure (y compris les annonces de reprise et de
    /// courtoisie).
    pub const fn is_key_sig_type(self) -> bool {
        matches!(
            self,
            Self::KeySig
                | Self::KeySigStartRepeatAnnounce
                | Self::KeySigRepeatAnnounce
                | Self::KeySigAnnounce
        )
    }

    /// Vrai pour une indication de mesure (y compris les annonces de reprise
    /// et de courtoisie).
    pub const fn is_time_sig_type(self) -> bool {
        matches!(
            self,
            Self::TimeSig
                | Self::TimeSigStartRepeatAnnounce
                | Self::TimeSigRepeatAnnounce
                | Self::TimeSigAnnounce
        )
    }
}

/// Élément rangé dans un segment.
///
/// Pendant de `EngravingItem` dans `engraving/dom/engravingitem.h`, limité aux
/// types utiles à l'export MusicXML.
#[derive(Clone, Debug, PartialEq)]
pub enum SegmentItem {
    /// Accord ou silence.
    ChordRest(ChordRest),
    /// Armure.
    KeySig(KeySig),
    /// Indication de mesure.
    TimeSig(TimeSig),
    /// Clef.
    Clef(Clef),
    /// Barre de mesure.
    BarLine(BarLine),
    /// Respiration.
    Breath(Breath),
}

impl SegmentItem {
    /// Accès à l'accord ou au silence, si l'élément en est un.
    pub fn as_chord_rest(&self) -> Option<&ChordRest> {
        match self {
            Self::ChordRest(value) => Some(value),
            _ => None,
        }
    }

    /// Accès mutable à l'accord ou au silence, si l'élément en est un.
    pub fn as_chord_rest_mut(&mut self) -> Option<&mut ChordRest> {
        match self {
            Self::ChordRest(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SegmentAnnotation {
    /// Piste à laquelle l'annotation est rattachée.
    pub track: Track,
    /// Annotation elle-même.
    pub annotation: Annotation,
}

impl SegmentAnnotation {
    /// Construit une annotation de segment rattachée à une piste.
    pub fn new(track: Track, annotation: Annotation) -> Self {
        Self { track, annotation }
    }

    /// Piste de l'annotation.
    pub const fn track(&self) -> Track {
        self.track
    }

    /// Annotation.
    pub fn annotation(&self) -> &Annotation {
        &self.annotation
    }

    /// Texte affiché, si l'annotation en a un.
    pub fn text(&self) -> &str {
        self.annotation.text()
    }
}

/// Segment vertical d'une mesure.
///
/// Correspond à `Segment` dans `engraving/dom/segment.h`. Tous les éléments
/// d'un segment commencent au même tick ; `elements` est indexé par
/// [`Track::raw`], avec `len = nb_portées * VOICES`. Les annotations portent
/// aussi leur piste, comme `EngravingItem::track()` dans MuseScore.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// Tick du segment.
    pub tick: Tick,
    /// Type du segment.
    pub kind: SegmentType,
    /// Annotations attachées au segment, avec leur piste.
    pub annotations: Vec<SegmentAnnotation>,
    elements: Vec<Option<SegmentItem>>,
}

impl Segment {
    /// Construit un segment vide pour un nombre donné de portées.
    pub fn new(kind: SegmentType, tick: Tick, staff_count: usize) -> Self {
        Self {
            tick,
            kind,
            annotations: Vec::new(),
            elements: vec![None; staff_count * VOICES],
        }
    }

    /// Élément de la piste, s'il existe.
    pub fn element(&self, track: Track) -> Option<&SegmentItem> {
        self.elements.get(track.index()).and_then(Option::as_ref)
    }

    /// Élément mutable de la piste, s'il existe.
    pub fn element_mut(&mut self, track: Track) -> Option<&mut SegmentItem> {
        self.elements
            .get_mut(track.index())
            .and_then(Option::as_mut)
    }

    /// Place un élément sur une piste, en agrandissant le vecteur si besoin.
    pub fn set_element(&mut self, track: Track, item: SegmentItem) -> Option<SegmentItem> {
        let index = track.index();
        if index >= self.elements.len() {
            self.elements.resize(index + 1, None);
        }
        self.elements[index].replace(item)
    }

    /// Retire l'élément d'une piste.
    pub fn take_element(&mut self, track: Track) -> Option<SegmentItem> {
        self.elements.get_mut(track.index()).and_then(Option::take)
    }

    /// Accès à l'accord ou au silence d'une piste.
    pub fn chord_rest(&self, track: Track) -> Option<&ChordRest> {
        self.element(track).and_then(SegmentItem::as_chord_rest)
    }

    /// Accès mutable à l'accord ou au silence d'une piste.
    pub fn chord_rest_mut(&mut self, track: Track) -> Option<&mut ChordRest> {
        self.element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
    }

    /// Ajoute une annotation au segment, rattachée à une piste.
    ///
    /// La piste joue le rôle de `EngravingItem::track()` dans MuseScore :
    /// elle indique la portée et la voix auxquelles l'export doit rattacher
    /// l'annotation (`Segment::annotations()` dans
    /// `engraving/dom/segment.h`).
    pub fn add_annotation(&mut self, track: Track, annotation: Annotation) {
        self.annotations
            .push(SegmentAnnotation::new(track, annotation));
    }

    /// Annotations d'une piste donnée.
    pub fn annotations_on(&self, track: Track) -> impl Iterator<Item = &Annotation> + '_ {
        self.annotations
            .iter()
            .filter(move |entry| entry.track == track)
            .map(|entry| &entry.annotation)
    }

    /// Éléments indexés par piste.
    pub fn elements(&self) -> &[Option<SegmentItem>] {
        &self.elements
    }

    /// Itère sur les pistes occupées.
    pub fn iter_elements(&self) -> impl Iterator<Item = (Track, &SegmentItem)> + '_ {
        self.elements
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item.as_ref()
                    .map(|item| (Track::from_raw(index as u32), item))
            })
    }

    /// Ajuste la taille du vecteur au nombre de portées.
    pub fn resize_staves(&mut self, staff_count: usize) {
        self.elements.resize(staff_count * VOICES, None);
    }
}
