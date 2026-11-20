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

"""Validation MusicXML 4.0 par `xmllint --schema` (nixpkgs libxml2).

La XSD normative est celle des specs locales ; ses deux imports (`xml.xsd`,
`xlink.xsd`) sont résolus par un catalogue XML local, car les URL du W3C ne
sont pas accessibles hors ligne. Les copies de travail vont dans ``--out`` :
aucune donnée tierce n'est versionnée.

Usage autonome :
  validite.py --xsd-dir <dossier> --schema <fichier> <fichier.xml> [...]

Les fonctions ``preparer_xsd`` et ``valider`` sont importées par ``mesure.py``.
"""

from __future__ import annotations

import argparse
from collections import Counter
import os
from pathlib import Path
import shutil
import subprocess

XSD = Path("/home/jacques/programmes/musique/specs/musicxml/musicxml-4.0.xsd")
IMPORTS = Path("/home/jacques/programmes/musique/corpus/musicxml-w3c/spec/schema")


def preparer_xsd(dossier: Path) -> tuple[Path, dict]:
    """Copie la XSD et ses imports, crée le catalogue, renvoie (xsd, environ)."""
    dossier.mkdir(parents=True, exist_ok=True)
    schema = dossier / XSD.name
    shutil.copyfile(XSD, schema)
    for nom in ("xml.xsd", "xlink.xsd"):
        shutil.copyfile(IMPORTS / nom, dossier / nom)
    catalogue = dossier / "catalog.xml"
    catalogue.write_text(
        '<?xml version="1.0"?>\n'
        '<catalog xmlns="urn:oasis:names:tc:entity:xmlns:xml:catalog">\n'
        '  <system systemId="http://www.musicxml.org/xsd/xml.xsd" uri="xml.xsd"/>\n'
        '  <system systemId="http://www.musicxml.org/xsd/xlink.xsd" uri="xlink.xsd"/>\n'
        '</catalog>\n',
        encoding="utf-8",
    )
    env = dict(os.environ)
    env["XML_CATALOG_FILES"] = str(catalogue)
    return schema, env


def valider(schema: Path, env: dict, chemin: Path) -> dict:
    """Valide un fichier ; renvoie ``{"valide", "messages", "erreurs"}``."""
    resultat = subprocess.run(
        ["xmllint", "--noout", "--schema", str(schema), str(chemin)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        check=False,
        timeout=30,
    )
    sortie = (resultat.stdout + resultat.stderr).decode("utf-8", "replace")
    messages: Counter = Counter()
    for ligne in sortie.splitlines():
        if "validity error" not in ligne and "validity warning" not in ligne:
            continue
        # On retire le préfixe « fichier:ligne: element X: Schemas validity … »
        # pour ne garder que le message, qui sert de clé de regroupement.
        message = ligne
        if "Schemas validity error : " in message:
            message = message.split("Schemas validity error : ", 1)[1]
        elif "Schemas validity warning : " in message:
            message = message.split("Schemas validity warning : ", 1)[1]
        messages[message.strip()] += 1
    valide = resultat.returncode == 0 and not messages
    return {
        "valide": valide,
        "messages": dict(messages),
        "erreurs": sum(messages.values()),
        "brut": sortie[-500:] if not valide else "",
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--xsd-dir", required=True, help="dossier de travail pour la XSD")
    p.add_argument("--schema", default="", help="fichier XSD (défaut : copie locale)")
    p.add_argument("fichiers", nargs="+", help="fichiers MusicXML à valider")
    args = p.parse_args()
    dossier = Path(args.xsd_dir)
    schema, env = preparer_xsd(dossier)
    if args.schema:
        schema = Path(args.schema)
    totaux = Counter()
    for f in args.fichiers:
        r = valider(schema, env, Path(f))
        print(f"{f}\t{'VALIDE' if r['valide'] else 'INVALIDE'}\t{r['erreurs']}")
        for message, n in sorted(r["messages"].items()):
            print(f"    {n:4d}  {message}")
        totaux.update(r["messages"])
    if totaux:
        print("\nTotaux par message :")
        for message, n in sorted(totaux.items(), key=lambda kv: -kv[1]):
            print(f"{n:5d}  {message}")


if __name__ == "__main__":
    main()
