#!/bin/bash
# Project Kyber: build-macos.sh
# Copyright © 2022-2026 Kyber SAS
# SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
#
# This file is both under dual license: AGPLv3 and a Commercial one.
#
# ----
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

set -e
set -x

realpath() {
    [[ $1 = /* ]] && echo "$1" || echo "$PWD/${1#./}"
}

# Architecture configuration
BUILD_ARCH=`uname -m | cut -d. -f1`
HOST_ARCH="x86_64"

BASE_DIR=$(realpath $(dirname $0))

usage()
{
cat << EOF
usage: $0 [options] [target]

Build Desktop Application.

OPTIONS:
   -h            Show some help
   -f            Force reset the dependencies to their pinned version
   -s            Skip native dependencies build
   -d            Build only native dependencies
   -c            Run clippy
   -t            Run tests
   -p            Generate a package of the app
   -o <path>     Install the built binaries in the absolute path (Default: ./rootfs-$HOST_ARCH-macos)
   -a <arch>     Use the specified arch (default: $HOST_ARCH)

TARGET:
   client        Build the desktop client (default)
   server        Build the host/server
   both          Build client and server

ENVIRONMENT:
   KYBER_CLIENT_APP_DIRECTORY   Override the kyclient bundle directory name
   KYBER_SERVER_APP_DIRECTORY   Override the server bundle directory name
   KYBER_APP_DIRECTORY          Legacy fallback consulted by both package steps
                                when the target-specific variable is unset
   KYBER_SERVER_SKIP_DMG        When set, package_server only zips the .app bundle
                                and skips the .pkg/.dmg installer build
EOF
}

git_clone() {
    local REPO=$1
    local BRANCH=$2

    if [[ -z "${CI_JOB_TOKEN}" ]]; then
        local FULL_REPO_URI="git@gitlab.com:${REPO}"
    else
        local FULL_REPO_URI="https://gitlab-ci-token:${CI_JOB_TOKEN}@gitlab.com/${REPO}"
    fi

    echo "Repo: '${FULL_REPO_URI}'"
    if [[ -z "${BRANCH}" ]]; then
        git clone --recurse-submodules ${FULL_REPO_URI}
    else
        git clone --recurse-submodules ${FULL_REPO_URI} -b $BRANCH
    fi
}

build_deps() {
    cd kysdk/kyctl
    case "$TARGET" in
        client)
            ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR build-kyclient
            ;;
        server)
            ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR build-kycontroller
            ;;
        both)
            ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR build-kyclient
            ./build-macos.sh -s -a $HOST_ARCH -o $OUT_DIR build-kycontroller
            ;;
    esac
    cd -
}

package_client() {
    local source_directory=$1

    if [[ -n "${KYBER_CLIENT_APP_DIRECTORY}" ]]; then
        local app_dir="${KYBER_CLIENT_APP_DIRECTORY}"
    elif [[ -n "${KYBER_APP_DIRECTORY}" ]]; then
        local app_dir="${KYBER_APP_DIRECTORY}"
    else
        local app_dir="Kyber-Client.app"
    fi

    echo "Creating $app_dir from $source_directory"
    mkdir -p $app_dir/Contents/MacOS $app_dir/Contents/Frameworks/plugins $app_dir/Contents/Resources

    cp \
        $source_directory/bin/kyclient \
        $source_directory/bin/kybertest_rootca_cert.pem \
        $app_dir/Contents/MacOS

    cp -a \
        $source_directory/lib/libkyaudioreg*.dylib \
        $source_directory/lib/libkynput*.dylib \
        $source_directory/lib/libkyclient*.dylib \
        $source_directory/lib/libvlc*.dylib \
        $source_directory/lib/libvlccore*.dylib \
        $source_directory/lib/libSDL2*.dylib \
        $app_dir/Contents/Frameworks/

    find "$source_directory/lib/vlc/plugins" -name 'lib*_plugin.dylib' -exec cp -a {} $app_dir/Contents/Frameworks/plugins \;

    # Fix VLC plugins: replace absolute build paths with @loader_path
    for plugin in $app_dir/Contents/Frameworks/plugins/lib*_plugin.dylib; do
        for dep in $(otool -L $plugin | grep $source_directory | awk '{print $1}'); do
            libname=$(basename $dep)
            # Copy missing dep to Frameworks if not already there
            if [ ! -e "$app_dir/Contents/Frameworks/$libname" ] && [ -e "$source_directory/lib/$libname" ]; then
                cp -a "$source_directory/lib/$libname" "$app_dir/Contents/Frameworks/"
            fi
            install_name_tool -change $dep @loader_path/../$libname $plugin
        done
    done

    # Copy resources
    cp \
        $source_directory/share/kyber/client-Info.plist \
        $app_dir/Contents/Info.plist

    cp \
        $source_directory/share/kyber/kyber.icns \
        $app_dir/Contents/Resources

    # Prepare bundle dylib fixes
    KYAUDIOREG_NAME=libkyaudioreg.0.1.dylib
    LIBKYNPUT_NAME=libkynput.0.1.dylib
    LIBKYCLIENT_NAME=libkyclient.0.1.dylib
    SDL2_NAME=libSDL2-2.0.0.dylib
    LIBVLC_NAME=libvlc.dylib
    LIBVLCCORE_NAME=libvlccore.9.dylib

    # Fix kyclient
    install_name_tool -add_rpath "@executable_path/../Frameworks/" $app_dir/Contents/MacOS/kyclient
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/MacOS/kyclient
    install_name_tool -change $source_directory/lib/$LIBKYCLIENT_NAME @rpath/$LIBKYCLIENT_NAME $app_dir/Contents/MacOS/kyclient

    # Fix libkyclient
    install_name_tool -id @rpath/$LIBKYCLIENT_NAME $app_dir/Contents/Frameworks/$LIBKYCLIENT_NAME
    install_name_tool -change $source_directory/lib/$KYAUDIOREG_NAME @rpath/$KYAUDIOREG_NAME $app_dir/Contents/Frameworks/$LIBKYCLIENT_NAME
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/Frameworks/$LIBKYCLIENT_NAME
    install_name_tool -change $source_directory/lib/$LIBVLC_NAME @rpath/$LIBVLC_NAME $app_dir/Contents/Frameworks/$LIBKYCLIENT_NAME

    # Fix libvlc
    install_name_tool -change $source_directory/lib/$LIBVLCCORE_NAME @rpath/$LIBVLCCORE_NAME $app_dir/Contents/Frameworks/$LIBVLC_NAME

    # Fix libkynput
    install_name_tool -change $source_directory/lib/$SDL2_NAME @rpath/$SDL2_NAME $app_dir/Contents/Frameworks/$LIBKYNPUT_NAME

    zip_app_bundle $app_dir
}

# Archive a .app bundle for distribution. --symlinks keeps the versioned dylib
# links intact; a plain directory artifact would flatten them and lose the
# executable bits.
zip_app_bundle() {
    local app_dir=$1
    local zipname=$(basename $app_dir).zip

    echo "Creating ${zipname}"
    zip --symlinks -r ${zipname} $app_dir
}

# Recursively bundle a Mach-O's rootfs dylib deps into Frameworks and rewrite
# every reference (and the copied dylibs' install ids) to @rpath.
fix_macho_deps() {
    local macho=$1 frameworks=$2 src=$3
    # otool's first line is the file's own id; skip it with NR>1.
    for dep in $(otool -L "$macho" | awk 'NR>1{print $1}' | grep "^$src/"); do
        local libname=$(basename "$dep")
        local newly=0
        if [ ! -e "$frameworks/$libname" ]; then
            # -L: deref versioned symlinks so the real dylib lands under $libname
            cp -pL "$dep" "$frameworks/$libname"
            chmod u+w "$frameworks/$libname"
            install_name_tool -id "@rpath/$libname" "$frameworks/$libname"
            newly=1
        fi
        install_name_tool -change "$dep" "@rpath/$libname" "$macho"
        if [ "$newly" = 1 ]; then
            fix_macho_deps "$frameworks/$libname" "$frameworks" "$src"
        fi
    done
    return 0
}

package_server() {
    local source_directory=$1

    if [[ -n "${KYBER_SERVER_APP_DIRECTORY}" ]]; then
        local app_dir="${KYBER_SERVER_APP_DIRECTORY}"
    elif [[ -n "${KYBER_APP_DIRECTORY}" ]]; then
        local app_dir="${KYBER_APP_DIRECTORY}"
    else
        local app_dir="Kyber-Server.app"
    fi

    echo "Creating $app_dir from $source_directory"
    mkdir -p $app_dir/Contents/MacOS $app_dir/Contents/Frameworks $app_dir/Contents/Resources

    cp \
        $source_directory/bin/kycontroller \
        $source_directory/bin/kyavserver \
        $source_directory/bin/kynputserver \
        $source_directory/bin/kyber_config.toml \
        $source_directory/bin/kybertest_cert.pem \
        $source_directory/bin/kybertest_key.pem \
        $source_directory/bin/kybertest_rootca_cert.pem \
        $app_dir/Contents/MacOS

    # Copy resources
    cp \
        $source_directory/share/kyber/server-Info.plist \
        $app_dir/Contents/Info.plist

    cp \
        $source_directory/share/kyber/kyber.icns \
        $app_dir/Contents/Resources

    local frameworks="$app_dir/Contents/Frameworks"

    # Seed liblua: dlopen'd by libtxproto's scripting, so otool -L can't see it.
    cp -pL $source_directory/lib/liblua.dylib "$frameworks/liblua.dylib"
    chmod u+w "$frameworks/liblua.dylib"
    install_name_tool -id "@rpath/liblua.dylib" "$frameworks/liblua.dylib"
    fix_macho_deps "$frameworks/liblua.dylib" "$frameworks" "$source_directory"

    # Walk each executable's real dependency graph into the bundle.
    for exe in kycontroller kyavserver kynputserver; do
        install_name_tool -add_rpath "@executable_path/../Frameworks/" \
            "$app_dir/Contents/MacOS/$exe"
        fix_macho_deps "$app_dir/Contents/MacOS/$exe" "$frameworks" "$source_directory"
    done

    # Ship the bundle on its own as well as inside the installer, for anyone
    # who'd rather unpack it by hand than run the .pkg.
    zip_app_bundle $app_dir

    if [[ -n "${KYBER_SERVER_SKIP_DMG:-}" ]]; then
        echo "KYBER_SERVER_SKIP_DMG set — skipping the .pkg/.dmg installer build."
    else
        bash ./scripts/macos/build-kyber-dmg.sh "$(basename "$app_dir" .app)"
    fi
}

get_cargo_target()
{
    case $1 in
        x86_64)
            echo "x86_64-apple-darwin"
        ;;
        arm64)
            echo "aarch64-apple-darwin"
        ;;
        *)
            echo "Unexpected arch $1"
            exit 1
        ;;
    esac
}

BUILD_DEPS=true
PACKAGE=false
ACTION="build"
TARGET="client"
OUT_DIR=""

while getopts "hfsdctpo:a:" OPTION
do
    case $OPTION in
        h)
            usage
            exit 1
            ;;
        s)
            BUILD_DEPS=false
            ;;
        d)
            ACTION="none"
            ;;
        c)
            ACTION="clippy"
            ;;
        t)
            ACTION="test"
            ;;
        p)
            PACKAGE=true
            ;;
        o)
            OUT_DIR=$OPTARG
            ;;
        a)
            HOST_ARCH=$OPTARG
            ;;
    esac
done
shift $(($OPTIND - 1))

# Get target from positional argument
if [ "x$1" != "x" ]; then
    TARGET=$1
    shift
fi

# Ensure no extra arguments
if [ "x$1" != "x" ]; then
    echo "Error: Unexpected argument '$1'" >&2
    usage
    exit 1
fi

if [ -z "$OUT_DIR" ]; then
    OUT_DIR=$(pwd)/rootfs-$HOST_ARCH-macos
fi

case "$TARGET" in
    client|server|both) ;;
    *)
        echo "Error: invalid target '$TARGET' (must be client, server, or both)" >&2
        usage
        exit 1
        ;;
esac

# Build local repository
export PATH=$OUT_DIR/bin:$PATH
export LD_LIBRARY_PATH=$OUT_DIR/lib
export PKG_CONFIG_PATH=$OUT_DIR/lib/pkgconfig

# libvlc + kyaudioreg are only needed by the client. Export the flag
# so kysdk/kymedia/contrib/build-macos.sh knows whether to build them.
if [ "$TARGET" = "client" ] || [ "$TARGET" = "both" ]; then
    export KYBER_BUILD_CLIENT_DEPS=1
fi

# Build dependencies
if [ "$BUILD_DEPS" = true ]; then
    build_deps
else
    echo "Skip dependencies build"
fi

# Prepare cargo configuration
if [ $HOST_ARCH != $BUILD_ARCH ]; then
    export PKG_CONFIG_ALLOW_CROSS=1
fi

CARGO_TARGET=`get_cargo_target $HOST_ARCH`

# Run action
case "$ACTION" in
    "build")
        # Shared resources used by either target
        mkdir -p $OUT_DIR/share/kyber
        cp kyclient/res/kyber.icns $OUT_DIR/share/kyber
        cp test/tls/kybertest_rootca_cert.pem $OUT_DIR/bin

        if [ "$TARGET" = "client" ] || [ "$TARGET" = "both" ]; then
            # Build the desktop kyclient binary. Overwrites any SDK kyclient
            # that the deps stage may have left at $OUT_DIR/bin/kyclient.
            cargo build \
                -p kyclient \
                --target $CARGO_TARGET \
                --release

            cp \
                target/$CARGO_TARGET/release/kyclient \
                $OUT_DIR/bin

            cp \
                kyclient/res/Info.plist \
                $OUT_DIR/share/kyber/client-Info.plist
        fi

        if [ "$TARGET" = "server" ] || [ "$TARGET" = "both" ]; then
            # Server runtime config and full TLS material
            cp \
                kyber_config.toml \
                $OUT_DIR/bin

            cp \
                test/tls/kybertest_cert.pem \
                test/tls/kybertest_key.pem \
                $OUT_DIR/bin

            cp \
                scripts/macos/server-Info.plist \
                $OUT_DIR/share/kyber/server-Info.plist
        fi
    ;;
    "clippy")
        if [ "$TARGET" = "client" ] || [ "$TARGET" = "both" ]; then
            cargo clippy -p kyclient --target $CARGO_TARGET -- -D warnings
        fi
    ;;
    "test")
        if [ "$TARGET" = "client" ] || [ "$TARGET" = "both" ]; then
            cargo test -p kyclient
        fi
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    if [ "$TARGET" = "client" ] || [ "$TARGET" = "both" ]; then
        package_client $OUT_DIR
    fi
    if [ "$TARGET" = "server" ] || [ "$TARGET" = "both" ]; then
        package_server $OUT_DIR
    fi
fi
