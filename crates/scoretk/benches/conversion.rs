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

//! Banc de performance natif de la façade : chaque sens de conversion et les
//! deux chaînes complètes, sur un échantillon du corpus `tests/corpus`.
//!
//! Les entrées sont lues et les modèles construits **hors mesure** : seules
//! les fonctions de la façade sont chronométrées. La méthode, la machine et
//! les résultats mesurés sont consignés dans `docs/agents/performance.md`.
//!
//! Exécution : `nix develop -c cargo bench -p scoretk --bench conversion`.
//! Le profil `bench` hérite de `[profile.release]` (`opt-level = "z"`,
//! `lto = true`) : les chiffres valent pour les binaires livrés.

use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use scoretk::{read, write, write_playback, Format, Options, Score};

/// Un fichier de l'échantillon, entièrement chargé et déjà lu en modèle.
struct Echantillon {
    /// Nom court utilisé dans les identifiants du banc (`v3/moonlight`).
    nom: &'static str,
    /// Octets du `.mscx` amont, tels quels.
    mscx: Vec<u8>,
    /// Octets de la référence MuseScore 4.7.4 (`*.ref.musicxml`).
    musicxml: Vec<u8>,
    /// Modèle pivot lu depuis `mscx` hors mesure.
    modele: Score,
}

/// Les six fichiers mesurés : les deux plus gros en octets
/// (`v3/moonlight`, 409 776 o ; `v4/voiceswitching`, 30 868 o), plus quatre
/// fichiers qui couvrent les deux versions du format, plusieurs portées,
/// plusieurs voix, les reprises et les éléments de mise en page.
const FICHIERS: [(&str, &str, &str); 6] = [
    (
        "v3/moonlight",
        "v3/moonlight.mscx",
        "v3/moonlight.ref.musicxml",
    ),
    (
        "v4/voiceswitching",
        "v4/voiceswitching.mscx",
        "v4/voiceswitching.ref.musicxml",
    ),
    (
        "v4/testExcludeInvisibleElements",
        "v4/testExcludeInvisibleElements.mscx",
        "v4/testExcludeInvisibleElements.ref.musicxml",
    ),
    (
        "v3/layout_elements",
        "v3/layout_elements.mscx",
        "v3/layout_elements.ref.musicxml",
    ),
    ("v3/voices", "v3/voices.mscx", "v3/voices.ref.musicxml"),
    (
        "v4/hairpins_and_repeats",
        "v4/hairpins_and_repeats.mscx",
        "v4/hairpins_and_repeats.ref.musicxml",
    ),
];

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn lire(nom: &str) -> Vec<u8> {
    let chemin = corpus().join(nom);
    std::fs::read(&chemin).unwrap_or_else(|erreur| {
        panic!("corpus illisible ({} : {erreur})", chemin.display());
    })
}

/// Charge l'échantillon une seule fois par processus : lectures et modèles
/// sont prêts avant le premier chronométrage de chaque groupe.
fn echantillons() -> &'static [Echantillon] {
    static ECHANTILLONS: OnceLock<Vec<Echantillon>> = OnceLock::new();
    ECHANTILLONS.get_or_init(|| {
        FICHIERS
            .iter()
            .map(|(nom, mscx, musicxml)| {
                let mscx = lire(mscx);
                let musicxml = lire(musicxml);
                let modele = read(&mscx, Format::Mscx).expect("lecture .mscx de référence");
                Echantillon {
                    nom,
                    mscx,
                    musicxml,
                    modele,
                }
            })
            .collect()
    })
}

