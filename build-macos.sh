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

checkout_deps() {
    # Checkout kymux
    if [[ ! -d kymux ]]; then
        git_clone "kyber-labs/kyber-streamer/kymux.git"
    else
        git -C kymux fetch
    fi

    if [ "$FORCE" = true ]; then
        local FORCE_OPT="-f"
    fi

    cd kymux
    git checkout $FORCE_OPT $KYMUX_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive
    cd -

    # Checkout libkypc
    if [[ ! -d libkypc ]]; then
        git_clone "kyber-labs/kyber-streamer/libkypc.git"
    else
        git -C libkypc fetch
    fi

    cd libkypc
    git checkout $FORCE_OPT $LIBKYPC_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive
    cd -

    # Checkout libkynput
    if [[ ! -d libkynput ]]; then
        git_clone "kyber-labs/kyber-streamer/libkynput.git"
    else
        git -C libkynput fetch
    fi

    cd libkynput
    git checkout $FORCE_OPT $LIBKYNPUT_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "kymux_client = { path = \"../kymux/kymux_client\" }" >> .cargo/config.toml
    echo "libkypc = { path = \"../libkypc\" }" >> .cargo/config.toml

    cd -

    # Checkout libclient
    if [[ ! -d libclient ]]; then
        git_clone "kyber-labs/kyber-streamer/libclient.git"
    else
        git -C libclient fetch
    fi

    cd libclient
    echo "Set libclient SHA1 to $LIBCLIENT_SHA1"
    git checkout $FORCE_OPT $LIBCLIENT_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "kymux = { path = \"../kymux/kymux\" }" >> .cargo/config.toml
    echo "kymux_client = { path = \"../kymux/kymux_client\" }" >> .cargo/config.toml
    echo "libkynput = { path = \"../libkynput/libkynput\" }" >> .cargo/config.toml
    echo "libkynput-rs = { path = \"../libkynput/libkynput-rs\" }" >> .cargo/config.toml
    echo "libkynput-sys = { path = \"../libkynput/libkynput-rs/libkynput-sys\" }" >> .cargo/config.toml

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

    if [[ -z "${KYBER_APP_DIRECTORY}" ]]; then
        local app_dir="kyber-client.app"
    else
        local app_dir="${KYBER_APP_DIRECTORY}"
    fi

    echo "Creating $app_dir from $source_directory"
    mkdir -p $app_dir/Contents/MacOS $app_dir/Contents/Frameworks/plugins $app_dir/Contents/Resources

    cp \
        $source_directory/bin/client \
        $source_directory/bin/kybertest_rootca_cert.pem \
        $app_dir/Contents/MacOS

    cp -a \
        $source_directory/lib/libkyaudioreg*.dylib \
        $source_directory/lib/liblibkynput*.dylib \
        $source_directory/lib/liblibclient*.dylib \
        $source_directory/lib/libvlc*.dylib \
        $source_directory/lib/libSDL2*.dylib \
        $app_dir/Contents/Frameworks/

    find "$source_directory/lib/vlc/plugins" -name 'lib*_plugin.dylib' -exec cp -a {} $app_dir/Contents/Frameworks/plugins \;

    # Copy resources
    cp \
        $source_directory/share/kyber/Info.plist \
        $app_dir/Contents

    cp \
        $source_directory/share/kyber/kyber.icns \
        $app_dir/Contents/Resources

    # Prepare bundle dylib fixes
    KYAUDIOREG_NAME=libkyaudioreg.0.1.dylib
    LIBKYNPUT_NAME=liblibkynput.0.1.dylib
    LIBCLIENT_NAME=liblibclient.0.1.dylib
    SDL2_NAME=libSDL2-2.0.0.dylib

    # Fix client
    install_name_tool -add_rpath "@executable_path/../Frameworks/" $app_dir/Contents/MacOS/client
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/MacOS/client
    install_name_tool -change $source_directory/lib/$LIBCLIENT_NAME @rpath/$LIBCLIENT_NAME $app_dir/Contents/MacOS/client

    # Fix libkyaudioreg_plugin
    install_name_tool -change $source_directory/lib/$KYAUDIOREG_NAME @rpath/$KYAUDIOREG_NAME $app_dir/Contents/Frameworks/plugins/libkyaudioreg_plugin.dylib

    # Fix libclient
    install_name_tool -change $source_directory/lib/$LIBKYNPUT_NAME @rpath/$LIBKYNPUT_NAME $app_dir/Contents/Frameworks/$LIBCLIENT_NAME

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

FETCH_SOURCES=true
FORCE=false
BUILD_DEPS=true
PACKAGE=false
ACTION="build"
OUT_DIR=$(pwd)/rootfs-$HOST_ARCH-macos

while getopts "hnfsdctpo:a:" OPTION
do
    case $OPTION in
        h)
            usage
            exit 1
            ;;
        n)
            FETCH_SOURCES=false
            ;;
        f)
            FORCE=true
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

        cp \
            target/$CARGO_TARGET/release/client \
            test/tls/kybertest_rootca_cert.pem \
            $OUT_DIR/bin
        
        mkdir -p $OUT_DIR/share/kyber

        cp \
            desktop/res/Info.plist \
            $OUT_DIR/share/kyber

        cp \
            desktop/res/kyber.icns \
            $OUT_DIR/share/kyber
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
