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

"""Shared helper of the measurement oracles: deliberately unsupported inputs.

`scoretk` exits with ``EXIT_UNSUPPORTED`` (3) when the input is deliberately
not supported: a MuseScore version outside the ported subset, or a format
without a converter in the requested direction (`scoretk::Error::is_unsupported`,
documented in `scoretk --help` and `docs/agents/scoretk.md`). Such a file is
*not* a conversion failure: counting it as one would lower the score for a
limitation announced up front, and would make the next survey look like a
regression.

Every driver therefore:

* treats code 3 as "unsupported", never as "failure" (a reader bug, a
  malformed XML or ZIP keeps its ordinary failure code — the CLI decides);
* excludes the file from its numerator **and** denominator, for scoretk and
  for the alternatives measured on the same batch;
* keeps the fact in the raw output (``exclu`` marker) for traceability;
* prints exactly one warning line per run and one line in ``resume.md``.

This module does not define what scoretk supports: the CLI exit code is the
single source of truth, so adding a supported version needs no change here.
"""

from __future__ import annotations

import sys

EXIT_UNSUPPORTED = 3
"""Exit code of the CLI for a deliberately unsupported input."""

STATUT = "NON_PRIS_EN_CHARGE"
"""Status written in the ABC reading table for an excluded tune."""


def est_non_pris_en_charge(code: int | None) -> bool:
    """True when ``code`` is the CLI's "unsupported input" exit code."""
    return code == EXIT_UNSUPPORTED


def marquer_exclu(releve: dict, source: str = "") -> dict:
    """Mark a raw row as excluded, keeping the input it came from."""
    releve["exclu"] = True
    releve["exclu_fichier"] = source or releve.get("fichier") or releve.get("id") or ""
    return releve


def est_exclu(releve: dict) -> bool:
    """True when the raw row was excluded as unsupported."""
    return bool(releve.get("exclu"))


def filtrer(releves: list[dict]) -> list[dict]:
    """Drop excluded rows before aggregating: neither numerator nor denominator."""
    return [r for r in releves if not est_exclu(r)]


def noms_exclus(releves: list[dict]) -> list[str]:
    """Distinct inputs excluded among the rows, in stable order."""
    noms: list[str] = []
    for r in releves:
        if not est_exclu(r):
            continue
        nom = r.get("exclu_fichier") or r.get("fichier") or r.get("id") or ""
        if nom not in noms:
            noms.append(nom)
    return noms


def compte_exclus(releves: list[dict]) -> int:
    """Number of distinct excluded inputs."""
    return len(noms_exclus(releves))


# Commandes LilyPond supprimées après 2.10 que `convert-ly` réécrit
# (`python/convertrules.py`, règles 2.11.51/52 ; mêmes noms que la table du
# lecteur `crates/scoretk-lilypond-import/src/parser.rs`).
COMMANDES_OBSOLETES = (
    "setTextCresc", "setTextDecresc", "setTextDecr", "setTextDim",
    "setHairpinCresc", "setHairpinDecresc", "setHairpinDim",
    "sustainUp", "sustainDown", "sostenutoUp", "sostenutoDown",
    "arpeggioUp", "arpeggioDown", "arpeggioNeutral", "octave",
)


def commandes_obsoletes(texte: str) -> list[str]:
    """Obsolete commands present in a LilyPond source, without duplication."""
    import re
    presents = set(re.findall(r"\\([A-Za-z]+)", texte))
    return [name for name in COMMANDES_OBSOLETES if name in presents]


def reference_lilypond_inutilisable(message: str) -> bool:
    return message.startswith("lilypond : code ")


def raison_hors_lecteur(releve: dict, texte: str) -> str:
    """Why a row is not the reader's fault, or an empty string.

    Reusable by every `.ly` reading oracle: a missing/unusable MIDI reference,
    a LilyPond compile failure on the source, or obsolete syntax that
    `convert-ly` would rewrite. Such inputs are excluded from both numerator
    and denominator via `marquer_exclu`, exactly like unsupported ones.
    """
    message = releve.get("erreur") or ""
    if "référence MIDI absente" in message or reference_lilypond_inutilisable(message):
        return "référence MIDI inutilisable"
    if releve.get("code") == 124:
        return "délai sur la référence LilyPond"
    obsolete = commandes_obsoletes(texte)
    if obsolete:
        return "source obsolète : " + ", ".join("\\" + name for name in obsolete[:3])
    return ""


def avertir(noms: list[str], prefix: str = "") -> None:
    """Print the single warning line, once per run; no-op when nothing is excluded."""
    if not noms:
        return
    apercu = ", ".join(noms[:5]) + (" …" if len(noms) > 5 else "")
    print(
        f"{prefix}AVERTISSEMENT : {len(noms)} entrée(s) non prise(s) en charge "
        f"(exclues) : {apercu}",
        file=sys.stderr,
    )


def ligne_resume(noms: list[str]) -> str:
    """Markdown line for ``resume.md``: the count, and why it is not a failure."""
    if not noms:
        return "Unsupported (excluded): 0"
    apercu = ", ".join(f"`{nom}`" for nom in noms[:5])
    if len(noms) > 5:
        apercu += " …"
    return (
        f"Unsupported (excluded): {len(noms)} — deliberately unsupported by "
        f"scoretk (exit code {EXIT_UNSUPPORTED}), not counted in numerators or "
        f"denominators: {apercu}"
    )


def selftest() -> None:
    """Check the exclusion contract without any corpus or process."""
    ok = {"fichier": "ok.mscz"}
    exclu = marquer_exclu({"fichier": "vieux.mscz", "ok": False}, "vieux.mscz")
    assert est_exclu(exclu) and not est_exclu(ok)
    assert filtrer([ok, exclu]) == [ok]
    assert noms_exclus([exclu, dict(exclu)]) == ["vieux.mscz"]
    assert compte_exclus([ok]) == 0
    assert "Unsupported (excluded): 0" == ligne_resume([])
    assert ligne_resume(["a.mscz"]).startswith("Unsupported (excluded): 1")
    assert est_non_pris_en_charge(EXIT_UNSUPPORTED)
    assert not est_non_pris_en_charge(1) and not est_non_pris_en_charge(None)
    assert commandes_obsoletes("\\setTextCresc c4") == ["setTextCresc"]
    assert commandes_obsoletes("\\crescTextCresc c4") == []
    assert raison_hors_lecteur({"erreur": "référence MIDI absente"}, "") == "référence MIDI inutilisable"
    assert raison_hors_lecteur({"code": 0}, "\\sustainDown c4").startswith("source obsolète")
    assert raison_hors_lecteur({"code": 0}, "c4 d4") == ""
    print("selftest common : OK")


if __name__ == "__main__":
    selftest()
