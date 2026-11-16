#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 les auteurs de mscz2xml
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

"""Vérifie que les dépendances liées dans les binaires ont une licence permise.

Le graphe est celui de `cargo tree`, hors réseau (`--offline`) : les crates
dont la licence n'est pas dans la liste blanche ci-dessous, ou dont la licence
est absente/illisible, font échouer le script (code 1). Les crates du
workspace (le projet lui-même) sont ignorées : leur licence est GPL-3.0-only.

Usage :
    scripts/check-licences.py

Le check de flake `licences` l'exécute sur les sources vendues par crane ; en
développement, il fonctionne avec le cache cargo local, sans accès réseau.
"""

import re
import subprocess
import sys

# Licences permissives compatibles GPL-3.0-only (voir docs/licences-tierces.md).
# Toute licence absente de cette liste doit être examinée à la main avant d'être
# ajoutée : ce n'est pas un simple confort, c'est la barrière du check.
LICENCES_PERMISES = {
    "0BSD",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MIT-0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Unlicense",
    "X11",
    "Zlib",
}


def graphe():
    """Retourne {(nom, version): (licence, paquet_du_workspace)} via cargo tree."""
    commande = [
        "cargo", "tree",
        "--offline",
        "--workspace",
        "--edges", "normal,build",
        "--prefix", "none",
        "--format", "{p}|{l}",
    ]
    resultat = subprocess.run(commande, capture_output=True, text=True)
    if resultat.returncode != 0:
        sys.stderr.write(resultat.stderr)
        sys.exit(f"échec de « {' '.join(commande)} »")

    paquets = {}
    for ligne in resultat.stdout.splitlines():
        if "|" not in ligne:
            continue
        description, licence = ligne.split("|", 1)
        description = description.removesuffix(" (*)").strip()
        licence = licence.removesuffix(" (*)").strip()
        # « nom vX.Y.Z (proc-macro) », « nom vX.Y.Z (/chemin/du/workspace) »
        correspondance = re.match(
            r"^(?P<nom>\S+) v(?P<version>\S+)(?: \((?P<suffixe>.*)\))?$",
            description)
        if not correspondance:
            sys.exit(f"ligne de cargo tree incomprise : {description!r}")
        nom = correspondance["nom"]
        version = correspondance["version"]
        suffixe = correspondance["suffixe"] or ""
        interieur = suffixe.startswith("/")
        paquets[(nom, version)] = (licence.strip(), interieur)
    return paquets


def jetons(expression):
    """Découpe une expression SPDX en jetons (identifiants, AND, OR, parenthèses)."""
    brut = expression.replace("(", " ( ").replace(")", " ) ").split()
    resultat = []
    i = 0
    while i < len(brut):
        # « Apache-2.0 WITH LLVM-exception » est un seul identifiant SPDX.
        if i + 2 < len(brut) and brut[i + 1] == "WITH":
            resultat.append(f"{brut[i]} WITH {brut[i + 2]}")
            i += 3
        else:
            resultat.append(brut[i])
            i += 1
    return resultat


def permis(expression):
    """Évalue l'expression SPDX ; AND lie plus fort que OR, comme dans la spec."""
    liste = jetons(expression) if expression else []
    if not liste:
        return False

    def atome(position):
        if position >= len(liste):
            raise ValueError("expression SPDX tronquée")
        jeton = liste[position]
        if jeton == "(":
            valeur, position = ou(position + 1)
            if position >= len(liste) or liste[position] != ")":
                raise ValueError("parenthèse fermante absente")
            return valeur, position + 1
        return jeton in LICENCES_PERMISES, position + 1

    def et(position):
        valeur, position = atome(position)
        while position < len(liste) and liste[position] == "AND":
            droite, position = atome(position + 1)
            valeur = valeur and droite
        return valeur, position

    def ou(position):
        valeur, position = et(position)
        while position < len(liste) and liste[position] == "OR":
            droite, position = et(position + 1)
            valeur = valeur or droite
        return valeur, position

    valeur, position = ou(0)
    if position != len(liste):
        raise ValueError(f"expression SPDX incomprise : {expression!r}")
    return valeur


def main():
    paquets = graphe()
    tierces = {cle: licence for cle, (licence, interieur) in paquets.items()
               if not interieur}
    if not tierces:
        sys.exit("aucune dépendance tierce trouvée : le graphe est-il complet ?")

    en_echec = []
    largeur = max(len(f"{nom} {version}") for nom, version in tierces)
    for (nom, version), licence in sorted(tierces.items()):
        try:
            autorisee = permis(licence)
        except ValueError as erreur:
            sys.exit(f"{nom} {version} : {erreur}")
        etat = "ok" if autorisee else "REFUSÉE"
        print(f"{f'{nom} {version}':<{largeur}}  "
              f"{licence or '(absente)':<40}  {etat}")
        if not autorisee:
            en_echec.append((nom, version, licence))

    print(f"\n{len(tierces)} dépendances tierces liées, "
          f"{len(en_echec)} en échec.")
    if en_echec:
        for nom, version, licence in en_echec:
            print(f"  {nom} {version} : {licence or '(licence absente)'}",
                  file=sys.stderr)
        print("Licence hors liste blanche : vérifier la compatibilité avec "
              "GPL-3.0-only et compléter scripts/check-licences.py.",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