/// Enregistre un groupe de benchmark d'écriture : `f` reçoit le modèle et les
/// options, la mesure couvre l'appel complet (allocation et libération de la
/// sortie comprises).
fn groupe_ecriture(
    c: &mut Criterion,
    nom: &str,
    format: Format,
    f: fn(&Score, Format, &Options) -> scoretk::Result<Vec<u8>>,
) {
    let mut groupe = c.benchmark_group(nom);
    for e in echantillons() {
        // Débit rapporté en octets d'entrée `.mscx` par seconde : il rend les
        // fichiers comparables entre eux pour un même sens d'écriture.
        groupe.throughput(Throughput::Bytes(e.mscx.len() as u64));
        let options = Options::default();
        groupe.bench_with_input(BenchmarkId::from_parameter(e.nom), e, |b, e| {
            b.iter(|| {
                black_box(f(black_box(&e.modele), format, black_box(&options)).expect("écriture"))
            });
        });
    }
    groupe.finish();
}

fn lecture_mscx(c: &mut Criterion) {
    let mut groupe = c.benchmark_group("lecture_mscx");
    for e in echantillons() {
        groupe.throughput(Throughput::Bytes(e.mscx.len() as u64));
        groupe.bench_with_input(BenchmarkId::from_parameter(e.nom), e, |b, e| {
            b.iter(|| black_box(read(black_box(&e.mscx), Format::Mscx).expect("lecture .mscx")));
        });
    }
    groupe.finish();
}

fn lecture_musicxml(c: &mut Criterion) {
    let mut groupe = c.benchmark_group("lecture_musicxml");
    for e in echantillons() {
        groupe.throughput(Throughput::Bytes(e.musicxml.len() as u64));
        groupe.bench_with_input(BenchmarkId::from_parameter(e.nom), e, |b, e| {
            b.iter(|| {
                black_box(read(black_box(&e.musicxml), Format::MusicXml).expect("lecture MusicXML"))
            });
        });
    }
    groupe.finish();
}

fn ecriture_musicxml(c: &mut Criterion) {
    groupe_ecriture(c, "ecriture_musicxml", Format::MusicXml, write);
}

fn ecriture_midi(c: &mut Criterion) {
    groupe_ecriture(c, "ecriture_midi", Format::Midi, write);
}

fn ecriture_mscx(c: &mut Criterion) {
    groupe_ecriture(c, "ecriture_mscx", Format::Mscx, write);
}

fn ecriture_lilypond(c: &mut Criterion) {
    groupe_ecriture(c, "ecriture_lilypond", Format::Ly, write);
}

fn ecriture_abc(c: &mut Criterion) {
    groupe_ecriture(c, "ecriture_abc", Format::Abc, write);
}

fn chaine_mscx_musicxml(c: &mut Criterion) {
    let mut groupe = c.benchmark_group("chaine_mscx_musicxml");
    for e in echantillons() {
        groupe.throughput(Throughput::Bytes(e.mscx.len() as u64));
        let options = Options::default();
        groupe.bench_with_input(BenchmarkId::from_parameter(e.nom), e, |b, e| {
            b.iter(|| {
                let modele = read(black_box(&e.mscx), Format::Mscx).expect("lecture .mscx");
                black_box(
                    write(black_box(&modele), Format::MusicXml, black_box(&options))
                        .expect("écriture MusicXML"),
                )
            });
        });
    }
    groupe.finish();
}

/// Équivalent de `convertWithTimeline` du crate wasm : MusicXML identifié,
/// MIDI et carte des temps en un appel.
fn playback(c: &mut Criterion) {
    let mut groupe = c.benchmark_group("playback");
    for e in echantillons() {
        groupe.throughput(Throughput::Bytes(e.mscx.len() as u64));
        let options = Options::default();
        groupe.bench_with_input(BenchmarkId::from_parameter(e.nom), e, |b, e| {
            b.iter(|| {
                black_box(
                    write_playback(black_box(&e.modele), black_box(&options))
                        .expect("write_playback"),
                )
            });
        });
    }
    groupe.finish();
}

criterion_group! {
    name = conversion;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets =
        lecture_mscx,
        lecture_musicxml,
        ecriture_musicxml,
        ecriture_midi,
        ecriture_mscx,
        ecriture_lilypond,
        ecriture_abc,
        chaine_mscx_musicxml,
        playback,
}

criterion_main!(conversion);
