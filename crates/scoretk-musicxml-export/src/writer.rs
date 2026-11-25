// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : l'écrivain XML, portage de
// `engraving/rw/xmlwriter.cpp` de MuseScore Studio, a été déplacé dans
// `scoretk-xml-common` pour être partagé avec l'écrivain MEI (règle des crates
// `-common`).
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

//! Réexport de l'écrivain XML partagé, pour ne pas changer les appelants de
//! l'export MusicXML.

pub use scoretk_xml_common::XmlWriter;
