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
usage: $0 -o <path> [options]

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
    ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1

    if [[ -z "${KYBER_APP_DIRECTORY}" ]]; then
        local app_dir="kyber-kyclient.app"
    else
        local app_dir="${KYBER_APP_DIRECTORY}"
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
        $source_directory/share/kyber/Info.plist \
        $app_dir/Contents

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

    local zipname=$(basename $app_dir).zip
    echo "Creating ${zipname}"
    zip --symlinks -r ${zipname} $app_dir
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
OUT_DIR=$(pwd)/rootfs-$HOST_ARCH-macos

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

if [ "x$1" != "x" ]; then
    usage
    exit 1
fi

# Build local repository
export PATH=$OUT_DIR/bin:$PATH
export LD_LIBRARY_PATH=$OUT_DIR/lib
export PKG_CONFIG_PATH=$OUT_DIR/lib/pkgconfig

# The desktop build requires the client-side kymedia dependencies.
export KYBER_BUILD_CLIENT_DEPS=1

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
        cargo build \
            -p kyclient \
            --target $CARGO_TARGET \
            --release

        cp \
            target/$CARGO_TARGET/release/kyclient \
            test/tls/kybertest_rootca_cert.pem \
            $OUT_DIR/bin
        
        mkdir -p $OUT_DIR/share/kyber

        cp \
            kyclient/res/Info.plist \
            $OUT_DIR/share/kyber

        cp \
            kyclient/res/kyber.icns \
            $OUT_DIR/share/kyber
    ;;
    "clippy")
        cargo clippy -p kyclient --target $CARGO_TARGET -- -D warnings
    ;;
    "test")
        cargo test -p kyclient
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    package $OUT_DIR
fi
