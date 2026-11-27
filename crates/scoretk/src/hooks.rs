// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Points d'accroche des convertisseurs optionnels.
//!
//! Chaque feature de `Cargo.toml` branche exactement un crate de sens, et le
//! point de branchement est ici, nulle part ailleurs ; la procédure complète
//! est dans `docs/agents/scoretk.md` § 3. Les douze paires fonction/sens :
//!
//! | Feature | Crate | Fonctions |
//! | --- | --- | --- |
//! | `musescore-import` | `scoretk-musescore-import` | [`read_musescore`] |
//! | `musescore-export` | `scoretk-musescore-export` | [`write_mscx`], [`write_mscz`] |
//! | `musicxml-import` | `scoretk-musicxml-import` | [`read_musicxml`], [`read_mxl`] |
//! | `musicxml-export` | `scoretk-musicxml-export` | [`write_musicxml`], [`write_mxl`] |
//! | `midi-import` | `scoretk-midi-import` | [`read_midi`] |
//! | `midi-export` | `scoretk-midi-export` | [`write_midi`], [`write_playback`] |
//! | `lilypond-import` | `scoretk-lilypond-import` | [`read_lilypond`], [`read_lilypond_with_includes`] |
//! | `lilypond-export` | `scoretk-lilypond-export` | [`write_lilypond`] |
//! | `abc-import` | `scoretk-abc-import` | [`read_abc`] |
//! | `abc-export` | `scoretk-abc-export` | [`write_abc`] |
//! | `mei-import` | `scoretk-mei-import` | [`read_mei`] |
//! | `mei-export` | `scoretk-mei-export` | [`write_mei`] |
//! | `gabc-import` | `scoretk-gabc-import` | [`read_gabc`] |
//! | `gabc-export` | `scoretk-gabc-export` | [`write_gabc`] |
//!
//! Chaque fonction a deux définitions exclusives : la vraie sous
//! `#[cfg(feature = "…")]`, et un repli sous `#[cfg(not(feature = "…"))]` qui
//! renvoie [`Error::FormatNonPrisEnCharge`]. Le repli est engendré par la
//! macro [`non_branche`] pour éviter vingt et un corps identiques ; il n'y a
//! aucun `cfg(not(any(…)))` global à maintenir.

use scoretk_core::model::Score;

use crate::error::Error;
use crate::format::Format;
use crate::options::Options;
use crate::Result;

/// Définit le repli d'une fonction d'accroche quand sa feature est absente.
///
/// La signature est celle de la vraie fonction ; tous les arguments sont
/// ignorés et l'erreur porte le format et le sens demandés.
macro_rules! non_branche {
    ($feature:literal, $nom:ident($($arg:ident: $ty:ty),*) -> $ret:ty, $format:expr, $sens:ident) => {
        #[cfg(not(feature = $feature))]
        pub(crate) fn $nom($($arg: $ty),*) -> $ret {
            $(let _ = $arg;)*
            Err(Error::FormatNonPrisEnCharge {
                format: $format,
                sens: crate::format::Sens::$sens,
            })
        }
    };
}

// --- MuseScore (lecture) --------------------------------------------------

/// Lit un `.mscz` ou un `.mscx` dans le modèle pivot.
///
/// `format` ne sert qu'au repli : il nomme le format réellement demandé dans
/// [`Error::FormatNonPrisEnCharge`].
#[cfg(feature = "musescore-import")]
pub(crate) fn read_musescore(bytes: &[u8], _format: Format) -> Result<Score> {
    let container = scoretk_musescore_import::MsczContainer::from_bytes(bytes)?;
    Ok(scoretk_musescore_import::read::read_score(&container)?)
}

non_branche!(
    "musescore-import",
    read_musescore(bytes: &[u8], format: Format) -> Result<Score>,
    format,
    Lecture
);

// --- MuseScore (écriture) -------------------------------------------------

#[cfg(feature = "musescore-export")]
pub(crate) fn write_mscx(score: &Score) -> Result<Vec<u8>> {
    scoretk_musescore_export::write_mscx(score)
        .map(String::into_bytes)
        .map_err(|error| Error::Mscx(error.to_string()))
}

non_branche!(
    "musescore-export",
    write_mscx(score: &Score) -> Result<Vec<u8>>,
    Format::Mscx,
    Ecriture
);

