// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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
    /// Masquer les portées vides (`hideEmptyStaves`).
    pub hide_empty_staves: bool,
    /// Ne pas masquer les portées vides du premier système
    /// (`dontHideStavesInFirstSystem`, vrai par défaut).
    pub dont_hide_staves_in_first_system: bool,
    /// Afficher / exporter les hauteurs en sons réels (`Sid::concertPitch`).
    /// MuseScore écrit alors `<concert-score/>` et `pitch2xml` ne soustrait
    /// pas la transposition de l'instrument.
    pub concert_pitch: bool,
    /// Reconstruire les multi-repos à la mise en page (`Sid::createMultiMeasureRests`).
    pub create_multi_measure_rests: bool,
    /// Nombre minimal de mesures vides regroupées en un silence multi-mesures
    /// (`Sid::minEmptyMeasures`).
    ///
    /// MuseScore 4.7.4 vaut 1 (`engraving/style/styledef.cpp:520`), mais les
    /// fichiers 3.x et antérieurs reçoivent 2 par les styles de compatibilité
    /// (`engraving/data/styles/legacy-style-defaults-v302.mss:329`,
    /// `:305` pour v1/v2). Cf. `MeasureLayout::createMultiMeasureRestsIfNeed`
    /// (`engraving/rendering/score/measurelayout.cpp:886-894`).
    pub min_empty_measures: usize,
    /// Agrandissement des portées en petit format (`Sid::smallStaffMag`,
    /// défaut 0.7). `Staff::staffMag` (`staff.cpp:806-809`) le multiplie par
    /// `userMag`, et `writeStaffDetails` (`exportmusicxml.cpp:8139-8144`) en
    /// tire `<staff-size scaling=…>`.
    pub small_staff_mag: f64,
    /// Distance from a bar line (or a system header) to the first chord or
    /// rest, in staff spaces (`Sid::barNoteDistance`, 1.25 sp).
    pub bar_note_distance_sp: f64,
    /// Distance from a bar line to a first chord with accidentals
    /// (`Sid::barAccidentalDistance`, 0.65 sp).
    pub bar_accidental_distance_sp: f64,
    /// Minimum width of a measure (`Sid::minMeasureWidth`, 8 sp).
    pub min_measure_width_sp: f64,
    /// Gap after a time signature at the start of a system
    /// (`Sid::systemHeaderTimeSigDistance`, 2 sp).
    pub system_header_time_sig_distance_sp: f64,
    /// Printable width in inches (`pagePrintableWidth`, 180 mm by default;
    /// 190 mm with the legacy margins). The pages of some corpus files
    /// override it independently of the margins.
    pub page_printable_width_in: f64,
    /// Indent the first system by `firstSystemIndentationValue` when true
    /// (`Sid::enableIndentationOnFirstSystem`, true in 4.x).
    pub enable_indentation_on_first_system: bool,
    /// Left indent of the first system (`Sid::firstSystemIndentationValue`,
    /// 5 sp).
    pub first_system_indentation_sp: f64,
    /// Fill below which the last system is not justified
    /// (`Sid::lastSystemFillLimit`, 0.3).
    pub last_system_fill_limit: f64,
    /// Distance between staves of different parts (`Sid::staffDistance`,
    /// 6.5 sp). // `styledef.cpp:55`.
    pub staff_distance_sp: f64,
    /// Distance between staves of the same part (`Sid::akkoladeDistance`,
    /// 6.5 sp). // `styledef.cpp:57`.
    pub akkolade_distance_sp: f64,
    /// Minimum distance between systems (`Sid::minSystemDistance`, 8.5 sp).
    pub min_system_distance_sp: f64,
    /// Maximum distance between systems when spread is off (15 sp).
    pub max_system_distance_sp: f64,
    /// Minimum distance between systems when spread is on
    /// (`Sid::minSystemSpread`, 8.5 sp). // `styledef.cpp:66`.
    pub min_system_spread_sp: f64,
    /// Whether MuseScore spreads the gaps of a page (`Sid::enableVerticalSpread`).
    pub enable_vertical_spread: bool,
    /// Stretch factor of an inter-system gap (`Sid::spreadSystem`, 2.5).
    pub spread_system: f64,
    /// Stretch factor around a square bracket (`Sid::spreadSquareBracket`, 1.2).
    pub spread_square_bracket: f64,
    /// Stretch factor around a curly brace (`Sid::spreadCurlyBracket`, 1.1).
    pub spread_curly_bracket: f64,
    /// Maximum extra spacing inside a gap (`Sid::maxPageFillSpread`, 6 sp).
    pub max_page_fill_spread_sp: f64,
    /// Spacing bounds used by the vertical spread pass (`styledef.cpp:64-70`).
    pub min_staff_spread_sp: f64,
    pub max_staff_spread_sp: f64,
    pub max_system_spread_sp: f64,
    pub max_akkolade_distance_sp: f64,
    /// Upper/lower borders of the printable area and frame distance
    /// (`styledef.cpp:52-54`, `:139`).
    pub staff_upper_border_sp: f64,
    pub staff_lower_border_sp: f64,
    pub frame_system_distance_sp: f64,
    /// Gap between an instrument name and the staff (`Sid::instrumentNameOffset`).
    pub instrument_name_offset_sp: f64,
    /// Clearance required between two outlines that share an x range
    /// (`Sid::minVerticalDistance`, 0.5 sp). // `styledef.cpp:749`.
    pub min_vertical_distance_sp: f64,
    /// Minimum horizontal overlap for two outlines to constrain each other
    /// (`Sid::skylineMinHorizontalClearance`, 0.25 sp). // `styledef.cpp:750`.
    pub skyline_min_horizontal_clearance_sp: f64,
    /// Offset of the first lyric line below the bottom staff line
    /// (`Sid::lyricsPosBelow`, 3 sp). // `styledef.cpp:75`.
    pub lyrics_pos_below_sp: f64,
    /// Minimum distance from the staff outline to the first lyric line
    /// (`Sid::lyricsMinTopDistance`, 1 sp). // `styledef.cpp:76`.
    pub lyrics_min_top_distance_sp: f64,
    /// Clearance below the last lyric line added to the staff's south skyline
    /// (`Sid::lyricsMinBottomDistance`, 1.5 sp). // `styledef.cpp:77`.
    pub lyrics_min_bottom_distance_sp: f64,
    /// Multiplier of the lyric font's line spacing (`Sid::lyricsLineHeight`,
    /// 1.0). // `styledef.cpp:79`.
    pub lyrics_line_height: f64,
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
            hide_empty_staves: false,
            dont_hide_staves_in_first_system: true,
            concert_pitch: false,
            create_multi_measure_rests: false,
            min_empty_measures: if legacy { 2 } else { 1 },
            small_staff_mag: 0.7,
            // 4.7.4 defaults (`engraving/style/styledef.cpp:143,186,225,227`).
            // The legacy `.mss` compatibility files may override these; the
            // corpus files that need it carry an explicit `<Style>` value.
            bar_note_distance_sp: 1.25,
            bar_accidental_distance_sp: 0.65,
            min_measure_width_sp: 8.0,
            system_header_time_sig_distance_sp: 2.0,
            page_printable_width_in: if legacy { 190.0 / 25.4 } else { 180.0 / 25.4 },
            enable_indentation_on_first_system: !legacy,
            first_system_indentation_sp: 5.0,
            last_system_fill_limit: 0.3,
            // 4.7.4 defaults (`engraving/style/styledef.cpp:52-70,139`).
            staff_distance_sp: 6.5,
            akkolade_distance_sp: 6.5,
            min_system_distance_sp: 8.5,
            max_system_distance_sp: 15.0,
            min_system_spread_sp: 8.5,
            enable_vertical_spread: true,
            spread_system: 2.5,
            spread_square_bracket: 1.2,
            spread_curly_bracket: 1.1,
            max_page_fill_spread_sp: 6.0,
            min_staff_spread_sp: 3.5,
            max_staff_spread_sp: 20.0,
            max_system_spread_sp: 32.0,
            max_akkolade_distance_sp: 6.5,
            staff_upper_border_sp: 7.0,
            staff_lower_border_sp: 7.0,
            frame_system_distance_sp: 7.0,
            instrument_name_offset_sp: 1.0,
            // 4.7.4 defaults (`engraving/style/styledef.cpp:75-79,749-750`).
            min_vertical_distance_sp: 0.5,
            skyline_min_horizontal_clearance_sp: 0.25,
            lyrics_pos_below_sp: 3.0,
            lyrics_min_top_distance_sp: 1.0,
            lyrics_min_bottom_distance_sp: 1.5,
            lyrics_line_height: 1.0,
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
            // MuseScore 4.x writes the key in lower case in `score_style.mss`
            // (`Sid::spatium`, styledef.cpp:752); 3.x inline styles use the
            // capitalised spelling.
            "spatium" | "Spatium" => Self::set_positive(&mut self.spatium_mm, value),
            "smallStaffMag" => Self::set_positive(&mut self.small_staff_mag, value),
            "pageWidth" => Self::set_positive(&mut self.page_width_in, value),
            "pageHeight" => Self::set_positive(&mut self.page_height_in, value),
            "pagePrintableWidth" => Self::set_positive(&mut self.page_printable_width_in, value),
            "enableIndentationOnFirstSystem" => {
                self.enable_indentation_on_first_system = flag(value);
            }
            "firstSystemIndentationValue" => {
                Self::set_number(&mut self.first_system_indentation_sp, value);
            }
            "lastSystemFillLimit" => Self::set_number(&mut self.last_system_fill_limit, value),
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
            "concertPitch" | "displayInConcertPitch" => self.concert_pitch = flag(value),
            "createMultiMeasureRests" => self.create_multi_measure_rests = flag(value),
            "minEmptyMeasures" => {
                if let Ok(count) = value.trim().parse::<usize>() {
                    self.min_empty_measures = count.max(1);
                }
            }
            "hideEmptyStaves" => self.hide_empty_staves = flag(value),
            "dontHideStavesInFirstSystem" => {
                self.dont_hide_staves_in_first_system = flag(value);
            }
            "barNoteDistance" => Self::set_positive(&mut self.bar_note_distance_sp, value),
            "barAccidentalDistance" => {
                Self::set_positive(&mut self.bar_accidental_distance_sp, value);
            }
            "minMeasureWidth" => Self::set_positive(&mut self.min_measure_width_sp, value),
            "systemHeaderTimeSigDistance" => {
                Self::set_positive(&mut self.system_header_time_sig_distance_sp, value);
            }
            "staffDistance" => Self::set_positive(&mut self.staff_distance_sp, value),
            "akkoladeDistance" => Self::set_positive(&mut self.akkolade_distance_sp, value),
            "minSystemDistance" => Self::set_positive(&mut self.min_system_distance_sp, value),
            "maxSystemDistance" => Self::set_positive(&mut self.max_system_distance_sp, value),
            "minSystemSpread" => Self::set_positive(&mut self.min_system_spread_sp, value),
            "enableVerticalSpread" => self.enable_vertical_spread = flag(value),
            "spreadSystem" => Self::set_positive(&mut self.spread_system, value),
            "spreadSquareBracket" => Self::set_positive(&mut self.spread_square_bracket, value),
            "spreadCurlyBracket" => Self::set_positive(&mut self.spread_curly_bracket, value),
            "maxPageFillSpread" => Self::set_positive(&mut self.max_page_fill_spread_sp, value),
            "minStaffSpread" => Self::set_positive(&mut self.min_staff_spread_sp, value),
            "maxStaffSpread" => Self::set_positive(&mut self.max_staff_spread_sp, value),
            "maxSystemSpread" => Self::set_positive(&mut self.max_system_spread_sp, value),
            "maxAkkoladeDistance" => {
                Self::set_positive(&mut self.max_akkolade_distance_sp, value);
            }
            "staffUpperBorder" => Self::set_positive(&mut self.staff_upper_border_sp, value),
            "staffLowerBorder" => Self::set_positive(&mut self.staff_lower_border_sp, value),
            "frameSystemDistance" => Self::set_positive(&mut self.frame_system_distance_sp, value),
            "instrumentNameOffset" => {
                Self::set_positive(&mut self.instrument_name_offset_sp, value);
            }
            "minVerticalDistance" => {
                Self::set_positive(&mut self.min_vertical_distance_sp, value);
            }
            "skylineMinHorizontalClearance" => {
                Self::set_positive(&mut self.skyline_min_horizontal_clearance_sp, value);
            }
            // `lyricsPosBelow` is a PointF in MuseScore; only its y matters
            // for the vertical pass (`lyricslayout.cpp:849`).
            "lyricsPosBelow" => {
                if let Some(y) = value.split(',').nth(1) {
                    Self::set_positive(&mut self.lyrics_pos_below_sp, y);
                }
            }
            "lyricsMinTopDistance" => {
                Self::set_positive(&mut self.lyrics_min_top_distance_sp, value);
            }
            "lyricsMinBottomDistance" => {
                Self::set_positive(&mut self.lyrics_min_bottom_distance_sp, value);
            }
            "lyricsLineHeight" => Self::set_positive(&mut self.lyrics_line_height, value),
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
    fn small_staff_mag_depuis_le_style() {
        let mut style = StyleSettings::default();
        style.set_value("smallStaffMag", "0.9");
        assert!((style.small_staff_mag - 0.9).abs() < 1e-9);
    }

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
