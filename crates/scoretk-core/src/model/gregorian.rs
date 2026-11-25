// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la syntaxe GABC de Gregorio (GPL-3.0-or-later) : `doc/Gabc.tex`,
// `src/gabc/gabc-score-determination.{y,l}` et `src/struct.h`, modifié
// (réécrit en Rust) en 2026. Chaque règle porte sa référence de section.
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

//! L6 payload for Gregorian chant (GABC).
//!
//! The syllable/neume stream is the **primary** data of GABC: the modern
//! projection into L0/L1 (one `Note` per pitch letter, an invented duration
//! and nominal measures) is derived and flagged as such
//! (`docs/agents/model-modularisation.md` §7.1). GABC has no metre, no measure
//! numbering and no fixed durations (`Gabc.tex:801-845`): separation bars
//! divide *sections*, not metric measures.
//!
//! This module is pure data: no I/O, no clock, no dependency, so the core
//! stays portable to `wasm32-unknown-unknown`.

use super::{
    Accidental, AccidentalType, BarLineType, Breath, BreathType, Chord, ChordRest, ChordRestKind,
    Clef, ClefType, Duration, DurationType, Fraction, Layers, Lyrics, LyricsSyllabic, Measure,
    Note, NoteSource, Ornament, OrnamentType, Part, Placement, Score, Segment, SegmentItem,
    SegmentType, Track,
};

/// One header field, in source order (`Gabc.tex:67-154`).
///
/// Multi-line values keep their embedded newlines; the terminating `;;`
/// (`:69-74`) is not part of the value.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GregorianHeader {
    /// Header name (`name`, `language`, `mode`, `office-part`, …).
    pub name: String,
    /// Header value as written between `:` and the final `;`.
    pub value: String,
}

/// A separation bar between sections (`Gabc.tex:801-821`).
///
/// `::` is the divisio finalis, not two divisio maiores (`:815-817`,
/// `src/dump/dump.c:805-821`): the variants are therefore parsed longest
/// first by [`BarKind::parse_prefix`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarKind {
    /// Virgula `` ` `` (`:808`).
    Virgula,
    /// Virgula on the ledger line above the staff `` `0 `` (`:809`).
    VirgulaLedger,
    /// Divisio minimis `^` / `^0` (`:810-811`).
    DivisioMinimis,
    /// Divisio minima `,` (`:812`).
    DivisioMinima,
    /// Divisio minima on the ledger line above `,0` (`:813`).
    DivisioMinimaLedger,
    /// Divisio minor `;` (`:814`).
    DivisioMinor,
    /// Divisio maior `:` (`:815`).
    DivisioMaior,
    /// Dotted divisio maior `:?` (`:816`).
    DivisioMaiorDot,
    /// Divisio finalis `::` (`:817`).
    DivisioFinalis,
    /// Dominican bar `;1` … `;8` (`:818-820`).
    Dominican(u8),
}

impl BarKind {
    /// Parses one bar at the beginning of `code`, returning the bar and the
    /// number of source characters it occupies.
    ///
    /// Two-character forms are tried before one-character forms so that `::`
    /// is one final bar and `;1` one Dominican bar (`Gabc.tex:815-820`).
    /// The optional `'` (vertical episema) and `_` (bar brace) decorations
    /// (`:823-830`) are handled by the caller, not by the bar token itself.
    pub fn parse_prefix(code: &str) -> Option<(Self, usize)> {
        if code.starts_with("::") {
            return Some((Self::DivisioFinalis, 2));
        }
        if code.starts_with(":?") {
            return Some((Self::DivisioMaiorDot, 2));
        }
        if code.starts_with("`0") {
            return Some((Self::VirgulaLedger, 2));
        }
        if code.starts_with(",0") {
            return Some((Self::DivisioMinimaLedger, 2));
        }
        if code.starts_with("^0") {
            return Some((Self::DivisioMinimis, 2));
        }
        let bytes = code.as_bytes();
        // Gregorio's lexer accepts `[,;][1-8]` as a Dominican bar
        // (`src/gabc/gabc-notes-determination.l:1246-1248`), although
        // `Gabc.tex:818-820` documents only `;n`.
        if bytes.len() >= 2 && matches!(bytes[0], b';' | b',') && (b'1'..=b'8').contains(&bytes[1])
        {
            return Some((Self::Dominican(bytes[1] - b'0'), 2));
        }
        match bytes.first().copied()? {
            b'`' => Some((Self::Virgula, 1)),
            b'^' => Some((Self::DivisioMinimis, 1)),
            b',' => Some((Self::DivisioMinima, 1)),
            b';' => Some((Self::DivisioMinor, 1)),
            b':' => Some((Self::DivisioMaior, 1)),
            _ => None,
        }
    }

    /// Canonical GABC spelling of the bar, used as the inventory value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Virgula => "`",
            Self::VirgulaLedger => "`0",
            Self::DivisioMinimis => "^",
            Self::DivisioMinima => ",",
            Self::DivisioMinimaLedger => ",0",
            Self::DivisioMinor => ";",
            Self::DivisioMaior => ":",
            Self::DivisioMaiorDot => ":?",
            Self::DivisioFinalis => "::",
            Self::Dominican(1) => ";1",
            Self::Dominican(2) => ";2",
            Self::Dominican(3) => ";3",
            Self::Dominican(4) => ";4",
            Self::Dominican(5) => ";5",
            Self::Dominican(6) => ";6",
            Self::Dominican(7) => ";7",
            Self::Dominican(8) => ";8",
            Self::Dominican(_) => ";",
        }
    }
}

