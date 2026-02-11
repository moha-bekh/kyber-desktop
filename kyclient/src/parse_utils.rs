// Project Kyber: parse_utils.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use log::error;

pub(crate) fn parse_bitrate(mut bitrate: String) -> Option<u32> {
    let Some(last_char) = bitrate.chars().last() else {
        error!("Empty bitrate");
        return None;
    };

    let scale = match last_char {
        'M' => Some(1_000_000),
        'K' => Some(1_000),
        _ => None,
    };

    if scale.is_some() {
        bitrate = bitrate[0..bitrate.len() - 1].into();
    }

    let bitrate: u32 = match bitrate.parse() {
        Ok(bitrate) => bitrate,
        Err(err) => {
            error!("Invalid bitrate {bitrate}: {err:?}");
            return None;
        }
    };

    match scale {
        Some(scale) => Some(bitrate * scale),
        None => Some(bitrate),
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_bitrate;

    #[test]
    fn bitrate_parser() {
        assert_eq!(parse_bitrate("".into()), None);
        assert_eq!(parse_bitrate("100aM".into()), None);
        assert_eq!(parse_bitrate("100zdee".into()), None);
        assert_eq!(parse_bitrate("100".into()), Some(100));
        assert_eq!(parse_bitrate("1K".into()), Some(1_000));
        assert_eq!(parse_bitrate("230K".into()), Some(230_000));
        assert_eq!(parse_bitrate("1M".into()), Some(1_000_000));
        assert_eq!(parse_bitrate("230M".into()), Some(230_000_000));
    }
}
