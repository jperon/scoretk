// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Capabilities reported by `scoretk::formats()` under different feature sets.
//!
//! Run the second test with
//! `cargo test -p scoretk --no-default-features --features abc-import --test formats_capabilities`
//! to check that disabled formats are not advertised.

use scoretk::{formats, Format};

#[test]
fn formats_lists_exactly_the_formats_with_an_active_direction() {
    let infos = formats();
    assert_eq!(
        infos.iter().map(|info| info.format).collect::<Vec<_>>(),
        Format::ALL
            .into_iter()
            .filter(|format| format.supports_read() || format.supports_write())
            .collect::<Vec<_>>()
    );
    for info in &infos {
        assert!(info.available(), "{} has no direction", info.name());
        assert_eq!(info.read, info.format.supports_read());
        assert_eq!(info.write, info.format.supports_write());
        assert_eq!(info.name(), info.format.as_str());
        assert_eq!(info.extensions(), info.format.extensions());
    }
}

#[cfg(all(feature = "abc-import", not(feature = "abc-export")))]
#[test]
fn formats_without_default_features_omits_disabled_formats() {
    let infos = formats();
    assert_eq!(infos.len(), 1, "{infos:?}");
    assert_eq!(infos[0].format, Format::Abc);
    assert!(infos[0].read);
    assert!(!infos[0].write);
    assert_eq!(infos[0].name(), "abc");
    assert_eq!(infos[0].extensions(), &["abc"][..]);
    assert!(!infos.iter().any(|info| info.format == Format::MusicXml));
    assert!(!infos.iter().any(|info| info.format == Format::Svg));
}

#[cfg(feature = "engrave")]
#[test]
fn formats_with_engrave_lists_svg_as_write_only() {
    let svg = formats()
        .into_iter()
        .find(|info| info.format == Format::Svg)
        .expect("svg must be listed when engrave is enabled");
    assert!(!svg.read);
    assert!(svg.write);
    assert_eq!(svg.name(), "svg");
    assert_eq!(svg.extensions(), &["svg"][..]);
}

#[cfg(not(feature = "engrave"))]
#[test]
fn formats_without_engrave_omits_svg() {
    assert!(formats().iter().all(|info| info.format != Format::Svg));
}
