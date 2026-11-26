// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de l'assemblage de `Score` et des objets d'`engraving/dom/` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026, à partir des événements datés d'une lecture
// LilyPond.
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

//! Construction de la partition à partir des voix interprétées.
//!
//! LilyPond ne connaît pas les mesures : elles se déduisent des indications de
//! mesure et de la musique. Cette passe établit donc une trame de mesures
//! (indications `\time`, `\set Timing.measureLength`, `\partial`, silences
//! multi-mesures), puis range les événements datés dans les segments de chaque
//! mesure, comme le fait le lecteur `.mscx` de MuseScore.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use scoretk_core::duration_list::to_duration_list;
use scoretk_core::model::{
    Accidental, AccidentalType, Annotation, ArticulationType, BarLine, BarLineType, Chord,
    ChordEngraving, ChordRest, ChordRestEngraving, ChordRestKind, Clef, Credit, CreditStyle,
    Duration, Dynamic, Fermata, Fingering, Fraction, Glissando, GlissandoType, GraceChord, Hairpin,
    HairpinType, Instrument, Jump, LineStyle, Lyrics, LyricsSyllabic, Marker, Measure, Note,
    NoteId, Part, PartId, PedalInfo, Placement, RehearsalMark, RepeatMark, Rest, RichText, Score,
    ScoreMetadata, Segment, SegmentAnnotation, SegmentItem, SegmentType, Spanner, SpannerAnchor,
    SpannerNotation, SpannerType, Staff, StaffBracket, StaffId, TboxText, Tempo, Track, Transpose,
    Trill, TrillType, Tuplet, TupletId, VOICES,
};
use scoretk_lilypond_common::midi_instruments;

mod articulate;

use crate::ast::{Context, Document, GroupKind, LyricsBlock, Music, PitchSpec, StaffBlock};
use crate::events::{
    arpeggio_object, articulation_objects, breath_object, ornament_objects, ottava_type,
    AnnotationEvent, ChordContent, ChordEvent, EventKind, GraceEvent, InterpretedVoice, MetaEvent,
    RepeatSection, StaffTarget, VoiceEvent,
};
use crate::pitch::OctaveState;
use crate::warnings::Warnings;
use crate::Interpreter;

/// Le `\version` du document régit le défaut d'un `\relative` sans argument.
///
/// Jusqu'à 2.17.14 inclus, LilyPond partait de `c'` ; la règle 2.17.15 de
/// `python/convertrules.py` (`\\relative -> \\relative c'`, avec refus de
/// `#(ly:set-option 'old-relative)`) marque le changement, et depuis 2.17.15
/// `ly/music-functions-init.ly` définit le défaut `f`. Un document qui déclare
/// une version ancienne doit donc être lu avec l'ancien défaut, même si le
/// compilateur installé est plus récent.
fn old_relative_version(version: Option<&str>) -> bool {
    let Some(version) = version else {
        return false;
    };
    let mut parties = version.split('.');
    let Some(majeur) = parties.next().and_then(|p| p.trim().parse::<u32>().ok()) else {
        return false;
    };
    let Some(mineur) = parties.next().and_then(|p| p.trim().parse::<u32>().ok()) else {
        return false;
    };
    let correctif = parties
        .next()
        .and_then(|p| p.trim().parse::<u32>().ok())
        .unwrap_or(0);
    (majeur, mineur, correctif) < (2, 17, 15)
}

/// Intervalle pendant lequel les barres automatiques sont suspendues.
///
/// Correspond à `Timing.timing = ##f` de LilyPond : la mesure en cours
/// s'allonge jusqu'à la fermeture de la cadence (`\cadenzaOff`) ou à la
/// première barre explicite. Les intervalles de toutes les voix sont
/// collectés tels quels : leurs bornes servent à la fois de coupures et de
/// test d'appartenance, sans fusion (deux cadences adjacentes séparées par un
/// `\bar` doivent rester deux mesures).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CadenzaSpan {
    /// Tick du `\cadenzaOn`.
    start: Fraction,
    /// Tick du `\cadenzaOff`, d'une barre explicite ou de la fin de la musique.
    end: Fraction,
}

/// Trame d'une mesure.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MeasureSlot {
    /// Tick de début.
    start: Fraction,
    /// Durée réelle.
    len: Fraction,
    /// Durée nominale de l'indication de mesure.
    nominal: Fraction,
    /// Nombre de mesures regroupées par un silence multi-mesures.
    multi_rest: Option<u16>,
}

/// Voix interprétée et rattachée à une portée.
struct VoiceBuild {
    /// Nom du contexte (`\new Voice = "…"`), pour `\lyricsto`.
    name: Option<String>,
    /// Piste du modèle.
    track: Track,
    /// Événements.
    interpreted: InterpretedVoice,
    /// N-olets globaux, dans l'ordre local.
    tuplets: Vec<TupletId>,
}

/// Voix en attente de rattachement à une portée.
///
/// Les voix ouvertes depuis l'intérieur d'une autre (`\new Voice`, branches
/// d'un `<< … >>`) sont traitées après celle qui les ouvre, jusqu'à
/// épuisement.
struct PendingVoice {
    /// Nom du contexte (`\new Voice = "…"`), pour `\lyricsto`.
    name: Option<String>,
    /// Paroles `\addlyrics` attachées au contexte ouvert.
    lyrics: Vec<LyricsBlock>,
    /// Branche d'un passage polyphonique temporaire (`<< … \\ … >>`).
    temporary: bool,
    /// `\new Staff` imbriqué : place la voix sur une portée distincte, avec
    /// son propre `Audio_staff` (`lily/staff-performer.cc:128-153`) et donc
    /// sa piste MIDI (`lily/performance.cc:69-110`).
    separate_staff: bool,
    /// Voix interprétée.
    interpreted: InterpretedVoice,
}

/// Paroles interprétées.
struct LyricsBuild {
    /// Nom de la voix cible.
    target: String,
    /// Numéro de couplet.
    verse: usize,
    /// Syllabes.
    syllables: Vec<crate::ast::Syllable>,
}

/// Portée construite.
struct StaffBuild {
    /// Identifiant de la portée.
    staff: StaffId,
    /// `Audio_staff` MIDI de cette portée.
    ///
    /// Une portée sœur ouverte pour loger une cinquième voix simultanée
    /// n'est qu'un artifact du modèle (quatre voies par portée) : pour
    /// LilyPond toutes ces voix restent dans le même contexte `Staff`, donc
    /// dans un seul `Audio_staff` (`lily/staff-performer.cc:114-143`,
    /// `get_audio_staff`), où `Midi_walker::do_start_note` fusionne les
    /// unissons de même attaque. Les portées ordinaires pointent vers elles-
    /// mêmes.
    audio_staff: StaffId,
    /// Nom du contexte (`\new Staff = "…"`), pour résoudre `\change`.
    name: Option<String>,
    /// Groupe du bloc `\score` d'appartenance (ordre de création des contextes).
    group: usize,
    /// Voix.
    voices: Vec<VoiceBuild>,
    /// Paroles attachées à cette portée.
    lyrics: Vec<LyricsBuild>,
}

/// `Audio_staff` auquel appartient réellement une portée du modèle.
///
/// `lily/context.cc` (`find_create_context`, l. 269-273, et `unchecked_find`,
/// l. 200-231) fait retrouver par `\context Staff = "nom"` le **premier**
/// contexte portant ce nom, là où `\new Staff = "nom"` en crée un nouveau.
/// Toutes les portées du modèle issues de ce même contexte partagent alors un
/// seul `Audio_staff` (`lily/staff-performer.cc` l. 164-182, `get_audio_staff`
/// avec le réglage par défaut `midiChannelMapping = #'staff`,
/// `ly/performer-init.ly` l. 257), donc une seule piste MIDI
/// (`lily/audio-staff.cc` l. 39-52). Un doublon de nom `\context` n'est qu'un
/// artefact du modèle : il doit être ramené à la première portée du nom.
fn audio_staff_of(staves: &[StaffBuild], staff_block: &StaffBlock, fallback: StaffId) -> StaffId {
    if !staff_block.existing {
        return fallback;
    }
    let Some(name) = staff_block.name.as_deref() else {
        return fallback;
    };
    staves
        .iter()
        .find(|staff| staff.name.as_deref() == Some(name))
        .map(|staff| staff.audio_staff)
        .unwrap_or(fallback)
}

/// Un point typographique mesure 2540/7227 mm (Notation Reference §27.2) ;
/// une portée de `n` points a une hauteur de `n × 2540/7227` mm, soit quatre
/// spatiums.
const MM_PAR_POINT: f64 = 2540.0 / 7227.0;

/// Applique `#(set-global-staff-size n)` et les tailles de texte propres à
/// scoretk, écrites dans `\header` (les styles de texte n'ont pas de champ
/// `\paper` standard, §26.6/§21.2).
fn apply_text_styles(score: &mut Score, document: &Document) {
    if let Some(size) = document.staff_size {
        score.engraving.style.spatium_mm = size * MM_PAR_POINT / 4.0;
    }
    for (name, target) in [
        (
            "scoretkTitleFontSize",
            &mut score.engraving.style.title_font_size,
        ),
        (
            "scoretkComposerFontSize",
            &mut score.engraving.style.composer_font_size,
        ),
        (
            "scoretkLyricistFontSize",
            &mut score.engraving.style.lyricist_font_size,
        ),
    ] {
        if let Some(value) = document
            .header
            .get(name)
            .and_then(|text| text.trim().parse::<f64>().ok())
        {
            if value.is_finite() && value > 0.0 {
                *target = value;
            }
        }
    }
    // Un autre outil LilyPond n'écrit pas les champs privés, mais peut mettre
    // la même taille dans un `\header` en markup standard, par exemple
    // `title = \markup { \fontsize #24 "Titre" }` (§8.3, §21.1.2).
    for (field, target) in [
        ("title", &mut score.engraving.style.title_font_size),
        ("composer", &mut score.engraving.style.composer_font_size),
        ("poet", &mut score.engraving.style.lyricist_font_size),
    ] {
        if let Some(value) = document
            .header
            .font_sizes
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| *value)
        {
            if value.is_finite() && value > 0.0 {
                *target = value;
            }
        }
    }
}

/// `\paper` ne porte que la géométrie de page (L5) : `paper-width`,
/// `paper-height`, `left-margin`, `right-margin`, `top-margin` et
/// `bottom-margin`, écrits en millimètres par l'exporteur. Les autres clés
/// (polices, `\layout`) n'ont pas de champ dans `StyleSettings`.
fn apply_paper(score: &mut Score, paper: &crate::ast::HeaderAst) {
    let millimeters = |name: &str| {
        paper
            .get(name)
            .and_then(|value| value.trim_end_matches("\\mm").trim().parse::<f64>().ok())
            .filter(|value| *value > 0.0 && value.is_finite())
            .map(|value| value / 25.4)
    };
    for (name, target) in [
        ("paper-width", 0),
        ("paper-height", 1),
        ("left-margin", 2),
        ("right-margin", 3),
        ("top-margin", 4),
        ("bottom-margin", 5),
    ] {
        let Some(inches) = millimeters(name) else {
            continue;
        };
        match target {
            0 => score.engraving.style.page_width_in = inches,
            1 => score.engraving.style.page_height_in = inches,
            2 => {
                score.engraving.style.page_odd_left_margin_in = inches;
                score.engraving.style.page_even_left_margin_in = inches;
            }
            3 => {
                score.engraving.style.page_odd_right_margin_in = inches;
                score.engraving.style.page_even_right_margin_in = inches;
            }
            4 => {
                score.engraving.style.page_odd_top_margin_in = inches;
                score.engraving.style.page_even_top_margin_in = inches;
            }
            _ => {
                score.engraving.style.page_odd_bottom_margin_in = inches;
                score.engraving.style.page_even_bottom_margin_in = inches;
            }
        }
    }
}

