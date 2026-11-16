//! Identifiants typés, pistes et constantes d'indexation.
//!
//! MuseScore indexe les éléments d'un segment par piste (`track_idx_t`) :
//! `track = staff_idx * VOICES + voice`, avec `VOICES = 4`
//! (`engraving/dom/segment.h`, `engraving/dom/score.h`). Les identifiants
//! ci-dessous remplacent les pointeurs C++ : ils désignent un objet dans une
//! arène `Vec` possédée par [`super::Score`].

/// Nombre de voix par portée (`VOICES` dans MuseScore).
pub const VOICES: usize = 4;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// Construit l'identifiant à partir de sa valeur brute.
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            /// Construit l'identifiant à partir d'un index d'arène.
            ///
            /// L'index doit tenir dans `u32`.
            pub const fn from_index(index: usize) -> Self {
                Self(index as u32)
            }

            /// Valeur brute de l'identifiant.
            pub const fn raw(self) -> u32 {
                self.0
            }

            /// Index correspondant dans l'arène.
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl From<$name> for usize {
            fn from(id: $name) -> Self {
                id.index()
            }
        }
    };
}

define_id! {
    /// Identifiant d'une partie dans [`super::Score::parts`].
    ///
    /// Correspond à l'index d'une `Part` dans `Score::parts()`
    /// (`engraving/dom/part.h`).
    PartId
}

define_id! {
    /// Identifiant d'une portée dans [`super::Score::staves`].
    ///
    /// Correspond à l'index d'un `Staff` dans `Score::staves()`
    /// (`engraving/dom/staff.h`).
    StaffId
}

define_id! {
    /// Identifiant d'une mesure dans [`super::Score::measures`].
    ///
    /// Correspond à l'index d'une `Measure` dans la liste des mesures
    /// (`engraving/dom/measure.h`).
    MeasureId
}

define_id! {
    /// Identifiant d'une note dans l'arène [`super::Score::notes`].
    ///
    /// Sert notamment aux liaisons de prolongation, qui référencent la note
    /// suivante ou précédente par [`NoteId`] (`engraving/dom/note.h`,
    /// `engraving/dom/tie.h`).
    NoteId
}

define_id! {
    /// Identifiant d'un n-olet dans l'arène [`super::Score::tuplets`].
    ///
    /// Correspond à un `Tuplet` dans `engraving/dom/tuplet.h`.
    TupletId
}

define_id! {
    /// Identifiant d'un spanner dans l'arène [`super::Score::spanners`].
    ///
    /// Correspond à un objet dérivé de `Spanner` dans
    /// `engraving/dom/spanner.h`.
    SpannerId
}

/// Piste d'un segment : `staff_idx * VOICES + voice`.
///
/// Correspond à `track_idx_t` dans MuseScore (`engraving/dom/segment.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Track(u32);

impl Track {
    /// Construit une piste à partir d'une portée et d'une voix.
    ///
    /// # Panics
    ///
    /// Panique si `voice >= VOICES`.
    pub const fn new(staff: StaffId, voice: u8) -> Self {
        assert!(
            (voice as usize) < VOICES,
            "voix hors plage : MuseScore n'a que quatre voix par portée"
        );
        Self(staff.raw() * VOICES as u32 + voice as u32)
    }

    /// Construit une piste à partir de sa valeur brute.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Valeur brute de la piste.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Index de la piste dans le vecteur d'éléments d'un segment
    /// (`Vec<Option<SegmentItem>>`).
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Index de la portée.
    pub const fn staff(self) -> StaffId {
        StaffId::from_raw(self.0 / VOICES as u32)
    }

    /// Indice de la voix (`0..VOICES`).
    pub const fn voice(self) -> u8 {
        (self.0 % VOICES as u32) as u8
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.staff().raw(), self.voice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piste_aller_retour() {
        let staff = StaffId::from_index(3);
        let track = Track::new(staff, 2);
        assert_eq!(track.raw(), 3 * VOICES as u32 + 2);
        assert_eq!(track.staff(), staff);
        assert_eq!(track.voice(), 2);
        assert_eq!(Track::from_raw(track.raw()), track);
    }

    #[test]
    #[should_panic(expected = "voix hors plage")]
    fn voix_invalide_refusee() {
        let _ = Track::new(StaffId::from_index(0), VOICES as u8);
    }
}
