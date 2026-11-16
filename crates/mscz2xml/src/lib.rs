//! Portage en Rust de l'export MusicXML de MuseScore.
//!
//! Cette bibliothèque est volontairement sans entrées/sorties : elle travaille
//! uniquement sur des octets en mémoire, sans système de fichiers, sans thread
//! et sans dépendance C. Elle doit donc compiler pour `wasm32-unknown-unknown`,
//! Android et iOS.
//!
//! Point d'entrée de cette première étape : [`MsczContainer`], qui extrait le
//! fichier `.mscx` principal d'un conteneur `.mscz` (ou accepte un `.mscx` nu),
//! et [`detect_version`], qui lit la version du format MuseScore.
//!
//! Le module [`model`] contient le modèle de partition en mémoire, sans aucune
//! lecture ni écriture XML : il est rempli par les lecteurs `.mscx` et parcouru
//! par l'export MusicXML.

mod container;
mod error;
pub mod model;
pub mod read;

pub use container::{detect_version, MscVersion, MsczContainer};
pub use error::Error;

/// Résultat de la bibliothèque.
pub type Result<T> = std::result::Result<T, Error>;
