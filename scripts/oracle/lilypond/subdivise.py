#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Sous-classification des écarts d'un oracle LilyPond par première divergence.
# Pour chaque fichier, on compare les couples (hauteur, attaque quantifiée) du
# MIDI de référence et de notre MusicXML, on repère le premier tick qui diffère,
# puis on relève la commande ou le contexte qui entoure cette mesure dans la
# source. Le résultat est une table (markdown) et un JSON réutilisable.

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent.parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "scripts" / "oracle" / "midi"))

from ly_notes import GRID, quant, read_xml  # noqa: E402
from midi_notes import read_midi  # noqa: E402

# Commandes/contextes significatifs, du plus précis au plus générique.
CONSTRUCTS = [
    ("grace", r"\\(?:grace|afterGrace|acciaccatura|appoggiatura)\b"),
    ("tuplet", r"\\(?:tuplet|times|scaleDurations)\b"),
    ("repeat/volta", r"\\(?:repeat|volta|alternative|segno|coda)\b"),
    ("cue/quote", r"\\(?:cueDuring|quoteDuring|addQuote)\b"),
    ("markup/tweak", r"\\(?:markup|tweak|override|once|mark)\b"),
    ("transpose/clef/key", r"\\(?:transpose|clef|key|ottava)\b"),
    ("voices", r"\\(?:voiceOne|voiceTwo|voiceThree|voiceFour|parallelMusic|partCombine)\b|\\\\"),
    ("lyrics", r"\\(?:lyricsto|lyricmode|addlyrics)\b"),
    ("relative", r"\\relative\b"),
    ("scheme", r"(?:#\(|\$\()"),
    ("include", r"\\include\b"),
    ("score/book", r"\\(?:score|book|bookpart)\b"),
]

BOILERPLATE = re.compile(
    r"\\(?:version|header|language|paper|layout|midi|new|context|set|unset)\b"
)


def charge_manifest(chemin: str | None) -> dict[str, int]:
    """Index du fichier dans le lot, tel que `mesure.py` nomme ses dossiers."""
    if not chemin:
        return {}
    lignes = [l.strip() for l in Path(chemin).read_text().splitlines() if l.strip()]
    return {nom: index for index, nom in enumerate(lignes)}


def autour_source(texte: str, position: float) -> str:
    """Extrait les lignes autour de la fraction `position` du morceau."""
    lignes = [l for l in texte.splitlines() if l.strip()]
    if not lignes:
        return ""
    debut = max(0, int(position * len(lignes)) - 3)
    fin = min(len(lignes), debut + 8)
    return "\n".join(lignes[debut:fin])


def premier_divergence(ref: list[tuple], test: list[tuple]) -> tuple[float, str]:
    """Premier tick où les multisets (hauteur, attaque) diffèrent, et le sens."""
    cr = Counter((int(n[0]), quant(n[1], GRID)) for n in ref)
    ct = Counter((int(n[0]), quant(n[1], GRID)) for n in test)
    manquants = sorted((cr - ct).elements(), key=lambda couple: (couple[1], couple[0]))
    supp = sorted((ct - cr).elements(), key=lambda couple: (couple[1], couple[0]))
    if not manquants and not supp:
        return 0.0, "aucune"
    tick = min([m[1] for m in manquants] + [s[1] for s in supp])
    sens = "note manquante" if any(m[1] == tick for m in manquants) else "note sup"
    return float(tick), sens


def analyse_ligne(args: tuple) -> dict:
    releve, work, index, texte = args
    fichier = releve["fichier"]
    dossier = work / str(index)
    try:
        ref = read_midi(str(dossier / "ref.mid"))
        test = read_xml(str(dossier / "tk.musicxml"))
    except Exception as exc:  # noqa: BLE001 - un fichier illisible ne bloque pas le lot
        return {
            "fichier": fichier,
            "drapeaux": releve.get("drapeaux", []),
            "erreur": str(exc)[:100],
        }
    tick, sens = premier_divergence(ref, test)
    duree = max((n[1] for n in ref), default=1.0) or 1.0
    extrait = autour_source(texte, min(1.0, max(0.0, tick / duree)))
    nettoye = BOILERPLATE.sub("", extrait)
    construct = "sans-construct"
    for nom, patron in CONSTRUCTS:
        if re.search(patron, nettoye):
            construct = nom
            break
    if construct == "sans-construct":
        # L'extrait local ne porte souvent que l'en-tête : repli sur le
        # constructeur dominant de la source entière.
        entier = BOILERPLATE.sub("", texte)
        for nom, patron in CONSTRUCTS:
            if re.search(patron, entier):
                construct = nom
                break
    return {
        "fichier": fichier,
        "nom": Path(fichier).name,
        "drapeaux": releve.get("drapeaux", []),
        "tick": round(tick, 3),
        "sens": sens,
        "construct": construct,
        "extrait": extrait[:500],
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--releve", required=True, help="JSONL de mesures")
    p.add_argument("--travail", required=True, help="dossier travail/ de la mesure")
    p.add_argument("--lot", help="manifeste du lot (pour les index de dossiers)")
    p.add_argument("--drapeau", action="append", help="drapeau à classer (répétable)")
    p.add_argument(
        "--ecarts",
        action="store_true",
        help="ne garder que les fichiers non exacts",
    )
    p.add_argument("--json", help="où écrire le JSON détaillé")
    p.add_argument("--table", help="où écrire la table markdown")
    p.add_argument(
        "--jobs",
        type=int,
        default=min(2, __import__("os").cpu_count() or 1),
        help="nombre de workers (défaut min(2, cpu_count))",
    )
    args = p.parse_args()

    indexes = charge_manifest(args.lot)
    releves = [json.loads(l) for l in Path(args.releve).read_text().splitlines()]
    releves = [
        r
        for r in releves
        if r.get("outil") == "scoretk"
        and not r.get("exclu")
        and (not args.ecarts or not r.get("exact"))
        and (not args.drapeau or any(f in (r.get("drapeaux") or []) for f in args.drapeau))
    ]
    travail = Path(args.travail)
    taches = []
    for releve in releves:
        chemin = Path(releve["fichier"])
        try:
            texte = chemin.read_text(errors="replace")
        except OSError:
            texte = ""
        taches.append((releve, travail, indexes[releve["fichier"]], texte))

    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        resultats = list(pool.map(analyse_ligne, taches))

    if args.json:
        Path(args.json).write_text(
            json.dumps(resultats, ensure_ascii=False, indent=2) + "\n"
        )
    groupes: dict[str, list[str]] = defaultdict(list)
    for r in resultats:
        groupes[r.get("construct", "erreur")].append(r.get("nom", "?"))
    lignes = [
        "| Constructeur à la première divergence | Fichiers | Exemple |",
        "| --- | ---: | --- |",
    ]
    for construct, noms in sorted(groupes.items(), key=lambda kv: -len(kv[1])):
        lignes.append(f"| {construct} | {len(noms)} | `{noms[0]}` |")
    table = "\n".join(lignes) + "\n"
    if args.table:
        Path(args.table).write_text(table)
    print(table, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
