// Project Kyber: build.rs
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

use std::process::Command;

use vergen::EmitBuilder;

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn generate_rc() {
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use std::process;

    // Write ressource file
    let out_dir = env::var("OUT_DIR").unwrap();

    let ico = "kyber.ico";
    println!("cargo:rerun-if-changed=res/{ico}");

    let rc_path = {
        let rc_path = format!("{out_dir}/kyber.rc");
        let mut rc = File::create(&rc_path).unwrap();
        rc.write_fmt(std::format_args!("1 ICON \"{ico}\"")).unwrap();

        rc_path
    };

    // Compile ressource file
    let rc_libname = "kyber_rc";
    let rc_lib_path = format!("{out_dir}/{rc_libname}.lib");

    println!("{:?}", std::env::current_dir());

    let status = process::Command::new("x86_64-w64-mingw32-windres")
        .arg("-I")
        .arg("res")
        .arg("-i")
        .arg(rc_path)
        .arg("-o")
        .arg(rc_lib_path)
        .status()
        .unwrap();
    assert!(status.success());

    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib={rc_libname}");
}

fn main() {
    // Fetch tags when none are reachable (shallow or detached checkout) so
    // git-describe resolves the release tag. Best-effort: offline builds fail
    // the fetch and fall back to the short SHA below. Bound a stalled HTTP fetch
    // (aborts below 1 KB/s for 15s) so it can't hang the build; ignored for ssh
    // remotes, which CI does not use. vergen emits rerun-if-changed on the git
    // HEAD, so this only runs when the commit moves.
    if git_output(&["describe", "--tags"]).is_none() {
        Command::new("git")
            .args(["fetch", "--tags", "--quiet"])
            .env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
            .env("GIT_HTTP_LOW_SPEED_TIME", "15")
            .status()
            .ok();
    }

    EmitBuilder::builder()
        .git_describe(false, true, None)
        .emit()
        .expect("vergen failed to emit git describe");

    // vergen leaves VERGEN_GIT_DESCRIBE empty when no tag is reachable, e.g. a
    // shallow CI checkout with no tags fetched. Fall back to the short commit
    // SHA so the logged revision and --version output are never blank.
    if git_output(&["describe", "--tags"]).is_none() {
        if let Some(sha) = git_output(&["rev-parse", "--short", "HEAD"]) {
            println!("cargo:rustc-env=VERGEN_GIT_DESCRIBE={sha}");
        }
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    println!("{target_os}");
    if target_os == "windows" {
        generate_rc();
    }
}
