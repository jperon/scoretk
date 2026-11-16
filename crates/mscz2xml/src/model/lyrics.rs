//! Paroles.
//!
//! Portage de `Lyrics` (`engraving/dom/lyrics.h`). Les paroles sont attachées
//! au [`super::ChordRest`], comme le champ `m_lyrics` de `ChordRest` dans
//! `engraving/dom/chordrest.h`.

use super::annotation::Placement;
use super::fraction::Fraction;

/// Syllabisation d'une syllabe de parole.
///
/// Correspond à `LyricsSyllabic` dans `engraving/types/types.h`, utilisé par
/// `Lyrics` dans `engraving/dom/lyrics.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LyricsSyllabic {
    /// Syllabe isolée.
    Single,
    /// Début d'un mot.
    Begin,
    /// Milieu d'un mot.
    Middle,
    /// Fin d'un mot.
    End,
}

/// Paroles d'un couplet sur un accord ou un silence.
///
/// Correspond à `Lyrics` dans `engraving/dom/lyrics.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lyrics {
    /// Indice de couplet, à partir de zéro comme `Lyrics::verse()` dans
    /// MuseScore. Le numéro MusicXML affiché vaut `verse + 1`.
    pub verse: usize,
    /// Syllabisation.
    pub syllabic: LyricsSyllabic,
    /// Texte de la syllabe.
    pub text: String,
    /// Durée du mélisme, si la syllabe est prolongée par un trait.
    pub melisma: Option<Fraction>,
    /// Placement au-dessus ou au-dessous de la portée.
    pub placement: Placement,
}

impl Lyrics {
    /// Construit des paroles sans mélisme, au-dessous de la portée.
    pub fn new(verse: usize, syllabic: LyricsSyllabic, text: impl Into<String>) -> Self {
        Self {
            verse,
            syllabic,
            text: text.into(),
            melisma: None,
            placement: Placement::Below,
        }
    }

    /// Numéro de couplet tel qu'écrit en MusicXML (1 pour le premier).
    pub const fn number(&self) -> usize {
        self.verse + 1
    }
}