/// Family of a symbol appended to a note or a bar (`Gabc.tex:702-797`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignKind {
    /// Punctum mora `.` / `.0` / `.1` / `..` (`:742-755`).
    PunctumMora,
    /// Vertical episema / ictus `'` / `'0` / `'1` (`:757-768`).
    Ictus,
    /// Horizontal episema `_` with numeric modifiers (`:769-797`).
    HorizontalEpisema,
    /// Oriscus `o` / `O` (`:469,567`).
    Oriscus,
    /// Quilisma `w` (`:470`).
    Quilisma,
    /// Virga `v` / virga reversa `V` (`:471-472`).
    Virga,
    /// Stropha `s` (`:473`).
    Stropha,
    /// Liquescent deminutus `~`, augmented `<`, diminished `>` (`:474-476`).
    Liquescentia,
    /// Linea `=` (`:477`).
    Linea,
    /// Cavum `r` / `r0` (`:478-480`).
    Cavum,
    /// Accent, circle or musica ficta `r1` … `r8` (`:717-724`).
    Accent,
    /// Flat `x` / `X`, sharp `#` / `##`, natural `y` / `Y` (`:481-489`).
    Alteration,
    /// Any other shape character, kept as raw glyph (`:556-579`).
    Other,
}

/// One neume sign attached to a note or a bar.
///
/// The `raw` spelling is kept: placement digits and fusion are part of the
/// source and a lossy projection must be able to report them (`§7.1` of the
/// modularisation plan).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeumeSign {
    /// Semantic family, for reporting and inventory.
    pub kind: SignKind,
    /// Exact source characters of the sign.
    pub raw: String,
}

impl NeumeSign {
    /// Builds a sign from a family and its source spelling.
    pub fn new(kind: SignKind, raw: impl Into<String>) -> Self {
        Self {
            kind,
            raw: raw.into(),
        }
    }
}

/// One pitch letter of a neume (`Gabc.tex:408-443`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GregorianNote {
    /// Typographical staff position `a`–`n` or `p`, **not** a concert pitch
    /// name (`:410-413`); upper case is a punctum inclinatum (`:454-468`).
    pub letter: char,
    /// Neume glyphs appended to the letter (`:452-579,603-700`), including
    /// shape hints and fusion markers. Empty for a plain punctum.
    pub glyph: String,
    /// Rhythmic and shape signs attached to this note (`:702-797`).
    pub signs: Vec<NeumeSign>,
    /// Décalage UTF-16 de la lettre chantée depuis le début du fichier GABC.
    ///
    /// Additif : `None` pour une note reconstruite sans source textuelle.
    pub source_offset: Option<usize>,
    /// Décalage UTF-16 juste après le jeton source de la note.
    pub source_end: Option<usize>,
}

/// One syllable and the neumes it carries (`Gabc.tex:228-317`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Syllable {
    /// Text outside parentheses, markup included (`:241-245,254-309`).
    pub text: String,
    /// Clef token opening this group, when one occurs here
    /// (`:847-870`). `Gregorian::clefs` keeps the source-order list; this
    /// per-syllable slot is what lets the writer place a *clef change* at the
    /// right point instead of inventing one.
    pub clef: Option<String>,
    /// Special-character markup such as `<sp>R/</sp>` (`:297-309,345-378`).
    pub rubric: Option<String>,
    /// Texte au-dessus de la portée `[alt:…]` (`:1088-1091`), sans les
    /// crochets ni le préfixe. Champ L6 atomique : le writer le réémet.
    pub above_text: Option<String>,
    /// Pitch letters of the parenthesised groups, in source order.
    pub notes: Vec<GregorianNote>,
    /// Signs attached to the groups rather than to one note.
    pub signs: Vec<NeumeSign>,
    /// Separation bars found in the groups (`:801-845`).
    pub bars: Vec<BarKind>,
    /// A space or an end of line followed the last `)` (`:60-61,241-245`).
    ///
    /// Spaces are music in GABC: this flag is part of the primary data even
    /// though the derived modern view ignores it.
    pub space_after: bool,
}

/// L6 payload of a GABC score.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gregorian {
    /// Header fields in source order (`Gabc.tex:67-154`).
    pub headers: Vec<GregorianHeader>,
    /// Number of staff lines, 2–5, default 4 (`:128,410-443`).
    pub staff_lines: u8,
    /// Clef tokens in source order, e.g. `c3`, `f4`, `cb3`, `c1@c4`
    /// (`:847-870`).
    pub clefs: Vec<String>,
    /// Primary data: syllables, their neumes and their separation bars.
    ///
    /// This is the **only** source the writer uses. No raw header or notation
    /// text is kept: every emitted token is rebuilt from these fields (an
    /// atomic GABC spelling such as a sign's `raw` or a clef token is the one
    /// deliberate exception, justified in `docs/agents/gabc.md` §8).
    pub syllables: Vec<Syllable>,
    /// True when the L0/L1 event spine was generated from this payload.
    ///
    /// GABC carries neither metrical measures nor fixed durations
    /// (`:801-845`); one `Note` per pitch letter and nominal measures are a
    /// *derived projection*, never source data (plan §7.1).
    pub derived_modern_view: bool,
}

