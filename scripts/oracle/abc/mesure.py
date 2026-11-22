#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
# FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
# more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see the LICENSE file at the root of this repository or
# <https://www.gnu.org/licenses/gpl-3.0.html>.

"""Relevé de base : lecture ABC → MusicXML, mesurée contre abc2midi.

Chaque fichier ABC est découpé en morceaux X:. Pour chaque morceau on écrit
un fichier autonome (en-tête global + bloc X:), puis on produit :

  * l'oracle  : abc2midi → MIDI, sur une copie où les signes de reprise sont
    remplacés par des barres simples et les ornements retirés (on mesure les
    notes ÉCRITES, pas les notes jouées ; abc2midi n'a pas d'option pour
    désactiver les reprises ni la réalisation des ornements). L'option -NGUI
    écarte les notes de guitare générées par les accords `"C"` : ce sont des
    indications d'harmonie, pas des notes écrites ;
  * scoretk et abc2xml.py reçoivent l'air d'origine, ornements inclus ; seul
    l'oracle est appauvri, pour ne pas compter une interprétation ;
  * l'alternative : abc2xml.py, avec le même oracle.

Les trois sorties sont comparées par cmp.py (N1 hauteurs, N2 attaques).
Les sorties brutes restent dans --out/brut/ ; le relevé est --out/mesures.tsv.
Aucune donnée tierce n'est copiée dans le dépôt.
"""

import argparse
import collections
import hashlib
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import cmp  # noqa: E402
import common  # noqa: E402

FIELD = re.compile(r"^[A-Za-z]\s*:")
TUNE = re.compile(r"^X\s*:\s*\d+")

# abc2midi réalise les ornements en notes supplémentaires (un `Tc` devient
# C-D-C-D-C) : c'est une interprétation, pas une note écrite. Les deux
# candidats ne gardent que le symbole d'ornement, donc l'oracle ne doit pas
# compter cette réalisation. abc2midi n'a aucune option pour la désactiver
# (`-NGRA` ne concerne que les notes de grâce) : on retire donc les ornements
# de la copie destinée à l'oracle. Noms repris de `abc2xml.py` 269,
# `MusicXml.ornMap` (lignes 1096-1097) et `MusicXml.uSyms` (lignes 1115-1116) :
# la table complète est dans README.md. Les ornements longs `!trill!` /
# `+trill+` et les abréviations `T`, `M`, `P`, `~` placées devant une hauteur.
ORNEMENT_LONG = re.compile(
    r"[!+](?:trill|turn|invertedturn|mordent|lowermordent|uppermordent|pralltriller|roll|T)[!+]"
)
# Un agrément court peut suivre immédiatement une note (`EATA>G`) : l'ancien
# lookbehind `(?<![A-Za-z])` ratait ce cas, abc2midi réalisait alors le trille
# en cinq notes et le relevé signalait des notes « manquantes » chez les deux
# candidats (ABC 2.1 §4.14 ; abc2xml.py 269 `ornMap` lignes 1096-1097).
ORNEMENT_COURT = re.compile(r"[TMP~](?=[=^_]?[A-Ga-g])")


def decoupe(src):
    """Découpe un ABC en (en-tête global, [(numéro X:, lignes du morceau)])."""
    with open(src, "r", encoding="utf-8", errors="replace") as handle:
        lines = handle.read().splitlines()
    starts = [i for i, line in enumerate(lines) if TUNE.match(line)]
    if not starts:
        return lines[:0], []
    header = lines[: starts[0]]
    tunes = []
    for n, begin in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(lines)
        number = re.search(r"\d+", lines[begin]).group()
        tunes.append((number, lines[begin:end]))
    return header, tunes


def sans_reprises(lines):
    """Remplace les barres de reprise par des barres simples (lignes de musique).

    La ligne P: est aussi retirée : abc2midi rejoue les parties selon P:, ce
    qui multiplierait les notes de l'oracle alors que MusicXML les écrit une
    seule fois. Les champs [P:A] en ligne sont conservés.
    """
    out = []
    for line in lines:
        if line.lstrip().startswith("P:"):
            continue
        if FIELD.match(line) or line.lstrip().startswith("%"):
            out.append(line)
            continue
        for pattern in (r"::", r"\|:", r":\s*\|+", r"\[[12]", r"\|[12]"):
            line = re.sub(pattern, "|", line)
        out.append(line)
    return out


