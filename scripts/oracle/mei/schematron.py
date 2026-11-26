#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Ce fichier est original : exécution des règles Schematron intégrées au
# schéma RelaxNG normatif MEI 5.1 (`mei-all-5.1.rng`), que `jing` ne vérifie
# pas. Les règles sont extraites du RNG, puis compilées en une feuille XSLT
# 3.0 par ce script (aucun stylesheet tiers n'est téléchargé ni exécuté) et
# appliquées avec Saxon HE. L'orthographe des attributs et des éléments est
# simultanément vérifiée par `jing` dans `valide-corpus.py`.
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

"""Run the ISO Schematron assertions embedded in MEI 5.1's RelaxNG schema.

``jing`` validates the RelaxNG grammar only. The normative
``mei-all-5.1.rng`` also contains 249 embedded ISO Schematron assertions
(``sch:pattern``/``sch:rule`` between ``sch:assert`` and ``sch:report``
elements). This script extracts them from the local RNG, compiles them into
one XSLT 3.0 stylesheet (no downloaded stylesheet is executed), and applies
it with Saxon HE 11 from nixpkgs.

Usage::

  schematron.py --rng <schema> --jar <saxon-he.jar> --mei <file> [<file> ...]
  schematron.py --selftest
"""

from __future__ import annotations

import argparse
from collections import Counter
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

SCH = "{http://purl.oclc.org/dsdl/schematron}"
MEI_NS = "http://www.music-encoding.org/ns/mei"

# The assertions in mei-all-5.1.rng use these namespace prefixes.
PREFIXES = {
    "mei": MEI_NS,
    "xml": "http://www.w3.org/XML/1998/namespace",
    "sch": "http://purl.oclc.org/dsdl/schematron",
    "rng": "http://relaxng.org/ns/structure/1.0",
    "xi": "http://www.w3.org/2001/XInclude",
    "a": "http://relaxng.org/ns/compatibility/annotations/1.0",
}


def xml_escape(value: str) -> str:
    """Escape an XPath expression or a message inside an XSLT attribute."""
    return (
        value.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def extract(rng: Path) -> list[tuple[str, list[tuple[str, str, str, str]]]]:
    """Group embedded rules by context: (context, [(kind, test, role, msg)])."""
    root = ET.parse(rng).getroot()
    rules: dict[str, list[tuple[str, str, str, str]]] = {}
    for rule in root.iter(SCH + "rule"):
        context = rule.get("context", ".")
        lets = [(child.get("name", ""), child.get("value", ""))
                for child in rule if child.tag == SCH + "let"]
        checks: list[tuple[str, str, str, str]] = []
        for child in rule:
            if child.tag not in (SCH + "assert", SCH + "report"):
                continue
            test = child.get("test", "true()")
            role = child.get("role", "error")
            message = " ".join("".join(child.itertext()).split())
            introduced: set[str] = set()
            # Iterate: a let's @value can itself reference another let of
            # the same rule (`$stafflines` uses `$thisstaff` in mei-all).
            for _ in range(len(lets) + 1):
                changed = False
                for name, value in lets:
                    if name in introduced:
                        continue
                    if re.search(rf"\${re.escape(name)}\b", test):
                        select = xml_escape(value)
                        test = f"let ${name} := ({select}) return ({test})"
                        introduced.add(name)
                        changed = True
                if not changed:
                    break
            checks.append((child.tag[len(SCH):], test, role, message))
        if checks:
            rules.setdefault(context, []).extend(checks)
    return sorted(rules.items())


def stylesheet(rules: list[tuple[str, list[tuple[str, str, str, str]]]]) -> str:
    """Generate a self-contained XSLT 3.0 checker for the extracted rules."""
    ns = " ".join(f'xmlns:{key}="{value}"' for key, value in PREFIXES.items())
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<xsl:stylesheet version="3.0" {ns} '
        'xmlns:xsl="http://www.w3.org/1999/XSL/Transform" exclude-result-prefixes="sch rng xi a">',
        '<xsl:output method="text" encoding="UTF-8"/>',
        '<xsl:template match="/"><xsl:apply-templates select="*|@*"/></xsl:template>',
        '<xsl:template match="text()|comment()|processing-instruction()"/>',
        '<xsl:template match="@*"/>',
    ]
    for index, (context, checks) in enumerate(rules):
        lines.append(f'<xsl:template match="{xml_escape(context)}">')
        for kind, test, role, message in checks:
            # assert: the rule holds when the test is true; report: the rule
            # is violated when the test is true.
            condition = f"not({test})" if kind == "assert" else f"({test})"
            line = (
                f"{role}|{index}|{xml_escape(context)}|{xml_escape(message)}"
            )
            lines.append(
                f'<xsl:if test="{xml_escape(condition)}">'
                f'<xsl:text>{line}&#10;</xsl:text></xsl:if>'
            )
        lines.append('<xsl:apply-templates select="*|@*"/>')
        lines.append("</xsl:template>")
    lines.append("</xsl:stylesheet>")
    return "\n".join(lines) + "\n"


def run(rng: Path, jar: Path, paths: list[Path]) -> Counter:
    rules = extract(rng)
    with tempfile.TemporaryDirectory(prefix="scoretk-schematron-") as scratch:
        xsl = Path(scratch) / "mei-schematron.xsl"
        xsl.write_text(stylesheet(rules), encoding="utf-8")
        failures: Counter[str] = Counter()
        for path in paths:
            result = subprocess.run(
                ["java", "-cp", str(jar), "net.sf.saxon.Transform",
                 f"-s:{path}", f"-xsl:{xsl}"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=120,
            )
            if result.returncode != 0:
                print(f"  SAXON FAILURE {path.name}: "
                      f"{result.stderr.decode('utf-8', 'replace').strip()[:400]}",
                      file=sys.stderr)
                failures["saxon-failure"] += 1
                continue
            for line in result.stdout.decode("utf-8", "replace").splitlines():
                role = line.split("|", 1)[0]
                failures[role] += 1
        return failures


def selftest() -> None:
    rules = extract(Path(os.environ.get(
        "SCORETK_MEI_RNG",
        "/home/jacques/programmes/musique/specs/mei/mei-all-5.1.rng")))
    assert rules, "the RNG must contain Schematron rules"
    xslt = stylesheet(rules)
    assert 'xmlns:mei=' in xslt and 'xsl:stylesheet' in xslt
    assert "@corresp" in xslt, "the @corresp warning must be compiled"
    print(f"selftest: {len(rules)} contexts, {sum(len(c) for _, c in rules)} assertions")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rng", type=Path, help="local MEI 5.1 RelaxNG schema")
    parser.add_argument("--jar", type=Path, help="Saxon HE jar (from nixpkgs)")
    parser.add_argument("--mei", type=Path, nargs="*", default=[])
    parser.add_argument("--selftest", action="store_true")
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.rng or not args.rng.is_file():
        parser.error("--rng must point to the local mei-all-5.1.rng")
    if not args.jar or not args.jar.is_file():
        parser.error("--jar must point to saxon-he-*.jar")
    if not args.mei:
        parser.error("at least one --mei file is required")
    failures = run(args.rng, args.jar, args.mei)
    counts = ", ".join(f"{role}={count}" for role, count in sorted(failures.items()))
    print(f"Schematron: {len(args.mei)} file(s), {counts or '0 errors, 0 warnings'}")
    # Warnings are advisory (notably unresolved @corresp targets in
    # third-party encodings); only hard errors fail the oracle.
    sys.exit(1 if failures.get("error", 0) or failures.get("saxon-failure", 0) else 0)


if __name__ == "__main__":
    main()
