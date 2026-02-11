// Project Kyber: build.rs
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

use vergen::EmitBuilder;

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
    EmitBuilder::builder()
        .git_describe(false, true, None)
        .emit()
        .unwrap();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    println!("{target_os}");
    if target_os == "windows" {
        generate_rc();
    }
}