/// Construit la partition d'un document LilyPond.
pub(crate) fn build(document: &Document, warnings: &mut Warnings) -> Score {
    let mut score = Score::new();
    // Transposition d'un second `\score` MIDI : ignorée par tous les écrivains
    // sauf MIDI, la partition gravée reste la référence du modèle.
    score.playback.transposition = document
        .playback_transposition
        .map(scoretk_core::model::PlaybackTransposition::new);
    score.playback.articulate = document.playback_articulate;
    score.playback.unfold_repeats = document.playback_unfold_repeats;
    write_metadata(&mut score, document);
    apply_text_styles(&mut score, document);
    apply_paper(&mut score, &document.paper);

    let mut staves: Vec<StaffBuild> = Vec::new();
    let mut octave_relatif = OctaveState {
        old_relative: old_relative_version(document.version.as_deref()),
        ..OctaveState::default()
    };
    // Identifiants des contextes `Voice` (un `Tie_performer` par voix,
    // `ly/performer-init.ly:100-109`), partagés entre toutes les portées.
    let mut next_voice_context = 0u32;
    // Branches numberées des `<< … \\ … >>`, réutilisées d'un passage à l'autre.
    let mut voice_contexts: Vec<u32> = Vec::new();
    let Some(score_ast) = &document.score else {
        return score;
    };
    // Un bloc `\new Lyrics \lyricsto "…"` peut être frère du `\new Voice`
    // dans un `<< … >>` de `\score` : il forme un groupe sans portée. Ces
    // paroles sont conservées et rattachées plus loin à la voix nommée, où
    // qu'elle ait été déclarée.
    let mut pending_lyrics: Vec<(usize, LyricsBuild)> = Vec::new();
    // `\transposition` est une propriété de Staff mais le parseur peut
    // séparer un `\context Staff` en plusieurs groupes : le `\global` (souvent
    // le premier, sans note) porte la propriété, les `\context Voice` qui
    // suivent portent la musique. On retient alors l'intervalle jusqu'à ce
    // qu'un groupe musical le reçoive.
    let mut pending_transposition: Option<(PitchSpec, bool)> = None;
    // Un `\context Staff` ne contenant que des propriétés (par exemple
    // `\transposition c` de `midiStuff`) est retrouvé par LilyPond : il ne
    // crée pas une nouvelle portée, il configure celle qui suit. Son
    // intervalle doit donc survivre au groupe musical suivant.
    let mut pending_from_empty_context = false;
    // Les ruptures de section sont posées après la création des mesures :
    // `section_break_before` force l'export MusicXML à réécrire `<transpose>`.
    let mut transposition_breaks: Vec<Fraction> = Vec::new();
    // Début de l'élément courant d'une séquence `{ … }` : les contextes
    // nommés qui y reparaissent continuent à cette date. Les éléments d'un
    // `<< … >>` ne portent pas la marque et gardent zéro.
    let mut element_start = Fraction::ZERO;
    // Portée du dernier `\context Staff = "…"` sans musique : les
    // `\context Voice` qui suivent sont ses voix (idiome `midiStuff` de
    // carcassi-op60-09/15), pas des portées séparées.
    let mut contexte_portee: Option<(StaffId, PartId, String)> = None;
    let mut derniere_portee: Option<(StaffId, PartId, String)> = None;
    for (group_index, group) in score_ast.groups.iter().enumerate() {
        if group.sequence_start {
            element_start = staves
                .iter()
                .flat_map(|staff| staff.voices.iter())
                .map(|voice| voice.interpreted.end)
                .max()
                .unwrap_or(Fraction::ZERO);
        }
        // Une branche ossia porte son début absolu : il remplace le début
        // global de la séquence, qui suit la fin de toutes les portées déjà
        // construites et placerait l'ossia après la musique voisine.
        let group_start = group.absolute_start.unwrap_or(element_start);
        if group.staves.is_empty() {
            // Portée précédente : c'est elle qui porte la voix la plus proche
            // quand les paroles n'ont pas de cible explicite.
            let previous = staves.len().saturating_sub(1);
            pending_lyrics.extend(
                group
                    .lyrics
                    .iter()
                    .map(|block| (previous, lyrics_build(block))),
            );
            continue;
        }
        // `\context Voice = "x"` qui reparaît dans une section ultérieure
        // continue la **même** voix : `find_create_context`
        // (`lily/context.cc` l. 269-273) retrouve le contexte nommé, et la
        // musique de la section reprend à la date de son élément dans la
        // séquence (`lily/parser.yy` `sequential_music`). La voix garde sa
        // piste (carcassi-op60-09/15 : `melodyVoiceA` rejoue la section C
        // après la section B).
        if group.existing && group.kind == GroupKind::Voice && group.staves.len() == 1 {
            let staff_block = &group.staves[0];
            if let Some(nom) = staff_block.name.clone() {
                // Portée cible : celle du dernier `\context Staff` vide
                // (`midiStuff`), sinon la portée portant le nom de la voix.
                let portee = contexte_portee.clone().or_else(|| {
                    staves
                        .iter()
                        .position(|staff| {
                            staff.name.as_deref() == Some(nom.as_str())
                                && staff
                                    .voices
                                    .iter()
                                    .any(|voice| voice.name.as_deref() == Some(nom.as_str()))
                        })
                        .map(|index| {
                            let staff_id = staves[index].staff;
                            (staff_id, score.staff(staff_id).part, nom.clone())
                        })
                });
                if let Some((staff_id, part_id, _nom_portee)) = portee {
                    let index = match staves.iter().position(|staff| staff.staff == staff_id) {
                        Some(index) => index,
                        None => {
                            staves.push(StaffBuild {
                                staff: staff_id,
                                audio_staff: staff_id,
                                name: Some(nom.clone()),
                                group: group_index,
                                voices: Vec::new(),
                                lyrics: Vec::new(),
                            });
                            staves.len() - 1
                        }
                    };
                    // Emplacement stable par nom de voix dans la portée :
                    // une voix continuée reprend sa piste, une nouvelle voix
                    // prend le premier emplacement libre.
                    let slot = staves[index]
                        .voices
                        .iter()
                        .find(|voice| voice.name.as_deref() == Some(nom.as_str()))
                        .map(|voice| (voice.track.raw() % VOICES as u32) as usize)
                        .or_else(|| {
                            let utilises: Vec<bool> = (0..VOICES)
                                .map(|slot| {
                                    staves[index].voices.iter().any(|voice| {
                                        (voice.track.raw() % VOICES as u32) as usize == slot
                                    })
                                })
                                .collect();
                            (0..VOICES).find(|slot| !utilises[*slot])
                        });
                    // La piste de la voix continuée doit être reprise : on
                    // bloque les autres emplacements pour `build_voices`, qui
                    // choisit le premier libre.
                    let mut occupied: Vec<Option<Fraction>> = vec![None; VOICES];
                    for (autre, value) in occupied.iter_mut().enumerate() {
                        if Some(autre) != slot {
                            *value = Some(element_start + Fraction::ONE);
                        }
                    }
                    let (built, octave_suivante) = build_voices(
                        &mut score,
                        staff_id,
                        part_id,
                        staff_block,
                        warnings,
                        &mut next_voice_context,
                        &mut voice_contexts,
                        octave_relatif,
                        &occupied,
                        element_start,
                    );
                    octave_relatif = octave_suivante;
                    let mut built = built.into_iter();
                    if let Some(premiere) = built.next() {
                        // La voix continuée reste sous la transposition de la
                        // portée (`\transposition` est une propriété de
                        // Staff) : sans ce décalage, la section rejouée
                        // sonnait une octave au-dessus (carcassi-op60-09,
                        // seconde section B).
                        let transposition = instrument_transpose(&staff_block.voices[0].music)
                            .or(pending_transposition);
                        let mut nouveau = StaffBuild {
                            staff: staff_id,
                            audio_staff: staff_id,
                            name: None,
                            group: group_index,
                            voices: premiere.voices,
                            lyrics: premiere.lyrics,
                        };
                        if let Some((pitch, true)) = transposition {
                            let interval = interval_from_pitch(&pitch);
                            let mut changes = transpositions(std::slice::from_ref(&nouveau));
                            if !changes.iter().any(|(tick, _)| tick.is_zero()) {
                                changes.insert(0, (Fraction::ZERO, interval));
                            }
                            decaler_vers_concert(std::slice::from_mut(&mut nouveau), &changes);
                            if transposition_depuis(std::slice::from_ref(&nouveau)).is_zero() {
                                score.part_mut(part_id).instrument.transpose = Some(interval);
                            }
                        }
                        staves[index].voices.extend(nouveau.voices);
                        staves[index]
                            .lyrics
                            .extend(staff_block.lyrics.iter().map(lyrics_build));
                        staves[index].lyrics.extend(nouveau.lyrics);
                    }
                    for sœur in built {
                        staves.push(StaffBuild {
                            staff: sœur.staff,
                            audio_staff: sœur.audio_staff,
                            name: None,
                            group: group_index,
                            voices: sœur.voices,
                            lyrics: sœur.lyrics,
                        });
                    }
                    continue;
                }
            }
        }
        // `\context Staff = "x"` retrouve la portée nommée déjà construite
        // (`lily/context.cc`, `Context::find_create_context` →
        // `find (FIND_CREATE)` : un contexte nommé est retrouvé, pas
        // dupliqué), alors que `\new Staff = "x"` en ouvre toujours une
        // nouvelle. La musique retrouvée s'ajoute donc à la **même** portée,
        // comme voix simultanée : c'est un seul `Audio_staff` MIDI
        // (`lily/staff-performer.cc`, `get_audio_staff`), où
        // `Midi_walker::do_start_note` fusionne les unissons d'attaque
        // (`lily/midi-walker.cc` l. 95-115). Les chorals de Mutopia écrivent
        // ainsi deux fois `\context Staff = "right"` pour ajouter une
        // registration transposée (bwv633, bwv544, bwv679…).
        // Seules les portées sont concernées ici : un `\context Voice = "x"`
        // qui reparaît dans une section ultérieure continue une voix dans le
        // temps (voir la reprise de contextes nommés) et ne doit pas être
        // empilé au tick 0.
        if group.existing && group.kind == GroupKind::Staff && group.staves.len() == 1 {
            let staff_block = &group.staves[0];
            if let Some(nom) = staff_block.name.clone() {
                if let Some(index) = staves
                    .iter()
                    .position(|staff| staff.name.as_deref() == Some(nom.as_str()))
                {
                    let staff_id = staves[index].staff;
                    let part_id = score.staff(staff_id).part;
                    // Les voix déjà construites occupent leur emplacement :
                    // les nouvelles musiques sont simultanées (même tick de
                    // départ), elles doivent donc prendre d'autres
                    // emplacements au lieu d'écraser les premières pistes.
                    let mut occupied: Vec<Option<Fraction>> = vec![None; VOICES];
                    for voice in &staves[index].voices {
                        let slot = (voice.track.raw() % VOICES as u32) as usize;
                        if slot < VOICES {
                            occupied[slot] = Some(voice.interpreted.end);
                        }
                    }
                    // `\context Staff = "x"` dans deux éléments successifs
                    // d'une séquence `{ … }` continue après la musique de
                    // l'élément précédent ; dans un même `<< … >>`, les deux
                    // éléments sont simultanés et partent du tick 0
                    // (`element_start` vaut alors zéro).
                    let start = element_start;
                    let (built, octave_suivante) = build_voices(
                        &mut score,
                        staff_id,
                        part_id,
                        staff_block,
                        warnings,
                        &mut next_voice_context,
                        &mut voice_contexts,
                        octave_relatif,
                        &occupied,
                        start,
                    );
                    octave_relatif = octave_suivante;
                    let mut built = built.into_iter();
                    if let Some(premiere) = built.next() {
                        let cible = &mut staves[index];
                        cible.voices.extend(premiere.voices);
                        cible
                            .lyrics
                            .extend(staff_block.lyrics.iter().map(lyrics_build));
                        cible.lyrics.extend(premiere.lyrics);
                    }
                    // Au-delà de quatre voix simultanées, `build_voices` a
                    // ouvert des portées sœurs : elles restent dans la même
                    // partie, comme pour un groupe neuf.
                    for sœur in built {
                        staves.push(StaffBuild {
                            staff: sœur.staff,
                            audio_staff: sœur.audio_staff,
                            name: None,
                            group: group_index,
                            voices: sœur.voices,
                            lyrics: sœur.lyrics,
                        });
                    }
                    continue;
                }
            }
        }
        // `\transposition` est une propriété de Staff. Une partie MusicXML
        // n'a qu'un `<transpose>` : on ne le porte donc que si le groupe ne
        // contient qu'une portée, seul cas où l'on sait à quel instrument
        // l'appliquer. Les `\context Staff` transparents de certaines sources
        // anciennes déposent la propriété dans un groupe plus large ; laisser
        // la transposition à la partie entière décalerait les autres portées.
        let propre = if group.staves.len() == 1 {
            // Les partitions de guitare posent souvent `\transposition c`
            // dans chacune des voix d'une même portée ; toutes portent alors
            // le même intervalle et la propriété vaut pour la portée entière.
            let transpositions: Vec<_> = group.staves[0]
                .voices
                .iter()
                .map(|voice| instrument_transpose(&voice.music))
                .collect();
            transpositions
                .first()
                .copied()
                .flatten()
                .filter(|_| transpositions.iter().all(|item| item == &transpositions[0]))
        } else {
            None
        };
        let a_des_notes = group.staves.iter().any(|staff| {
            staff
                .voices
                .iter()
                .any(|voice| music_has_chords(&voice.music))
        });
        if propre.is_some() {
            pending_transposition = propre;
            pending_from_empty_context = !a_des_notes && group.existing;
        } else if (group.staves.iter().any(|staff| {
            staff
                .voices
                .iter()
                .any(|voice| music_has_pitched(&voice.music))
        }) || group.staves.iter().any(|staff| staff.name.is_some()))
            && !matches!(group.kind, GroupKind::Voice)
            && !pending_from_empty_context
        {
            // Un nouveau groupe structural (StaffGroup, Staff…) ferme la
            // portée qui portait la propriété : ne pas la propager au-delà.
            // Un groupe anonyme sans note (le `\global` d'une section,
            // simple préambule de silences) ne ferme rien : il ne crée pas
            // de portée qui recevrait la musique (carcassi-op60-09, section B).
            pending_transposition = None;
            contexte_portee = None;
        }
        // `pendante` : la propriété vient d'un groupe précédent (un
        // `\context Staff` sans musique, idiome `\global`/`midiStuff`) et
        // vaut pour les voix qui suivent sur la même portée.
        let pendante = propre.is_none();
        let transposition = propre.or(pending_transposition);
        let first = &group.staves[0];
        let long_name = group
            .instrument_name
            .clone()
            .or_else(|| first.instrument_name.clone())
            .unwrap_or_default();
        let short_name = group
            .short_instrument_name
            .clone()
            .or_else(|| first.short_instrument_name.clone())
            .unwrap_or_default();
        let midi_instrument = group
            .midi_instrument
            .as_deref()
            .or(first.midi_instrument.as_deref())
            .map(str::to_owned);
        let mut part = Part::new(long_name.clone(), short_name.clone());
        part.name = long_name.clone();
        part.instrument = instrument_for(&long_name, &short_name, midi_instrument.as_deref());
        apply_midi_levels(&mut part.instrument, first.midi_expression, first.midi_pan);
        let part_id = score.add_part(part);

        let first_staff_index = staves.len();
        // Début du dernier élément de séquence rencontré : ses portées
        // `sequence_continue` commencent au même instant que la première.
        let mut element_start_current = element_start;
        for (position, staff_block) in group.staves.iter().enumerate() {
            let staff_start = staves.len();
            let staff_transposition = if group.staves.len() > 1 {
                staff_block
                    .voices
                    .iter()
                    .find_map(|voice| instrument_transpose(&voice.music))
            } else {
                None
            };
            let staff_part_id = if let Some((pitch, true)) = staff_transposition {
                // Portée transpositrice dans un groupe : instrument distinct,
                // transposition portée par le Part, hauteurs du modèle écrites.
                let interval = interval_from_pitch(&pitch);
                let nom = staff_block.name.clone().unwrap_or_default();
                let court = staff_block
                    .short_instrument_name
                    .clone()
                    .unwrap_or_default();
                let midi_staff = staff_block
                    .midi_instrument
                    .as_deref()
                    .map(str::to_owned)
                    .or_else(|| midi_instrument.clone());
                let mut part = Part::new(nom.clone(), court.clone());
                part.name = nom;
                part.instrument = instrument_for(&part.name, &court, midi_staff.as_deref());
                apply_midi_levels(
                    &mut part.instrument,
                    staff_block.midi_expression,
                    staff_block.midi_pan,
                );
                part.instrument.transpose = Some(interval);
                score.add_part(part)
            } else {
                part_id
            };
            let mut staff = Staff::new(staff_part_id);
            staff.muted = staff_block.muted;
            staff.visible = staff_block.visible;
            if let Some(bracket) = group_bracket(group.kind.clone(), position, group.staves.len()) {
                staff.brackets.push(bracket);
            }
            let staff_id = score.add_staff_with(staff_part_id, staff);
            derniere_portee = Some((
                staff_id,
                staff_part_id,
                staff_block.name.clone().unwrap_or_default(),
            ));
            // Portée d'une séquence de groupe (`\new OneStaff { \new Staff …
            // \new Staff … }`) : elle commence après les précédentes du même
            // groupe (voir `StaffBlock::sequence_start`).
            let debut_sequence = if staff_block.sequence_start {
                element_start_current = staves[first_staff_index..]
                    .iter()
                    .flat_map(|staff| staff.voices.iter())
                    .map(|voice| voice.interpreted.end)
                    .max()
                    .unwrap_or(element_start);
                element_start_current
            } else if staff_block.sequence_continue {
                element_start_current
            } else {
                group_start
            };
            let (built, octave_suivante) = build_voices(
                &mut score,
                staff_id,
                staff_part_id,
                staff_block,
                warnings,
                &mut next_voice_context,
                &mut voice_contexts,
                octave_relatif,
                &[],
                debut_sequence,
            );
            octave_relatif = octave_suivante;
            let mut built = built.into_iter();
            if let Some(premiere) = built.next() {
                let mut lyrics: Vec<LyricsBuild> =
                    staff_block.lyrics.iter().map(lyrics_build).collect();
                lyrics.extend(premiere.lyrics);
                staves.push(StaffBuild {
                    staff: staff_id,
                    audio_staff: audio_staff_of(&staves, staff_block, staff_id),
                    name: staff_block.name.clone(),
                    group: group_index,
                    voices: premiere.voices,
                    lyrics,
                });
            }
            // Portées sœurs créées uniquement pour les voix réellement
            // simultanées au-delà de quatre : elles gardent les mêmes paroles
            // que la portée d'origine, adressées par nom de voix.
            for sœur in built {
                staves.push(StaffBuild {
                    staff: sœur.staff,
                    audio_staff: sœur.audio_staff,
                    name: None,
                    group: group_index,
                    voices: sœur.voices,
                    lyrics: sœur.lyrics,
                });
            }
            if let Some((_pitch, true)) = staff_transposition {
                let changes = transpositions(&staves[staff_start..]);
                decaler_vers_concert(&mut staves[staff_start..], &changes);
            }
        }
        // Paroles du groupe (hors portée) : rattachées à sa première portée.
        if !group.lyrics.is_empty() {
            if let Some(target) = staves.get_mut(first_staff_index) {
                for block in &group.lyrics {
                    target.lyrics.push(lyrics_build(block));
                }
            }
        }
        // C'est ici, une fois la structure connue, que la concert est décalée
        // et que le `<transpose>` est posé sur la partie.
        if let Some((pitch, a_decaler)) = transposition {
            // Une partie n'a qu'un `<transpose>` : ne le poser que si elle
            // contient une seule portée, seul cas où l'on sait à quel
            // instrument l'appliquer. Une portée à plusieurs voix peut porter
            // la propriété si toutes les voix la portent aussi (voir `propre`).
            if staves[first_staff_index..].len() == 1 {
                // Un groupe sans note ne porte pas la transposition : il ne
                // sert que de relais vers les voix musicales qui le suivent
                // (idiome `\global` des partitions anciennes).
                let a_des_notes = staves[first_staff_index..].iter().any(|staff| {
                    staff.voices.iter().any(|voice| {
                        voice
                            .interpreted
                            .events
                            .iter()
                            .any(|event| matches!(&event.kind, EventKind::Chord(_)))
                    })
                });
                if a_des_notes {
                    let interval = interval_from_pitch(&pitch);
                    let depuis = transposition_depuis(&staves[first_staff_index..]);
                    if a_decaler {
                        let mut changes = transpositions(&staves[first_staff_index..]);
                        // Un `\context Staff` sans musique a posé la
                        // propriété dans un groupe précédent : la retrouver
                        // quand le groupe musical ne la porte pas lui-même.
                        if pendante && !changes.iter().any(|(tick, _)| tick.is_zero()) {
                            changes.insert(0, (Fraction::ZERO, interval));
                        }
                        decaler_vers_concert(&mut staves[first_staff_index..], &changes);
                    }
                    if depuis.is_zero() {
                        score.part_mut(part_id).instrument.transpose = Some(interval);
                    }
                    // Un changement de `\transposition` en cours de pièce est
                    // aussi un changement d'instrument : le modèle le porte
                    // avec `set_instrument_at`, comme l'import MuseScore
                    // (`read302/measure.rs`, `read4xx/mod.rs`).
                    let base = score.part(part_id).instrument.clone();
                    for (tick, change) in transpositions(&staves[first_staff_index..]) {
                        if tick.is_zero() {
                            continue;
                        }
                        let mut instrument = base.clone();
                        instrument.transpose = Some(change);
                        score.part_mut(part_id).set_instrument_at(tick, instrument);
                        transposition_breaks.push(tick);
                    }
                    pending_from_empty_context = false;
                    // La propriété vaut pour toute la portée : les voix
                    // musicales suivantes la reçoivent aussi. Un groupe
                    // structural (`Staff`, `StaffGroup…`) la referme plus haut.
                } else {
                    pending_transposition = Some((pitch, a_decaler));
                }
            } else if propre.is_some() {
                // Une portée source a été dépliée en plusieurs portées (voix
                // nommées d'une guitare) : `\transposition` est une propriété
                // du Staff (`lily/staff-performer.cc`, `instrumentTransposition`),
                // donc elle vaut pour chacune de ses voix. Le modèle reste en
                // hauteurs concert ; la partie reçoit `instrument.transpose`
                // (une partie MusicXML n'a qu'un `<transpose>`) et le MusicXML
                // écrit la hauteur écrite.
                let interval = interval_from_pitch(&pitch);
                decaler_vers_concert(
                    &mut staves[first_staff_index..],
                    &[(Fraction::ZERO, interval)],
                );
                if transposition_depuis(&staves[first_staff_index..]).is_zero() {
                    score.part_mut(part_id).instrument.transpose = Some(interval);
                }
            }
        }
        // Un `\context Staff` existant sans musique reste la portée courante :
        // les `\context Voice` qui suivent lui appartiennent (midiStuff).
        if pending_from_empty_context {
            contexte_portee = derniere_portee.clone();
        }
    }

    if staves.is_empty() {
        return score;
    }
    // `ly/articulate.ly` transforme la musique *avant* le calcul des mesures :
    // les notes courtes de l'excursion tombent dans la durée de la note écrite.
    articulate::expand_articulate(&mut staves, score.playback.articulate);
    for (index, lyrics) in pending_lyrics {
        if let Some(staff) = staves.get_mut(index) {
            staff.lyrics.push(lyrics);
        }
    }

    apply_initial_clefs(&mut score, &staves);
    let slots = plan_measures(&staves, document.staff_local_timing, warnings);
    create_measures(&mut score, &slots);
    for tick in transposition_breaks {
        if let Some(index) = score.measures.iter().position(|measure| {
            tick >= measure.start_tick && tick < measure.start_tick + measure.len
        }) {
            score.measures[index].section_break_before = true;
        }
    }
    place_measures(&mut score, &staves, &slots, warnings);
    apply_accidentals(&mut score);
    place_spanners(&mut score, &staves);
    place_repeats(&mut score, &staves);
    sync_end_bar_segments(&mut score);
    place_lyrics(&mut score, &staves, document.melisma_busy_disabled);
    let midi_voices_as_staves = document.score.as_ref().is_some_and(|score_ast| {
        score_ast
            .groups
            .iter()
            .any(|group| group.staves.iter().any(|staff| staff.midi_voices_as_staves))
    });
    link_ties(&mut score, &staves, midi_voices_as_staves);
    // Après `link_ties` : une note liée n'est pas une attaque et ne peut pas
    // masquer la note déplacée (`lily/tie-performer.cc`).
    fusionne_unissons_entre_portees(&mut score, &staves);
    resolve_after_grace_ties(&mut score);
    // LilyPond ne réalise pas \arpeggio au MIDI ; ses ornements ne sont
    // développés que par `\articulate` (`ly/articulate.ly`), pas par le
    // performeur de note (`lily/note-performer.cc`).
    let staff_ids: Vec<scoretk_core::model::StaffId> =
        score.staves.iter().map(|staff| staff.id).collect();
    for staff in staff_ids {
        for measure in &mut score.measures {
            for segment in &mut measure.segments {
                for voice in 0..scoretk_core::model::VOICES {
                    let track = scoretk_core::model::Track::new(staff, voice as u8);
                    let Some(chord_rest) = segment.chord_rest_mut(track) else {
                        continue;
                    };
                    let scoretk_core::model::ChordRestKind::Chord(chord) = &mut chord_rest.kind
                    else {
                        continue;
                    };
                    if let Some(arpeggio) = &mut chord.arpeggio {
                        arpeggio.play = false;
                    }
                    if !document.playback_articulate {
                        for ornament in &mut chord.ornaments {
                            ornament.play = false;
                        }
                    }
                }
            }
        }
    }
    if document.show_first_length.is_some() || document.show_last_length.is_some() {
        cut_score(
            &mut score,
            document.show_first_length.unwrap_or(Fraction::ZERO),
            document.show_last_length.unwrap_or(Fraction::ZERO),
        );
    }
    score
}