#[cfg(feature = "musescore-export")]
pub(crate) fn write_mscz(score: &Score) -> Result<Vec<u8>> {
    scoretk_musescore_export::write_mscz(score).map_err(|error| Error::Mscx(error.to_string()))
}

non_branche!(
    "musescore-export",
    write_mscz(score: &Score) -> Result<Vec<u8>>,
    Format::Mscz,
    Ecriture
);

// --- MusicXML (lecture) ---------------------------------------------------

#[cfg(feature = "musicxml-import")]
pub(crate) fn read_musicxml(bytes: &[u8]) -> Result<Score> {
    scoretk_musicxml_import::read_musicxml(bytes)
        .map_err(|error| Error::MusicXmlImport(error.to_string()))
}

non_branche!(
    "musicxml-import",
    read_musicxml(bytes: &[u8]) -> Result<Score>,
    Format::MusicXml,
    Lecture
);

#[cfg(feature = "musicxml-import")]
pub(crate) fn read_mxl(bytes: &[u8]) -> Result<Score> {
    scoretk_musicxml_import::read_mxl(bytes)
        .map_err(|error| Error::MusicXmlImport(error.to_string()))
}

non_branche!(
    "musicxml-import",
    read_mxl(bytes: &[u8]) -> Result<Score>,
    Format::Mxl,
    Lecture
);

// --- MusicXML (écriture) --------------------------------------------------

#[cfg(feature = "musicxml-export")]
pub(crate) fn write_musicxml(score: &Score, options: &Options) -> Result<Vec<u8>> {
    Ok(
        scoretk_musicxml_export::export_musicxml_with(score, &options.export_options())?
            .into_bytes(),
    )
}

non_branche!(
    "musicxml-export",
    write_musicxml(score: &Score, options: &Options) -> Result<Vec<u8>>,
    Format::MusicXml,
    Ecriture
);

#[cfg(feature = "musicxml-export")]
pub(crate) fn write_mxl(score: &Score, options: &Options) -> Result<Vec<u8>> {
    Ok(scoretk_musicxml_export::export_mxl_with(
        score,
        &options.export_options(),
    )?)
}

non_branche!(
    "musicxml-export",
    write_mxl(score: &Score, options: &Options) -> Result<Vec<u8>>,
    Format::Mxl,
    Ecriture
);

// --- MIDI (lecture) -------------------------------------------------------

#[cfg(feature = "midi-import")]
pub(crate) fn read_midi(bytes: &[u8]) -> Result<Score> {
    scoretk_midi_import::read_midi(bytes).map_err(|error| Error::MidiImport(error.to_string()))
}

non_branche!(
    "midi-import",
    read_midi(bytes: &[u8]) -> Result<Score>,
    Format::Midi,
    Lecture
);

// --- MIDI (écriture) ------------------------------------------------------

#[cfg(feature = "midi-export")]
pub(crate) fn write_midi(score: &Score, options: &Options) -> Result<Vec<u8>> {
    scoretk_midi_export::write_midi_with(
        score,
        scoretk_midi_export::MidiOptions {
            expand_repeats: options.midi.expand_repeats,
            smf_text: options.midi.smf_text,
            full_support: options.midi_full_support,
        },
    )
    .map_err(|error| Error::Midi(error.to_string()))
}

non_branche!(
    "midi-export",
    write_midi(score: &Score, options: &Options) -> Result<Vec<u8>>,
    Format::Midi,
    Ecriture
);

/// Écrit la sortie MIDI et renvoie aussi la carte des temps de la performance.
///
/// Sous-produit du même rendu : les octets MIDI sont identiques à ceux de
/// [`write_midi`] pour les mêmes options. Absente sans la feature `midi-export`
/// (c'est alors [`crate::write_playback`] qui manque aussi).
#[cfg(feature = "midi-export")]
pub(crate) fn write_playback(
    score: &Score,
    options: &Options,
) -> Result<(Vec<u8>, scoretk_midi_export::Timemap)> {
    scoretk_midi_export::write_midi_with_timemap_and(
        score,
        scoretk_midi_export::MidiOptions {
            expand_repeats: options.midi.expand_repeats,
            smf_text: options.midi.smf_text,
            full_support: options.midi_full_support,
        },
    )
    .map_err(|error| Error::Midi(error.to_string()))
}

// --- LilyPond (lecture) ---------------------------------------------------

/// Rappel de résolution des `\include` LilyPond.
///
/// Reçoit le chemin demandé et celui du fichier qui inclut, et renvoie le
/// texte du fichier inclus ou `None`.
pub(crate) type LilypondResolver<'a> = dyn FnMut(&str, Option<&str>) -> Option<String> + 'a;

