#!/bin/bash
# Project Kyber: build-linux.sh
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
   -o <path>     Install the built binaries in the absolute path (Default: ./rootfs-x86_64-linux-gnu)
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
    ./build-linux.sh -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1

    if [[ -z "${KYBER_STAGING_DIRECTORY}" ]]; then
        local staging_directory="kyber-linux-x86_64"
    else
        local staging_directory="${KYBER_STAGING_DIRECTORY}"
    fi

    local staging_directory_realpath=$(realpath $staging_directory)

    local src_bin=$source_directory/bin

    mkdir -p "$staging_directory/bin"
    mkdir -p "$staging_directory/lib"
    mkdir -p "$staging_directory/share"

    # Copy bin directory
    cp \
        $src_bin/kyavserver \
        $src_bin/kyclient \
        $src_bin/kycontroller \
        $src_bin/ffmpeg \
        $src_bin/kynputserver \
        $src_bin/txproto \
        $src_bin/kyber_config.toml \
        $staging_directory/bin

    # Copy test TLS cert/key
    cp \
        $src_bin/kybertest_cert.pem \
        $src_bin/kybertest_key.pem \
        $src_bin/kybertest_rootca_cert.pem \
        $staging_directory/bin

    # Copy ressources
    cp \
        $source_directory/share/kyber.ico \
        $staging_directory/share

    # Copy libraries
    cd $source_directory
    find lib -name \*.so\* -exec cp -a --parents {} $staging_directory_realpath \;
    cd -

    # Strip all binaries and libraries
    find $staging_directory/bin -type f -executable -exec strip {} \;
    find $staging_directory/lib -name \*.so\* -exec strip {} \;

    # Copy scripts
    cp \
        scripts/linux/run_kyclient.sh \
        scripts/linux/run_kycontroller.sh \
        $staging_directory

    tar cfvj $(basename $staging_directory).tar.bz2 $staging_directory
}

BUILD_DEPS=true
PACKAGE=false
ACTION="build"
OUT_DIR=$(pwd)/rootfs-x86_64-linux-gnu

while getopts "hfsdctpo:" OPTION
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
    esac
done
shift $(($OPTIND - 1))

if [ "x$1" != "x" ]; then
    usage
    exit 1
fi

# Build local repository
export PATH=$OUT_DIR/bin:$PATH
export LD_LIBRARY_PATH=$OUT_DIR/lib:$OUT_DIR/lib/x86_64-linux-gnu:$OUT_DIR/lib64
export PKG_CONFIG_PATH=$OUT_DIR/lib/pkgconfig:$OUT_DIR/lib/x86_64-linux-gnu/pkgconfig:$OUT_DIR/lib64/pkgconfig

# Build dependencies
if [ "$BUILD_DEPS" = true ]; then
    build_deps
else
    echo "Skip dependencies build"
fi

# Run action
case "$ACTION" in
    "build")
        cargo build -p client --release
        cp target/release/client $OUT_DIR/bin

        # Copy default configuration
        cp \
            kyber_config.toml \
            $OUT_DIR/bin

        # Copy test TLS cert/key
        cp \
            test/tls/kybertest_cert.pem \
            test/tls/kybertest_key.pem \
            test/tls/kybertest_rootca_cert.pem \
            $OUT_DIR/bin

        # Copy resources
        cp \
            desktop/res/kyber.ico \
            $OUT_DIR/share
    ;;
    "clippy")
        cargo clippy -p client -- -D warnings
    ;;
    "test")
        cargo test -p client
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    package $OUT_DIR
fi