def sans_ornements(lines):
    """Retire de la copie oracle les ornements réalisés en notes par abc2midi.

    Seules les lignes de musique sont touchées : les champs d'en-tête et les
    directives `%%` sont laissés tels quels. Le mot-clé `U:` (définition de
    symbole) n'est pas modifié non plus : un symbole redéfini qui produirait
    des notes est déjà écarté par `abc2xml.py` et le lecteur, et retirer sa
    définition changerait la lecture des deux candidats, pas seulement
    l'oracle.
    """
    out = []
    for line in lines:
        if FIELD.match(line) or line.lstrip().startswith("%"):
            out.append(line)
            continue
        out.append(ORNEMENT_COURT.sub("", ORNEMENT_LONG.sub("", line)))
    return out


def ecrit(path, lines):
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")


def lance(command, log, budget=45):
    """Exécute une commande avec plafond de temps ; renvoie (code, message).

    Le plafond est configurable depuis ``ecriture.TIMEOUT`` : les copies de
    `Vingt_danses` (17 616 notes) dépassent 10 s et échouaient au hasard.
    """
    try:
        done = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=budget,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return 124, "délai dépassé"
    message = done.stdout.decode("utf-8", errors="replace").strip()
    if message:
        with open(log, "a", encoding="utf-8") as handle:
            handle.write(" ".join(command) + "\n" + message + "\n")
    return done.returncode, message


def cause(status, m):
    """Regroupe les écarts en causes lisibles pour le relevé."""
    if status != "ok":
        return status
    if m["exact1"]:
        return "OK"
    if m["ref"] == 0:
        return "ORACLE_VIDE"
    if m["cand"] == 0:
        return "VIDE"
    if m["shift_notes"] >= m["ref"] / 2 and m["shift"] != 0:
        return "TRANSPOSITION"
    if m["oct"] >= m["ref"] / 2:
        return "OCTAVE"
    if m["cand"] < m["ref"] * 0.6:
        return "REPRISE_OU_TRONCATURE"
    if m["shift_notes"] >= max(5, m["ref"] * 0.15) and m["shift"] != 0:
        return "ARMURE"  # décalage d'une partie seulement des degrés
    if m["com1"] == 0:
        return "DESACCORD_TOTAL"
    if m["com1"] == m["cand"] and m["cand"] < m["ref"]:
        return "NOTES_MANQUANTES"
    if m["com1"] == m["ref"] and m["cand"] > m["ref"]:
        return "NOTES_SUPPLEMENTAIRES"
    if m["com1"] == m["ref"]:
        return "RYTHME"  # hauteurs exactes, attaques fausses
    if m["cand"] > m["ref"] and m["cand"] - m["ref"] <= 4:
        return "ORNEMENT"
    if m["com1"] < m["cand"] and m["com1"] < m["ref"]:
        return "DESACCORD_PARTIEL"  # notes manquantes et supplémentaires mêlées
    return "AUTRE"