/// Coupe la partition selon `showFirstLength`/`showLastLength`
/// (`scm/music-functions.scm:1526-1600`) : garde le début de longueur
/// `first` et la fin de longueur `last`, et replie le temps sauté
/// (`lily/score-performer.cc:151-176`, `midiSkipOffset`) pour que la fin
/// commence juste après le début.
fn cut_score(score: &mut Score, first: Fraction, last: Fraction) {
    if first <= Fraction::ZERO && last <= Fraction::ZERO {
        return;
    }
    let total = score
        .measures
        .last()
        .map_or(Fraction::ZERO, |measure| measure.start_tick + measure.len);
    if first + last >= total {
        return;
    }
    let suffix_start = total - last;
    let shift = suffix_start - first;
    let mut kept = Vec::new();
    for mut measure in std::mem::take(&mut score.measures) {
        let start = measure.start_tick;
        if start < first {
            kept.push(measure);
        } else if start >= suffix_start {
            measure.start_tick -= shift;
            for segment in &mut measure.segments {
                segment.tick -= shift;
            }
            measure.number = kept.len();
            kept.push(measure);
        }
    }
    let mut spanners = Vec::new();
    for mut spanner in std::mem::take(&mut score.spanners) {
        if spanner.start.tick < first {
            spanners.push(spanner);
        } else if spanner.start.tick >= suffix_start {
            spanner.start.tick -= shift;
            spanner.end.tick -= shift;
            spanners.push(spanner);
        }
    }
    score.measures = kept;
    score.spanners = spanners;
}

/// Vrai si la voix porte au moins un accord, un silence ou un temps réservé
/// (tout événement de type [`EventKind::Chord`]).
fn has_chord_events(interpreted: &InterpretedVoice) -> bool {
    interpreted
        .events
        .iter()
        .any(|event| matches!(&event.kind, EventKind::Chord(_)))
}

/// Replie les voix sans accord dans la voix musicale `target` et les retire.
///
/// Le préambule passe avant la musique dans l'ordre d'émission : clefs,
/// armures et indications de mesure précèdent les premières notes de la
/// portée.
fn merge_preamble(kept: Vec<PendingVoice>, target: usize) -> Vec<PendingVoice> {
    let mut preamble: Vec<VoiceEvent> = Vec::new();
    let mut end = Fraction::ZERO;
    let mut rest: Vec<PendingVoice> = Vec::new();
    let mut target_index = 0;
    for (position, pending) in kept.into_iter().enumerate() {
        if position == target {
            target_index = rest.len();
            end = end.max(pending.interpreted.end);
            rest.push(pending);
        } else if has_chord_events(&pending.interpreted) {
            rest.push(pending);
        } else {
            end = end.max(pending.interpreted.end);
            preamble.extend(pending.interpreted.events);
        }
    }
    if let Some(target) = rest.get_mut(target_index) {
        preamble.append(&mut target.interpreted.events);
        for (order, event) in preamble.iter_mut().enumerate() {
            event.order = order;
        }
        // Le préambule précède la musique à tick égal ; l'ordre chronologique
        // reste ainsi l'ordre du vecteur pour les lecteurs qui ne trient pas.
        preamble.sort_by_key(|event| (event.tick, event.order));
        target.interpreted.events = preamble;
        target.interpreted.end = target.interpreted.end.max(end);
    }
    rest
}

/// Fusionne les branches de passages polyphoniques temporaires.
///
/// Une branche de `<< … \\ … >>` (voix anonyme marquée `temporary`) ne dure
/// que le temps du passage. Quand une branche ultérieure ne chevauche pas une
/// branche déjà retenue, elle reprend la même piste : la portée garde deux
/// voix (la voix principale et une voix temporaire) quel que soit le nombre
/// de passages, au lieu d'en ouvrir une par passage et de dépasser les quatre
/// voix du modèle.
fn merge_temporary_voices(kept: Vec<PendingVoice>) -> Vec<PendingVoice> {
    let mut merged: Vec<PendingVoice> = Vec::new();
    for pending in kept {
        let mergeable = pending.temporary && pending.name.is_none() && !pending.separate_staff;
        if mergeable {
            if let Some(target) = merged.iter_mut().find(|target| {
                target.temporary
                    && target.name.is_none()
                    && !target.separate_staff
                    && !voice_ranges_overlap(&target.interpreted, &pending.interpreted)
            }) {
                merge_voice_into(target, pending);
                continue;
            }
        }
        merged.push(pending);
    }
    merged
}

/// Plage de ticks occupée par une voix, d'après ses événements.
fn voice_range(voice: &InterpretedVoice) -> Option<(Fraction, Fraction)> {
    let first = voice.events.iter().map(|event| event.tick).min()?;
    Some((first, voice.end))
}

/// Vrai si les plages de deux voix se chevauchent (une plage vide ne
/// chevauche rien).
fn voice_ranges_overlap(left: &InterpretedVoice, right: &InterpretedVoice) -> bool {
    match (voice_range(left), voice_range(right)) {
        (Some((left_start, left_end)), Some((right_start, right_end))) => {
            left_start < right_end && right_start < left_end
        }
        _ => false,
    }
}

/// Ajoute les événements d'une voix temporaire à une autre, en décalant les
/// parents de n-olets et en renumérotant l'ordre d'émission.
fn merge_voice_into(target: &mut PendingVoice, source: PendingVoice) {
    let base = target.interpreted.tuplets.len();
    for mut tuplet in source.interpreted.tuplets {
        if let Some(parent) = tuplet.parent {
            tuplet.parent = Some(parent + base);
        }
        target.interpreted.tuplets.push(tuplet);
    }
    target.interpreted.events.extend(source.interpreted.events);
    for (order, event) in target.interpreted.events.iter_mut().enumerate() {
        event.order = order;
    }
    target
        .interpreted
        .repeats
        .extend(source.interpreted.repeats);
    target.interpreted.end = target.interpreted.end.max(source.interpreted.end);
    target.lyrics.extend(source.lyrics);
}

/// Paroles d'un bloc.
fn lyrics_build(block: &LyricsBlock) -> LyricsBuild {
    LyricsBuild {
        target: block.target.clone(),
        verse: block.verse,
        syllables: block.syllables.clone(),
    }
}

/// Voix et paroles effectivement placées sur une portée construite.
struct BuiltVoices {
    staff: StaffId,
    audio_staff: StaffId,
    voices: Vec<VoiceBuild>,
    lyrics: Vec<LyricsBuild>,
}

/// Répartit des voix sur les quatre emplacements d'une portée.
///
/// Un contexte Voice meurt à la fin de sa musique (`lily/context.cc`) : un
/// emplacement libéré est réutilisé par la voix suivante, même si elle porte un
/// autre nom. Seule une voix réellement simultanée avec quatre autres passe sur
/// une portée sœur, avec un avertissement, au lieu d'être perdue.
fn assign_voice_slots(
    score: &mut Score,
    staff: StaffId,
    pending: Vec<PendingVoice>,
    mut ends: Vec<Option<Fraction>>,
) -> (Vec<VoiceBuild>, Vec<LyricsBuild>, Vec<PendingVoice>) {
    // Fin du dernier événement occupant chaque emplacement, ou `None` si libre.
    ends.resize(VOICES, None);
    let mut voices = Vec::new();
    let mut lyrics = Vec::new();
    let mut overflow = Vec::new();
    for item in pending {
        let PendingVoice {
            name,
            lyrics: voice_lyrics,
            temporary,
            separate_staff: _,
            interpreted,
        } = item;
        let start = interpreted
            .events
            .iter()
            .map(|event| event.tick)
            .min()
            .unwrap_or(Fraction::ZERO);
        let end = interpreted.end;
        let Some(slot) = ends
            .iter()
            .position(|occupied| occupied.is_none_or(|until| until <= start))
        else {
            overflow.push(PendingVoice {
                name,
                lyrics: voice_lyrics,
                temporary,
                separate_staff: false,
                interpreted,
            });
            continue;
        };
        ends[slot] = Some(end);
        lyrics.extend(voice_lyrics.iter().map(lyrics_build));
        let mut tuplets = Vec::new();
        for tuplet in &interpreted.tuplets {
            let mut value = Tuplet::new(tuplet.ratio, tuplet.base_len, tuplet.tick, tuplet.len);
            value.actual_notes = tuplet.actual_notes;
            value.normal_notes = tuplet.normal_notes;
            value.number_type = tuplet.number_type;
            value.bracket = !(tuplet.base_len < Fraction::new(1, 8)
                || (tuplet.base_len == Fraction::new(1, 8) && tuplet.ratio != Fraction::ONE));
            if let Some(parent) = tuplet.parent {
                value.parent = tuplets.get(parent).copied();
            }
            tuplets.push(score.add_tuplet(value));
        }
        voices.push(VoiceBuild {
            name,
            track: Track::new(staff, u8::try_from(slot).unwrap_or(0)),
            interpreted,
            tuplets,
        });
    }
    (voices, lyrics, overflow)
}

/// Construit les voix d'une portée et leurs n-olets.
///
/// Une voix peut en ouvrir d'autres (`\new Voice` dans sa musique, branches
/// d'un `<< … >>`) : elles sont interprétées avec leur tick de départ et
/// ajoutées à la portée, dans l'ordre d'ouverture. Une voix sans aucun
/// événement (simple conteneur d'un `\new Voice`, par exemple) est ignorée
/// pour ne pas décaler les pistes du modèle.
#[allow(clippy::too_many_arguments)]
fn build_voices(
    score: &mut Score,
    staff: StaffId,
    part: PartId,
    staff_block: &StaffBlock,
    warnings: &mut Warnings,
    next_voice_context: &mut u32,
    voice_contexts: &mut Vec<u32>,
    octave: OctaveState,
    occupied: &[Option<Fraction>],
    start: Fraction,
) -> (Vec<BuiltVoices>, OctaveState) {
    let mut queue: VecDeque<PendingVoice> = VecDeque::new();
    // `lily/music-sequence.cc:143-158` : les voix d'un `<< … >>` s'enchaînent
    // pour la conversion relative, chacune partant de la dernière hauteur de
    // la précédente. L'état est mémorisé par l'interpréteur avant que le
    // la précédente. L état est mémorisé avant restauration.
    let mut octave = octave;
    for block in &staff_block.voices {
        // Seul un `\relative` distribué sur un `<< … >>` hérite de la voix
        // précédente ; une portée sans ce wrapper reste en octave absolue.
        let seme = match &block.music {
            Music::Context(crate::ast::Context::Relative {
                chain: true,
                reference: Some(_),
                music,
                ..
            }) if matches!(
                music.as_ref(),
                Music::New(group) if group.kind == crate::ast::GroupKind::Voice
            ) =>
            {
                OctaveState {
                    old_relative: octave.old_relative,
                    ..OctaveState::default()
                }
            }
            Music::Context(crate::ast::Context::Relative { chain: true, .. }) => octave,
            _ => OctaveState {
                old_relative: octave.old_relative,
                ..OctaveState::default()
            },
        };
        let (interpreted, octave_suivante) = Interpreter::run(
            &block.music,
            warnings,
            next_voice_context,
            voice_contexts,
            seme,
            start,
            block.name.as_deref(),
        );
        octave = octave_suivante;
        queue.push_back(PendingVoice {
            name: block.name.clone(),
            lyrics: Vec::new(),
            temporary: false,
            separate_staff: false,
            interpreted,
        });
    }
    let mut kept: Vec<PendingVoice> = Vec::new();
    while let Some(pending) = queue.pop_front() {
        let PendingVoice {
            name,
            lyrics,
            temporary,
            separate_staff,
            interpreted,
        } = pending;
        let InterpretedVoice {
            events,
            tuplets,
            repeats,
            end,
            spawned,
        } = interpreted;
        for voice in spawned {
            queue.push_back(PendingVoice {
                name: voice.name,
                lyrics: voice.lyrics,
                temporary: voice.temporary,
                separate_staff: voice.separate_staff,
                interpreted: voice.interpreted,
            });
        }
        if events.is_empty() {
            continue;
        }
        kept.push(PendingVoice {
            name,
            lyrics,
            temporary,
            separate_staff,
            interpreted: InterpretedVoice {
                events,
                tuplets,
                repeats,
                end,
                spawned: Vec::new(),
            },
        });
    }

    // Un `\midi` qui déplace `Staff_performer` vers `Voice` donne à chaque
    // voix son propre `Audio_staff` (`bwv869b.ly`) : ses unissons ne doivent
    // donc pas être fusionnés avec ceux des voix sœurs de la portée.
    if staff_block.midi_voices_as_staves
        && kept
            .iter()
            .filter(|pending| has_chord_events(&pending.interpreted))
            .count()
            > 1
    {
        for pending in &mut kept {
            pending.separate_staff = true;
        }
    }

    // Un `\new Staff` imbriqué n'est pas une voix de la portée : le séparer
    // avant toute fusion de voix temporaires ou d'unissons, sinon ses notes
    // seraient mélangées à celles de la portée d'origine.
    let (separate, kept): (Vec<_>, Vec<_>) =
        kept.into_iter().partition(|pending| pending.separate_staff);

    // Les branches d'un passage polyphonique temporaire (`<< … \\ … >>`)
    // reprennent la même piste d'une occurrence à l'autre : deux passages
    // successifs n'ouvrent pas deux voix de plus sur la portée (MuseScore
    // n'en connaît que quatre). La fusion n'a lieu qu'entre branches
    // anonymes dont les plages de ticks ne se chevauchent pas.
    let kept = merge_temporary_voices(kept);

    // Une voix sans accord (préambule de portée : `\clef`, `\key`,
    // `\cadenzaOn`…) n'occupe pas de piste : ses événements sont repliés en
    // tête de la première voix qui porte de la musique, les clefs et armures
    // appartenant à la portée et non à une voix.
    let mut kept = match kept
        .iter()
        .position(|pending| has_chord_events(&pending.interpreted))
    {
        Some(target) => merge_preamble(kept, target),
        None => kept,
    };

    if staff_block.merge_unisons {
        fusionner_unissons(&mut kept);
    }

    let (voices, lyrics, mut overflow) = assign_voice_slots(score, staff, kept, occupied.to_vec());
    let mut built = vec![BuiltVoices {
        staff,
        audio_staff: staff,
        voices,
        lyrics,
    }];
    // Plus de quatre voix *simultanées* : le modèle ne peut pas les contenir
    // sur une seule portée. Ouvrir une portée sœur conserve toutes les notes ;
    // l'avertissement signale l'écart avec LilyPond.
    let mut garde = 0;
    while !overflow.is_empty() && garde < 1024 {
        garde += 1;
        warnings.push(
            0,
            0,
            "plus de quatre voix simultanées : voix reportée sur une portée sœur".to_owned(),
        );
        let extra_staff = score.add_staff_with(part, Staff::new(part));
        let (voices, lyrics, reste) =
            assign_voice_slots(score, extra_staff, overflow, vec![None; VOICES]);
        if !voices.is_empty() {
            built.push(BuiltVoices {
                staff: extra_staff,
                audio_staff: staff,
                voices,
                lyrics,
            });
        }
        overflow = reste;
    }
    if !overflow.is_empty() {
        warnings.push(
            0,
            0,
            "voix surnuméraires non placées : trop de portées simultanées".to_owned(),
        );
    }
    // `\new Staff` rencontré dans une voix : une portée et un `Audio_staff`
    // propres, donc une piste MIDI distincte dans la référence
    // (`lily/staff-performer.cc:128-153`, `lily/performance.cc:69-110`).
    for pending in separate {
        let extra_staff = score.add_staff_with(part, Staff::new(part));
        let (voices, lyrics, reste) =
            assign_voice_slots(score, extra_staff, vec![pending], vec![None; VOICES]);
        if !voices.is_empty() {
            built.push(BuiltVoices {
                staff: extra_staff,
                audio_staff: extra_staff,
                voices,
                lyrics,
            });
        }
        if !reste.is_empty() {
            warnings.push(
                0,
                0,
                "voix d'une portée imbriquée non placée : trop de voix simultanées".to_owned(),
            );
        }
    }
    (built, octave)
}

/// `midiMergeUnisons = ##t` (`lily/midi-walker.cc:95-115`) : deux notes de
/// même hauteur qui se chevauchent dans une même piste ne donnent qu'une
/// attaque. La note la plus tardive reste dans le modèle (gravure) mais est
/// marquée `unperformed`, comme les musiques sans performer MIDI.
fn fusionner_unissons(voix: &mut [PendingVoice]) {
    let mut notes: Vec<(usize, usize, usize, Fraction, Fraction, u8)> = Vec::new();
    for (v, pending) in voix.iter().enumerate() {
        for (e, event) in pending.interpreted.events.iter().enumerate() {
            let EventKind::Chord(chord) = &event.kind else {
                continue;
            };
            let ChordContent::Notes(content) = &chord.content else {
                continue;
            };
            for (n, note) in content.iter().enumerate() {
                notes.push((v, e, n, event.tick, event.tick + chord.actual, note.pitch));
            }
        }
    }
    notes.sort_by_key(|(v, e, n, debut, _, _)| (*debut, *v, *e, *n));
    let mut jouees: Vec<(Fraction, Fraction, u8)> = Vec::new();
    for (v, e, n, debut, fin, pitch) in notes {
        if jouees
            .iter()
            .any(|(d, f, p)| *p == pitch && *d <= debut && debut < *f)
        {
            if let EventKind::Chord(chord) = &mut voix[v].interpreted.events[e].kind {
                if let ChordContent::Notes(content) = &mut chord.content {
                    content[n].unperformed = true;
                }
            }
        } else {
            jouees.push((debut, fin, pitch));
        }
    }
}

/// Accolade d'un groupe de portées.
fn group_bracket(kind: GroupKind, position: usize, count: usize) -> Option<StaffBracket> {
    if position != 0 || count < 2 {
        return None;
    }
    let kind = match kind {
        GroupKind::PianoStaff => 1,
        GroupKind::ChoirStaff => 0,
        _ => return None,
    };
    Some(StaffBracket {
        kind,
        span: count,
        visible: true,
    })
}

/// Instrument approché d'après son nom.
///
/// LilyPond ne porte ni identifiant MusicXML ni programme MIDI dans le
/// sous-ensemble lu : les noms d'instrument sont le seul indice. La table
/// couvre les instruments courants du corpus ; à défaut, l'instrument garde
/// son nom et aucun programme.
/// Vrai si une musique AST contient au moins un accord joué (pas seulement
/// des propriétés de contexte comme `\transposition`).
/// Vrai si la musique porte une **hauteur** (un accord non vide), par
/// opposition à un `\skip` ou un silence : un préambule de silences ne crée
/// pas de portée et ne ferme donc pas une transposition en attente.
fn music_has_pitched(music: &crate::ast::Music) -> bool {
    use crate::ast::Music;
    match music {
        Music::Chord(chord) => {
            matches!(&chord.kind, crate::ast::ChordKind::Notes(notes) if !notes.is_empty())
                || matches!(&chord.kind, crate::ast::ChordKind::Repeat)
        }
        Music::Seq(items) | Music::Sim(items) => items.iter().any(music_has_pitched),
        Music::Context(crate::ast::Context::Transpose { music, .. })
        | Music::Context(crate::ast::Context::Harmonic { music, .. })
        | Music::Context(crate::ast::Context::Fixed { music, .. })
        | Music::Context(crate::ast::Context::Relative { music, .. })
        | Music::Context(crate::ast::Context::NonRelative { music })
        | Music::Context(crate::ast::Context::Tuplet { music, .. })
        | Music::Context(crate::ast::Context::Grace { music, .. })
        | Music::Context(crate::ast::Context::Repeat { body: music, .. })
        | Music::Context(crate::ast::Context::UnfoldRepeats { music }) => music_has_pitched(music),
        Music::New(group) => group.staves.iter().any(|staff| {
            staff
                .voices
                .iter()
                .any(|voice| music_has_pitched(&voice.music))
        }),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => music_has_pitched(music),
        _ => false,
    }
}

