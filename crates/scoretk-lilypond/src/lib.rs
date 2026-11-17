// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances de notation (articulations, ornements, nuances, clés,
// armures) s'inspirent de `musicxml2ly` de LilyPond, Copyright (C) 2016--2026
// John Gourlay et Werner Lemberg, GPL-3.0-or-later ; le code ci-dessous est
// une réécriture en Rust, sans ligne copiée.
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

//! **scoretk-lilypond** : lecture et écriture LilyPond (`.ly`) du modèle pivot.
//! L'écriture ([`write_lilypond`]) est décrite ci-dessous ; la lecture
//! ([`read_lilypond`]) porte un sous-ensemble de la grammaire de LilyPond, cf.
//! `docs/agents/lilypond.md`.
//!
//! Le crate ne fait aucune entrée/sortie : [`write_lilypond`] renvoie le texte
//! en mémoire, ce qui le garde compilable pour `wasm32-unknown-unknown`,
//! Android et iOS.
//!
//! # Choix de notation
//!
//! - Les hauteurs sont écrites en mode [`\fixed`] (`\fixed c' { … }`), jamais
//!   en `\relative` : l'octave est absolue par rapport à `c'` (do central), ce
//!   qui résiste au copier-coller et au réordonnancement des voix.
//! - `\language "english"` est déclaré explicitement, pour que les altérations
//!   s'écrivent `cs`/`bf`/`css` quel que soit le réglage local de LilyPond.
//! - Une variable LilyPond par voix (`voiceOne`, `voiceTwo`…), regroupée dans
//!   des `\new Voice` nommés pour que `\lyricsto` puisse s'y accrocher.
//! - Les crochets de portée deviennent `PianoStaff` (accolade), `ChoirStaff`
//!   (crochet) ou `StaffGroup` (plusieurs portées sans crochet).
//!
//! La correspondance modèle → LilyPond et les limites connues sont décrites
//! dans `docs/agents/lilypond.md`.
//!
//! # Exemple
//!
//! ```
//! use scoretk_core::model::{Duration, Measure, Note, Score, tpc};
//! use scoretk_lilypond::write_lilypond;
//!
//! let mut score = Score::new();
//! let part = score.add_part(scoretk_core::model::Part::new("Piano", "Pno."));
//! let staff = score.add_staff(part);
//! let mut measure = Measure::new(0, scoretk_lilypond::Fraction::ZERO, Duration::whole().ticks());
//! let mut segment = scoretk_core::model::Segment::new(
//!     scoretk_core::model::SegmentType::ChordRest,
//!     scoretk_lilypond::Fraction::ZERO,
//!     1,
//! );
//! let mut chord = scoretk_core::model::ChordRest::chord(Duration::whole());
//! let note = score.add_note(Note::new(60, tpc::C));
//! chord.chord_mut().expect("accord").notes.push(note);
//! segment.set_element(
//!     scoretk_core::model::Track::new(staff, 0),
//!     scoretk_core::model::SegmentItem::ChordRest(chord),
//! );
//! measure.add_segment(segment);
//! score.add_measure(measure);
//!
//! let lilypond = write_lilypond(&score).expect("écriture");
//! assert!(lilypond.starts_with("\\version \"2.24.0\""));
//! assert!(lilypond.contains("\\fixed c' {"));
//! assert!(lilypond.contains('c'));
//! ```

mod annotations;
mod chord;
mod duration;
mod error;
mod event;
mod pitch;
mod read;
mod spanners;
mod structure;
mod text;
mod voice;
mod writer;

pub use error::{Error, Result};
pub use read::{read_lilypond, read_lilypond_reporting, Warning};
pub use scoretk_core::model::Fraction;

use scoretk_core::model::Score;

/// Écrit une partition du modèle pivot au format LilyPond.
///
/// La sortie est un document complet, reproductible (aucune horloge, aucun
/// identifiant aléatoire), terminé par un saut de ligne.
pub fn write_lilypond(score: &Score) -> Result<String> {
    writer::Writer::new(score).render()
}