impl Gregorian {
    /// Rebuilds the derived modern-notation view (L0/L1) from this payload.
    ///
    /// GABC has no metre, measure numbering or fixed durations
    /// (`Gabc.tex:801-845`); the one-quarter-per-pitch-letter spine is a
    /// deliberate projection, not source data. The core owns this projection
    /// so both the GABC reader and the MusicXML reader can rebuild the *same*
    /// view when they carry the L6 payload. It is flagged in `Score::report`
    /// and in [`Gregorian::derived_modern_view`].
    ///
    /// Portage de `src/struct.c:1273-1288` (`initial_key`) et
    /// `src/struct.h:809-813` (hauteurs), modifié (réécrit en Rust) en 2026.
    ///
    /// **Altérations** (`Gabc.tex:481-489,512-540`). GregorioTeX traite
    /// `x`/`X` (bémol), `#`/`##` (dièse) et `y`/`Y` (naturel) comme des
    /// « neumes d'une note » : ce ne sont pas des notes chantées
    /// (`:514-515`) et l'effet d'une altération est réglé par
    /// `\gresetalterationeffect` (`Command_Index_User.tex:1490-1504`), dont
    /// la valeur par défaut est `line` ; les options `word` et `bar` y sont
    /// décrites comme *à venir*, jamais implémentées. Gregorio ne propage
    /// donc aucune alteration au niveau MIDI. La projection moderne retenue ici
    /// applique l'altération aux notes suivantes **de la même lettre**
    /// jusqu'à la fin du mot, à la prochaine barre de séparation ou à un
    /// naturel (`x`, `y`, `#` et leurs variantes douces/parenthesées), ce qui
    /// est la convention chantée et l'attendu du consommateur Cantus ; elle
    /// est documentée comme choix dérivé dans `docs/agents/gabc.md`.
    ///
    /// Un glyphe d'alteration n'étant pas une attaque, l'alteration **affichée**
    /// est reportée sur la première note chantée suivante de la même lettre :
    /// `gx g` projette un `g` avec `AccidentalType::Flat`, donc
    /// `<alter>-1</alter>` **et** `<accidental>flat</accidental>` en MusicXML.
    /// Les notes suivantes sous la même portée gardent l'<alter> sans signe
    /// affiché ; un `y`/`Y` suivant est un naturel visible qui annule la
    /// portée. XSD 4.0 distingue en effet `<pitch>/<alter>` (hauteur sonore)
    /// de `<accidental>` (signe réellement noté), tous deux nécessaires à
    /// Verovio pour dessiner le bémol.
    ///
    /// **Avertissement de fin de mot** (`docs/agents/gabc.md` §7). La portée
    /// GABC s'arrête à la fin du mot (`Gabc.tex:241-245`), mais une altération
    /// moderne reste valable jusqu'à la barre de mesure. Quand un mot se
    /// termine *sans* barre moderne, la prochaine note chantée d'une lettre
    /// dont l'altération sonore diffère de celle encore affichée dans la
    /// mesure porte un signe d'avertissement (naturel le plus souvent). Ce
    /// signe est marqué « utilisateur » sans crochets : il s'exporte en
    /// `cautionary="yes" parentheses="no"`, et l'inverse GABC ne doit jamais
    /// le réémettre en glyphe `y`.
    #[must_use]
    pub fn build_derived_score(&self) -> Score {
        let mut score = Score::new();
        let clef = self.clefs.first().map_or("c4", String::as_str);
        let part = score.add_part(Part::new("Gregorian", "Gre"));
        let staff = score.add_staff(part);
        score.staves[staff.index()].lines = self.staff_lines;
        let track = Track::new(staff, 0);

        // `Gabc.tex:241-245,314-317` : un espace après la dernière syllabe
        // termine le mot. Le `<syllabic>` MusicXML se déduit donc du groupe
        // de syllabes, pas de la seule syllabe courante.
        let syllabics = self.syllabics();
        // Une entrée par lettre de hauteur (`a`…`n`, `p` ; `o` est l'oriscus).
        let mut alterations = [0i8; 16];
        // Signe affiché en attente pour la première note chantée de chaque
        // lettre après un glyphe d'alteration (`Gabc.tex:512-540`).
        let mut displayed = [None::<Accidental>; 16];
        // Dernière altération *affichée* pour chaque lettre dans la mesure
        // moderne en cours. Contrairement à `displayed`, elle ne s'efface pas
        // à la fin d'un mot GABC : seule une vraie barre moderne la remet à
        // zéro (`docs/agents/gabc.md` §7).
        let mut measure_displayed = [None::<i8>; 16];
        let mut start = Fraction::ZERO;
        let mut cursor = Fraction::ZERO;
        let mut measure_index = 0usize;
        let mut measure: Option<Measure> = None;
        for (index, syllable) in self.syllables.iter().enumerate() {
            let has_sung = syllable
                .notes
                .iter()
                .any(|note| alteration_value(note).is_none());
            if (has_sung || !syllable.bars.is_empty()) && measure.is_none() {
                let mut opened = Measure::new(measure_index, start, Fraction::ONE);
                if measure_index == 0 {
                    let mut segment = Segment::new(SegmentType::Clef, Fraction::ZERO, 1);
                    segment
                        .set_element(track, SegmentItem::Clef(Clef::new(derived_clef_type(clef))));
                    opened.add_segment(segment);
                }
                measure = Some(opened);
            }
            let mut syllable_sung = false;
            for (note_index, note) in syllable.notes.iter().enumerate() {
                if let Some(value) = alteration_value(note) {
                    // L'alteration est un glyphe, pas une attaque MIDI.
                    if let Some(slot) = letter_slot(note.letter) {
                        alterations[slot] = value;
                        // `x`/`X`, `y`/`Y`, `#`/`##` et leurs formes
                        // parenthesées portent le signe à dessiner : on le
                        // remet à la première note chantée de cette lettre.
                        displayed[slot] = alteration_accidental(note, value);
                    }
                    continue;
                }
                let step = derived_step(note.letter, clef);
                let alter = letter_slot(note.letter).map_or(0, |slot| alterations[slot]);
                let pitch = (i16::from(derived_pitch(step)) + i16::from(alter)).clamp(0, 127) as u8;
                let tpc = (i16::from(derived_tpc(step)) + 7 * i16::from(alter)).clamp(0, 26) as i8;
                let mut projected = Note::new(pitch, tpc);
                // Le glyphe n'est pas une note : c'est la première note
                // chantée de la lettre qui porte le signe visible. Les notes
                // suivantes sous la même portée gardent `<alter>` sans signe.
                if let Some(slot) = letter_slot(note.letter) {
                    let mut sign = displayed[slot].take();
                    if sign.is_none() && measure_displayed[slot].is_some_and(|shown| shown != alter)
                    {
                        // Fin de mot GABC sans barre moderne : l'ancien signe
                        // reste lu dans la mesure alors que la hauteur sonore
                        // a changé. Avertissement nécessaire (XSD 4.0
                        // `cautionary="yes"`, jamais un glyphe GABC `y`).
                        sign = courtesy_accidental(alter);
                    }
                    if sign.is_some() {
                        measure_displayed[slot] = Some(alter);
                    }
                    projected.accidental = sign;
                }
                projected.source = Some(NoteSource {
                    offset: note.source_offset.unwrap_or(0),
                    end: note.source_end.unwrap_or(0),
                    gregorian_syllable: index,
                    gregorian_note: note_index,
                });
                let id = score.add_note(projected);
                // `Gabc.tex:742-797` : le punctum mora double la note,
                // l'épisème horizontal l'allonge d'un facteur 1,5 (choix de
                // projection documenté). Rien d'autre dans GABC n'est une
                // durée fixe (`:801-845`).
                let duration = derived_duration(note);
                let mut chord_rest = ChordRest::chord(duration);
                // Un quilisma est vu comme un ornement (`gabctk.py:1051-1095`
                // : `\prall` en LilyPond, `P` en ABC) et non comme une durée.
                // L'ornement est donc porté par la projection moderne et
                // traverse MusicXML dans `<ornaments><inverted-mordent/>`.
                let mut chord = Chord {
                    notes: vec![id],
                    ..Chord::default()
                };
                if note
                    .signs
                    .iter()
                    .any(|sign| sign.kind == SignKind::Quilisma)
                {
                    chord
                        .ornaments
                        .push(Ornament::new(OrnamentType::InvertedMordent));
                }
                chord_rest.kind = ChordRestKind::Chord(chord);
                // Les notes avant un quilisma sont retenues progressivement ;
                // `ticks` porte la durée réelle quand elle diffère de la
                // figure notée (`ChordRest::ticks`, convention MuseScore).
                let held = quilisma_hold(syllable, note_index);
                if held != Fraction::ONE {
                    // La retenue est une durée d'exécution : elle ne décale
                    // pas la grille d'attaques déjà inventée par la projection
                    // (une noire par lettre), sinon un aller-retour par du
                    // MusicXML standard perdrait toutes les mesures dérivées.
                    chord_rest.ticks = Some(duration.ticks() * held);
                }
                // Segment ticks are absolute in the pivot (`pass2.rs:495-518`),
                // not relative to the measure.
                let tick = start + cursor;
                if !syllable_sung {
                    let text = plain_lyric(&syllable.text);
                    if !text.is_empty() {
                        chord_rest
                            .lyrics
                            .push(Lyrics::new(0, syllabics[index], text));
                    }
                    syllable_sung = true;
                }
                let mut segment = Segment::new(SegmentType::ChordRest, tick, 1);
                segment.set_element(track, SegmentItem::ChordRest(chord_rest));
                measure
                    .as_mut()
                    .expect("a sung note opens a measure")
                    .add_segment(segment);
                cursor += duration.ticks();
            }
            // Portée de l'alteration : fin de mot ou barre de séparation
            // (`Gabc.tex:801-845`, `:241-245`). Un naturel la cancelle déjà
            // quand il est rencontré comme note alterée ci-dessus.
            if syllable.space_after || !syllable.bars.is_empty() {
                alterations = [0; 16];
                // Aucune note de la portée suivante ne doit hériter d'un signe
                // dont le glyphe n'a pas été répété après la barre/le mot.
                displayed = [None; 16];
            }
            // Une mesure dérivée correspond à une **barre réelle** du `.gabc`
            // (`,` `;` `:` `::`, `Gabc.tex:801-845`), pas à un groupe de
            // syllabes : les syllables sans barre s'accumulent dans la même
            // mesure. Plusieurs barres dans un même groupe ouvrent plusieurs
            // mesures (l'ordre exact des barres à l'intérieur du groupe est
            // une donnée L6 hors de portée, documentée en §10).
            for bar in &syllable.bars {
                let opened = measure
                    .take()
                    .unwrap_or_else(|| Measure::new(measure_index, start, Fraction::ONE));
                let mut opened = opened.with_len(cursor.max(Fraction::new(1, 4)));
                opened.end_bar_line = derived_bar_line(*bar);
                // Une barre de séparation est une respiration : le MIDI
                // insère une pause (0,5 s, choix de projection documenté)
                // et le MusicXML écrit `<breath-mark/>` sur la dernière note.
                let mut breath = Segment::new(SegmentType::Breath, opened.end_tick(), 1);
                breath.set_element(
                    track,
                    SegmentItem::Breath(Breath {
                        kind: BreathType::Comma,
                        placement: Placement::Above,
                        pause_ms: Some(500),
                    }),
                );
                opened.add_segment(breath);
                start = opened.end_tick();
                score.add_measure(opened);
                measure_index += 1;
                cursor = Fraction::ZERO;
                // Une barre moderne réinitialise la portée des altérations
                // affichées : après elle, plus aucun avertissement n'est dû.
                measure_displayed = [None; 16];
            }
        }
        if let Some(opened) = measure {
            let opened = opened.with_len(cursor.max(Fraction::new(1, 4)));
            score.add_measure(opened);
        }

        score.report.record(
            Layers::COMMON,
            "derived-modern-view: one note per sung letter, bars as measures, unmetered quarter durations",
            None,
            None,
        );
        score
    }

