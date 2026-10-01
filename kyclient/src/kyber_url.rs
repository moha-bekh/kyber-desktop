// Project Kyber: kyber_url.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
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

//! Translate a `kyber://` URL into command-line arguments so the OS can launch
//! the client from a clicked link without the user typing flags.
//!
//! A URL query parameter is the CLI flag spelled with underscores (`tls_tofu`
//! -> `--tls-tofu`). A boolean parameter carries a value (`software_decode=true`)
//! but emits a bare flag (`--software-decode`). The CLI definition is the
//! source of truth: only the arguments listed in `URL_ARGS` are accepted.

use std::ffi::OsString;

use url::{Host, Url};

use crate::error::{Error, Result};
use crate::FLAG_PORT;

const SCHEME: &str = "kyber://";

/// Detect a `kyber://` URL among the process arguments and translate it into a
/// synthetic argv vector using the client's long flag names, suitable for
/// `clap`'s `get_matches_from`.
///
/// Returns `Ok(None)` when no `kyber://` URL is present. Returns `Err` when a
/// `kyber://` URL is malformed or is not the only argument: the OS hands a
/// link over as one argument, so anything next to it did not come from the
/// link and must not reach clap as a flag.
pub(crate) fn maybe_translate_url(args: &[OsString]) -> Result<Option<Vec<String>>> {
    let is_url = |arg: &OsString| arg.to_str().is_some_and(|s| s.starts_with(SCHEME));
    if !args.iter().skip(1).any(is_url) {
        return Ok(None);
    }
    let only_argument = || Error::Config("a kyber:// URL must be the only argument".to_string());
    let [program, url] = args else {
        return Err(only_argument());
    };
    let url = url
        .to_str()
        .filter(|s| s.starts_with(SCHEME))
        .ok_or_else(only_argument)?;

    let url = Url::parse(url).map_err(|e| Error::Config(format!("invalid kyber:// URL: {e}")))?;

    // IPv6 hosts serialize with brackets; the connection code wants the bare address.
    let host = match url.host() {
        Some(Host::Ipv6(addr)) => addr.to_string(),
        Some(host) => host.to_string(),
        None => return Err(Error::Config("kyber:// URL missing host".to_string())),
    };
    if host.is_empty() {
        return Err(Error::Config("kyber:// URL missing host".to_string()));
    }
    // The host becomes the first positional argv entry. A host starting with
    // `-` would be read by clap as an option (e.g. `kyber://--help` injects
    // `--help`), so reject it: a web-triggerable link must not smuggle flags.
    if host.starts_with('-') {
        return Err(Error::Config(
            "kyber:// URL host must not start with '-'".to_string(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::Config(
            "kyber:// URL must not carry credentials in the authority".to_string(),
        ));
    }

    // A `port` query parameter overrides the port in the authority.
    let mut port = url.port().map(|p| p.to_string());
    let mut flags: Vec<String> = Vec::new();

    let cli = crate::build_cli();
    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        let value = value.as_ref();
        // A key is a CLI name with `_` for `-`. Match the key against each
        // name's underscore form, so the dash spelling is not a second alias.
        let (arg, flag) = cli
            .get_arguments()
            .filter(|a| crate::URL_ARGS.contains(&a.get_id()))
            .find_map(|a| {
                a.get_long()
                    .into_iter()
                    .chain(a.get_all_aliases().unwrap_or_default())
                    .find(|name| name.replace('-', "_") == key)
                    .map(|name| (a, name))
            })
            .ok_or_else(|| {
                Error::Config(format!(
                    "kyber:// URL parameter '{key}' is unknown or not allowed in a link"
                ))
            })?;

        if flag == FLAG_PORT {
            port = Some(value.to_owned());
        } else if arg.is_takes_value_set() {
            push_value(&mut flags, flag, value);
        } else {
            push_flag(&mut flags, flag, key, value)?;
        }
    }

    let mut argv = vec![program.to_string_lossy().into_owned(), host];
    if let Some(port) = port {
        argv.extend([format!("--{FLAG_PORT}"), port]);
    }
    argv.append(&mut flags);

    Ok(Some(argv))
}

fn push_value(flags: &mut Vec<String>, flag: &str, value: &str) {
    flags.extend([format!("--{flag}"), value.to_string()]);
}

fn push_flag(flags: &mut Vec<String>, flag: &str, key: &str, value: &str) -> Result<()> {
    let enabled = parse_bool(value).ok_or_else(|| {
        Error::Config(format!(
            "kyber:// URL parameter '{key}' expects a boolean value"
        ))
    })?;
    if enabled {
        flags.push(format!("--{flag}"));
    }
    Ok(())
}

/// Parse a boolean string: `true`/`1`/empty → true, `false`/`0` → false,
/// anything else → None.
fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(url: &str) -> Vec<OsString> {
        vec!["client".into(), url.into()]
    }

    #[test]
    fn url_args_exist_in_cli() {
        let cli = crate::build_cli();
        for id in crate::URL_ARGS {
            assert!(
                cli.get_arguments().any(|a| a.get_id() == *id),
                "URL_ARGS lists '{id}', but the CLI has no such argument"
            );
        }
    }

    fn translate(url: &str) -> Vec<String> {
        maybe_translate_url(&args(url)).unwrap().unwrap()
    }

    fn expected(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_kyber_url_returns_none() {
        let normal: Vec<OsString> = vec![
            "client".into(),
            "streamer.example".into(),
            "--tls-tofu".into(),
        ];
        assert!(maybe_translate_url(&normal).unwrap().is_none());
    }

    #[test]
    fn kyber_url_with_extra_arguments_is_error() {
        // The OS passes a link as one argument; anything next to it did not
        // come from the link and must not reach clap.
        let extra: Vec<OsString> = vec![
            "client".into(),
            "kyber://h".into(),
            "--tls-skip-verification".into(),
        ];
        assert!(maybe_translate_url(&extra).is_err());
        let extra: Vec<OsString> = vec!["client".into(), "--tls-tofu".into(), "kyber://h".into()];
        assert!(maybe_translate_url(&extra).is_err());
    }

    #[test]
    fn userinfo_rejected() {
        assert!(maybe_translate_url(&args("kyber://user:pw@h")).is_err());
        assert!(maybe_translate_url(&args("kyber://user@h")).is_err());
    }

    #[test]
    fn simple_host_only() {
        assert_eq!(
            translate("kyber://streamer.example"),
            expected(&["client", "streamer.example"])
        );
    }

    #[test]
    fn host_with_port_in_authority() {
        assert_eq!(
            translate("kyber://host:9000"),
            expected(&["client", "host", "--port", "9000"])
        );
    }

    #[test]
    fn query_port_overrides_authority() {
        assert_eq!(
            translate("kyber://host:9000?port=443"),
            expected(&["client", "host", "--port", "443"])
        );
    }

    #[test]
    fn ipv6_authority_brackets_stripped() {
        assert_eq!(
            translate("kyber://[::1]:8080?software_decode=true"),
            expected(&["client", "::1", "--port", "8080", "--software-decode"])
        );
    }

    #[test]
    fn value_params_map_to_flags() {
        assert_eq!(
            translate("kyber://h?bitrate=20000000&video_codec=hevc&display_index=1"),
            expected(&[
                "client",
                "h",
                "--bitrate",
                "20000000",
                "--video-codec",
                "hevc",
                "--display-index",
                "1",
            ])
        );
    }

    #[test]
    fn bool_value_params_pass_through() {
        assert_eq!(
            translate("kyber://h?audio=false&inputs=true"),
            expected(&["client", "h", "--audio", "false", "--inputs", "true"])
        );
    }

    #[test]
    fn truthy_flag_emitted() {
        for v in ["true", "1", "TRUE", ""] {
            assert_eq!(
                translate(&format!("kyber://h?software_decode={v}")),
                expected(&["client", "h", "--software-decode"]),
                "value {v:?}"
            );
        }
    }

    #[test]
    fn bare_flag_in_uri_is_truthy() {
        // `?yuv444` (no `=`) produces an empty value; it should enable the flag.
        assert_eq!(
            translate("kyber://h?yuv444"),
            expected(&["client", "h", "--yuv444"])
        );
    }

    #[test]
    fn falsy_flag_not_set() {
        for v in ["false", "0"] {
            assert_eq!(
                translate(&format!("kyber://h?software_decode={v}")),
                expected(&["client", "h"]),
                "value {v}"
            );
        }
    }

    #[test]
    fn non_boolean_flag_is_error() {
        assert!(maybe_translate_url(&args("kyber://h?software_decode=maybe")).is_err());
    }

    #[test]
    fn tls_and_auth_keys() {
        assert_eq!(
            translate("kyber://streamer.example?tls_tofu=true&tls_known_hosts=true"),
            expected(&[
                "client",
                "streamer.example",
                "--tls-tofu",
                "--tls-known-hosts"
            ])
        );
    }

    #[test]
    fn auth_token_value() {
        assert_eq!(
            translate("kyber://h?auth_token=abc.def.ghi"),
            expected(&["client", "h", "--auth-token", "abc.def.ghi"])
        );
    }

    #[test]
    fn auth_oidc_token_value() {
        assert_eq!(
            translate("kyber://h?auth_oidc_token=abc.def.ghi&tls_tofu=true"),
            expected(&[
                "client",
                "h",
                "--auth-oidc-token",
                "abc.def.ghi",
                "--tls-tofu",
            ])
        );
    }

    #[test]
    fn percent_decoded_value() {
        assert_eq!(
            translate("kyber://h?auth_token=a%2Bb%2Fc+d"),
            expected(&["client", "h", "--auth-token", "a+b/c d"])
        );
    }

    #[test]
    fn trust_and_sign_in_keys_are_errors() {
        for key in [
            "tls_skip_verification",
            "tls_fingerprint",
            "tls_root_cert",
            "tls_host",
            "tls_tofu_interactive",
            "auth_oidc",
            "auth_password",
        ] {
            assert!(
                maybe_translate_url(&args(&format!("kyber://h?{key}=x"))).is_err(),
                "{key} must be rejected"
            );
        }
    }

    #[test]
    fn tls_skip_verification_is_error() {
        // A web-triggerable link must not be able to disable cert checking.
        assert!(maybe_translate_url(&args("kyber://h?tls_skip_verification=true")).is_err());
    }

    #[test]
    fn dash_spelling_is_error() {
        // Only the underscore spelling is documented; a key spelled like the
        // CLI flag is unknown.
        assert!(maybe_translate_url(&args("kyber://h?tls-tofu=true")).is_err());
        assert!(maybe_translate_url(&args("kyber://h?display-index=1")).is_err());
    }

    #[test]
    fn unknown_key_is_error() {
        assert!(maybe_translate_url(&args("kyber://h?foo=bar&bitrate=1")).is_err());
    }

    #[test]
    fn malformed_empty_host() {
        assert!(maybe_translate_url(&args("kyber://")).is_err());
        assert!(maybe_translate_url(&args("kyber://?x=y")).is_err());
    }

    #[test]
    fn leading_dash_host_rejected() {
        // A `-`-leading host must never reach clap as an injected flag.
        assert!(maybe_translate_url(&args("kyber://--tls-skip-verification")).is_err());
        assert!(maybe_translate_url(&args("kyber://-h")).is_err());
    }

    #[test]
    fn non_numeric_bitrate_rejected_not_panicked() {
        // A malformed `--bitrate` from a kyber:// link must be a clean parse
        // error, not a startup panic. The URL translator passes the value
        // through; clap's value parser rejects it.
        let cli = crate::build_cli();
        assert!(cli
            .try_get_matches_from(translate("kyber://h?bitrate=abc"))
            .is_err());
        let cli = crate::build_cli();
        assert!(cli
            .try_get_matches_from(translate("kyber://h?bitrate=20M"))
            .is_ok());
    }

    #[test]
    fn empty_oidc_token_is_an_error() {
        let cli = crate::build_cli();
        assert!(cli
            .try_get_matches_from(translate("kyber://h?auth_oidc_token="))
            .is_err());
    }

    #[test]
    fn oidc_token_conflicts_with_explicit_basic_auth() {
        // Default username/password must not conflict, explicit ones must.
        let cli = crate::build_cli();
        assert!(cli
            .try_get_matches_from(translate("kyber://h?auth_oidc_token=tok"))
            .is_ok());
        let cli = crate::build_cli();
        assert!(cli
            .try_get_matches_from(translate("kyber://h?auth_oidc_token=tok&auth_username=u"))
            .is_err());
    }

    #[test]
    fn portal_launch_url_parses() {
        // The shape the portal builds for a native client, with every
        // session profile parameter set.
        let argv = translate(
            "kyber://desktop.kyber.internal:52789/?auth_oidc_token=TOK&tls_tofu=true&video_codec=hevc&bitrate=10000000&audio=true&clipboard=true&inputs=false",
        );
        assert_eq!(
            argv,
            expected(&[
                "client",
                "desktop.kyber.internal",
                "--port",
                "52789",
                "--auth-oidc-token",
                "TOK",
                "--tls-tofu",
                "--video-codec",
                "hevc",
                "--bitrate",
                "10000000",
                "--audio",
                "true",
                "--clipboard",
                "true",
                "--inputs",
                "false",
            ])
        );
        assert!(crate::build_cli().try_get_matches_from(argv).is_ok());
    }

    #[test]
    fn host_is_first_positional() {
        assert_eq!(
            translate("kyber://streamer.example?tls_tofu=true"),
            expected(&["client", "streamer.example", "--tls-tofu"])
        );
    }
}
