//! Modèle de partition en mémoire.
//!
//! Ce module ne lit ni n'écrit aucun XML : il ne contient que les structures
//! manipulées par les lecteurs `.mscx` et par l'export MusicXML. Il est
//! volontairement sans entrées/sorties, sans threads, sans `Rc<RefCell<_>>` et
//! sans pointeur cyclique, afin de rester `Send + Sync`, portable et
//! compilable pour `wasm32-unknown-unknown`.
//!
//! # Temps
//!
//! Les positions et durées sont des [`Fraction`] exactes, en unités de **ronde**
//! comme dans MuseScore : `1/1` est une ronde, `1/4` une noire. Une mesure 4/4
//! vaut donc `1/1`. [`Score::division`] donne le nombre de divisions MusicXML
//! par noire ; [`Score::tick_to_divisions`] et [`Score::divisions_to_tick`]
//! font la conversion.
//!
//! # Pistes
//!
//! Chaque [`Segment`] range ses éléments par piste (`Track`). La formule est
//! celle de MuseScore : `track = staff_idx * VOICES + voice`, avec
//! [`VOICES`] = 4. Les éléments non rythmiques (clef, armure, mesure, barre de
//! mesure) occupent en pratique la voix 0 de leur portée.
//!
//! # Arènes et identifiants
//!
//! Les objets qui ont besoin d'être référencés depuis plusieurs endroits
//! (notes, n-olet, spanners) vivent dans des `Vec` possédées par [`Score`] et
//! sont désignés par des identifiants typés ([`NoteId`], [`TupletId`],
//! [`SpannerId`]). Les accords et silences restent possédés par leur segment,
//! comme dans MuseScore. Aucun type ne contient de pointeur vers son parent :
//! les liaisons de prolongation sont portées par des [`NoteId`] dans [`Note`].
//!
//! # Correspondance avec MuseScore
//!
//! Chaque type public cite, dans sa documentation, le fichier C++ dont il est
//! le portage. Les sources de référence sont dans
//! `src/engraving/dom/` et `src/importexport/musicxml/internal/export/`.

pub mod fraction;

pub use fraction::{Fraction, Tick};