fn music_has_chords(music: &crate::ast::Music) -> bool {
    use crate::ast::Music;
    match music {
        Music::Chord(_) => true,
        Music::Seq(items) | Music::Sim(items) => items.iter().any(music_has_chords),
        Music::Context(crate::ast::Context::Transpose { music, .. })
        | Music::Context(crate::ast::Context::Harmonic { music, .. })
        | Music::Context(crate::ast::Context::Fixed { music, .. })
        | Music::Context(crate::ast::Context::Relative { music, .. })
        | Music::Context(crate::ast::Context::NonRelative { music })
        | Music::Context(crate::ast::Context::Tuplet { music, .. })
        | Music::Context(crate::ast::Context::Grace { music, .. })
        | Music::Context(crate::ast::Context::Repeat { body: music, .. })
        | Music::Context(crate::ast::Context::UnfoldRepeats { music }) => music_has_chords(music),
        Music::New(group) => group.staves.iter().any(|staff| {
            staff
                .voices
                .iter()
                .any(|voice| music_has_chords(&voice.music))
        }),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => music_has_chords(music),
        _ => false,
    }
}

/// Cherche un `\transposition` dans une musique de voix.
///
/// La propriété est posée par une commande sans musique, souvent au début
/// d'une variable `global` partagée : la recherche descend donc dans les
/// séquences, simultanéités, contextes et groupes imbriqués. Le booléen dit
/// si le décalage doit encore être appliqué aux notes : faux pour le
/// `\transposition` du bloc `\midi`, déjà porté par toutes les voix.
fn instrument_transpose(music: &Music) -> Option<(PitchSpec, bool)> {
    match music {
        Music::Context(Context::InstrumentTranspose(pitch)) => Some((*pitch, true)),
        Music::Context(Context::MidiInstrumentTranspose(pitch)) => Some((*pitch, false)),
        Music::Seq(items) | Music::Sim(items) => items.iter().find_map(instrument_transpose),
        Music::Context(Context::Transpose { music, .. })
        | Music::Context(Context::Harmonic { music, .. })
        | Music::Context(Context::Fixed { music, .. })
        | Music::Context(Context::Relative { music, .. })
        | Music::Context(Context::NonRelative { music })
        | Music::Context(Context::Tuplet { music, .. })
        | Music::Context(Context::Grace { music, .. })
        | Music::Context(Context::Repeat { body: music, .. })
        | Music::Context(Context::UnfoldRepeats { music }) => instrument_transpose(music),
        Music::Context(Context::AfterGrace { main, .. }) => instrument_transpose(main),
        Music::New(group) => group
            .staves
            .iter()
            .flat_map(|staff| staff.voices.iter())
            .find_map(|voice| instrument_transpose(&voice.music)),
        _ => None,
    }
}

/// Premier tick où `\transposition` est posé dans un ensemble de portées.
fn transposition_depuis(staves: &[StaffBuild]) -> Fraction {
    staves
        .iter()
        .flat_map(|staff| staff.voices.iter())
        .flat_map(|voice| voice.interpreted.events.iter())
        .find_map(|event| match &event.kind {
            EventKind::Meta(MetaEvent::Transposition(_)) => Some(event.tick),
            _ => None,
        })
        .unwrap_or(Fraction::ZERO)
}

/// Décale d'un intervalle chromatique les hauteurs d'une portée transpositrice.
///
/// `\transposition` ne change pas la notation : la hauteur écrite reste celle
/// de la source, c'est la hauteur concert qui est décalée. Le modèle stocke la
/// concert et l'instrument porte l'intervalle, pour que l'export réécrive la
/// hauteur écrite en retranchant `<transpose>`. Appelé seulement quand la
/// partie porte effectivement `<transpose>`, sinon la hauteur écrite doit
/// rester intacte (cas des orchestres, où plusieurs portées de transpositions
/// différentes partagent la même partie MusicXML).
fn decaler_vers_concert(staves: &mut [StaffBuild], changes: &[(Fraction, Transpose)]) {
    for staff in staves {
        for voice in &mut staff.voices {
            for event in &mut voice.interpreted.events {
                // `\transposition` est une propriété de contexte : elle peut
                // changer en cours de partition (clarinette en la puis en si
                // bémol). Chaque accord prend le dernier intervalle posé avant
                // lui (Notation Reference §1.3.4).
                let interval = changes
                    .iter()
                    .take_while(|(tick, _)| *tick <= event.tick)
                    .last()
                    .map(|(_, interval)| *interval);
                let Some(interval) = interval else {
                    continue;
                };
                if let EventKind::Chord(chord) = &mut event.kind {
                    decaler_accord(&mut chord.content, interval, true);
                    for grace in chord
                        .grace_before
                        .iter_mut()
                        .chain(chord.grace_after.iter_mut())
                    {
                        decaler_notes(&mut grace.notes, interval.chromatic);
                    }
                }
            }
        }
    }
}

/// Tous les changements de `\transposition` d'un ensemble de portées, triés.
fn transpositions(staves: &[StaffBuild]) -> Vec<(Fraction, Transpose)> {
    let mut changes: Vec<_> = staves
        .iter()
        .flat_map(|staff| staff.voices.iter())
        .flat_map(|voice| voice.interpreted.events.iter())
        .filter_map(|event| match &event.kind {
            EventKind::Meta(MetaEvent::Transposition(pitch)) => {
                Some((event.tick, interval_from_pitch(pitch)))
            }
            _ => None,
        })
        .collect();
    changes.sort_by_key(|(tick, _)| *tick);
    changes.dedup_by_key(|(tick, _)| *tick);
    changes
}

/// Intervalle diatonique/chromatique d'une hauteur de `\transposition`.
fn interval_from_pitch(pitch: &crate::ast::PitchSpec) -> Transpose {
    let (diatonic, chromatic) = crate::pitch::instrument_transpose(pitch);
    Transpose::new(diatonic, chromatic)
}

/// Décale les hauteurs d'un accord (les silences n'ont pas de hauteur) et
/// transpose son orthographe si le modèle doit rester concert.
fn decaler_accord(content: &mut ChordContent, interval: Transpose, concert_tpc: bool) {
    if let ChordContent::Notes(notes) = content {
        let tpc_avant: Vec<i8> = notes.iter().map(|note| note.tpc).collect();
        decaler_notes(notes, interval.chromatic);
        if concert_tpc {
            for (note, tpc) in notes.iter_mut().zip(tpc_avant) {
                note.tpc = scoretk_core::model::transpose_tpc(tpc, interval);
            }
        }
    }
}

/// Décale des notes dans la plage MIDI, en écartant celles qui en sortent.
fn decaler_notes(notes: &mut Vec<crate::events::NoteEvent>, chromatic: i32) {
    notes.retain_mut(|note| {
        let concert = i32::from(note.pitch) + chromatic;
        match u8::try_from(concert) {
            Ok(pitch) if pitch <= 127 => {
                note.pitch = pitch;
                true
            }
            _ => false,
        }
    });
}

/// `midiExpression` et `midiPanPosition` sont des fractions de la plage MIDI
/// (Notation Reference §24.8) : le modèle stocke les contrôleurs 7 et 10.
fn apply_midi_levels(instrument: &mut Instrument, expression: Option<f64>, pan: Option<f64>) {
    let to_controller = |fraction: f64| (fraction * 127.0).round().clamp(0.0, 127.0) as u8;
    if let Some(expression) = expression {
        instrument.volume = Some(to_controller(expression));
    }
    if let Some(pan) = pan {
        instrument.pan = Some(to_controller(pan));
    }
}

fn instrument_for(long_name: &str, short_name: &str, midi_instrument: Option<&str>) -> Instrument {
    let mut instrument = Instrument::new(long_name, long_name, short_name);
    instrument.track_name = long_name.to_owned();
    let mut name = long_name.to_lowercase();
    if let Some(midi) = midi_instrument {
        name.push(' ');
        name.push_str(&midi.to_lowercase());
    }
    // (motif, identifiant MusicXML, programme MIDI base zéro, nom de piste).
    let table: [(&str, &str, u8, &str); 31] = [
        ("piccolo", "wind.flutes.piccolo", 72, "Piccolo"),
        ("flute", "wind.flutes.flute", 73, "Flute"),
        ("oboe", "wind.reed.oboe", 68, "Oboe"),
        ("english horn", "wind.reed.english-horn", 69, "English Horn"),
        ("bassoon", "wind.reed.bassoon", 70, "Bassoon"),
        ("clarinet", "wind.reed.clarinet.bflat", 71, "Clarinet"),
        ("sax", "wind.reed.saxophone", 65, "Saxophone"),
        ("horn", "brass.french-horn", 60, "Horn"),
        ("trumpet", "brass.trumpet", 56, "Trumpet"),
        ("trombone", "brass.trombone", 57, "Trombone"),
        ("tuba", "brass.tuba", 58, "Tuba"),
        ("timpani", "percussion.timpani", 47, "Timpani"),
        ("violin", "strings.violin", 40, "Violin"),
        ("viola", "strings.viola", 41, "Viola"),
        ("violoncello", "strings.cello", 42, "Violoncello"),
        ("cello", "strings.cello", 42, "Violoncello"),
        ("contrabass", "strings.contrabass", 43, "Contrabass"),
        ("double bass", "strings.contrabass", 43, "Contrabass"),
        ("guitar", "pluck.guitar", 24, "Guitar"),
        ("harp", "pluck.harp", 46, "Harp"),
        ("harpsichord", "keyboard.harpsichord", 6, "Harpsichord"),
        ("organ", "keyboard.organ", 19, "Organ"),
        ("piano", "keyboard.piano", 0, "Piano"),
        ("voice", "voice.vocals", 52, "Voice"),
        ("choir", "voice.vocals", 52, "Choir"),
        ("soprano", "voice.vocals", 52, "Soprano"),
        ("alto", "voice.vocals", 52, "Alto"),
        ("tenor", "voice.vocals", 52, "Tenor"),
        ("bass", "voice.vocals", 52, "Bass"),
        ("drum", "percussion.drumset", 0, "Drumset"),
        ("", "", 0, ""),
    ];
    if let Some((_, id, program, track_name)) = table
        .iter()
        .take(table.len() - 1)
        .find(|(needle, _, _, _)| name.contains(needle))
    {
        instrument.id = (*id).to_owned();
        instrument.midi_program = Some(*program);
        instrument.track_name = (*track_name).to_owned();
    }
    // `scm/midi.scm::midi-program` : la chaîne de `midiInstrument` est la
    // source du programme, même quand le nom gravé suggère un autre instrument.
    if let Some(entry) = midi_instrument.and_then(midi_instruments::lookup) {
        instrument.midi_program = Some(entry.program);
        // `scm/midi.scm::percussion?` : les kits utilisent le canal 9.
        if entry.percussion {
            instrument.midi_channel = Some(9);
        }
    }
    instrument
}

/// Métadonnées et crédits de `\header`.
fn write_metadata(score: &mut Score, document: &Document) {
    let header = &document.header;
    let field = |name: &str| header.get(name).map(str::trim).unwrap_or_default();
    // `title` est le champ `\header` standard du titre de l'œuvre
    // (Notation Reference §21.1.2). `workTitle` reste prioritaire quand il est
    // présent, mais un fichier LilyPond écrit par un autre outil n'a que
    // `title` : le modèle doit tout de même retrouver le titre.
    let fallback = |standard: &str, private: &str| {
        let private = field(private);
        if private.is_empty() {
            field(standard)
        } else {
            private
        }
    };
    let metadata = ScoreMetadata {
        // `\header title` en markup gras est le signal standard d'un titre
        // d'œuvre (Notation Reference §8.3) ; un titre simple est un crédit.
        work_title: if document
            .header
            .bold_fields
            .iter()
            .any(|name| name == "title")
        {
            field("title").to_owned()
        } else {
            field("workTitle").to_owned()
        },
        movement_title: field("movementTitle").to_owned(),
        movement_number: field("movementNumber").to_owned(),
        // `opus` est le champ standard pour un numéro d'œuvre (§21.1.2).
        work_number: fallback("opus", "workNumber").to_owned(),
        composer: field("composer").to_owned(),
        lyricist: field("poet").to_owned(),
        arranger: field("arranger").to_owned(),
        copyright: field("copyright").to_owned(),
        source: field("source").to_owned(),
        subtitle: field("subtitle").to_owned(),
        ..ScoreMetadata::default()
    };
    score.metadata = metadata;

    // Des champs `\header` privés portent les crédits et les textes de cadre
    // du modèle quand ils ne se réduisent pas aux champs de titre.
    let rank = |name: &str| {
        name.chars()
            .next()
            .and_then(|letter| (letter as u8).checked_sub(b'A'))
            .map(usize::from)
    };
    let private_credits: Vec<(usize, String)> = header
        .entries
        .iter()
        .filter_map(|(name, value)| {
            let index = rank(name.strip_prefix("scoretkCredit")?)?;
            Some((index, value.trim().to_owned()))
        })
        .collect();
    let private_tbox: Vec<(usize, usize, String)> = header
        .entries
        .iter()
        .filter_map(|(name, value)| {
            let index = rank(name.strip_prefix("scoretkTbox")?)?;
            let (measure, text) = value.trim().split_once('|')?;
            Some((index, measure.parse::<usize>().ok()?, text.to_owned()))
        })
        .collect();
    if !private_credits.is_empty() || !private_tbox.is_empty() {
        score.engraving.credits.clear();
        let mut credits = private_credits;
        credits.sort_by_key(|(index, _)| *index);
        for (_, text) in credits {
            score.add_credit(Credit::new(RichText::new(text), CreditStyle::Title));
        }
        let mut tbox_texts = private_tbox;
        tbox_texts.sort_by_key(|(index, _, _)| *index);
        for (_, measure, text) in tbox_texts {
            if !text.is_empty() {
                score.add_tbox_text(TboxText::new(RichText::new(text), measure));
            }
        }
        return;
    }

    // Champs `\header` standards réservés aux crédits supplémentaires
    // (Notation Reference §21.1.2) : quand ils sont présents, ils portent la
    // liste complète des crédits dans l'ordre, exactement comme le tableau
    // partagé par l'écrivain. Sinon, les crédits reprennent les champs de
    // titre, comme MuseScore pour les cadres de titre.
    let standard_credits: Vec<String> = scoretk_lilypond_common::headers::CREDIT_HEADER_FIELDS
        .iter()
        .filter_map(|name| {
            let value = field(name);
            (!value.is_empty()).then(|| value.to_owned())
        })
        .collect();
    if !standard_credits.is_empty() {
        for text in standard_credits {
            score.add_credit(Credit::new(RichText::new(text), CreditStyle::Title));
        }
        return;
    }
    let credits = [
        (CreditStyle::Title, field("title")),
        (CreditStyle::Subtitle, field("subtitle")),
        (CreditStyle::Composer, field("composer")),
        (CreditStyle::Lyricist, field("poet")),
    ];
    for (style, text) in credits {
        if text.is_empty() {
            continue;
        }
        score.add_credit(Credit::new(RichText::new(text), style));
    }
}

/// Clefs d'en-tête : la première clef de chaque portée devient sa clef par
/// défaut.
fn apply_initial_clefs(score: &mut Score, staves: &[StaffBuild]) {
    for build in staves {
        let clef = build.voices.iter().find_map(|voice| {
            voice.interpreted.events.iter().find_map(|event| {
                if event.tick.is_zero() {
                    if let EventKind::Meta(MetaEvent::Clef(clef)) = event.kind {
                        return Some(clef);
                    }
                }
                None
            })
        });
        if let Some(clef) = clef {
            score.staff_mut(build.staff).default_clef = Clef::new(clef);
        }
    }
}

// ----------------------------------------------------------------------
// Trame des mesures
// ----------------------------------------------------------------------

/// Indications de mesure d'un tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MeterAt {
    nominal: Option<Fraction>,
    length: Option<Fraction>,
    partial: Option<Fraction>,
}