    /// `<syllabic>` de chaque syllabe chantée (`Gabc.tex:241-245,314-317`).
    ///
    /// Un mot est une suite de syllabes se terminant par `space_after` ; les
    /// groupes qui ne contiennent que des barres n'en font pas partie.
    fn syllabics(&self) -> Vec<LyricsSyllabic> {
        let mut result = vec![LyricsSyllabic::Single; self.syllables.len()];
        let mut word: Vec<usize> = Vec::new();
        let assign = |word: &[usize], result: &mut Vec<LyricsSyllabic>| {
            for (position, index) in word.iter().enumerate() {
                result[*index] = if word.len() == 1 {
                    LyricsSyllabic::Single
                } else if position == 0 {
                    LyricsSyllabic::Begin
                } else if position + 1 == word.len() {
                    LyricsSyllabic::End
                } else {
                    LyricsSyllabic::Middle
                };
            }
        };
        for (index, syllable) in self.syllables.iter().enumerate() {
            if syllable.text.trim().is_empty()
                && syllable
                    .notes
                    .iter()
                    .all(|note| alteration_value(note).is_some())
            {
                continue;
            }
            word.push(index);
            if syllable.space_after {
                assign(&word, &mut result);
                word.clear();
            }
        }
        assign(&word, &mut result);
        result
    }
}

