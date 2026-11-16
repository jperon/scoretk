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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Placement;
    use crate::read::read4xx::dom;

    #[test]
    fn lyrics_placement_du_style_ou_defaut() {
        let without = dom::parse(b"<Score></Score>").expect("XML");
        assert_eq!(
            read(&without, MscVersion::from_raw(400)).lyrics_placement,
            Placement::Below
        );

        let with = dom::parse(
            b"<Score><Style><lyricsPlacement>above</lyricsPlacement>\
              <pageWidth>8.5</pageWidth></Style></Score>",
        )
        .expect("XML");
        let style = read(
            with.child("Style").expect("Style"),
            MscVersion::from_raw(400),
        );
        assert_eq!(style.lyrics_placement, Placement::Above);
        assert!((style.page_width_in - 8.5).abs() < 1e-9);
    }
}