/// Intervalles de cadence de toutes les voix.
///
/// Toutes les voix partageant les mesures, une cadence dans une seule voix
/// suffit à suspendre les barres automatiques pour la partition entière
/// pendant cet intervalle (voir § 7 de `docs/agents/lilypond.md`).
fn cadenza_spans(staves: &[StaffBuild], end_tick: Fraction) -> Vec<CadenzaSpan> {
    let mut spans = Vec::new();
    for staff in staves {
        for voice in &staff.voices {
            let mut events: Vec<&VoiceEvent> = voice.interpreted.events.iter().collect();
            events.sort_by_key(|event| (event.tick, event.order));
            let mut open: Option<Fraction> = None;
            for event in events {
                match &event.kind {
                    EventKind::Meta(MetaEvent::Cadenza(true)) => {
                        if open.is_none() {
                            open = Some(event.tick);
                        }
                    }
                    EventKind::Meta(MetaEvent::Cadenza(false)) => {
                        if let Some(start) = open.take() {
                            if event.tick > start {
                                spans.push(CadenzaSpan {
                                    start,
                                    end: event.tick,
                                });
                            }
                        }
                    }
                    // `\bar "…"` pendant la cadence : la mesure se ferme là,
                    // mais la cadence continue ensuite (le contrôle de mesure
                    // `|` ne produit aucun événement et ne coupe rien).
                    EventKind::Meta(MetaEvent::Bar(_)) => {
                        if let Some(start) = open {
                            if event.tick > start {
                                spans.push(CadenzaSpan {
                                    start,
                                    end: event.tick,
                                });
                                open = Some(event.tick);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(start) = open {
                if end_tick > start {
                    spans.push(CadenzaSpan {
                        start,
                        end: end_tick,
                    });
                }
            }
        }
    }
    spans
}

/// Construit la trame des mesures.
fn plan_measures(
    staves: &[StaffBuild],
    staff_local_timing: bool,
    warnings: &mut Warnings,
) -> Vec<MeasureSlot> {
    if staff_local_timing {
        return plan_measures_polymetric(staves, warnings);
    }
    plan_measures_global(staves, warnings)
}

/// Trame quand `Timing_translator` a été déplacé dans chaque `Staff` : chaque
/// portée a ses propres barres, donc sa propre mesure. Le modèle n'a qu'une
/// trame globale : on prend l'union des frontières de toutes les portées, en
/// conservant l'indication `\time` portée par chaque frontière.
fn plan_measures_polymetric(staves: &[StaffBuild], warnings: &mut Warnings) -> Vec<MeasureSlot> {
    let mut bornes: BTreeSet<Fraction> = BTreeSet::new();
    bornes.insert(Fraction::ZERO);
    let mut nominales: BTreeMap<Fraction, Fraction> = BTreeMap::new();
    for staff in staves {
        let seuls = std::slice::from_ref(staff);
        for slot in plan_measures_global(seuls, warnings) {
            bornes.insert(slot.start);
            bornes.insert(slot.start + slot.len);
            nominales.entry(slot.start).or_insert(slot.nominal);
        }
    }
    let fin = bornes.iter().copied().max().unwrap_or(Fraction::ZERO);
    let debuts: Vec<Fraction> = bornes.into_iter().filter(|tick| *tick < fin).collect();
    let mut slots = Vec::new();
    let mut nominal = Fraction::ONE;
    for (index, start) in debuts.iter().enumerate() {
        if let Some(value) = nominales.get(start) {
            nominal = *value;
        }
        let next = debuts.get(index + 1).copied().unwrap_or(fin);
        slots.push(MeasureSlot {
            start: *start,
            len: next - *start,
            nominal,
            multi_rest: None,
        });
    }
    slots
}

fn plan_measures_global(staves: &[StaffBuild], warnings: &mut Warnings) -> Vec<MeasureSlot> {
    let mut meter: BTreeMap<Fraction, MeterAt> = BTreeMap::new();
    // `\set Timing.measurePosition` par tick (dernier écrit gagne).
    let mut measure_position: BTreeMap<Fraction, Fraction> = BTreeMap::new();
    for (staff_index, staff) in staves.iter().enumerate() {
        for voice in &staff.voices {
            for event in &voice.interpreted.events {
                let EventKind::Meta(meta) = &event.kind else {
                    continue;
                };
                let entry = meter.entry(event.tick).or_default();
                match meta {
                    MetaEvent::Time {
                        numerator,
                        denominator,
                    } => {
                        if staff_index == 0 || entry.nominal.is_none() {
                            entry.nominal = Some(Fraction::new(
                                i64::from(*numerator),
                                i64::from(*denominator),
                            ));
                        }
                    }
                    MetaEvent::MeasureLength(length) => {
                        if staff_index == 0 || entry.length.is_none() {
                            entry.length = Some(*length);
                        }
                    }
                    MetaEvent::Partial(length) => {
                        entry.partial = Some(*length);
                    }
                    MetaEvent::MeasurePosition(position) => {
                        measure_position
                            .entry(event.tick)
                            .and_modify(|value| *value = *position)
                            .or_insert(*position);
                    }
                    _ => {}
                }
            }
        }
    }

    let end_tick = staves
        .iter()
        .flat_map(|staff| staff.voices.iter())
        .map(|voice| voice.interpreted.end)
        .max()
        .unwrap_or(Fraction::ZERO);
    let spans = cadenza_spans(staves, end_tick);
    let mut slots = Vec::new();
    let mut start = Fraction::ZERO;
    let mut nominal = Fraction::ONE;
    // `measureLength` est une propriété de contexte (`lily/timing-translator.cc`
    // l. 170) : elle reste en vigueur jusqu'au prochain `\time` ou `\set`.
    // La conserver ici évite de couper une note qui dépasse la mesure
    // suivante quand le `\set` n'est posé qu'à la première mesure concernée.
    let mut measure_length: Option<Fraction> = None;

    while start < end_tick && slots.len() < 100_000 {
        if let Some(at) = meter.get(&start) {
            if let Some(value) = at.nominal {
                nominal = value;
                measure_length = None;
            }
            if let Some(value) = at.length {
                measure_length = Some(value);
            }
        }
        let at = meter.get(&start).copied().unwrap_or_default();
        let mut len = measure_length.or(at.partial).unwrap_or(nominal);
        if !len.is_positive() {
            warnings.push(0, 0, "mesure de longueur nulle ignorée".to_owned());
            break;
        }
        // Une indication de mesure postérieure coupe la mesure.
        if let Some((tick, _)) = meter
            .range(start..)
            .find(|(tick, value)| **tick > start && value.nominal.is_some())
        {
            if *tick < start + len {
                len = *tick - start;
            }
        }
        // `\set Timing.measurePosition = #n/d` : LilyPond remplace la position
        // courante dans la mesure (`lily/timing-translator.cc` l. 470-505).
        // Une position négative `-d` fait tomber la barre de retour à zéro à
        // `t + d` (anacrouse avant une reprise) ; une position positive `p`
        // ferme la mesure courante à `t + len - p`.
        if let Some((tick, position)) = measure_position
            .range(start..)
            .find(|(tick, _)| **tick < start + len)
        {
            let end = if *position >= Fraction::ZERO {
                *tick + len - *position
            } else {
                *tick - *position
            };
            if end > *tick && end != start + len {
                len = end - start;
            }
        }
        // Pendant une cadence, aucune barre automatique : la mesure en cours
        // s'allonge jusqu'à la fermeture de la cadence ou à sa barre
        // explicite la plus proche. La durée nominale reste celle du `\time`,
        // ce qui donne une mesure irrégulière (`len != nominal_len`) comme
        // MuseScore pour une mesure de durée réelle différente de la signature.
        let end = start + len;
        if spans
            .iter()
            .any(|span| span.start < end && start < span.end)
        {
            if let Some(cut) = spans
                .iter()
                .map(|span| span.end)
                .filter(|tick| *tick > start)
                .min()
            {
                len = cut - start;
            }
        }
        slots.push(MeasureSlot {
            start,
            len,
            nominal,
            multi_rest: None,
        });
        start += len;
    }

    merge_multi_rests(&mut slots, staves);
    slots
}

/// Regroupe les mesures couvertes par un silence multi-mesures.
fn merge_multi_rests(slots: &mut Vec<MeasureSlot>, staves: &[StaffBuild]) {
    let mut merged = Vec::new();
    let mut index = 0usize;
    while index < slots.len() {
        let start = slots[index].start;
        let mut count = 1usize;
        if let Some(length) = multi_rest_length(staves, start).filter(|length| length.is_positive())
        {
            let mut covered = Fraction::ZERO;
            let mut candidate = 0usize;
            while index + candidate < slots.len() {
                covered += slots[index + candidate].len;
                candidate += 1;
                if covered == length {
                    count = candidate;
                    break;
                }
                if covered > length {
                    break;
                }
            }
        }
        if count > 1 {
            let mut slot = slots[index].clone();
            let mut total = Fraction::ZERO;
            for offset in 0..count {
                total += slots[index + offset].len;
            }
            slot.len = total;
            slot.multi_rest = Some(u16::try_from(count).unwrap_or(u16::MAX));
            merged.push(slot);
        } else {
            merged.push(slots[index].clone());
        }
        index += count;
    }
    *slots = merged;
}

/// Longueur d'un silence multi-mesures commençant à un tick.
fn multi_rest_length(staves: &[StaffBuild], start: Fraction) -> Option<Fraction> {
    let mut length = None;
    for staff in staves {
        let mut local = None;
        for voice in &staff.voices {
            for event in &voice.interpreted.events {
                if event.tick != start {
                    continue;
                }
                if let EventKind::Chord(chord) = &event.kind {
                    match chord.content {
                        ChordContent::MultiRest { .. } => local = Some(chord.actual),
                        // Une portée joue à ce tick : les mesures ne peuvent
                        // pas être fusionnées pour les autres portées.
                        ChordContent::Notes(_) | ChordContent::Rest { .. } => return None,
                        ChordContent::Skip { .. } => {}
                    }
                }
            }
        }
        if let Some(value) = local {
            if length.is_some_and(|previous| previous != value) {
                return None;
            }
            length = Some(value);
        }
    }
    length
}

/// Le modèle porte la barre de fin sur la mesure **et** sur son segment final,
/// comme MuseScore. Les événements `\bar` ne mettent à jour que la mesure :
/// synchroniser le segment évite de perdre `End`, `Double` et `Dashed` (ABC
/// 2.1 §4.8 / Notation Reference §2.5.1) à l'aller-retour.
fn sync_end_bar_segments(score: &mut Score) {
    let staff_count = score.staves.len();
    for measure in &mut score.measures {
        let kind = measure.end_bar_line;
        let end = measure.end_tick();
        for segment in &mut measure.segments {
            if segment.tick != end || !segment.kind.is_bar_line_type() {
                continue;
            }
            for staff in 0..staff_count {
                let track = Track::new(StaffId::from_index(staff), 0);
                if segment.element(track).is_some() {
                    segment.set_element(track, SegmentItem::BarLine(BarLine::new(kind)));
                }
            }
        }
    }
}

/// Crée les mesures de la trame.
fn create_measures(score: &mut Score, slots: &[MeasureSlot]) {
    for (index, slot) in slots.iter().enumerate() {
        let mut measure = Measure::new(index, slot.start, slot.nominal);
        measure.len = slot.len;
        measure.multi_measure_rest = slot.multi_rest;
        // MuseScore porte une barre sur le segment final de chaque mesure,
        // même pour une barre normale sans symbole dans le `.ly`.
        let end = measure.end_tick();
        let mut segment = Segment::new(SegmentType::EndBarLine, end, score.staves.len().max(1));
        for index in 0..score.staves.len() {
            segment.set_element(
                Track::new(StaffId::from_index(index), 0),
                SegmentItem::BarLine(BarLine::new(BarLineType::Normal)),
            );
        }
        measure.segments.push(segment);
        score.add_measure(measure);
    }
}

/// Indice de la mesure contenant un tick.
fn measure_at(slots: &[MeasureSlot], tick: Fraction) -> Option<usize> {
    slots
        .partition_point(|slot| slot.start <= tick)
        .checked_sub(1)
        .filter(|index| *index < slots.len())
}

/// Segment de la mesure, créé au besoin.
fn ensure_segment(
    score: &mut Score,
    measure_index: usize,
    kind: SegmentType,
    tick: Fraction,
) -> usize {
    let staves = score.staves.len();
    let measure = &mut score.measures[measure_index];
    if let Some(position) = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
    {
        return position;
    }
    measure.add_segment(Segment::new(kind, tick, staves));
    measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == kind)
        .unwrap_or(0)
}

// ----------------------------------------------------------------------
// Placement des événements
// ----------------------------------------------------------------------

/// Unissons d'une note déplacée par `\change Staff` vers une portée qui joue
/// déjà la même hauteur au même instant.
///
/// `lily/midi-walker.cc:95-115` (`Midi_walker::do_start_note`) ne produit
/// qu'une attaque quand deux notes de même hauteur commencent au même tick
/// dans le même `Audio_staff` (`lily/staff-performer.cc:114-143`,
/// `get_audio_staff`) : la condition `now_ticks == queued_ticks` s'applique
/// même quand `midiMergeUnisons` est faux. `\change Staff` déplace en effet
/// la voix dans l'autre contexte `Staff` (`lily/change-iterator.cc`), sans
/// égard au nom des deux portées : une note gravée sur la portée d'origine
/// mais jouée par l'autre portée ne donne qu'un seul `note_on` si cette
/// dernière joue déjà la même hauteur. On garde la note dans le modèle en la
/// marquant `unperformed`, comme les autres musiques sans performer MIDI.
///
/// Une liaison (`tie_back`) n'ouvre pas d'attaque : `lily/tie-performer.cc`
/// ne lui fait pas émettre de `note_on`. Elle ne peut donc pas masquer la
/// note déplacée, et une note déjà partie vers une autre portée ne sonne pas
/// sur celle qu'elle a quittée.
///
/// Une portée sœur créée pour loger une cinquième voix simultanée partage
/// l'`Audio_staff` de la portée d'origine : les voix n'ont pas changé de
/// contexte `Staff` dans LilyPond, seul le modèle les sépare en deux portées.
/// Le même regroupement s'applique donc aux notes des portées sœurs et à
/// celles déplacées par `\change`.
fn fusionne_unissons_entre_portees(score: &mut Score, staves: &[StaffBuild]) {
    let audio_staffs: BTreeMap<StaffId, StaffId> = staves
        .iter()
        .map(|staff| (staff.staff, staff.audio_staff))
        .collect();
    let audio_of = |staff: StaffId| audio_staffs.get(&staff).copied().unwrap_or(staff);

    // (note, portée du modèle, Audio_staff, tick, hauteur, voix déplacée).
    let mut sons: Vec<(NoteId, StaffId, StaffId, Fraction, u8, bool)> = Vec::new();
    for raw in 0..score.staves.len() * VOICES {
        let track = Track::from_raw(raw as u32);
        let origine = audio_of(track.staff());
        let index = staves.iter().position(|s| s.staff == track.staff());
        for measure in &score.measures {
            for segment in &measure.segments {
                let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(track) else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                // Une note déplacée ne sonne plus sur la portée qu'elle a
                // quittée. Une cible introuvable (fin de `\score`, portée
                // supprimée) laisse la note sur sa portée d'origine.
                let (cible, deplacee) = if chord.staff_move == 0 {
                    (origine, false)
                } else {
                    index
                        .and_then(|i| i.checked_add_signed(isize::from(chord.staff_move)))
                        .and_then(|target| staves.get(target))
                        .map(|target| (audio_of(target.staff), true))
                        .unwrap_or((origine, false))
                };
                let tick = measure.start_tick + segment.tick;
                for id in &chord.notes {
                    let note = score.note(*id);
                    if note.playback.unperformed || !note.playback.play || note.is_tie_stop() {
                        continue;
                    }
                    sons.push((*id, track.staff(), cible, tick, note.pitch, deplacee));
                }
            }
        }
    }
    if sons.is_empty() {
        return;
    }
    sons.sort_by_key(|(_, _, audio, tick, pitch, _)| (*audio, *tick, *pitch));
    let mut debut = 0;
    while debut < sons.len() {
        let mut fin = debut + 1;
        while fin < sons.len()
            && sons[fin].2 == sons[debut].2
            && sons[fin].3 == sons[debut].3
            && sons[fin].4 == sons[debut].4
        {
            fin += 1;
        }
        let groupe = &sons[debut..fin];
        // Deux notes de la même portée du modèle sont déjà fusionnées par
        // l'oracle (`ly_notes.py`, unisson par partie/portée). Seuls les
        // doublons entre portées distinctes ou après `\change` sont des
        // artefacts du modèle.
        let portees: BTreeSet<StaffId> = groupe.iter().map(|son| son.1).collect();
        if groupe.len() > 1 && (portees.len() > 1 || groupe.iter().any(|son| son.5)) {
            let gardee = groupe
                .iter()
                .min_by_key(|son| (son.5, son.1 != son.2, son.0))
                .expect("groupe non vide");
            for son in groupe {
                if son.0 != gardee.0 {
                    score.note_mut(son.0).playback.unperformed = true;
                }
            }
        }
        debut = fin;
    }
}

/// Range les événements de toutes les voix dans les mesures.
fn place_measures(
    score: &mut Score,
    staves: &[StaffBuild],
    slots: &[MeasureSlot],
    warnings: &mut Warnings,
) {
    // `Devnull` (et `NullVoice`) jette la musique qu'on lui donne
    // (`ly/engraver-init.ly:1125-1133`). Les notes déplacées par
    // `\change Staff` vers une telle portée ne sonnent pas : elles restent
    // dans le modèle, silencieuses (comme `Context::Silent`).
    let silent_staff_names: BTreeSet<String> = staves
        .iter()
        .filter(|candidate| {
            let staff = score.staff(candidate.staff);
            staff.muted && !staff.visible
        })
        .filter_map(|candidate| candidate.name.clone())
        .collect();
    // `lily/context.cc` `Context::core_find` (`find_context`, FIND_ONLY) :
    // le contexte courant est examiné en premier, puis les contextes du nom
    // dans l'ordre de création. Deux `\new Staff = right` du même `\score`
    // désignent donc le **premier** contexte du nom, même s'il appartient à
    // une autre partie (vérifié avec LilyPond 2.26).
    let mut names: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in staves.iter().enumerate() {
        if let Some(name) = candidate.name.as_deref() {
            names.entry(name).or_default().push(index);
        }
    }
    for (staff_index, staff) in staves.iter().enumerate() {
        for voice_index in 0..staff.voices.len() {
            // Un seul avertissement par nom inconnu et par voix, daté du
            // `\change` qui l'a introduit plutôt que de chaque accord.
            let mut warned: BTreeSet<&str> = BTreeSet::new();
            // `\change` change le parent de la voix : la résolution suivante
            // part de la portée où la musique vient d'être déplacée.
            let mut current = staff_index;
            let mut events: Vec<VoiceEvent> = staff.voices[voice_index].interpreted.events.clone();
            events.sort_by_key(|event| (event.tick, event.order));
            for event in &events {
                match &event.kind {
                    EventKind::Chord(chord) => {
                        let silent = chord
                            .staff
                            .as_ref()
                            .is_some_and(|target| silent_staff_names.contains(&target.name));
                        let staff_move = if silent {
                            0
                        } else if let Some(target) = &chord.staff {
                            match staff_move_of(
                                staves,
                                &names,
                                current,
                                target,
                                &mut warned,
                                warnings,
                            ) {
                                Some(index) => {
                                    current = index;
                                    i8::try_from(index).unwrap_or(i8::MAX)
                                        - i8::try_from(staff_index).unwrap_or(i8::MAX)
                                }
                                None => 0,
                            }
                        } else {
                            0
                        };
                        place_chord(
                            score,
                            slots,
                            staff,
                            voice_index,
                            event.tick,
                            chord,
                            staff_move,
                            silent,
                            warnings,
                        );
                    }
                    EventKind::Meta(meta) => {
                        place_meta(
                            score,
                            slots,
                            staff.voices[voice_index].track,
                            event.tick,
                            meta,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Portée cible d'un `\change`, ou `None` si le nom est inconnu (les notes
/// restent alors sur leur portée d'origine). L'avertissement
/// n'est émis qu'une fois par nom et par voix, à la position du `\change`.
fn staff_move_of<'a>(
    staves: &[StaffBuild],
    names: &BTreeMap<&str, Vec<usize>>,
    from: usize,
    target: &'a StaffTarget,
    warned: &mut BTreeSet<&'a str>,
    warnings: &mut Warnings,
) -> Option<usize> {
    let name = target.name.as_str();
    // `core_find` teste d'abord le contexte lui-même : depuis la seconde
    // portée nommée `right`, `\change Staff = right` ne déplace rien.
    if staves[from].name.as_deref() == Some(name) {
        return Some(from);
    }
    let found = names.get(name).and_then(|indices| {
        let first = indices
            .iter()
            .copied()
            .min_by_key(|&index| (staves[index].group, index));
        match indices.as_slice() {
            // Noms dupliqués : `find_context` renvoie le premier contexte créé,
            // même s'il appartient à une autre partie (les parties MusicXML
            // n'existent pas dans `lily/context.cc`).
            [_, _, ..] => first,
            // Un seul porteur du nom : `find_context` le retrouve même si le
            // modèle en a fait une partie distincte (les parties MusicXML
            // n'existent pas dans `lily/context.cc`).
            [only] => Some(*only),
            _ => None,
        }
    });
    if found.is_none() && warned.insert(name) {
        warnings.push(
            target.line,
            target.column,
            format!("portée inconnue : {}", target.name),
        );
    }
    found
}

/// Place un accord ou un silence.
// Le contexte de placement (partition, mesure, portée, voix, tick, événement,
// déplacement de portée et avertissements) est passé explicitement : un struct
// alourdirait tous les appelants pour un seul appel.
#[allow(clippy::too_many_arguments)]
fn place_chord(
    score: &mut Score,
    slots: &[MeasureSlot],
    staff: &StaffBuild,
    voice_index: usize,
    tick: Fraction,
    event: &ChordEvent,
    staff_move: i8,
    silent: bool,
    warnings: &mut Warnings,
) {
    let voice_track = staff.voices[voice_index].track;
    let tuplets = &staff.voices[voice_index].tuplets;
    // L'écrivain comble les trous d'une voix secondaire par des silences de
    // mesure entière : ces silences ne sont pas dans le modèle d'origine.
    // Les relire créerait des `<rest measure="yes"/>` que la référence
    // n'a pas. On les ignore pour les voix autres que la première.
    if matches!(event.content, ChordContent::MultiRest { .. }) && voice_index > 0 {
        return;
    }
    // `Timing` est un alias de `Score` (`ly/engraver-init.ly`) : toutes les
    // portées partagent la grille. Un `R` dont la durée écrite franchit une
    // barre ne crée pas une mesure propre : sa musique continue après la
    // barre. Découper le silence à chaque frontière donne à l'écrivain des
    // durées explicites, au lieu d'un `<rest measure="yes"/>` tronqué à la
    // longueur de la mesure levée (NR §2.2.3, §2.3.3).
    if matches!(event.content, ChordContent::MultiRest { .. }) {
        let end = tick + event.actual;
        let fits = measure_at(slots, tick)
            .map(|index| end <= slots[index].start + slots[index].len)
            .unwrap_or(false);
        if !fits {
            let mut cursor = tick;
            let mut garde = 0;
            while cursor < end && garde < 1000 {
                garde += 1;
                let Some(index) = measure_at(slots, cursor) else {
                    warnings.push(0, 0, "silence hors mesure ignoré".to_owned());
                    break;
                };
                let measure_end = slots[index].start + slots[index].len;
                let piece_end = end.min(measure_end);
                if piece_end <= cursor {
                    break;
                }
                let piece = piece_end - cursor;
                let mut position = cursor;
                for figure in to_duration_list(piece, true, 4, false) {
                    let segment_index =
                        ensure_segment(score, index, SegmentType::ChordRest, position);
                    let chord_rest = ChordRest {
                        duration: figure,
                        ticks: None,
                        tuplet: None,
                        lyrics: Vec::new(),
                        engraving: ChordRestEngraving::default(),
                        kind: ChordRestKind::Rest(Rest {
                            full_measure: false,
                            gap: false,
                            visible: true,
                            note_dots: Vec::new(),
                            grace_chords: if cursor == tick {
                                build_rest_graces(score, event)
                            } else {
                                Vec::new()
                            },
                        }),
                    };
                    if let Some(segment) = score.measures[index].segments.get_mut(segment_index) {
                        segment.set_element(voice_track, SegmentItem::ChordRest(chord_rest));
                        for kind in crate::events::fermata_objects(&event.articulations) {
                            segment.annotations.push(SegmentAnnotation::new(
                                voice_track,
                                Annotation::Fermata(Fermata::new(kind)),
                            ));
                        }
                    }
                    position += figure.ticks();
                }
                cursor = piece_end;
            }
            return;
        }
    }
    // Un accord qui franchit une barre est coupé à la frontière et ses
    // morceaux sont liés : le modèle n'a qu'une durée par mesure. Sans cela,
    // l'accord allonge sa mesure dans le MusicXML et décale toutes les
    // attaques suivantes (`lily/tie-performer.cc` joue les morceaux liés
    // comme une seule note).
    if matches!(
        event.content,
        ChordContent::Notes(_) | ChordContent::Rest { .. }
    ) && event.actual.is_positive()
    {
        let end = tick + event.actual;
        let depasse = measure_at(slots, tick)
            .is_some_and(|index| end > slots[index].start + slots[index].len);
        if depasse {
            let mut cursor = tick;
            let mut precedent: Option<Vec<NoteId>> = None;
            let mut garde = 0;
            while cursor < end && garde < 1000 {
                garde += 1;
                let Some(index) = measure_at(slots, cursor) else {
                    warnings.push(0, 0, "accord hors mesure ignoré".to_owned());
                    break;
                };
                let piece_end = end.min(slots[index].start + slots[index].len);
                if piece_end <= cursor {
                    break;
                }
                for figure in to_duration_list(piece_end - cursor, true, 4, false) {
                    let len = figure.ticks();
                    if !len.is_positive() {
                        continue;
                    }
                    let segment_index =
                        ensure_segment(score, index, SegmentType::ChordRest, cursor);
                    let mut chord_rest = ChordRest {
                        duration: figure,
                        ticks: None,
                        tuplet: event.tuplet.and_then(|n| tuplets.get(n).copied()),
                        lyrics: Vec::new(),
                        engraving: ChordRestEngraving::default(),
                        kind: ChordRestKind::Rest(Rest {
                            full_measure: false,
                            gap: false,
                            visible: true,
                            note_dots: Vec::new(),
                            grace_chords: Vec::new(),
                        }),
                    };
                    match &event.content {
                        ChordContent::Notes(notes) => {
                            let mut chord = Chord {
                                engraving: ChordEngraving { stem: event.stem },
                                staff_move,
                                articulations: articulation_objects(&event.articulations),
                                ornaments: ornament_objects(&event.ornaments),
                                arpeggio: event.arpeggio.map(arpeggio_object),
                                tremolo: event.tremolo,
                                chord_line: event.chord_line,
                                ..Chord::default()
                            };
                            for note in notes {
                                let id = score.add_note(build_note(note, &event.fingerings));
                                chord.notes.push(id);
                            }
                            if cursor == tick {
                                for grace in &event.grace_before {
                                    let built = build_grace(score, grace, None);
                                    link_grace_block_tie(score, grace, &built, &chord.notes);
                                    chord.grace_chords.push(built);
                                }
                            }
                            if cursor + len == end {
                                for grace in &event.grace_after {
                                    let built =
                                        build_grace(score, grace, Some(len * grace.fraction));
                                    // La cible d'un `~` d'agrément après est
                                    // hors du bloc : laisser un marqueur à
                                    // résoudre après la poses des mesures.
                                    link_grace_block_tie(score, grace, &built, &[]);
                                    chord.grace_chords.push(built);
                                }
                            }
                            chord_rest.kind = ChordRestKind::Chord(chord);
                        }
                        ChordContent::Rest { visible } => {
                            let mut graces = Vec::new();
                            if cursor == tick {
                                for grace in &event.grace_before {
                                    graces.push(build_grace(score, grace, None));
                                }
                            }
                            if cursor + len == end {
                                for grace in &event.grace_after {
                                    graces.push(build_grace(
                                        score,
                                        grace,
                                        Some(len * grace.fraction),
                                    ));
                                }
                            }
                            chord_rest.kind = ChordRestKind::Rest(Rest {
                                full_measure: false,
                                gap: false,
                                visible: *visible,
                                note_dots: Vec::new(),
                                grace_chords: graces,
                            });
                        }
                        _ => unreachable!("cas filtré avant la coupure"),
                    }
                    if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                        let ids = chord.notes.clone();
                        if let Some(previous) = precedent.take() {
                            for (avant, apres) in previous.iter().zip(ids.iter()) {
                                score.note_mut(*avant).tie_forward = Some(*apres);
                                score.note_mut(*apres).tie_back = Some(*avant);
                            }
                        }
                        precedent = Some(ids);
                    }
                    if let Some(segment) = score.measures[index].segments.get_mut(segment_index) {
                        segment.set_element(voice_track, SegmentItem::ChordRest(chord_rest));
                        for kind in crate::events::fermata_objects(&event.articulations) {
                            segment.annotations.push(SegmentAnnotation::new(
                                voice_track,
                                Annotation::Fermata(Fermata::new(kind)),
                            ));
                        }
                    }
                    cursor += len;
                }
            }
            if let Some(breath) = event.breath {
                place_breath(score, slots, voice_track, tick, breath);
            }
            return;
        }
    }
    let Some(measure_index) = measure_at(slots, tick) else {
        warnings.push(0, 0, "événement hors mesure ignoré".to_owned());
        return;
    };
    let segment_index = ensure_segment(score, measure_index, SegmentType::ChordRest, tick);
    let mut chord_rest = match &event.content {
        ChordContent::Notes(notes) => {
            let mut chord = Chord {
                engraving: ChordEngraving { stem: event.stem },
                staff_move,
                articulations: articulation_objects(&event.articulations),
                ornaments: ornament_objects(&event.ornaments),
                arpeggio: event.arpeggio.map(arpeggio_object),
                tremolo: event.tremolo,
                chord_line: event.chord_line,
                ..Chord::default()
            };
            for note in notes {
                let id = score.add_note(build_note(note, &event.fingerings));
                chord.notes.push(id);
            }
            for grace in &event.grace_before {
                let built = build_grace(score, grace, None);
                link_grace_block_tie(score, grace, &built, &chord.notes);
                chord.grace_chords.push(built);
            }
            let main_ticks = event
                .explicit_ticks
                .unwrap_or_else(|| event.written.ticks());
            for grace in &event.grace_after {
                let built = build_grace(score, grace, Some(main_ticks * grace.fraction));
                link_grace_block_tie(score, grace, &built, &[]);
                chord.grace_chords.push(built);
            }
            ChordRest {
                duration: event.written,
                ticks: event.explicit_ticks,
                tuplet: event.tuplet.and_then(|index| tuplets.get(index).copied()),
                lyrics: Vec::new(),
                engraving: ChordRestEngraving::default(),
                kind: ChordRestKind::Chord(chord),
            }
        }
        // `Grace_iterator::process` (`lily/grace-iterator.cc:24-42`) insère
        // les grâces du moment courant avant l'élément principal, silence
        // compris. Sans ces lignes, `\grace { … } r4` perdait ses attaques
        // (vérifié avec LilyPond 2.27 : 4 notes jouées contre 3 écrites).
        ChordContent::Rest { visible } => {
            let graces = build_rest_graces(score, event);
            ChordRest {
                duration: event.written,
                ticks: event.explicit_ticks,
                tuplet: event.tuplet.and_then(|index| tuplets.get(index).copied()),
                lyrics: Vec::new(),
                engraving: ChordRestEngraving::default(),
                kind: ChordRestKind::Rest(Rest {
                    full_measure: false,
                    gap: false,
                    visible: *visible,
                    note_dots: Vec::new(),
                    grace_chords: graces,
                }),
            }
        }
        ChordContent::Skip { .. } => return,
        // `R4.` (figure explicite plus courte que la mesure) est un silence
        // daté chez LilyPond (`lily/parser.yy:3117`) : l'écrivain doit lui
        // donner sa figure, sinon il émet trois `<rest measure="yes"/>` dans
        // la même mesure et l'oracle décale toutes les attaques suivantes.
        ChordContent::MultiRest { .. }
            if event.multi_rest_explicit
                && measure_at(slots, tick).is_some_and(|index| event.actual < slots[index].len) =>
        {
            ChordRest {
                duration: event.written,
                ticks: event.explicit_ticks,
                tuplet: None,
                lyrics: Vec::new(),
                engraving: ChordRestEngraving::default(),
                kind: ChordRestKind::Rest(Rest {
                    full_measure: false,
                    gap: false,
                    visible: true,
                    note_dots: Vec::new(),
                    grace_chords: build_rest_graces(score, event),
                }),
            }
        }
        ChordContent::MultiRest { .. } => ChordRest {
            duration: Duration::measure(),
            ticks: None,
            tuplet: None,
            lyrics: Vec::new(),
            engraving: ChordRestEngraving::default(),
            kind: ChordRestKind::Rest(Rest {
                full_measure: true,
                gap: false,
                visible: true,
                note_dots: Vec::new(),
                grace_chords: build_rest_graces(score, event),
            }),
        },
    };
    chord_rest.ticks = chord_rest
        .ticks
        .filter(|ticks| *ticks != chord_rest.duration.ticks());
    if silent {
        if let ChordRestKind::Chord(chord) = &chord_rest.kind {
            for id in &chord.notes {
                let note = score.note_mut(*id);
                note.playback.play = false;
                note.playback.unperformed = true; // musique déplacée vers une portée muette
            }
            for grace in &chord.grace_chords {
                for id in &grace.chord.notes {
                    let note = score.note_mut(*id);
                    note.playback.play = false;
                    note.playback.unperformed = true;
                }
            }
        }
    }
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.set_element(voice_track, SegmentItem::ChordRest(chord_rest));
        for kind in crate::events::fermata_objects(&event.articulations) {
            segment.annotations.push(SegmentAnnotation::new(
                voice_track,
                Annotation::Fermata(Fermata::new(kind)),
            ));
        }
    }
    if let Some(breath) = event.breath {
        place_breath(score, slots, voice_track, tick, breath);
    }
}

/// Construit une note du modèle.
fn build_note(note: &crate::events::NoteEvent, fingerings: &[String]) -> Note {
    let mut value = Note::new(note.pitch, note.tpc);
    value.tpc_transposed = note.tpc_transposed;
    value.head_group = note.head_group.clone();
    value.small = note.small;
    value.visible = note.visible;
    value.playback.play = note.play;
    value.playback.unperformed = note.unperformed;
    if note.cautionary {
        value.accidental = Some(Accidental {
            kind: accidental_for(note.tpc),
            bracket: false,
            parentheses: false,
            visible: true,
            user: true,
            small: false,
        });
    }
    value.fingerings = fingerings
        .iter()
        .map(|text| Fingering::new(text.clone()))
        .collect();
    value
}

/// Altération affichée d'après une classe de hauteur tonale.
fn accidental_for(tpc: i8) -> AccidentalType {
    let alter = (i32::from(tpc) - 13).div_euclid(7);
    match alter {
        -2 => AccidentalType::DoubleFlat,
        -1 => AccidentalType::Flat,
        0 => AccidentalType::Natural,
        1 => AccidentalType::Sharp,
        2 => AccidentalType::DoubleSharp,
        _ => AccidentalType::None,
    }
}

/// Relie la dernière note d'un bloc d'agrément dont le `~` est encore
/// pendant à la note principale de même hauteur. `def-grace-function`
/// (`scm/music-functions.scm:1237-1248`) enveloppe le bloc dans une musique
/// séquentielle : le `~` de la dernière note d'agrément vise la note qui
/// suit le bloc, mais `link_ties` ne voit pas les notes d'agrément (elles ne
/// sont pas dans les événements datés de la voix).
fn link_grace_block_tie(
    score: &mut Score,
    grace: &crate::events::GraceEvent,
    built: &GraceChord,
    main_notes: &[NoteId],
) {
    for (note, &id) in grace.notes.iter().zip(built.chord.notes.iter()) {
        if !note.tie {
            continue;
        }
        if let Some(&main) = main_notes
            .iter()
            .find(|&&candidate| score.note(candidate).pitch == score.note(id).pitch)
        {
            score.note_mut(id).tie_forward = Some(main);
            score.note_mut(main).tie_back = Some(id);
        } else {
            // Agrément *après* la note : la cible est dans une mesure
            // suivante, encore à construire. Marquer par une auto-liaison,
            // résolue par `resolve_after_grace_ties`.
            score.note_mut(id).tie_forward = Some(id);
        }
    }
}

/// Relie les `~` d'agréments après (`\afterGrace`) à la prochaine note de
/// même hauteur. Le `~` est émis par l'écrivain sur la dernière note du bloc
/// (`def-grace-function`, `scm/music-functions.scm:1237-1248`) ; la cible est
/// hors du bloc, dans une mesure suivante.
fn resolve_after_grace_ties(score: &mut Score) {
    let mut notes: Vec<(scoretk_core::model::NoteId, scoretk_core::model::Tick, u8)> = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_track, item) in segment.iter_elements() {
                let Some(chord_rest) = item.as_chord_rest() else {
                    continue;
                };
                let Some(chord) = chord_rest.chord_ref() else {
                    continue;
                };
                for grace in &chord.grace_chords {
                    for &id in &grace.chord.notes {
                        notes.push((id, segment.tick, score.note(id).pitch));
                    }
                }
                for &id in &chord.notes {
                    notes.push((id, segment.tick, score.note(id).pitch));
                }
            }
        }
    }
    let pending: Vec<_> = notes
        .iter()
        .filter(|(id, _, _)| score.note(*id).tie_forward == Some(*id))
        .cloned()
        .collect();
    for (id, tick, pitch) in pending {
        let Some((next, _, _)) = notes
            .iter()
            .filter(|(other, other_tick, other_pitch)| {
                *other != id && *other_tick > tick && *other_pitch == pitch
            })
            .min_by_key(|(_, other_tick, _)| *other_tick)
        else {
            score.note_mut(id).tie_forward = None;
            continue;
        };
        score.note_mut(id).tie_forward = Some(*next);
        score.note_mut(*next).tie_back = Some(id);
    }
}

/// Construit un accord d'agrément.
fn build_grace(
    score: &mut Score,
    grace: &GraceEvent,
    played_offset: Option<Fraction>,
) -> GraceChord {
    let mut value = GraceChord::new(grace.kind, grace.duration);
    value.played_offset = played_offset;
    value.chord.grace_slash = grace.slash;
    for note in &grace.notes {
        let id = score.add_note(build_note(note, &[]));
        value.chord.notes.push(id);
    }
    value
}

/// Agréments d'un silence : `Grace_iterator::process`
/// (`lily/grace-iterator.cc:24-42`) les joue avant l'élément principal,
/// silence compris (`\grace { … } r4`, `\grace { … } R1`).
fn build_rest_graces(score: &mut Score, event: &ChordEvent) -> Vec<GraceChord> {
    let main_ticks = event
        .explicit_ticks
        .unwrap_or_else(|| event.written.ticks());
    let mut graces = Vec::new();
    for grace in &event.grace_before {
        graces.push(build_grace(score, grace, None));
    }
    for grace in &event.grace_after {
        graces.push(build_grace(score, grace, Some(main_ticks * grace.fraction)));
    }
    graces
}

/// Place une respiration.
fn place_breath(
    score: &mut Score,
    slots: &[MeasureSlot],
    track: Track,
    tick: Fraction,
    kind: scoretk_core::model::BreathType,
) {
    let Some(measure_index) = measure_at(slots, tick) else {
        return;
    };
    let segment_index = ensure_segment(score, measure_index, SegmentType::Breath, tick);
    let placed = breath_object(kind, Placement::Above);
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.set_element(track, SegmentItem::Breath(placed));
    }
}

/// Place un changement de contexte.
fn place_meta(
    score: &mut Score,
    slots: &[MeasureSlot],
    track: Track,
    tick: Fraction,
    meta: &MetaEvent,
) {
    let Some(measure_index) = measure_at(slots, tick) else {
        return;
    };
    match meta {
        // Liaison de silence : seul `link_ties` l'utilise, elle n'ajoute
        // aucun élément au modèle.
        MetaEvent::RestTie(_) => {}
        MetaEvent::Clef(clef) => {
            // MuseScore place la clef d'en-tête dans `HeaderClef` et une
            // éventuelle seconde clef au même tick dans `Clef` (v3/clef-1).
            // Deux `\clef` successifs en tête de portée doivent donc rester
            // deux éléments relus, pas écraser le premier.
            let header_taken = score.measures.first().is_some_and(|measure| {
                measure.segments.iter().any(|segment| {
                    segment.kind == SegmentType::HeaderClef
                        && segment
                            .element(track)
                            .is_some_and(|item| matches!(item, SegmentItem::Clef(_)))
                })
            });
            let (target_measure, kind) = if tick.is_zero() {
                (
                    0,
                    if header_taken {
                        SegmentType::Clef
                    } else {
                        SegmentType::HeaderClef
                    },
                )
            } else if slots
                .get(measure_index)
                .is_some_and(|slot| slot.start == tick)
            {
                (measure_index.saturating_sub(1), SegmentType::Clef)
            } else {
                (measure_index, SegmentType::Clef)
            };
            let segment_index = ensure_segment(score, target_measure, kind, tick);
            if let Some(segment) = score.measures[target_measure]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(track, SegmentItem::Clef(Clef::new(*clef)));
            }
        }
        MetaEvent::Key(key) => {
            let segment_index = ensure_segment(score, measure_index, SegmentType::KeySig, tick);
            if let Some(segment) = score.measures[measure_index]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(track, SegmentItem::KeySig(key.clone()));
            }
        }
        MetaEvent::Time {
            numerator,
            denominator,
        } => {
            let segment_index = ensure_segment(score, measure_index, SegmentType::TimeSig, tick);
            if let Some(segment) = score.measures[measure_index]
                .segments
                .get_mut(segment_index)
            {
                segment.set_element(
                    track,
                    SegmentItem::TimeSig(scoretk_core::model::TimeSig::new(
                        *numerator,
                        *denominator,
                    )),
                );
            }
        }
        MetaEvent::Bar(BarLineType::Normal) => {
            // `|` est la barre par défaut du modèle : la poser écraserait une
            // barre plus forte (`\bar "|."`, fin de reprise) au même tick.
            // La trame des mesures s'en sert seule pour couper les cadences.
        }
        MetaEvent::Bar(kind) => {
            // Une barre à la fin d'une mesure ferme cette mesure ; au début,
            // elle ouvre la suivante.
            let target = slots
                .iter()
                .position(|slot| slot.start + slot.len == tick)
                .map(|index| (index, false))
                .or_else(|| {
                    slots
                        .iter()
                        .position(|slot| slot.start == tick)
                        .map(|index| (index, true))
                });
            if let Some((index, start)) = target {
                let measure = &mut score.measures[index];
                if start {
                    measure.start_bar_line = *kind;
                } else {
                    measure.end_bar_line = *kind;
                }
            }
        }
        MetaEvent::Section => {
            if let Some(measure) = score.measures.get_mut(measure_index) {
                measure.section_break_before = true;
            }
        }
        MetaEvent::Break => {
            let index = slots
                .iter()
                .position(|slot| slot.start + slot.len == tick)
                .unwrap_or(measure_index);
            if let Some(measure) = score.measures.get_mut(index) {
                measure.line_break = true;
            }
        }
        MetaEvent::Partial(_)
        | MetaEvent::MeasureLength(_)
        | MetaEvent::MeasurePosition(_)
        | MetaEvent::TieWaitForNote(_)
        | MetaEvent::Cadenza(_)
        | MetaEvent::Transposition(_) => {}
    }
}

// ----------------------------------------------------------------------
// Spanners, reprises, paroles, liaisons
// ----------------------------------------------------------------------

/// Place les spanners et annotations.
fn place_spanners(score: &mut Score, staves: &[StaffBuild]) {
    for staff in staves {
        for voice in &staff.voices {
            let mut open_slurs: BTreeMap<u8, Fraction> = BTreeMap::new();
            let mut hairpin: Option<(Fraction, HairpinType)> = None;
            let mut trill: Option<Fraction> = None;
            let mut ottava: Option<(Fraction, i8, bool)> = None;
            let mut pedal: Option<Fraction> = None;
            let mut pending_glissando: Option<Fraction> = None;

            let mut events: Vec<&VoiceEvent> = voice.interpreted.events.iter().collect();
            events.sort_by_key(|event| (event.tick, event.order));
            for event in events {
                let tick = event.tick;
                match &event.kind {
                    EventKind::Chord(_) => {
                        if let Some(start) = pending_glissando.take() {
                            add_spanner(
                                score,
                                SpannerType::Glissando,
                                start,
                                tick,
                                voice.track,
                                |spanner| {
                                    spanner.notation = Some(SpannerNotation::Glissando(
                                        Glissando::new(GlissandoType::Straight(LineStyle::Solid)),
                                    ));
                                },
                            );
                        }
                    }
                    EventKind::SlurStart(number) => {
                        open_slurs.insert(*number, tick);
                    }
                    EventKind::SlurStop(number) => {
                        if let Some(start) = open_slurs.remove(number) {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Slur,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| {
                                        spanner.notation = Some(SpannerNotation::Slur(
                                            scoretk_core::model::Slur::default(),
                                        ));
                                    },
                                );
                            }
                        }
                    }
                    EventKind::HairpinStart(kind) => {
                        hairpin.get_or_insert((tick, *kind));
                    }
                    EventKind::HairpinStop => {
                        if let Some((start, kind)) = hairpin.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Hairpin(kind),
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| spanner.hairpin = Some(Hairpin::new()),
                                );
                            }
                        }
                    }
                    EventKind::TrillStart => trill = Some(tick),
                    EventKind::TrillStop => {
                        if let Some(start) = trill.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Trill,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| {
                                        spanner.notation = Some(SpannerNotation::Trill(
                                            Trill::new(TrillType::TrillLine),
                                        ));
                                    },
                                );
                            }
                        }
                    }
                    EventKind::Ottava(shift, visible) => {
                        if *shift == 0 {
                            if let Some((start, shift, visible)) = ottava.take() {
                                if start < tick {
                                    if let Some(kind) = ottava_type(shift) {
                                        add_spanner(
                                            score,
                                            SpannerType::Ottava(kind),
                                            start,
                                            tick,
                                            voice.track,
                                            // `\ottava` ne transpose pas le MIDI
                                            // LilyPond (Notation Reference §1.3.3)
                                            // : la portée ne joue pas ce spanner.
                                            |spanner| {
                                                spanner.play = false;
                                                spanner.visible = visible;
                                            },
                                        );
                                    }
                                }
                            }
                        } else {
                            ottava = Some((tick, *shift, *visible));
                        }
                    }
                    EventKind::Pedal(on) => {
                        if *on {
                            pedal = Some(tick);
                        } else if let Some(start) = pedal.take() {
                            if start < tick {
                                add_spanner(
                                    score,
                                    SpannerType::Pedal,
                                    start,
                                    tick,
                                    voice.track,
                                    |spanner| spanner.pedal = Some(PedalInfo::new()),
                                );
                            }
                        }
                    }
                    EventKind::Glissando => pending_glissando = Some(tick),
                    EventKind::Annotation(annotation) => {
                        place_annotation(score, tick, voice.track, annotation);
                    }
                    EventKind::Breath(_) | EventKind::Meta(_) => {}
                }
            }
        }
    }
}