/// Facteurs d'allongement des notes *précédant* un quilisma sur la même
/// syllabe (`gabctk.py:35` `DUREE_AVANT_QUILISMA`, `:1051-1059` ; décision de
/// l'utilisateur du 2026-11-25). Le facteur à distance `k` est `1 + 2^-k` :
/// la note immédiatement avant le quilisma est doublée (`k = 0`), la suivante
/// vers l'arrière est multipliée par 1,5 (`k = 1`), puis 1,25, 1,125, …
///
/// Ces facteurs sont des choix de projection moderne, au même titre que le
/// punctum mora (×2) et l'épisème horizontal (×1,5) décrits ci-dessous ; GABC
/// n'a aucune durée fixe (`Gabc.tex:801-845`). Voir `docs/agents/gabc.md` §7.
pub const QUILISMA_HOLD_IMMEDIATE: Fraction = Fraction::from_integer(2);
/// distance 1 : 1 + 1/2.
pub const QUILISMA_HOLD_SECOND: Fraction = Fraction::from_ratio(3, 2);
/// distance 2 : 1 + 1/4.
pub const QUILISMA_HOLD_THIRD: Fraction = Fraction::from_ratio(5, 4);
/// distance 3 : 1 + 1/8.
pub const QUILISMA_HOLD_FOURTH: Fraction = Fraction::from_ratio(9, 8);
/// Les quatre premiers facteurs, pour les syllabes courantes. Au-delà,
/// [`quilisma_hold_factor`] prolonge la même série.
pub const QUILISMA_HOLD_FACTORS: [Fraction; 4] = [
    QUILISMA_HOLD_IMMEDIATE,
    QUILISMA_HOLD_SECOND,
    QUILISMA_HOLD_THIRD,
    QUILISMA_HOLD_FOURTH,
];

/// Facteur `1 + 2^-distance` appliqué à une note avant un quilisma.
fn quilisma_hold_factor(distance: usize) -> Fraction {
    if let Some(factor) = QUILISMA_HOLD_FACTORS.get(distance) {
        return *factor;
    }
    // Au-delà de 16 notes chantées, l'écart est déjà sous la précision d'une
    // figure notée : on plafonne la série au lieu de déborder les `i64`.
    let exponent = (distance as u32).min(16);
    let denominator = 1i64 << exponent;
    Fraction::new(denominator + 1, denominator)
}

/// Facteur d'allongement d'une note avant un quilisma de la même syllabe.
/// Un quilisma n'atteint jamais la syllabe précédente : la marche arrière
/// s'arrête à la note courante (`Gabc.tex:228-245`). Les alterations ne sont
/// pas des notes chantées (`:512-540`) et ne comptent donc pas dans la
/// distance. En cas de plusieurs quilismas, le facteur le plus grand gagne.
/// Calculé sans allocation, ce qui garde la projection légère en wasm.
fn quilisma_hold(syllable: &Syllable, note_index: usize) -> Fraction {
    let mut factor = Fraction::ONE;
    for (quilisma, note) in syllable.notes.iter().enumerate() {
        if quilisma <= note_index
            || !note
                .signs
                .iter()
                .any(|sign| sign.kind == SignKind::Quilisma)
        {
            continue;
        }
        let mut distance = 0;
        for index in (note_index + 1..quilisma).rev() {
            if alteration_value(&syllable.notes[index]).is_none() {
                distance += 1;
            }
        }
        let candidate = quilisma_hold_factor(distance);
        if candidate > factor {
            factor = candidate;
        }
    }
    factor
}

/// Durée projetée d'une note (`Gabc.tex:742-797`).
///
/// La noire est la durée de base. Le punctum mora `.` la double (blanche),
/// l'épisème horizontal `_` la multiplie par **1,5** (noire pointée, donc un
/// `<dot/>` MusicXML) ; les deux ensemble donnent une blanche pointée. Ce
/// facteur est un choix de projection : GABC n'a aucune durée fixe
/// (`:801-845`) et Gregorio ne stocke que les signes.
fn derived_duration(note: &GregorianNote) -> Duration {
    let mora = note
        .signs
        .iter()
        .any(|sign| sign.kind == SignKind::PunctumMora);
    let episema = note
        .signs
        .iter()
        .any(|sign| sign.kind == SignKind::HorizontalEpisema);
    match (mora, episema) {
        (true, true) => Duration::with_dots(DurationType::Half, 1),
        (true, false) => Duration::half(),
        (false, true) => Duration::with_dots(DurationType::Quarter, 1),
        (false, false) => Duration::quarter(),
    }
}

/// Valeur diatonique d'un signe d'alteration GABC (`Gabc.tex:481-489`).
///
/// Les formes douces (`X`, `##`, `Y`) et parenthesées (`x?`, `#?`, `y?`)
/// changent seulement l'impression dans GregorioTeX ; la hauteur projetée
/// est la même que celle de la forme ordinaire.
fn alteration_value(note: &GregorianNote) -> Option<i8> {
    let sign = note
        .signs
        .iter()
        .find(|sign| sign.kind == SignKind::Alteration)?;
    match sign.raw.as_str() {
        "x" | "x?" | "X" => Some(-1),
        "y" | "y?" | "Y" => Some(0),
        "#" | "#?" | "##" => Some(1),
        _ => None,
    }
}

/// Signe affiché correspondant à un glyphe d'alteration GABC.
///
/// `x`/`X` → bémol, `y`/`Y` → bécarre, `#`/`##` → dièse
/// (`Gabc.tex:481-489`). Les formes `x?`/`y?`/`#?` sont parenthesées
/// (`:484-486`) et le restent dans MusicXML via `parentheses="yes"`. Comme
/// toute altération GABC est un glyphe distinct de la note chantée
/// (`:512-540`), le signe est *visible* : XSD 4.0 `<accidental>` décrit
/// l'altération réellement notée, distincte du `<alter>` sonore.
fn alteration_accidental(note: &GregorianNote, value: i8) -> Option<Accidental> {
    let sign = note
        .signs
        .iter()
        .find(|sign| sign.kind == SignKind::Alteration)?;
    let kind = match value {
        -1 => AccidentalType::Flat,
        0 => AccidentalType::Natural,
        1 => AccidentalType::Sharp,
        _ => return None,
    };
    let mut accidental = Accidental::new(kind);
    accidental.parentheses = sign.raw.ends_with('?');
    Some(accidental)
}

