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
# this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

# `lily/audio-item.cc:122` : une grâce sonne à `main + 9/40 × grace_part` ;
# un groupe de plusieurs grâces peut donc insérer plus d'un temps avant sa
# note principale (`grace-auto-beam-engraver.ly` : 1,2 temps).

from ly_notes import apparier_graces


def test_groupe_de_graces_plus_long_qu_un_temps():
    reference = [
        (72, 34.80, 0.05, 0),
        (72, 34.90, 0.05, 0),
        (71, 35.00, 0.05, 0),
        (72, 36.00, 4.0, 0),
    ]
    graces = [(72, 36.0, False), (72, 36.0, False), (71, 36.0, False)]
    _, refs, tests, matched = apparier_graces(reference, graces, window=2.0)
    assert (refs, tests, matched) == (3, 3, 3), (refs, tests, matched)


if __name__ == "__main__":
    test_groupe_de_graces_plus_long_qu_un_temps()
    print("ly_notes test_graces_groupe_long : OK")