/// Ajoute un spanner au modèle.
fn add_spanner(
    score: &mut Score,
    kind: SpannerType,
    start: Fraction,
    end: Fraction,
    track: Track,
    configure: impl FnOnce(&mut Spanner),
) {
    let mut spanner = Spanner::new(
        kind,
        SpannerAnchor::new(start, track),
        SpannerAnchor::new(end, track),
    );
    configure(&mut spanner);
    score.add_spanner(spanner);
}

/// Place une annotation sur le segment du tick.
fn place_annotation(score: &mut Score, tick: Fraction, track: Track, annotation: &AnnotationEvent) {
    let Some(measure_index) = score
        .measures
        .partition_point(|measure| measure.start_tick <= tick)
        .checked_sub(1)
    else {
        return;
    };
    match annotation {
        AnnotationEvent::Marker { label, kind } => {
            score.measures[measure_index]
                .repeat_marks
                .push(RepeatMark::Marker(Marker {
                    kind: *kind,
                    label: label.clone(),
                    text: label.clone(),
                    placement: Placement::Above,
                    track,
                    visible: true,
                }));
            return;
        }
        AnnotationEvent::Jump {
            text,
            jump_to,
            play_until,
            continue_at,
        } => {
            score.measures[measure_index]
                .repeat_marks
                .push(RepeatMark::Jump(Jump::new(
                    text.clone(),
                    jump_to.clone(),
                    play_until.clone(),
                    continue_at.clone(),
                    Placement::Above,
                    track,
                )));
            return;
        }
        _ => {}
    }
    let segment_index = ensure_segment(score, measure_index, SegmentType::ChordRest, tick);
    let value = match annotation {
        AnnotationEvent::Dynamic {
            text,
            placement,
            custom,
            velocity,
        } => Annotation::Dynamic(Dynamic {
            text: text.clone(),
            velocity: *velocity,
            placement: *placement,
            custom: *custom,
            visible: true,
        }),
        AnnotationEvent::Tempo {
            text,
            beats_per_minute,
        } => Annotation::Tempo(Tempo {
            text: RichText::new(text.clone()),
            beats_per_second: beats_per_minute.map(|value| value / 60.0),
            follow_text: false,
            placement: Placement::Above,
            visible: true,
            kind: Default::default(),
        }),
        AnnotationEvent::Mark(text) => Annotation::RehearsalMark(RehearsalMark::new(text.clone())),
        AnnotationEvent::Marker { .. } | AnnotationEvent::Jump { .. } => {
            unreachable!("marqueurs et sauts sont placés avant le segment")
        }
        AnnotationEvent::Text { text, placement } => {
            let mut staff_text = scoretk_core::model::StaffText::new(text.clone());
            staff_text.placement = *placement;
            Annotation::StaffText(staff_text)
        }
    };
    if let Some(segment) = score.measures[measure_index]
        .segments
        .get_mut(segment_index)
    {
        segment.add_annotation(track, value);
    }
}