/// Signe d'avertissement de la projection moderne, quand la portée GABC
/// s'arrête au mot alors qu'aucune barre de mesure ne sépare encore la note
/// suivante : l'altération déjà affichée dans la mesure continue d'être lue,
/// mais la hauteur sonore a changé (`docs/agents/gabc.md` §7).
///
/// Le rôle « utilisateur » sans crochets ni parenthèses est exporté par
/// `scoretk-musicxml-export` en `cautionary="yes" parentheses="no"`. Ce marquage
/// distingue le signe ajouté par la projection d'un vrai glyphe GABC :
/// l'inverse MusicXML → GABC ne doit pas le réémettre, en particulier pas
/// sous forme de `y` (natures GABC, `Gabc.tex:483`).
fn courtesy_accidental(value: i8) -> Option<Accidental> {
    let kind = match value {
        -1 => AccidentalType::Flat,
        0 => AccidentalType::Natural,
        1 => AccidentalType::Sharp,
        _ => return None,
    };
    let mut accidental = Accidental::new(kind);
    accidental.user = true;
    Some(accidental)
}

/// Index d'une lettre de hauteur pour la table d'alterations (`o` exclu).
fn letter_slot(letter: char) -> Option<usize> {
    let low = letter.to_ascii_lowercase();
    match low {
        'a'..='n' => Some((low as u8 - b'a') as usize),
        'p' => Some(15),
        _ => None,
    }
}

/// Removes GABC typography to get the sung word
/// (`Gabc.tex:247-309,314-343`).
fn plain_lyric(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut stack: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '<' => {
                if let Some(end) = chars[index..].iter().position(|char| *char == '>') {
                    let end = index + end;
                    let tag: String = chars[index + 1..end].iter().collect();
                    let name = tag
                        .trim_start_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if tag.starts_with('/') {
                        stack.pop();
                    } else {
                        stack.push(name);
                    }
                    index = end + 1;
                    continue;
                }
                index += 1;
            }
            '$' => {
                index += 1;
                if index < chars.len() {
                    out.push(chars[index]);
                    index += 1;
                }
            }
            '{' | '}' => index += 1,
            char => {
                if stack.iter().all(|tag| {
                    matches!(
                        tag.as_str(),
                        "b" | "c" | "i" | "sc" | "tt" | "ul" | "e" | "eu" | "nlba"
                    )
                }) {
                    out.push(char);
                }
                index += 1;
            }
        }
    }
    out.trim().to_owned()
}

/// Clef key of a GABC token (`src/struct.c:1273-1288`).
fn derived_clef_key(token: &str) -> i32 {
    let first = token.split('@').next().unwrap_or(token);
    let line = i32::from(
        first
            .bytes()
            .rev()
            .find(|byte| byte.is_ascii_digit())
            .unwrap_or(b'4')
            - b'0',
    );
    if first.starts_with('f') {
        2 * line - 4
    } else {
        2 * line - 1
    }
}

/// Staff height of a letter, `a` = 3 (`src/struct.h:809-813`; `dump.c:78-88`).
fn derived_height(letter: char) -> i32 {
    let low = letter.to_ascii_lowercase();
    let base = (low as i32) - ('a' as i32) + 3;
    if low > 'o' {
        base + 1
    } else {
        base
    }
}

/// Diatonic step (C = 0) of a pitch letter under a clef.
fn derived_step(letter: char, clef: &str) -> i32 {
    derived_height(letter) - derived_clef_key(clef) - 5
}

/// Concert MIDI pitch of a derived step (step 0 = middle C).
fn derived_pitch(step: i32) -> u8 {
    const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    (60 + SEMITONES[step.rem_euclid(7) as usize] + 12 * step.div_euclid(7)).clamp(0, 127) as u8
}

/// Tonal pitch class of a derived step (C = 14, D = 16, …).
fn derived_tpc(step: i32) -> i8 {
    const TPC: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];
    TPC[step.rem_euclid(7) as usize]
}

/// Closest modern bar line to a separation bar
/// (`Gabc.tex:801-845`); the rich vocabulary stays in the payload.
fn derived_bar_line(bar: BarKind) -> BarLineType {
    match bar {
        // `Gabc.tex:801-821` : minima = « quarter bar », minor = « half bar »,
        // maior = « full bar », finalis = barre finale. MusicXML les porte
        // respectivement comme `tick`, `short`, `light-light` et
        // `light-heavy` (XSD 4.0 `bar-style` ; mapping MuseScore dans
        // `engraving/rw/write/repeats.cpp`).
        BarKind::DivisioMinima | BarKind::DivisioMinimaLedger => BarLineType::Tick,
        BarKind::DivisioMinor => BarLineType::Short,
        BarKind::Dominican(_) => BarLineType::Dotted,
        BarKind::DivisioMaior | BarKind::DivisioMaiorDot => BarLineType::Double,
        BarKind::DivisioFinalis => BarLineType::End,
        BarKind::Virgula | BarKind::VirgulaLedger | BarKind::DivisioMinimis => BarLineType::Tick,
    }
}