#[cfg(feature = "lilypond-import")]
pub(crate) fn read_lilypond(bytes: &[u8]) -> Result<Score> {
    let source = std::str::from_utf8(bytes).map_err(|error| Error::LilyPond(error.to_string()))?;
    scoretk_lilypond_import::read_lilypond(source)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

non_branche!(
    "lilypond-import",
    read_lilypond(bytes: &[u8]) -> Result<Score>,
    Format::Ly,
    Lecture
);

/// Lit un LilyPond `.ly` en résolvant ses `\include` par un rappel.
///
/// Même contrat que `scoretk_lilypond_import::read_lilypond_with_includes` :
/// le rappel reçoit le chemin demandé et celui du fichier qui inclut, et
/// renvoie le texte ou `None`. Aucune entrée/sortie ici : la lecture des
/// fichiers appartient à l'appelant (la CLI).
#[cfg(feature = "lilypond-import")]
pub(crate) fn read_lilypond_with_includes(
    bytes: &[u8],
    resolve: &mut LilypondResolver<'_>,
) -> Result<Score> {
    let source = std::str::from_utf8(bytes).map_err(|error| Error::LilyPond(error.to_string()))?;
    scoretk_lilypond_import::read_lilypond_with_includes(source, resolve)
        .map(|(score, _)| score)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

non_branche!(
    "lilypond-import",
    read_lilypond_with_includes(bytes: &[u8], resolve: &mut LilypondResolver<'_>) -> Result<Score>,
    Format::Ly,
    Lecture
);

/// Lit un `.ly` avec le dialecte grégorien explicitement activé ou non.
#[cfg(feature = "lilypond-import")]
pub(crate) fn read_lilypond_with_dialect(bytes: &[u8], gregorian: bool) -> Result<Score> {
    let source = std::str::from_utf8(bytes).map_err(|error| Error::LilyPond(error.to_string()))?;
    scoretk_lilypond_import::read_lilypond_with_dialect(source, gregorian)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

non_branche!(
    "lilypond-import",
    read_lilypond_with_dialect(bytes: &[u8], gregorian: bool) -> Result<Score>,
    Format::Ly,
    Lecture
);

/// Lit un `.ly` avec résolveur d'`\include` et dialecte grégorien choisi.
#[cfg(feature = "lilypond-import")]
pub(crate) fn read_lilypond_with_includes_and_dialect(
    bytes: &[u8],
    resolve: &mut LilypondResolver<'_>,
    gregorian: bool,
) -> Result<Score> {
    let source = std::str::from_utf8(bytes).map_err(|error| Error::LilyPond(error.to_string()))?;
    scoretk_lilypond_import::read_lilypond_with_includes_and_dialect(source, resolve, gregorian)
        .map(|(score, _)| score)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

// Le repli est écrit à la main : la macro `non_branche` n'accepte pas de
// virgule finale dans une signature aussi longue, que rustfmt introduit.
#[cfg(not(feature = "lilypond-import"))]
pub(crate) fn read_lilypond_with_includes_and_dialect(
    bytes: &[u8],
    resolve: &mut LilypondResolver<'_>,
    gregorian: bool,
) -> Result<Score> {
    let _ = (bytes, resolve, gregorian);
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Ly,
        sens: crate::format::Sens::Lecture,
    })
}

// --- LilyPond (écriture) --------------------------------------------------

#[cfg(feature = "lilypond-export")]
pub(crate) fn write_lilypond(score: &Score, gregorian: bool) -> Result<Vec<u8>> {
    scoretk_lilypond_export::write_lilypond_with(score, gregorian)
        .map(String::into_bytes)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

non_branche!(
    "lilypond-export",
    write_lilypond(score: &Score, gregorian: bool) -> Result<Vec<u8>>,
    Format::Ly,
    Ecriture
);

// --- ABC (lecture) --------------------------------------------------------

#[cfg(feature = "abc-import")]
pub(crate) fn read_abc(bytes: &[u8], gregorian: bool) -> Result<Score> {
    let text = std::str::from_utf8(bytes).map_err(|error| Error::Abc(error.to_string()))?;
    let score = if gregorian {
        scoretk_abc_import::read_abc_gregorian(text)
    } else {
        scoretk_abc_import::read_abc(text)
    };
    score.map_err(|error| Error::Abc(error.to_string()))
}

non_branche!(
    "abc-import",
    read_abc(bytes: &[u8], gregorian: bool) -> Result<Score>,
    Format::Abc,
    Lecture
);

// --- ABC (écriture) -------------------------------------------------------

#[cfg(feature = "abc-export")]
pub(crate) fn write_abc(score: &Score, gregorian: bool) -> Result<Vec<u8>> {
    scoretk_abc_export::write_abc_with(score, gregorian)
        .map(String::into_bytes)
        .map_err(|error| Error::Abc(error.to_string()))
}

non_branche!(
    "abc-export",
    write_abc(score: &Score, gregorian: bool) -> Result<Vec<u8>>,
    Format::Abc,
    Ecriture
);

// --- MEI (lecture) --------------------------------------------------------

#[cfg(feature = "mei-import")]
pub(crate) fn read_mei(bytes: &[u8]) -> Result<Score> {
    scoretk_mei_import::read_mei(bytes).map_err(|error| Error::Mei(error.to_string()))
}

non_branche!(
    "mei-import",
    read_mei(bytes: &[u8]) -> Result<Score>,
    Format::Mei,
    Lecture
);

// --- MEI (écriture) -------------------------------------------------------

#[cfg(feature = "mei-export")]
pub(crate) fn write_mei(score: &Score, options: &Options) -> Result<Vec<u8>> {
    scoretk_mei_export::write_mei_with(
        score,
        &scoretk_mei_export::MeiOptions {
            full_support: options.mei_full_support,
            gabc_private: !options.mei_full_support,
            ..Default::default()
        },
    )
    .map(String::into_bytes)
    .map_err(|error| Error::Mei(error.to_string()))
}

non_branche!(
    "mei-export",
    write_mei(score: &Score, options: &Options) -> Result<Vec<u8>>,
    Format::Mei,
    Ecriture
);

// --- GABC (lecture) -------------------------------------------------------

#[cfg(feature = "gabc-import")]
pub(crate) fn read_gabc(bytes: &[u8]) -> Result<Score> {
    let text = std::str::from_utf8(bytes).map_err(|error| Error::Gabc(error.to_string()))?;
    scoretk_gabc_import::read_gabc(text).map_err(|error| Error::Gabc(error.to_string()))
}

non_branche!(
    "gabc-import",
    read_gabc(bytes: &[u8]) -> Result<Score>,
    Format::Gabc,
    Lecture
);

// --- GABC (écriture) ------------------------------------------------------

#[cfg(feature = "gabc-export")]
pub(crate) fn write_gabc(score: &Score) -> Result<Vec<u8>> {
    scoretk_gabc_export::write_gabc(score)
        .map(String::into_bytes)
        .map_err(|error| Error::Gabc(error.to_string()))
}

non_branche!(
    "gabc-export",
    write_gabc(score: &Score) -> Result<Vec<u8>>,
    Format::Gabc,
    Ecriture
);

// --- Gravure SVG (feature `engrave`) ---------------------------------------

/// Grave une page d'une partition à une seule portée en SVG.
#[cfg(feature = "engrave")]
pub(crate) fn write_svg(score: &Score, page: usize) -> Result<Vec<u8>> {
    let engraver = scoretk_engrave::Engraver::from_score(score)
        .map_err(|error| Error::Engrave(error.to_string()))?;
    Ok(scoretk_render_svg::render_page(&engraver.page(page), engraver.spec()).into_bytes())
}

/// Description compacte d'une page gravée (JSON, coordonnées en 1/1024 sp).
#[cfg(feature = "engrave")]
pub(crate) fn write_layout(score: &Score, page: usize) -> Result<Vec<u8>> {
    let engraver = scoretk_engrave::Engraver::from_score(score)
        .map_err(|error| Error::Engrave(error.to_string()))?;
    Ok(engraver.page_layout(page).to_json().into_bytes())
}

/// Page contenant un identifiant stable, équivalent de `getPageWithElement`.
#[cfg(feature = "engrave")]
pub(crate) fn page_of_element(score: &Score, id: &str) -> Result<Option<usize>> {
    let engraver = scoretk_engrave::Engraver::from_score(score)
        .map_err(|error| Error::Engrave(error.to_string()))?;
    Ok(engraver.page_of_element(id))
}

non_branche!(
    "engrave",
    write_svg(score: &Score, page: usize) -> Result<Vec<u8>>,
    Format::Svg,
    Ecriture
);
