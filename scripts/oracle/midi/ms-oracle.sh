#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon
#
# Oracle LilyPond : import MIDI par MuseScore 4.7.4, en parallèle, avec cache
# hors dépôt et reprise. Écrit `$OUT/done.txt` une fois toutes les conversions
# terminées (condition d'attente pour l'orchestrateur).

set -euo pipefail

MEASURE=${1:-/tmp/oracle-midi-import/beatroot-final}
OUT=${2:-/tmp/oracle-midi-import/ms-oracle}
PARALLEL=${3:-$(nproc)}
mkdir -p "$OUT"

python3 - "$MEASURE" "$OUT" <<'PY'
import json, sys
from pathlib import Path
measure, out = Path(sys.argv[1]), Path(sys.argv[2])
rows = [json.loads(line) for line in (measure / 'import.jsonl').read_text().splitlines()]
with (out / 'manifest.tsv').open('w', encoding='utf-8') as fic:
    for row in rows:
        if not row['id'].startswith('ly-'):
            continue
        safe = row['id'].replace('/', '_').replace(' ', '_')
        fic.write(f"{row['id']}\t{row['mid']}\t{out / (safe + '.musicxml')}\n")
PY

convert_one() {
    local id="$1" mid="$2" dest="$3"
    if [[ -s "$dest" ]]; then
        echo "SKIP $id" >> "$OUT/progress.txt"
        return 0
    fi
    if timeout 240 env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen \
        mscore "$mid" -o "$dest" >/dev/null 2>&1 && [[ -s "$dest" ]]; then
        echo "OK $id" >> "$OUT/progress.txt"
    else
        rm -f "$dest"
        echo "FAIL $id" >> "$OUT/progress.txt"
    fi
}
export -f convert_one
export OUT

: > "$OUT/progress.txt"
rm -f "$OUT/done.txt"
awk -F '\t' '{ printf "%s\t%s\t%s\n", $1, $2, $3 }' "$OUT/manifest.tsv" | \
    xargs -P "$PARALLEL" -d '\n' -I{} bash -c 'IFS=$'"'"'\t'"'"' read -r a b c <<< "{}"; convert_one "$a" "$b" "$c"'

printf 'DONE %s conversions\n' "$(wc -l < "$OUT/manifest.tsv")" > "$OUT/done.txt"