/// Place les reprises et volatas.
fn place_repeats(score: &mut Score, staves: &[StaffBuild]) {
    let mut sections: Vec<RepeatSection> = Vec::new();
    if let Some(staff) = staves.first() {
        for voice in &staff.voices {
            sections.extend(voice.interpreted.repeats.iter().cloned());
        }
    }
    sections.sort_by_key(|section| section.start);
    sections.dedup_by_key(|section| section.start);

    for section in &sections {
        if section.count < 2 {
            continue;
        }
        let Some(start_index) = score
            .measures
            .iter()
            .position(|measure| measure.start_tick == section.start)
        else {
            continue;
        };
        let end_index = score
            .measures
            .iter()
            .position(|measure| measure.start_tick + measure.len == section.end)
            .or_else(|| {
                score
                    .measures
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, measure)| measure.start_tick < section.end)
                    .map(|(index, _)| index)
            });
        if let Some(measure) = score.measures.get_mut(start_index) {
            if measure.start_bar_line == BarLineType::Normal {
                measure.start_bar_line = BarLineType::StartRepeat;
            }
        }
        if let Some(end_index) = end_index {
            if let Some(measure) = score.measures.get_mut(end_index) {
                measure.end_bar_line = BarLineType::EndRepeat;
                measure.repeat_count = section.count;
            }
        }
        // LilyPond rejoue chaque bloc `\alternative` aux passes qui lui
        // reviennent : avec N lectures et k blocs, le bloc i (à partir de 1) se
        // joue aux passes i, i+k, i+2k… tant qu'elles ne dépassent pas N.
        // C'est la règle que MuseScore stocke dans `<endings>` (par exemple
        // `1, 3` pour le premier bloc d'un `\repeat volta 3` à deux blocs).
        let blocks = u32::try_from(section.alternatives.len())
            .unwrap_or(1)
            .max(1);
        let plays = u32::from(section.count);
        for (index, alternative) in section.alternatives.iter().enumerate() {
            let mut endings = Vec::new();
            let mut pass = u32::try_from(index).unwrap_or(0) + 1;
            while pass <= plays {
                endings.push(u8::try_from(pass).unwrap_or(u8::MAX));
                pass += blocks;
            }
            if endings.is_empty() {
                endings.push(alternative.number);
            }
            let mut volta = Spanner::new(
                SpannerType::Volta,
                SpannerAnchor::new(alternative.start, Track::from_raw(0)),
                SpannerAnchor::new(alternative.end, Track::from_raw(0)),
            );
            volta.volta = Some(scoretk_core::model::VoltaInfo {
                endings,
                closed: true,
            });
            score.add_spanner(volta);
        }
    }
}

/// Attache les paroles aux accords des voix ciblées.
fn place_lyrics(score: &mut Score, staves: &[StaffBuild], melisma_busy_disabled: bool) {
    // Numérotation automatique des couplets : sans `\set stanza`, les blocs
    // successifs d'une même voix prennent les numéros 1, 2, 3… dans l'ordre.
    let mut next_verse: HashMap<String, usize> = HashMap::new();
    for staff in staves {
        for lyrics in &staff.lyrics {
            // `\lyricsto "nom"` cherche la voix dans toute la partition :
            // elle peut être déclarée dans une autre portée ou un autre
            // groupe. À défaut de nom (ou de voix trouvée), on garde le
            // comportement d'\addlyrics : la première voix de la portée.
            let voice = if lyrics.target.is_empty() {
                staff.voices.first()
            } else {
                staves
                    .iter()
                    .flat_map(|staff| staff.voices.iter())
                    .find(|voice| voice.name.as_deref() == Some(lyrics.target.as_str()))
                    .or_else(|| staff.voices.first())
            };
            let Some(voice) = voice else {
                continue;
            };
            let verse = if lyrics.target.is_empty() {
                lyrics.verse
            } else {
                let counter = next_verse.entry(lyrics.target.clone()).or_insert(0);
                if lyrics.verse == 0 {
                    let assigned = *counter;
                    *counter += 1;
                    assigned
                } else {
                    *counter = (*counter).max(lyrics.verse + 1);
                    lyrics.verse
                }
            };
            attach_lyrics(
                score,
                voice,
                verse,
                &lyrics.syllables,
                melisma_busy_disabled,
            );
        }
    }
}

/// Attache les syllabes aux accords d'une voix.
fn attach_lyrics(
    score: &mut Score,
    voice: &VoiceBuild,
    verse: usize,
    syllables: &[crate::ast::Syllable],
    melisma_busy_disabled: bool,
) {
    // Par défaut, `melismaBusyProperties` contient `slurMelismaBusy` et
    // `tieMelismaBusy` : `\lyricsto` saute les notes à l'intérieur d'une
    // liaison d'expression et les continuations de liaison (`~`). Un
    // `\set melismaBusyProperties = #'()` désactive ces deux règles.
    let melisma_busy = !melisma_busy_disabled;
    let mut events: Vec<&VoiceEvent> = voice.interpreted.events.iter().collect();
    events.sort_by_key(|event| (event.tick, event.order));

    // Portées des liaisons d'expression. La note qui porte `(` ouvre la
    // liaison mais n'est pas elle-même « occupée » ; celles qui suivent,
    // jusqu'à celle qui porte `)` comprise, le sont. Une liaison non fermée
    // couvre jusqu'à la fin de la voix.
    let mut open_slurs: BTreeMap<u8, Fraction> = BTreeMap::new();
    let mut slur_spans: Vec<(Fraction, Option<Fraction>)> = Vec::new();
    for event in &events {
        match &event.kind {
            EventKind::SlurStart(number) => {
                open_slurs.insert(*number, event.tick);
            }
            EventKind::SlurStop(number) => {
                if let Some(start) = open_slurs.remove(number) {
                    slur_spans.push((start, Some(event.tick)));
                }
            }
            _ => {}
        }
    }
    slur_spans.extend(open_slurs.into_values().map(|start| (start, None)));
    let under_slur = |tick: Fraction| {
        slur_spans
            .iter()
            .any(|(start, end)| *start < tick && end.is_none_or(|end| tick <= end))
    };

    // `\lyricsto` aligne une syllabe par note éligible, mais saute les
    // silences et les notes occupées : seules les têtes qui commencent une
    // nouvelle note hors liaison reçoivent une syllabe.
    let mut chords = Vec::new();
    let mut tied_from_previous = false;
    for event in events {
        if let EventKind::Chord(chord) = &event.kind {
            if let ChordContent::Notes(notes) = &chord.content {
                let all_tied = !notes.is_empty() && notes.iter().all(|note| note.tie);
                let skip = (melisma_busy && tied_from_previous)
                    || (melisma_busy && under_slur(event.tick));
                if !skip {
                    chords.push(event.tick);
                }
                tied_from_previous = all_tied;
            } else {
                tied_from_previous = false;
            }
        }
    }
    // Les paroles écrites avec des durées explicites (`la4 li2`) s'alignent
    // par tick sur la voix ; sans durée, une syllabe par note, comme
    // `\lyricsto` le fait.
    let use_ticks = syllables.iter().any(|syllable| syllable.ticks.is_some());
    let mut cursor = Fraction::ZERO;
    let mut current_duration = Fraction::ONE;
    let mut previous_hyphen = false;
    let mut chord_index = 0;
    for syllable in syllables {
        let tick = if use_ticks {
            if let Some(ticks) = syllable.ticks {
                current_duration = ticks;
            }
            let target = cursor;
            cursor += current_duration;
            let Some(index) = chords.iter().position(|tick| *tick >= target) else {
                break;
            };
            chord_index = index + 1;
            chords[index]
        } else {
            let Some(tick) = chords.get(chord_index).copied() else {
                break;
            };
            chord_index += 1;
            tick
        };
        if syllable.skip || syllable.text.is_empty() {
            // `\skip` est une note sans syllabe, pas une fin de mot : le `--`
            // qui précède s'applique à la syllabe suivante (Notation
            // Reference §2.1.2 "Lyrics hyphens"). Réinitialiser ici faisait
            // lire `Begin`/`End` comme `Single` après un mélisme.
            continue;
        }
        let syllabic = if syllable.hyphen {
            if previous_hyphen {
                LyricsSyllabic::Middle
            } else {
                LyricsSyllabic::Begin
            }
        } else if previous_hyphen {
            LyricsSyllabic::End
        } else {
            LyricsSyllabic::Single
        };
        previous_hyphen = syllable.hyphen;
        let value = Lyrics {
            verse,
            syllabic,
            text: syllable.text.clone(),
            melisma: syllable.extender.then_some(Fraction::ONE),
            placement: Placement::Below,
        };
        let Some(measure_index) = score
            .measures
            .partition_point(|measure| measure.start_tick <= tick)
            .checked_sub(1)
        else {
            continue;
        };
        for segment in &mut score.measures[measure_index].segments {
            if segment.tick != tick || segment.kind != SegmentType::ChordRest {
                continue;
            }
            if let Some(chord_rest) = segment.chord_rest_mut(voice.track) {
                chord_rest.lyrics.push(value);
                break;
            }
        }
        // Un `__` prolonge la syllabe sur la note suivante : celle-ci ne
        // reçoit pas de syllabe, comme pour `\lyricsto` dans LilyPond.
        if syllable.extender && !use_ticks {
            chord_index += 1;
        }
    }
}

