// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/style/style.h` et
// `importexport/musicxml/internal/export/exportmusicxml.cpp` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Réglages de style globaux utiles à l'export MusicXML.
//!
//! Portage partiel de `MStyle` (`engraving/style/style.h`) et des propriétés
//! lues par `ExportMusicXml::defaults`, `writePageFormat` et `credits`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! Le modèle ne fait pas de mise en page : il conserve seulement les valeurs
//! du `<Style>` du fichier `.mscx` dont l'export a besoin (format de page,
//! marges, `lyricsPlacement`, tailles de police des crédits), complétées par
//! les valeurs par défaut de MuseScore pour la version du fichier. Les valeurs
//! de page et de marge sont stockées en pouces, comme dans le `.mscx`, et
//! converties en dixièmes (`tenths`) à l'export, comme le fait `MStyle` avec
//! `getTenthsFromInches`.

use super::annotation::Placement;
use super::text::CreditStyle;

/// Nombre de dixièmes par spatium (`Constants::TENTHS`).
///
/// MuseScore exporte toujours `<tenths>40</tenths>` et une hauteur de page
/// avec quatre espaces par dixième : un spatium vaut donc 10 dixièmes.
pub const TENTHS: u32 = 40;

/// Réglages globaux d'une partition.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleSettings {
    /// Largeur d'un spatium en millimètres (`Spatium`).
    pub spatium_mm: f64,
    /// Largeur de page en pouces (`pageWidth`).
    pub page_width_in: f64,
    /// Hauteur de page en pouces (`pageHeight`).
    pub page_height_in: f64,
    /// Marge gauche des pages paires (`pageEvenLeftMargin`, pouces).
    pub page_even_left_margin_in: f64,
    /// Marge gauche des pages impaires (`pageOddLeftMargin`, pouces).
    pub page_odd_left_margin_in: f64,
    /// Marge droite des pages paires (`pageEvenRightMargin`, pouces).
    pub page_even_right_margin_in: f64,
    /// Marge droite des pages impaires (`pageOddRightMargin`, pouces).
    pub page_odd_right_margin_in: f64,
    /// Marge haute des pages paires (`pageEvenTopMargin`, pouces).
    pub page_even_top_margin_in: f64,
    /// Marge haute des pages impaires (`pageOddTopMargin`, pouces).
    pub page_odd_top_margin_in: f64,
    /// Marge basse des pages paires (`pageEvenBottomMargin`, pouces).
    pub page_even_bottom_margin_in: f64,
    /// Marge basse des pages impaires (`pageOddBottomMargin`, pouces).
    pub page_odd_bottom_margin_in: f64,
    /// Pages en vis-à-vis (`pageTwosided`).
    pub page_two_sided: bool,
    /// Placement par défaut des paroles (`lyricsPlacement`, défaut `below`).
    pub lyrics_placement: Placement,
    /// Nombre de pages de la partition (les `<credit>` de copyright sont
    /// répétés sur chacune).
    pub page_count: u32,
    /// Taille de police du style « title » (`titleFontSize`).
    pub title_font_size: f64,
    /// Taille de police du style « subtitle » (`subtitleFontSize`).
    pub subtitle_font_size: f64,
    /// Taille de police du style « composer » (`composerFontSize`).
    pub composer_font_size: f64,
    /// Taille de police du style « lyricist » (`lyricistFontSize`).
    pub lyricist_font_size: f64,
    /// Taille de police du style « frame » (`frameFontSize`).
    pub frame_font_size: f64,
    /// Taille de police du style « instrument_excerpt ».
    pub instrument_excerpt_font_size: f64,
    /// Taille de police du pied de page (`footerFontSize`), utilisée pour le
    /// crédit `rights` du copyright.
    pub footer_font_size: f64,
    /// Hauteur d'un soufflet en espaces (`hairpinHeight`), exportée en
    /// `spread` (dixièmes) sur les `<wedge>`.
    pub hairpin_height_sp: f64,
}