def principal():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--liste", required=True, help="fichier : lot<TAB>chemin ABC")
    parser.add_argument("--out", required=True, help="dossier des sorties brutes")
    parser.add_argument("--scoretk", required=True, help="binaire release scoretk")
    parser.add_argument("--abc2xml", help="chemin de abc2xml.py (alternative)")
    parser.add_argument("--limite", type=int, default=0, help="nombre maximal de morceaux")
    args = parser.parse_args()

    brut = os.path.join(args.out, "brut")
    os.makedirs(brut, exist_ok=True)
    rows = []
    saw_error = []
    exclus = []
    with open(args.liste, encoding="utf-8") as handle:
        entries = [line.rstrip("\n").split("\t") for line in handle if line.strip()]
    for entry in entries:
        lot, source = entry[0], entry[1]
        ident = hashlib.md5(source.encode()).hexdigest()[:12]
        header, tunes = decoupe(source)
        if not tunes:
            rows.append((lot, os.path.basename(source), "sans X:", "-", "alternatif",
                         0, 0, 0, 0, 0.0, 0, 0.0, "SANS_X", "SANS_X", source))
            rows.append((lot, os.path.basename(source), "sans X:", "-", "scoretk",
                         0, 0, 0, 0, 0.0, 0, 0.0, "SANS_X", "SANS_X", source))
            continue
        for number, body in tunes:
            if args.limite and len(rows) >= args.limite * 2:
                break
            tag = f"{ident}_t{number}"
            work = os.path.join(brut, tag)
            os.makedirs(work, exist_ok=True)
            air = os.path.join(work, "air.abc")
            plain = os.path.join(work, "air_plain.abc")
            ecrit(air, header + body)
            # L'oracle reçoit une copie sans reprises ni ornements réalisés :
            # on mesure les notes ÉCRITES, une seule fois chacune.
            ecrit(plain, sans_ornements(sans_reprises(header + body)))
            log = os.path.join(work, "journal.log")
            midi = os.path.join(work, "ref.mid")
            xml = os.path.join(work, "score.musicxml")
            alt = os.path.join(work, "alt")
            os.makedirs(alt, exist_ok=True)
            for path in (midi, xml, log):
                if os.path.exists(path):
                    os.remove(path)
            for old in os.listdir(alt):
                os.remove(os.path.join(alt, old))

            # Oracle : abc2midi, appelé depuis le nix-shell qui le fournit.
            r_oracle, _ = lance(
                ["timeout", "10", "abc2midi", plain, "-o", midi, "-silent", "-NGRA", "-NGUI"], log
            )
            r_tk, _ = lance(
                ["timeout", "10", "prlimit", "--as=2000000000", args.scoretk,
                 air, "-o", xml], log
            )
            # Entrée délibérément non prise en charge : le morceau sort du
            # numérateur et du dénominateur, pour scoretk comme pour
            # l'alternative mesurée sur le même morceau.
            if common.est_non_pris_en_charge(r_tk):
                for name in ("scoretk", "alternatif"):
                    rows.append((lot, os.path.basename(source), number, tag, name,
                                 0, 0, 0, 0, 0.0, 0, 0.0, common.STATUT,
                                 common.STATUT, source))
                exclus.append(f"{os.path.basename(source)} X:{number}")
                continue
            r_alt = 1
            if args.abc2xml:
                r_alt, _ = lance(
                    ["timeout", "10", "python3", args.abc2xml, air, "-o", alt], log
                )

            # abc2midi renvoie parfois un code d'erreur pour un avertissement
            # récupérable (accord inconnu…) tout en ayant écrit un MIDI valide :
            # c'est le MIDI produit qui fait foi, pas le code de retour.
            reference = midi if os.path.exists(midi) and os.path.getsize(midi) else None
            # Le nombre de notes de l'oracle est identique pour les deux
            # candidats, y compris quand l'un d'eux échoue : sans cela les
            # pourcentages de notes justes ne sont pas comparables.
            ref_count = sum(cmp.midi(reference)[0].values()) if reference else 0
            for name, candidate, code in (("scoretk", xml, r_tk), ("alternatif", alt, r_alt)):
                status = "ok"
                if reference is None:
                    status = "ORACLE_ECHEC"
                elif code != 0:
                    status = "ECHEC_" + ("ALT" if name == "alternatif" else "TK")
                elif name == "alternatif":
                    files = [os.path.join(alt, f) for f in sorted(os.listdir(alt))
                             if f.endswith(".xml")]
                    candidate = files[0] if files else ""
                    if not candidate:
                        status = "ALT_ECHEC"
                if status != "ok" or not os.path.exists(candidate):
                    if status == "ok":
                        status = "ECHEC_" + ("ALT" if name == "alternatif" else "TK")
                    rows.append((lot, os.path.basename(source), number, tag, name,
                                 ref_count, 0, 0, 0, 0.0, 0, 0.0, status, status, source))
                    if status.startswith("ECHEC") or status == "ORACLE_ECHEC":
                        saw_error.append(f"{tag} {name} {status}")
                    continue
                try:
                    m = cmp.compare(reference, candidate)
                except Exception as error:  # sortie mal formée
                    rows.append((lot, os.path.basename(source), number, tag, name,
                                 ref_count, 0, 0, 0, 0.0, 0, 0.0, "PARSE_ECHEC",
                                 "PARSE_ECHEC", source))
                    saw_error.append(f"{tag} {name} PARSE_ECHEC {error}")
                    continue
                rows.append((lot, os.path.basename(source), number, tag, name,
                             m["ref"], m["cand"], m["com1"], m["oct"], m["n1"],
                             m["com2"], m["n2"], "ok", cause("ok", m), source))

    tsv = os.path.join(args.out, "mesures.tsv")
    with open(tsv, "w", encoding="utf-8") as handle:
        handle.write("lot\tfichier\tx\tid\tcandidat\tref\tcand\tcom1\toct\tn1\t"
                     "com2\tn2\tstatut\tcause\tchemin\n")
        for row in rows:
            handle.write("\t".join(str(v) for v in row) + "\n")
    with open(os.path.join(args.out, "erreurs.log"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(saw_error) + ("\n" if saw_error else ""))
    common.avertir(exclus)
    print(f"{len(rows)} mesures dans {tsv}")


if __name__ == "__main__":
    principal()
