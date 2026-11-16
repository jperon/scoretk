//! Lecture d'un `.mscx` vers le [modèle de partition](crate::model::Score).
//!
//! Le point d'entrée unique est [`read_score`] : il détecte la version du
//! fichier et la répartit vers le lecteur correspondant, exactement comme
//! `RWRegister::reader` (`engraving/rw/rwregister.cpp`) :
//!
//! | Version | Lecteur |
//! | --- | --- |
//! | 4.00 – 5.00 | `read4xx` (portage de `read400`, `read410`, `read460`, `read500`) |
//! | 3.00 – 3.99 | `read302` (portage de `read302`) |
//!
//! Les versions antérieures ou inconnues renvoient
//! [`Error::UnsupportedVersion`](crate::Error::UnsupportedVersion).

/// Lecteur des partitions MuseScore 3.00 à 3.99 (portage de `read302`).
pub mod read302;
/// Lecteur des partitions MuseScore 4.00 à 5.00 (portage de `read4xx`).
pub mod read4xx;

use crate::container::MsczContainer;
use crate::model::Score;
use crate::{Error, Result};

/// Lit un conteneur `.mscz` ou un `.mscx` nu et renvoie la partition.
///
/// La version est lue dans l'attribut `version` de `<museScore>`, jamais dans
/// `<programVersion>`.
pub fn read_score(container: &MsczContainer) -> Result<Score> {
    let version = container.version()?;
    match version.as_u32() {
        400..=500 => read4xx::read(container.main_mscx(), version),
        300..=399 => read302::read(container.main_mscx(), version),
        _ => Err(Error::UnsupportedVersion(version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(version: &str) -> MsczContainer {
        let xml = format!(
            r#"<museScore version="{version}"><Score><Division>480</Division></Score></museScore>"#
        );
        MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur .mscx nu")
    }

    #[test]
    fn repartit_selon_la_version() {
        for version in ["4.00", "4.10", "4.60", "5.00"] {
            assert!(
                read_score(&container(version)).is_ok(),
                "version {version} lue par le lecteur 4.x"
            );
        }
    }

    #[test]
    fn refuse_les_versions_non_prises_en_charge() {
        for version in ["2.06", "5.01"] {
            let error = read_score(&container(version)).unwrap_err();
            assert!(
                matches!(error, Error::UnsupportedVersion(_)),
                "version {version} : {error}"
            );
        }
    }

    #[test]
    fn accepte_les_versions_3_et_4() {
        for version in ["3.02", "4.00"] {
            assert!(
                read_score(&container(version)).is_ok(),
                "version {version} lue"
            );
        }
    }
}