/// Closest modern clef to a GABC clef token (`Gabc.tex:847-870`).
fn derived_clef_type(token: &str) -> ClefType {
    let first = token.split('@').next().unwrap_or(token);
    let line = first
        .bytes()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    if first.starts_with('f') {
        match line {
            3 => ClefType::FBaritone,
            5 => ClefType::FSubbass,
            _ => ClefType::F,
        }
    } else {
        match line {
            1 => ClefType::C1,
            2 => ClefType::C2,
            3 => ClefType::C3,
            5 => ClefType::C5,
            _ => ClefType::C4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_is_not_a_lyric() {
        assert_eq!(plain_lyric("<sc>tus</sc>"), "tus");
        assert_eq!(plain_lyric("{DE}us"), "DEus");
        assert_eq!(plain_lyric("$(a)"), "(a)");
        assert_eq!(plain_lyric("<sp>V/</sp>."), ".");
    }

    #[test]
    fn derived_view_marks_itself_as_a_projection() {
        let payload = Gregorian {
            clefs: vec!["c3".to_owned()],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        assert!(score.report.losses.iter().any(|loss| {
            loss.layer == Layers::COMMON && loss.item.contains("derived-modern-view")
        }));
    }

    #[test]
    fn finalis_is_one_bar_not_two_maiores() {
        assert_eq!(
            BarKind::parse_prefix("::"),
            Some((BarKind::DivisioFinalis, 2))
        );
        assert_eq!(
            BarKind::parse_prefix(":?"),
            Some((BarKind::DivisioMaiorDot, 2))
        );
        assert_eq!(BarKind::parse_prefix(":"), Some((BarKind::DivisioMaior, 1)));
    }

    #[test]
    fn bars_cover_the_specified_vocabulary() {
        for (token, expected) in [
            ("`", BarKind::Virgula),
            ("`0", BarKind::VirgulaLedger),
            ("^", BarKind::DivisioMinimis),
            ("^0", BarKind::DivisioMinimis),
            (",", BarKind::DivisioMinima),
            (",0", BarKind::DivisioMinimaLedger),
            (";", BarKind::DivisioMinor),
            (":", BarKind::DivisioMaior),
            ("::", BarKind::DivisioFinalis),
            (";1", BarKind::Dominican(1)),
            (";8", BarKind::Dominican(8)),
        ] {
            let (kind, used) = BarKind::parse_prefix(token).expect("a specified bar");
            assert_eq!(kind, expected, "{token}");
            assert_eq!(used, token.len(), "{token}");
        }
        // Out-of-range Dominican digits fall back to a divisio minor plus
        // an unparsed trailing character, as in the source grammar.
        assert_eq!(
            BarKind::parse_prefix(",1"),
            Some((BarKind::Dominican(1), 2))
        );
        assert_eq!(
            BarKind::parse_prefix(";3"),
            Some((BarKind::Dominican(3), 2))
        );
        assert_eq!(
            BarKind::parse_prefix(";9"),
            Some((BarKind::DivisioMinor, 1))
        );
        assert_eq!(BarKind::parse_prefix("g"), None);
    }

    #[test]
    fn a_word_produces_begin_middle_end_syllabics() {
        // `Gabc.tex:241-245` : `Ma-rí-a` est un mot de trois syllabes.
        let payload = Gregorian {
            syllables: ["Ma", "rí", "a", "sol"]
                .into_iter()
                .map(|text| Syllable {
                    text: text.to_owned(),
                    notes: vec![GregorianNote::default()],
                    space_after: text == "a" || text == "sol",
                    ..Syllable::default()
                })
                .collect(),
            ..Gregorian::default()
        };
        assert_eq!(
            payload.syllabics(),
            [
                LyricsSyllabic::Begin,
                LyricsSyllabic::Middle,
                LyricsSyllabic::End,
                LyricsSyllabic::Single,
            ]
        );
        let score = payload.build_derived_score();
        let lyrics: Vec<_> = score
            .measures
            .iter()
            .flat_map(|measure| &measure.segments)
            .filter_map(|segment| {
                segment
                    .element(Track::new(super::super::StaffId::from_raw(0), 0))
                    .and_then(|item| match item {
                        SegmentItem::ChordRest(chord_rest) => {
                            chord_rest.lyrics.first().map(|lyric| lyric.syllabic)
                        }
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(
            lyrics,
            [
                LyricsSyllabic::Begin,
                LyricsSyllabic::Middle,
                LyricsSyllabic::End,
                LyricsSyllabic::Single,
            ]
        );
    }

    #[test]
    fn an_alteration_is_a_sign_not_a_sung_note() {
        // `Gabc.tex:481-489,512-540` : `gixi` = G, alteration flat on `i`,
        // then the sung `i` (B-flat). Gregorio's dump has three glyph notes,
        // but only two are sung; see `docs/agents/gabc.md`.
        let payload = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![Syllable {
                text: "A".to_owned(),
                notes: vec![
                    GregorianNote {
                        letter: 'g',
                        ..GregorianNote::default()
                    },
                    GregorianNote {
                        letter: 'i',
                        glyph: "x".to_owned(),
                        signs: vec![NeumeSign::new(SignKind::Alteration, "x")],
                        ..GregorianNote::default()
                    },
                    GregorianNote {
                        letter: 'i',
                        ..GregorianNote::default()
                    },
                ],
                space_after: true,
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        assert_eq!(score.notes.len(), 2);
        assert_eq!(score.notes[0].pitch, 55);
        assert_eq!(score.notes[1].pitch, 58);
        assert_eq!(score.notes[1].tpc, super::super::tpc::B_FLAT);
        assert_eq!(score.notes[1].source.unwrap().gregorian_note, 2);
        // Le glyphe n'est pas une note, mais le bémol est visible sur la
        // première note chantée qui en porte l'effet (`<accidental>` MusicXML).
        assert_eq!(score.notes[0].accidental, None);
        assert_eq!(
            score.notes[1].accidental,
            Some(Accidental::new(AccidentalType::Flat))
        );
    }

    #[test]
    fn alteration_effect_ends_at_word_bar_or_natural() {
        let note = |letter| GregorianNote {
            letter,
            ..GregorianNote::default()
        };
        let altered = |letter, raw: &'static str| GregorianNote {
            letter,
            glyph: raw.to_owned(),
            signs: vec![NeumeSign::new(SignKind::Alteration, raw)],
            ..GregorianNote::default()
        };
        let payload = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![
                Syllable {
                    notes: vec![altered('g', "x"), note('g'), note('g')],
                    space_after: true,
                    ..Syllable::default()
                },
                Syllable {
                    notes: vec![note('g')],
                    space_after: false,
                    ..Syllable::default()
                },
                Syllable {
                    notes: vec![altered('g', "#"), note('g')],
                    bars: vec![BarKind::DivisioMaior],
                    ..Syllable::default()
                },
                Syllable {
                    notes: vec![note('g')],
                    ..Syllable::default()
                },
            ],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
        // g-flat, g-flat inside the word, then natural after the word,
        // g-sharp after `#`, natural again after the bar.
        assert_eq!(pitches, [54, 54, 55, 56, 55]);
        let accidentals: Vec<_> = score.notes.iter().map(|note| note.accidental).collect();
        // Un seul signe par glyphe : la seconde note sous la portée n'en a
        // pas. Le mot sans barre laisse le bémol affiché dans la mesure : la
        // note suivante, sonée nature, porte un bécarre d'avertissement.
        // Après une vraie barre, aucun signe n'hérite du dièse.
        let mut courtesy = Accidental::new(AccidentalType::Natural);
        courtesy.user = true;
        assert_eq!(
            accidentals,
            [
                Some(Accidental::new(AccidentalType::Flat)),
                None,
                Some(courtesy),
                Some(Accidental::new(AccidentalType::Sharp)),
                None,
            ]
        );
        assert!(!accidentals[2].unwrap().bracket);
        assert!(!accidentals[2].unwrap().parentheses);
    }

    fn note_with_quilisma(letter: char, quilisma: bool) -> GregorianNote {
        GregorianNote {
            letter,
            signs: if quilisma {
                vec![NeumeSign::new(SignKind::Quilisma, "w")]
            } else {
                Vec::new()
            },
            ..GregorianNote::default()
        }
    }

    fn events(score: &Score) -> Vec<&ChordRest> {
        let track = Track::new(super::super::StaffId::from_raw(0), 0);
        score
            .measures
            .iter()
            .flat_map(|measure| &measure.segments)
            .filter_map(move |segment| match segment.element(track) {
                Some(SegmentItem::ChordRest(chord_rest)) => Some(chord_rest),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn quilisma_factors_are_the_named_series() {
        assert_eq!(QUILISMA_HOLD_IMMEDIATE, Fraction::from_integer(2));
        assert_eq!(QUILISMA_HOLD_SECOND, Fraction::new(3, 2));
        assert_eq!(QUILISMA_HOLD_THIRD, Fraction::new(5, 4));
        assert_eq!(QUILISMA_HOLD_FOURTH, Fraction::new(9, 8));
        assert_eq!(QUILISMA_HOLD_FACTORS[0], quilisma_hold_factor(0));
        assert_eq!(QUILISMA_HOLD_FACTORS[3], quilisma_hold_factor(3));
        assert_eq!(quilisma_hold_factor(4), Fraction::new(17, 16));
    }

    #[test]
    fn quilisma_is_an_inverted_mordent_ornament() {
        let payload = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![Syllable {
                text: "De".to_owned(),
                notes: vec![
                    note_with_quilisma('g', false),
                    note_with_quilisma('h', true),
                ],
                space_after: true,
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        let chords: Vec<&Chord> = events(&score)
            .into_iter()
            .filter_map(|event| event.chord_ref())
            .collect();
        assert!(chords[0].ornaments.is_empty());
        assert_eq!(
            chords[1].ornaments,
            vec![Ornament::new(OrnamentType::InvertedMordent)]
        );
    }

    #[test]
    fn three_notes_before_a_quilisma_are_held_progressively() {
        // `1 + 2^-k` en remontant depuis le quilisma : la note immédiatement
        // avant est doublée (1/2), puis 1,5 (3/8) et 1,25 (5/16) ; la note
        // chantée du quilisma garde sa durée de base (noire).
        let payload = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![Syllable {
                text: "gloria".to_owned(),
                notes: vec![
                    note_with_quilisma('g', false),
                    note_with_quilisma('h', false),
                    note_with_quilisma('i', false),
                    note_with_quilisma('j', true),
                ],
                space_after: true,
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        let ticks: Vec<Fraction> = events(&score)
            .iter()
            .map(|event| event.ticks.unwrap_or_else(|| event.duration.ticks()))
            .collect();
        assert_eq!(
            ticks,
            [
                Fraction::new(5, 16),
                Fraction::new(3, 8),
                Fraction::new(1, 2),
                Fraction::QUARTER,
            ]
        );
    }

    #[test]
    fn a_quilisma_as_second_note_stops_at_the_syllable_start() {
        let payload = Gregorian {
            clefs: vec!["c4".to_owned()],
            syllables: vec![
                Syllable {
                    text: "et".to_owned(),
                    notes: vec![note_with_quilisma('g', false)],
                    space_after: true,
                    ..Syllable::default()
                },
                Syllable {
                    text: "in".to_owned(),
                    notes: vec![
                        note_with_quilisma('h', false),
                        note_with_quilisma('i', true),
                    ],
                    space_after: true,
                    ..Syllable::default()
                },
            ],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        let events = events(&score);
        assert_eq!(events[0].ticks, None, "pas d'allongement entre syllabes");
        assert_eq!(events[1].ticks, Some(Fraction::new(1, 2)));
        assert_eq!(
            events[2].ticks, None,
            "la note du quilisma n'est pas allongée"
        );
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn payload_stays_an_in_memory_value() {
        let mut gregorian = Gregorian::default();
        assert!(!gregorian.derived_modern_view);
        gregorian.headers.push(GregorianHeader {
            name: "name".to_owned(),
            value: "Veni Sancte Spiritus".to_owned(),
        });
        gregorian.syllables.push(Syllable {
            above_text: Some("Fa".to_owned()),
            text: "DE(g)us".to_owned(),
            clef: None,
            rubric: Some("<sp>R/</sp>".to_owned()),
            notes: vec![GregorianNote {
                letter: 'g',
                glyph: "h_".to_owned(),
                signs: vec![NeumeSign::new(SignKind::HorizontalEpisema, "_")],
                ..GregorianNote::default()
            }],
            signs: Vec::new(),
            bars: vec![BarKind::DivisioFinalis],
            space_after: true,
        });
        assert_eq!(gregorian.headers[0].value, "Veni Sancte Spiritus");
        assert_eq!(gregorian.syllables[0].notes[0].letter, 'g');
    }
}
