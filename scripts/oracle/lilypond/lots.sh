#!/usr/bin/env bash
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

# Construit les manifestes des trois lots à partir de la colonne 6 des listes
# de référence hors dépôt. Aucune donnée tierce n'est copiée dans le dépôt.
#
# Usage : BASEDIR=<dossier des *-base.txt> scripts/oracle/lilypond/lots.sh <sortie>
# Le dossier par défaut est l'espace de travail hors dépôt utilisé par la
# campagne de mesure ; le surcharger pour une autre installation.

set -euo pipefail

# Règle d'extraction : sauter exactement les cinq premiers champs et imprimer
# le reste de la ligne tel quel. `print $6` tronque au premier espace du chemin.
# `index($0, $6)` n'est pas sûr non plus : le sixième champ peut apparaître
# plus tôt dans la ligne. `match` sur les cinq champs initiaux conserve les
# espaces consécutifs et ne dépend pas du contenu du chemin.
EXTRACTION='match($0, /^([^[:space:]]+[[:space:]]+){5}/) { print substr($0, RLENGTH + 1) }'

# Vérification réduite de la règle, avec les deux chemins réels du lot `ra`
# qui avaient été coupés avant la correction (Ave Maria De Lourdes).
if [[ ${LOTS_SELFTEST:-0} == 1 ]]; then
  while IFS='|' read -r ligne attendu; do
    extrait=$(printf '%s\n' "$ligne" | awk "$EXTRACTION")
    [[ $extrait == "$attendu" ]] || {
      echo "selftest lots.sh : chemin avec espaces tronqué : $extrait (attendu $attendu)" >&2
      exit 1
    }
  done <<'CAS'
id123 258 341 258 0 /tmp/Jean Baptiste/ Sonis (copie 1).ly|/tmp/Jean Baptiste/ Sonis (copie 1).ly
id456 1 2 3 0 .../lyluatex/ly/Ave Maria De Lourdes/Ave Maria De Lourdes.ly|.../lyluatex/ly/Ave Maria De Lourdes/Ave Maria De Lourdes.ly
id789 1 2 3 0 .../lyluatex/ly/Ave Maria De Lourdes/Ave Maria.ly|.../lyluatex/ly/Ave Maria De Lourdes/Ave Maria.ly
id000 1 2 3 0 /tmp/entre_ le_ boeuf.ly|/tmp/entre_ le_ boeuf.ly
CAS
  echo 'lots.sh selftest OK'
  exit 0
fi

OUT=${1:?dossier de sortie exigé}
BASEDIR=${BASEDIR:-/tmp/nix-shell.7XCuVq/claude-1000/-home-jacques-programmes-musique-musescore/2ae55baa-d7cc-4f59-831d-f77ca3cc7ebd/scratchpad/xmlcmp}
mkdir -p "$OUT/manifestes"
for lot in mus ra mut; do
  source="$BASEDIR/$lot-base.txt"
  if [[ ! -f "$source" ]]; then
    echo "liste absente : $source (poser BASEDIR)" >&2
    exit 1
  fi
  # Les cinq premiers champs sont séparés par des espaces ; le sixième est un
  # chemin qui peut lui-même contenir des espaces (`Veni, Creator …`, `entre_
  # le_ boeuf`, `Ave Maria De Lourdes/Ave Maria.ly`). On imprime tout le reste
  # de la ligne à partir du sixième champ, sans normaliser les espaces.
  awk "$EXTRACTION" "$source" > "$OUT/manifestes/$lot.txt"
  # Garde-fou : une ligne de manifeste est un chemin absolu, jamais une entrée
  # vide ni un reste de champ tronqué par un ancien générateur.
  awk 'NF == 0 || $0 !~ /^\// { print FILENAME ":" FNR ": chemin invalide : " $0 > "/dev/stderr"; e = 1 } END { exit e }' \
    "$OUT/manifestes/$lot.txt"
done
for f in "$OUT"/manifestes/*.txt; do
  printf '%-8s %6d\n' "$(basename "$f")" "$(wc -l < "$f")"
done
