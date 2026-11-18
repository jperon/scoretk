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

//! Robustesse des entrées de la façade : toute entrée invalide doit renvoyer
//! une `Err` typée, jamais paniquer (un panic est un abort en wasm).

#![cfg(all(feature = "midi-export", feature = "musicxml-import"))]

mod common;

use scoretk::{read, Format};

fn assert_erreur(label: &str, bytes: &[u8], format: Format) {
    // `catch_unwind` transforme un panic éventuel en échec lisible ; le
    // résultat normal doit être une `Err` de la façade.
    let outcome = std::panic::catch_unwind(|| read(bytes, format));
    match outcome {
        Ok(Ok(score)) => panic!("{label} : entrée invalide lue sans erreur ({score:?})"),
        Ok(Err(error)) => {
            println!("{label} : {error}");
        }
        Err(_) => panic!("{label} : panic dans le lecteur"),
    }
}

#[test]
fn entrees_invalides_sans_panic() {
    // 1. Entrée vide.
    assert_erreur("vide / mscx", &[], Format::Mscx);

    // 2. ZIP valide sans `.mscx` interne.
    let zip_sans_mscx = common::zip_from_entries(&[("lisez-moi.txt", b"pas un .mscx")]);
    assert_erreur("ZIP sans .mscx", &zip_sans_mscx, Format::Mscz);

    // 3. ZIP tronqué : premiers octets d'un `.mscz` valide.
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let mscz = common::mscz_from_mscx(&mscx);
    let coupe = mscz.len().min(64);
    assert_erreur("ZIP tronqué", &mscz[..coupe], Format::Mscz);

    // 4. MusicXML mal formé.
    assert_erreur(
        "MusicXML mal formé",
        b"<score-partwise><part",
        Format::MusicXml,
    );

    // 5. Octets aléatoires, en MuseScore et en MIDI.
    let aleatoire: Vec<u8> = (0..256u16)
        .map(|value| (value.wrapping_mul(37) ^ 0x5a) as u8)
        .collect();
    assert_erreur("octets aléatoires / mscx", &aleatoire, Format::Mscx);
    assert_erreur("octets aléatoires / midi", &aleatoire, Format::Midi);

    // 6. MIDI tronqué : en-tête `MThd` et quatre octets seulement.
    assert_erreur("MIDI tronqué", b"MThd\0\0\0\x06", Format::Midi);
}

/// Remplace la taille décompressée annoncée par les en-têtes ZIP (local et
/// central) par une valeur énorme, sans toucher aux données compressées.
fn annoncer_taille_zip_geante(mut mscz: Vec<u8>) -> Vec<u8> {
    const GEANTE: u32 = u32::MAX;
    let mut index = 0;
    while index + 4 <= mscz.len() {
        let signature = &mscz[index..index + 4];
        let offset = if signature == b"PK\x03\x04" {
            index + 22 // taille décompressée de l'en-tête local
        } else if signature == b"PK\x01\x02" {
            index + 24 // taille décompressée de l'en-tête central
        } else {
            index += 1;
            continue;
        };
        if offset + 4 <= mscz.len() {
            mscz[offset..offset + 4].copy_from_slice(&GEANTE.to_le_bytes());
        }
        index += 1;
    }
    mscz
}

#[test]
fn zip_taille_annoncee_geante_allocation_bornee() {
    // Une entrée ZIP peut annoncer une taille décompressée démesurée : la
    // pré-allocation est plafonnée (`MAX_ENTRY_CAPACITY`) au lieu de réserver
    // la taille annoncée, qui provoquerait un abort d'allocation en wasm.
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let mscz = annoncer_taille_zip_geante(common::mscz_from_mscx(&mscx));
    let score = read(&mscz, Format::Mscz).expect("ZIP à taille annoncée géante lisible");
    assert!(!score.measures.is_empty(), "partition vide après lecture");
}
