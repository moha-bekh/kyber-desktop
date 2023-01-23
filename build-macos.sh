#!/bin/bash

set -e
set -x

realpath() {
    [[ $1 = /* ]] && echo "$1" || echo "$PWD/${1#./}"
}

source component-versions.sh

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
   -n            Skip sources fetch
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

checkout_deps() {
    # Checkout libkynput
    if [[ ! -d libkynput ]]; then
        git_clone "kyber-labs/kyber-streamer/libkynput.git"
    else
        git -C libkynput fetch
    fi

    LIBKYNPUT_PATH=$(realpath libkynput)

    cd libkynput
    git reset --hard
    git checkout $LIBKYNPUT_SHA1
    git submodule update
    cd -

    # Checkout libclient
    if [[ ! -d libclient ]]; then
        git_clone "kyber-labs/kyber-streamer/libclient.git"
    else
        git -C libclient fetch
    fi

    cd libclient
    echo "Set libclient SHA1 to $LIBCLIENT_SHA1"
    git reset --hard
    git checkout $LIBCLIENT_SHA1
    git submodule update

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "libkynput-rs = { path = \"$LIBKYNPUT_PATH/libkynput-rs\" }" >> .cargo/config.toml
    echo "libkynput-sys = { path = \"$LIBKYNPUT_PATH/libkynput-rs/libkynput-sys\" }" >> .cargo/config.toml

    cd -
}

build_deps() {
    # Build libkynput
    cd libkynput
    ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR
    cd -

    # Build libclient
    cd libclient
    ./build-macos.sh -a $HOST_ARCH -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1
    local app_dir="kyber-client.app"

    echo "Creating $app_dir from $source_directory"
    mkdir -p $app_dir/Contents/MacOS $app_dir/Contents/Frameworks/plugins

    cp $source_directory/bin/client $app_dir/Contents/MacOS

    cp -a \
        $source_directory/lib/liblibkynput*.dylib \
        $source_directory/lib/liblibclient*.dylib \
        $source_directory/lib/libvlc*.dylib \
        $source_directory/lib/libSDL2*.dylib \
        $app_dir/Contents/Frameworks/

    find "$source_directory/lib/vlc/plugins" -name 'lib*_plugin.dylib' -exec cp -a {} $app_dir/Contents/Frameworks/plugins \;

    # Prepare bundle dylib fixes
    LIBKYNPUT_NAME=liblibkynput.0.1.dylib
    LIBCLIENT_NAME=liblibclient.0.1.dylib
    SDL2_NAME=libSDL2-2.0.0.dylib

    # Fix client
    install_name_tool -add_rpath "@executable_path/../Frameworks/" $app_dir/Contents/MacOS/client
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/MacOS/client
    install_name_tool -change $source_directory/lib/$LIBCLIENT_NAME @rpath/$LIBCLIENT_NAME $app_dir/Contents/MacOS/client

    # Fix libclient
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/Frameworks/$LIBCLIENT_NAME

    # Fix libkynput
    install_name_tool -change $source_directory/lib/$SDL2_NAME @rpath/$SDL2_NAME $app_dir/Contents/Frameworks/$LIBKYNPUT_NAME

    local zipname=$(basename $app_dir).zip
    echo "Creating ${zipname}"
    zip -r ${zipname} $app_dir
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

FETCH_SOURCES=true
BUILD_DEPS=true
PACKAGE=false
ACTION="build"
OUT_DIR=$(pwd)/rootfs-$HOST_ARCH-macos

while getopts "hnsdctpo:a:" OPTION
do
    case $OPTION in
        h)
            usage
            exit 1
            ;;
        n)
            FETCH_SOURCES=false
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

# Checkout dependencies
if [ "$FETCH_SOURCES" = true ]; then
    checkout_deps
else
    echo "Skip dependencies source fetch"
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
        cargo build \
            --target $CARGO_TARGET \
            --release

        cp target/$CARGO_TARGET/release/client $OUT_DIR/bin
    ;;
    "clippy")
        cargo clippy --target $CARGO_TARGET -- -D warnings
    ;;
    "test")
        cargo test
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    package $OUT_DIR
fi
