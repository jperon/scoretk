//! Lecture du `<Style>` d'une partition 4.x.
//!
//! Portage partiel de `ReadStyle` (`engraving/rw/read400/readstyle.cpp`) : seules
//! les propriétés nécessaires à l'export MusicXML sont conservées (format de
//! page, marges, `lyricsPlacement` et tailles de police des crédits). Les
//! valeurs absentes gardent les défauts de MuseScore pour la version du
//! fichier.

use super::dom::Element;
use crate::container::MscVersion;
use crate::model::StyleSettings;

/// Lit un `<Style>` 4.x et complète les valeurs par défaut de `version`.
pub(super) fn read(element: &Element, version: MscVersion) -> StyleSettings {
    let mut style = StyleSettings::for_version(version.major());
    for child in &element.children {
        style.set_value(&child.name, child.text());
    }
    style
}