impl StyleSettings {
    /// Valeurs par défaut de MuseScore pour une version majeure donnée.
    ///
    /// Les tailles de page sont les mêmes (A4), mais les marges et certaines
    /// tailles de police diffèrent entre MuseScore 3.x et 4.x : les `.mscx`
    /// du corpus qui n'ont pas de `<Style>` produisent respectivement des
    /// marges de 10/20 mm et 15 mm, et des titres en 24 pt puis 22 pt.
    pub fn for_version(major: u32) -> Self {
        let legacy = major <= 3;
        let (spatium_mm, margins, composer_size, title_size, hairpin_height) = if legacy {
            // MuseScore 3.x : marges 10 mm (20 mm en bas), compositeur 12 pt.
            (1.764, (0.3937008, 0.7874016), 12.0, 24.0, 1.2)
        } else {
            // MuseScore 4.x : marges 15 mm, compositeur 10 pt.
            (1.75, (0.5905512, 0.5905512), 10.0, 22.0, 1.15)
        };
        let (vertical, bottom) = margins;
        Self {
            spatium_mm,
            page_width_in: 8.27,
            page_height_in: 11.69,
            page_even_left_margin_in: vertical,
            page_odd_left_margin_in: vertical,
            page_even_right_margin_in: vertical,
            page_odd_right_margin_in: vertical,
            page_even_top_margin_in: vertical,
            page_odd_top_margin_in: vertical,
            page_even_bottom_margin_in: bottom,
            page_odd_bottom_margin_in: bottom,
            page_two_sided: true,
            lyrics_placement: Placement::Below,
            page_count: 1,
            title_font_size: title_size,
            subtitle_font_size: 14.0,
            composer_font_size: composer_size,
            lyricist_font_size: composer_size,
            frame_font_size: 12.0,
            instrument_excerpt_font_size: 18.0,
            footer_font_size: 8.0,
            hairpin_height_sp: hairpin_height,
        }
    }

    /// Applique une valeur de `<Style>`.
    ///
    /// Les noms sont ceux du `.mscx` (`pageWidth`, `lyricsPlacement`…). Les
    /// valeurs non reconnues ou non numériques sont ignorées : le style garde
    /// alors sa valeur par défaut.
    pub fn set_value(&mut self, name: &str, value: &str) {
        let value = value.trim();
        match name {
            "Spatium" => Self::set_positive(&mut self.spatium_mm, value),
            "pageWidth" => Self::set_positive(&mut self.page_width_in, value),
            "pageHeight" => Self::set_positive(&mut self.page_height_in, value),
            "pageEvenLeftMargin" => Self::set_number(&mut self.page_even_left_margin_in, value),
            "pageOddLeftMargin" => Self::set_number(&mut self.page_odd_left_margin_in, value),
            "pageEvenRightMargin" => Self::set_number(&mut self.page_even_right_margin_in, value),
            "pageOddRightMargin" => Self::set_number(&mut self.page_odd_right_margin_in, value),
            "pageEvenTopMargin" => Self::set_number(&mut self.page_even_top_margin_in, value),
            "pageOddTopMargin" => Self::set_number(&mut self.page_odd_top_margin_in, value),
            "pageEvenBottomMargin" => Self::set_number(&mut self.page_even_bottom_margin_in, value),
            "pageOddBottomMargin" => Self::set_number(&mut self.page_odd_bottom_margin_in, value),
            "pageTwosided" => self.page_two_sided = flag(value),
            "lyricsPlacement" => {
                self.lyrics_placement = match value {
                    "above" | "1" => Placement::Above,
                    _ => Placement::Below,
                };
            }
            "titleFontSize" => Self::set_number(&mut self.title_font_size, value),
            "subtitleFontSize" => Self::set_number(&mut self.subtitle_font_size, value),
            "composerFontSize" => Self::set_number(&mut self.composer_font_size, value),
            "lyricistFontSize" => Self::set_number(&mut self.lyricist_font_size, value),
            "frameFontSize" => Self::set_number(&mut self.frame_font_size, value),
            "instrumentExcerptFontSize" => {
                Self::set_number(&mut self.instrument_excerpt_font_size, value);
            }
            "footerFontSize" => Self::set_number(&mut self.footer_font_size, value),
            "hairpinHeight" => Self::set_positive(&mut self.hairpin_height_sp, value),
            _ => {}
        }
    }

    fn set_number(field: &mut f64, value: &str) {
        if let Ok(number) = value.parse::<f64>() {
            if number >= 0.0 && number.is_finite() {
                *field = number;
            }
        }
    }

    fn set_positive(field: &mut f64, value: &str) {
        if let Ok(number) = value.parse::<f64>() {
            if number > 0.0 && number.is_finite() {
                *field = number;
            }
        }
    }

    /// Dixièmes par pouce (`INCH / millimeters * tenths`).
    pub fn tenths_per_inch(&self) -> f64 {
        25.4 * f64::from(TENTHS) / (4.0 * self.spatium_mm)
    }

    /// Dixièmes par millimètre.
    pub fn tenths_per_mm(&self) -> f64 {
        f64::from(TENTHS) / (4.0 * self.spatium_mm)
    }

    /// Dixièmes par spatium (toujours 10).
    pub fn tenths_per_spatium(&self) -> f64 {
        f64::from(TENTHS) / 4.0
    }