/// Ajoute les altérations affichées d'après l'armure et la mesure.
///
/// MuseScore stocke un objet `Accidental` pour chaque altération affichée ;
/// LilyPond, lui, laisse l'éditeur décider. On applique donc la règle de
/// gravure : une note porte une altération quand son `tpc` écrit diffère de
/// l'altération en vigueur pour ce degré et cette octave (armure au début de
/// la mesure, dernière altération affichée ensuite).
fn apply_accidentals(score: &mut Score) {
    let staff_count = score.staves.len();
    for staff_index in 0..staff_count {
        let staff = StaffId::from_index(staff_index);
        let track = Track::new(staff, 0);
        let mut key_fifths = 0i8;
        for measure_index in 0..score.measures.len() {
            // Armure en vigueur au début de la mesure.
            for segment in &score.measures[measure_index].segments {
                if !segment.kind.is_key_sig_type()
                    || segment.tick > score.measures[measure_index].start_tick
                {
                    continue;
                }
                if let Some(SegmentItem::KeySig(key)) = segment.element(track) {
                    key_fifths = key.transposed_key.fifths();
                }
            }
            let mut state: BTreeMap<(u8, i8), i8> = BTreeMap::new();
            let mut positions: Vec<(Fraction, usize)> = score.measures[measure_index]
                .segments
                .iter()
                .enumerate()
                .filter(|(_, segment)| segment.kind == SegmentType::ChordRest)
                .map(|(index, segment)| (segment.tick, index))
                .collect();
            positions.sort();
            for (_, segment_index) in positions {
                for voice in 0..VOICES {
                    let note_ids: Vec<NoteId> = score.measures[measure_index].segments
                        [segment_index]
                        .chord_rest(Track::new(staff, u8::try_from(voice).unwrap_or(0)))
                        .and_then(ChordRest::chord_ref)
                        .map(|chord| chord.notes.clone())
                        .unwrap_or_default();
                    for id in note_ids {
                        let note = score.note(id);
                        let tpc = note.tpc_transposed;
                        if !(0..=40).contains(&tpc) {
                            continue;
                        }
                        // `tpc2step` de MuseScore : la spirale des quintes
                        // parcourt fa, do, sol, ré, la, mi, si.
                        let index =
                            usize::try_from((i32::from(tpc) + 8).rem_euclid(7)).unwrap_or(0);
                        let step = [3u8, 0, 4, 1, 5, 2, 6][index];
                        let alter = ((i32::from(tpc) + 8) / 7 - 3) as i8;
                        let octave = (i32::from(note.pitch) - i32::from(alter)).div_euclid(12) - 1;
                        let octave = i8::try_from(octave).unwrap_or(4);
                        let current = state
                            .get(&(step, octave))
                            .copied()
                            .unwrap_or_else(|| key_alter(key_fifths, step));
                        if alter == current {
                            continue;
                        }
                        state.insert((step, octave), alter);
                        if note.accidental.is_some() {
                            continue;
                        }
                        let kind = match alter {
                            -2 => AccidentalType::DoubleFlat,
                            -1 => AccidentalType::Flat,
                            0 => AccidentalType::Natural,
                            1 => AccidentalType::Sharp,
                            2 => AccidentalType::DoubleSharp,
                            _ => continue,
                        };
                        score.note_mut(id).accidental = Some(Accidental {
                            kind,
                            bracket: false,
                            parentheses: false,
                            visible: true,
                            user: false,
                            small: false,
                        });
                    }
                }
            }
        }
    }
}

/// Altération d'un degré naturel donnée par une armure.
fn key_alter(fifths: i8, step: u8) -> i8 {
    let sharps = [3u8, 0, 4, 1, 5, 2, 6];
    let flats = [6u8, 2, 5, 1, 4, 0, 3];
    if fifths > 0 {
        let count = usize::try_from(fifths).unwrap_or(0).min(7);
        if sharps[..count].contains(&step) {
            return 1;
        }
    } else if fifths < 0 {
        let count = usize::try_from(-fifths).unwrap_or(0).min(7);
        if flats[..count].contains(&step) {
            return -1;
        }
    }
    0
}

/// (attaque, fin écrite, fin MIDI, id, liaison, tête d'un `<< … >>`,
/// contexte `Voice`).
type TieEvent = (Fraction, Fraction, Fraction, NoteId, bool, bool, u32, bool);

/// Relie les notes de prolongation.
fn link_ties(score: &mut Score, staves: &[StaffBuild], midi_voices_as_staves: bool) {
    // Sous `\articulate`, `ac:getactions` (`ly/articulate.ly` l. 503-560)
    // retire les événements d'articulation de l'accord et *compresse les
    // durées* selon le dernier facteur vu (1/2 pour `\staccato`, 3/4 pour
    // `\portato`, 1 pour `\tenuto` ou un `\tie`). Le `midi-length` natif de
    // `ly/script-init.ly` n'est donc plus appliqué : le lecteur ne modelant
    // pas encore cette compression, il ne doit pas non plus raccourcir la fin
    // MIDI des notes (sinon `Celle.ly` perd ses liaisons).
    let articulate = score.playback.articulate;
    // Index des segments ChordRest par tick absolu : la recherche linéaire
    // dans toutes les mesures coûtait O(événements × mesures × segments)
    // (plus de 300 ms sur un concerto de 455 mesures).
    let mut segments_by_tick: BTreeMap<Fraction, (usize, usize)> = BTreeMap::new();
    for (measure_index, measure) in score.measures.iter().enumerate() {
        for (segment_index, segment) in measure.segments.iter().enumerate() {
            if segment.kind == SegmentType::ChordRest {
                segments_by_tick
                    .entry(segment.tick)
                    .or_insert((measure_index, segment_index));
            }
        }
    }
    // Notes datées de toute la partition et têtes de `\repeatTie` : la
    // prolongation peut traverser une alternative et donc changer de voix.
    let mut toutes_notes: Vec<(Fraction, NoteId, u8, bool)> = Vec::new();
    // Même index, mais avec l'identifiant du contexte `Voice` : une liaison
    // peut être fermée par une voix placée sur une autre portée du modèle
    // quand le `\midi` donne une piste par voix (`bwv869b.ly`).
    let mut toutes_notes_ctx: Vec<(Fraction, NoteId, u8, u32)> = Vec::new();
    let mut repeat_ties: Vec<(Fraction, NoteId)> = Vec::new();
    let mut rest_ties: Vec<(Fraction, Fraction)> = Vec::new();
    for staff in staves {
        for voice in &staff.voices {
            for event in &voice.interpreted.events {
                if let EventKind::Meta(MetaEvent::RestTie(duree)) = &event.kind {
                    rest_ties.push((event.tick, event.tick + *duree));
                }
            }
        }
    }
    for staff in staves {
        // Première passe : construire les suites datées de chaque voix. Elles
        // servent ensuite à la fois aux liaisons propres à la voix et aux
        // liaisons partagées d'un `<< … >>` sans `\\`, dont la suite du flux
        // se trouve souvent dans une autre voix.
        let mut sequences: Vec<Vec<TieEvent>> = Vec::new();
        let mut tie_waits: Vec<Vec<(Fraction, bool)>> = Vec::new();
        for voice in &staff.voices {
            // `\\set tieWaitForNote = ##t` : `Tie_performer::
            // stop_translation_timestep` ne retire plus les liaisons pendantes
            // dont la fin est passée (`lily/tie-performer.cc` l. 166-180) ; une
            // note de même hauteur, même plus tard, vient les fermer
            // (`acknowledge_audio_element`, l. 116-150). Le reglage est lu au
            // moment où la liaison serait jetée, d'où une chronologie par voix.
            let mut tie_wait: Vec<(Fraction, bool)> = voice
                .interpreted
                .events
                .iter()
                .filter_map(|event| match &event.kind {
                    EventKind::Meta(MetaEvent::TieWaitForNote(on)) => Some((event.tick, *on)),
                    _ => None,
                })
                .collect();
            tie_wait.sort_by_key(|(tick, _)| *tick);
            // Une liaison n'attend que le pas de temps où la note liée cesse
            // de sonner : `Tie_performer::stop_translation_timestep` retire
            // les liaisons pendantes dont `end_moment_` est dépassé
            // (`lily/tie-performer.cc` l. 147-185). `end_moment_` vient de la
            // **durée MIDI** de la note (`lily/note-performer.cc` l. 86-97),
            // raccourcie par les articulations pourvues d'un `midi-length`
            // (`ly/script-init.ly` l. 46-71) : `\staccato` divise par deux,
            // `\portato` par 3/4. Le pas de temps est global à la partition :
            // une note dans une autre portée à `end_moment_` suffit à faire
            // jeter la liaison, qui redevient une attaque à sa tête droite.
            // Sans pas de temps intermédiaire, la tête pendante survit et la
            // liaison tient (`Fuge_Opus137`, `bwv735`, `bwv855a`, …).
            // `tieWaitForNote` désactive cette éviction. La file est en outre
            // propre au contexte `Voice` qui porte le performer : une branche
            // de `<< … \\ … >>` ouvre un contexte neuf, si bien qu'une liaison
            // ouverte avant le `<<` y devient une nouvelle attaque
            // (`bach-prelude17`, `bwv855a`).
            let mut sequence: Vec<TieEvent> = Vec::new();
            for event in &voice.interpreted.events {
                let EventKind::Chord(chord) = &event.kind else {
                    continue;
                };
                let ChordContent::Notes(notes) = &chord.content else {
                    continue;
                };
                let Some(&(measure_index, segment_index)) = segments_by_tick.get(&event.tick)
                else {
                    continue;
                };
                let segment = &score.measures[measure_index].segments[segment_index];
                let ids: Vec<NoteId> = segment
                    .chord_rest(voice.track)
                    .and_then(ChordRest::chord_ref)
                    .map(|chord| chord.notes.clone())
                    .unwrap_or_default();
                let end = event.tick + chord.actual;
                let raccourci = if articulate {
                    None
                } else {
                    chord
                        .articulations
                        .iter()
                        .any(|articulation| {
                            matches!(
                                articulation,
                                ArticulationType::Staccato
                                    | ArticulationType::Spiccato
                                    | ArticulationType::SoftAccentStaccato
                                    | ArticulationType::SoftAccentTenutoStaccato
                            )
                        })
                        .then_some(Fraction::new(1, 2))
                        .or_else(|| {
                            chord
                                .articulations
                                .contains(&ArticulationType::Portato)
                                .then_some(Fraction::new(3, 4))
                        })
                };
                let end_midi = raccourci
                    .map(|facteur| event.tick + chord.actual * facteur)
                    .unwrap_or(end);
                for (note, id) in notes.iter().zip(ids) {
                    sequence.push((
                        event.tick,
                        end,
                        end_midi,
                        id,
                        note.tie,
                        chord.sim_chord,
                        chord.voice_context,
                        note.repeat_tie,
                    ));
                    toutes_notes.push((event.tick, id, note.pitch, note.tie));
                    toutes_notes_ctx.push((event.tick, id, note.pitch, chord.voice_context));
                    if note.repeat_tie {
                        repeat_ties.push((event.tick, id));
                    }
                }
            }
            sequence.sort_by_key(|(tick, end, _, _, _, _, _, _)| (*tick, *end));
            sequences.push(sequence);
            tie_waits.push(tie_wait);
        }

        let mut deja_lies: HashSet<NoteId> = HashSet::new();
        for voice_index in 0..staff.voices.len() {
            let sequence = &sequences[voice_index];
            let tie_wait = &tie_waits[voice_index];
            let wait_for_note = |tick: Fraction| {
                tie_wait
                    .iter()
                    .rev()
                    .find(|(start, _)| *start <= tick)
                    .is_some_and(|(_, on)| *on)
            };
            let mut instants: BTreeSet<Fraction> = segments_by_tick.keys().copied().collect();
            instants.extend(sequence.iter().map(|(tick, _, _, _, _, _, _, _)| *tick));
            // Une file par hauteur : un accord peut contenir deux têtes de
            // même hauteur (`<bis' bis>`, `tie-chord-broken-extremal.ly`),
            // qu'une simple entrée par hauteur écrasait ; chaque tête lie
            // alors la tête de même rang de l'accord suivant. Le contexte
            // `Voice` est porté par chaque tête : `heads_to_tie_` est propre
            // au `Tie_performer` de ce contexte (`lily/tie-performer.cc`
            // l. 41-48, 116-150 ; `ly/performer-init.ly` l. 100-109).
            let mut open: BTreeMap<u8, VecDeque<(NoteId, Fraction, Fraction, u32)>> =
                BTreeMap::new();
            let mut index = 0usize;
            for now in instants {
                let mut fin = index;
                while fin < sequence.len() && sequence[fin].0 < now {
                    fin += 1;
                }
                let debut = fin;
                while fin < sequence.len() && sequence[fin].0 == now {
                    fin += 1;
                }
                let wait = wait_for_note(now);
                for &(_, _, _, id, _, _, context, _) in &sequence[debut..fin] {
                    let pitch = score.note(id).pitch;
                    // Sans `tieWaitForNote`, seule la note liée qui se termine
                    // exactement ici peut être fermée. Avec, LilyPond cherche
                    // n'importe quelle tête pendante de même hauteur. Dans les
                    // deux cas la tête pendante appartient au même contexte
                    // `Voice` : un `<< … \\ … >>` crée un performer distinct,
                    // la liaison n'y traverse pas (`bach-prelude17`,
                    // `bwv855a`), mais deux têtes d'une branche ou d'un accord
                    // `<< … >>` sans `\\` partagent le leur.
                    let closes = open.get_mut(&pitch).and_then(|queue| {
                        queue
                            .iter()
                            .position(|&(_, end, _, left)| left == context && (end == now || wait))
                            .map(|position| queue.remove(position).expect("liaison à fermer"))
                    });
                    if let Some((previous, _, _, _)) = closes {
                        score.note_mut(previous).tie_forward = Some(id);
                        score.note_mut(id).tie_back = Some(previous);
                        deja_lies.insert(previous);
                        deja_lies.insert(id);
                    }
                }
                if !wait {
                    open.retain(|_, queue| {
                        queue.retain(|&(_, _, end_midi, _)| end_midi > now);
                        !queue.is_empty()
                    });
                }
                for &(_, end, end_midi, id, tie, _, context, _) in &sequence[debut..fin] {
                    if tie {
                        open.entry(score.note(id).pitch)
                            .or_default()
                            .push_back((id, end, end_midi, context));
                    }
                }
                index = fin;
            }
        }

        // Un `<< … >>` sans `\\` forme un accord dans la voix courante
        // (`lily/parser.yy:1581-1588`) : le lecteur répartit ses têtes en voix
        // sœurs pour la gravure, alors que LilyPond garde une seule voix. Une
        // liaison posée sur une de ces têtes peut donc être fermée par la suite
        // du flux, dans une autre voix. Ce rattrapage est **additif** : il ne
        // touche qu'une liaison encore pendante après le traitement normal, et
        // exige la même hauteur au tick exact de sa fin.
        for sequence in &sequences {
            for &(_, end, _, id, tie, sim_chord, context, _) in sequence {
                if !tie || !sim_chord || deja_lies.contains(&id) {
                    continue;
                }
                let pitch = score.note(id).pitch;
                let cible = sequences.iter().flat_map(|seq| seq.iter()).find_map(
                    |&(tick, _, _, other, _, _, other_context, _)| {
                        (tick == end
                            && other_context == context
                            && !deja_lies.contains(&other)
                            && score.note(other).pitch == pitch)
                            .then_some(other)
                    },
                );
                if let Some(cible) = cible {
                    score.note_mut(id).tie_forward = Some(cible);
                    score.note_mut(cible).tie_back = Some(id);
                    deja_lies.insert(id);
                    deja_lies.insert(cible);
                }
            }
        }

        // Une liaison peut aussi être ouverte dans une voix et fermée par
        // une **autre** voix du même contexte `Voice` : le lecteur répartit
        // ces voix en pistes sœurs pour la gravure, alors que LilyPond garde
        // un seul `Tie_performer` (`lily/tie-performer.cc:41-48`). La file de
        // liaisons étant tenue par voix (`link_ties` ci-dessus), la tête
        // droite tombait sinon dans une voix déjà traitée et la note était
        // réattaquée (`bwv869b.ly`, mesure 23). On exige le même identifiant
        // de contexte pour ne pas traverser un `<< … \\ >>`, qui ouvre un
        // performer distinct (`bach-prelude17.ly`, `bwv855a.ly`).
        for (voice_index, sequence) in sequences.iter().enumerate() {
            for &(_, end, _, id, tie, sim_chord, context, _) in sequence {
                if !tie || sim_chord || deja_lies.contains(&id) {
                    continue;
                }
                let pitch = score.note(id).pitch;
                // La recherche globale (au-delà de la portée courante) ne
                // s'applique qu'au cas où le `\midi` a distribué les voix en
                // autant de portées : sinon un pas de temps d'une autre
                // portée doit pouvoir rompre la liaison
                // (`staccato_relache_la_liaison_…`).
                let cible = if midi_voices_as_staves {
                    toutes_notes_ctx.iter().find_map(
                        |&(tick, other, other_pitch, other_context)| {
                            (tick == end
                                && other != id
                                && other_context == context
                                && other_pitch == pitch
                                && !deja_lies.contains(&other)
                                && score.note(other).tie_back.is_none())
                            .then_some(other)
                        },
                    )
                } else {
                    sequences.iter().enumerate().find_map(|(index, seq)| {
                        if index == voice_index {
                            return None;
                        }
                        seq.iter().find_map(|&(tick, _, _, other, _, _, other_context, _)| {
                            (tick == end
                                && other_context == context
                                && !deja_lies.contains(&other)
                                && score.note(other).pitch == pitch)
                                .then_some(other)
                        })
                    })
                };
                if let Some(cible) = cible {
                    score.note_mut(id).tie_forward = Some(cible);
                    score.note_mut(cible).tie_back = Some(id);
                    deja_lies.insert(id);
                    deja_lies.insert(cible);
                }
            }
        }
    }

    // Une liaison portée par un silence ne crée pas d'attaque : elle relie
    // les notes de même hauteur aux deux bouts dans les autres branches du
    // même contexte (`volta-spec-tie.ly`).
    for (debut, fin) in &rest_ties {
        let Some((_, source, pitch, _)) = toutes_notes
            .iter()
            .find(|(tick, id, _, _)| *tick == *debut && score.note(*id).tie_forward.is_none())
        else {
            continue;
        };
        let Some((_, cible, _, _)) = toutes_notes.iter().find(|(tick, id, p, _)| {
            *tick == *fin && *p == *pitch && *id != *source && score.note(*id).tie_back.is_none()
        }) else {
            continue;
        };
        score.note_mut(*source).tie_forward = Some(*cible);
        score.note_mut(*cible).tie_back = Some(*source);
    }

    // `\repeatTie` (`ly/declarations-init.ly:104`) ne supprime une attaque
    // que s'il existe une liaison encore pendante à fermer : sans elle,
    // LilyPond rejoue la note (`repeat-tie.ly`, `vshape.ly`). On ne relie
    // donc qu'à une tête de `~` restée sans but dans `link_ties`, toutes
    // voix et portées confondues puisque l'alternative peut changer de
    // voix (`entertainer.ly`). L'écriture MusicXML produit alors
    // `<tie type="stop"/>`, que l'oracle ne compte pas comme une attaque.
    let mut deja_relies: HashSet<NoteId> = HashSet::new();
    for (tick, id) in &repeat_ties {
        if score.note(*id).tie_back.is_some() {
            continue;
        }
        let pitch = score.note(*id).pitch;
        let Some((_, previous, _, _)) = toutes_notes
            .iter()
            .filter(|(t, other, p, pending)| {
                *t < *tick
                    && *p == pitch
                    && *pending
                    && *other != *id
                    && !deja_relies.contains(other)
                    && score.note(*other).tie_forward.is_none()
            })
            .max_by_key(|(t, _, _, _)| *t)
        else {
            continue;
        };
        score.note_mut(*previous).tie_forward = Some(*id);
        score.note_mut(*id).tie_back = Some(*previous);
        deja_relies.insert(*previous);
        deja_relies.insert(*id);
    }
}
