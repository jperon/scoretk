// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/rwregister.cpp` de MuseScore Studio, Copyright (C)
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

//! Lecture d'un `.mscx` vers le [modèle de partition](scoretk_core::model::Score).
//!
//! Le point d'entrée unique est [`read_score`] : il détecte la version du
//! fichier et la répartit vers le lecteur correspondant, exactement comme
//! `RWRegister::reader` (`engraving/rw/rwregister.cpp`) :
//!
//! | Version | Lecteur |
//! | --- | --- |
//! | 4.00 – 5.00 | `read4xx` (portage de `read400`, `read410`, `read460`, `read500`) |
//! | 3.00 – 3.99 | `read302` (portage de `read302`) |
//! | 1.15 – 2.07 | `read206` (portage de `read206`) |
//!
//! Comme `RWRegister::reader` (`rw/rwregister.cpp:45-49`), les versions
//! `<= 114` restent sans lecteur porté et renvoient
//! [`Error::UnsupportedVersion`](crate::Error::UnsupportedVersion).

/// Catalogue d'instruments de MuseScore, pour les fichiers 3.x sans
/// identifiant MusicXML.
pub(crate) mod instruments;

/// Lecteur des partitions MuseScore 1.15 à 2.07 (portage de `read206`).
pub mod read206;
/// Lecteur des partitions MuseScore 3.00 à 3.99 (portage de `read302`).
pub mod read302;
/// Lecteur des partitions MuseScore 4.00 à 5.00 (portage de `read4xx`).
pub mod read4xx;

mod measure_repeat;
mod notation_names;

use quick_xml::events::Event;

use crate::container::MsczContainer;
use crate::{Error, Result};
use scoretk_core::model::{Measure, Score};

/// Reprend du `<Style>` séparé d'un `.mscz` la seule valeur que le pivot ne
/// peut pas déduire du `.mscx`.
///
/// `MscLoader::read` (`rw/mscloader.cpp:86-94`) appelle
/// `MscReader::readStyleFile()` (`infrastructure/mscreader.cpp:127-132`) et
/// `masterScore->style().read(&buf)` *avant* de lire la partition : le style
/// inline du `.mscx` l'emporte ensuite. Comme le lecteur lit déjà le style
/// inline, on ne reprend ici que `createMultiMeasureRests` (`styledef.cpp:571`)
/// — seule valeur nécessaire à la reconstruction des multi-repos
/// (`dom/measure.cpp:2899`) — afin de ne pas écraser les autres réglages.
fn apply_style_file(score: &mut Score, mss: &[u8]) -> Result<()> {
    let mut reader = quick_xml::Reader::from_reader(mss);
    reader.config_mut().trim_text(true);
    let mut in_style = false;
    let mut in_flag = false;
    let mut in_style_name = String::new();
    loop {
        match reader.read_event()? {
            Event::Start(element) => {
                let local = element.name();
                match local.as_ref() {
                    "Style" => in_style = true,
                    "createMultiMeasureRests" if in_style => {
                        in_flag = true;
                        in_style_name = "createMultiMeasureRests".to_owned();
                    }
                    "smallStaffMag" if in_style => {
                        in_flag = true;
                        in_style_name = "smallStaffMag".to_owned();
                    }
                    _ => {}
                }
            }
            Event::Text(value) if in_flag => {
                if in_style_name == "createMultiMeasureRests" {
                    score.style.create_multi_measure_rests = value.as_ref().trim() == "1";
                } else if in_style_name == "smallStaffMag" {
                    if let Ok(mag) = value.as_ref().trim().parse::<f64>() {
                        score.style.small_staff_mag = mag;
                    }
                }
            }
            Event::End(element) => match element.name().as_ref() {
                "Style" => in_style = false,
                "createMultiMeasureRests" | "smallStaffMag" => in_flag = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

/// Mesure de départ du groupe de silences multi-mesures qui couvre `target`.
///
/// Le modèle porte le compteur du groupe sur sa première mesure
/// (`Measure::multi_measure_rest`) : un texte de cadre placé sous une mesure
/// couverte par le groupe appartient en réalité à la mesure qui remplace le
/// groupe (`findTextFramesToWriteAsWordsBelow` et `coveringMMRestOrThis`).
pub(crate) fn mmrest_group_start(measures: &[Measure], target: usize) -> usize {
    for (index, measure) in measures.iter().enumerate().take(target) {
        if let Some(count) = measure.multi_measure_rest {
            if target < index + usize::from(count) {
                return index;
            }
        }
    }
    target
}

/// Lit un conteneur `.mscz` ou un `.mscx` nu et renvoie la partition.
///
/// La version est lue dans l'attribut `version` de `<museScore>`, jamais dans
/// `<programVersion>`.
pub fn read_score(container: &MsczContainer) -> Result<Score> {
    let version = container.version()?;
    let mut score = match version.as_u32() {
        400..=500 => read4xx::read(container.main_mscx(), version)?,
        300..=399 => read302::read(container.main_mscx(), version)?,
        115..=207 => read206::read(container.main_mscx(), version)?,
        _ => return Err(Error::UnsupportedVersion(version)),
    };
    if let Some(mss) = container.style_mss() {
        apply_style_file(&mut score, mss)?;
    }
    // `Measure::checkMeasure` est appelé juste après la lecture de chaque
    // mesure par MuseScore ; les silences « gap » qui en résultent font
    // avancer le curseur d'export (`<forward>`/`<backup>`).
    scoretk_core::gap_rests::fill(&mut score);
    Ok(score)
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
        for version in ["1.14", "5.01", "6.00"] {
            let error = read_score(&container(version)).unwrap_err();
            assert!(
                matches!(error, Error::UnsupportedVersion(_)),
                "version {version} : {error}"
            );
        }

        assert!(
            read_score(&container("3.02")).is_ok(),
            "version 3.02 lue par le lecteur 3.x"
        );
    }

    #[test]
    fn lit_les_versions_2() {
        for version in ["2.06", "2.07"] {
            assert!(
                read_score(&container(version)).is_ok(),
                "version {version} lue par le lecteur 2.x"
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