    /// Largeur de page en dixièmes.
    pub fn page_width_tenths(&self) -> f64 {
        self.tenths_from_inches(self.page_width_in)
    }

    /// Hauteur de page en dixièmes.
    pub fn page_height_tenths(&self) -> f64 {
        self.tenths_from_inches(self.page_height_in)
    }

    /// Convertit des pouces en dixièmes.
    pub fn tenths_from_inches(&self, inches: f64) -> f64 {
        inches * self.tenths_per_inch()
    }

    /// Convertit des millimètres en dixièmes.
    pub fn tenths_from_mm(&self, millimeters: f64) -> f64 {
        millimeters * self.tenths_per_mm()
    }

    /// Décalage vertical du style « subtitle » sous le haut du cadre.
    ///
    /// MuseScore place le sous-titre 10 mm sous le titre ; la position est
    /// indépendante du format de page et du contenu des autres textes.
    pub fn subtitle_offset_tenths(&self) -> f64 {
        self.tenths_from_mm(10.0)
    }

    /// Hauteur d'un cadre vertical en dixièmes.
    ///
    /// La hauteur d'un `VBox` est stockée en spatiums ; elle sert au calcul de
    /// `default-y` des crédits alignés en bas (`textAsCreditWords` soustrait
    /// `parentHeight(text)`).
    pub fn vbox_height_tenths(&self, height_spatium: f64) -> f64 {
        height_spatium * self.tenths_per_spatium()
    }

    /// Taille de police par défaut d'un style de crédit.
    pub fn credit_font_size(&self, style: &CreditStyle) -> f64 {
        match style {
            CreditStyle::Title => self.title_font_size,
            CreditStyle::Subtitle => self.subtitle_font_size,
            CreditStyle::Composer => self.composer_font_size,
            CreditStyle::Lyricist => self.lyricist_font_size,
            CreditStyle::Other(name) => match name.to_ascii_lowercase().as_str() {
                "frame" => self.frame_font_size,
                "instrument_excerpt" => self.instrument_excerpt_font_size,
                "rights" => self.footer_font_size,
                // Style inconnu : MuseScore retombe sur la police des textes
                // de portée (`staffTextFontSize`, 10 pt par défaut).
                _ => 10.0,
            },
        }
    }
}

impl Default for StyleSettings {
    fn default() -> Self {
        Self::for_version(4)
    }
}

/// Lit un booléen de style (`0`/`1`).
fn flag(value: &str) -> bool {
    !matches!(value, "0" | "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marges_par_defaut_selon_la_version() {
        let v3 = StyleSettings::for_version(3);
        // Spatium par défaut 1,764 mm : 11,69 in valent 1683,25 dixièmes.
        assert!((v3.page_height_tenths() - 1683.25).abs() < 0.01);
        assert!((v3.tenths_from_inches(v3.page_odd_top_margin_in) - 56.69).abs() < 0.01);
        assert!((v3.tenths_from_inches(v3.page_odd_bottom_margin_in) - 113.3787).abs() < 0.01);

        let v4 = StyleSettings::for_version(4);
        assert!((v4.page_width_tenths() - 1200.33).abs() < 0.01);
        assert!((v4.tenths_from_inches(v4.page_odd_top_margin_in) - 85.71).abs() < 0.01);
        assert_eq!(v4.title_font_size, 22.0);
        assert_eq!(v3.title_font_size, 24.0);
        // `lyricsPlacement` par défaut : below, dans les deux versions.
        assert_eq!(v3.lyrics_placement, Placement::Below);
        assert_eq!(v4.lyrics_placement, Placement::Below);
    }

    #[test]
    fn surcharges_de_style() {
        let mut style = StyleSettings::for_version(4);
        style.set_value("Spatium", "1.76389");
        style.set_value("pageWidth", "8.5");
        style.set_value("pageHeight", "11");
        style.set_value("pageOddLeftMargin", "0.393701");
        style.set_value("lyricsPlacement", "above");
        style.set_value("titleFontSize", "30");

        assert!((style.page_width_tenths() - 1224.0).abs() < 0.01);
        assert!((style.page_height_tenths() - 1584.0).abs() < 0.01);
        assert!((style.tenths_from_inches(style.page_odd_left_margin_in) - 56.69).abs() < 0.01);
        assert_eq!(style.lyrics_placement, Placement::Above);
        assert_eq!(style.title_font_size, 30.0);

        // Une marge nulle est valide (MuseScore l'accepte) ; un spatium nul
        // serait invalide et garde sa valeur.
        style.set_value("pageEvenTopMargin", "0");
        assert_eq!(style.page_even_top_margin_in, 0.0);
        style.set_value("Spatium", "0");
        assert!(style.spatium_mm > 0.0);
    }
}
