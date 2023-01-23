#!/bin/bash

set -e
set -x

source component-versions.sh

BASE_DIR=$(realpath $(dirname $0))

usage()
{
cat << EOF
usage: $0 -o <path> [options]

Build Desktop Application.

OPTIONS:
   -h            Show some help
   -s            Skip native dependencies build
   -d            Build only native dependencies
   -c            Run clippy
   -t            Run tests
   -p            Generate a package of the app
   -o <path>     Install the built binaries in the absolute path (Default: ./rootfs-x86_64-w64-mingw32)
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

    # Checkout avserver
    if [[ ! -d avserver ]]; then
        git_clone "kyber-labs/kyber-streamer/avserver.git"
    else
        git -C avserver fetch
    fi

    cd avserver
    git reset --hard
    git checkout $AVSERVER_SHA1
    git submodule update
    cd -

    # Checkout controller
    if [[ ! -d controller ]]; then
        git_clone "kyber-labs/kyber-streamer/controller.git"
    else
        git -C controller fetch
    fi

    cd controller
    git reset --hard
    git checkout $CONTROLLER_SHA1
    git submodule update
    cd -
}

build_deps() {
    # Build libkynput
    cd libkynput
    ./build-win32.sh -o $OUT_DIR
    cd -

    # Build libclient
    cd libclient
    ./build-win32.sh -o $OUT_DIR
    cd -

    # Build avserver
    cd avserver
    ./build-win32.sh -o $OUT_DIR
    cd -

    # Build controller
    cd controller
    ./build-win32.sh -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1
    local stagging_directory="kyber-windows-x86_64"
    local stagging_directory_realpath=$(realpath $stagging_directory)

    local src_bin=$source_directory/bin
    local src_lib=$source_directory/lib

    mkdir -p "$stagging_directory"

    # Copy all dll
    cp \
        $src_bin/avcodec-59.dll \
        $src_bin/avdevice-59.dll \
        $src_bin/avfilter-8.dll \
        $src_bin/avformat-59.dll \
        $src_bin/avutil-57.dll \
        $src_bin/liblua.dll \
        $src_bin/libtxproto.dll \
        $src_bin/libvlccore.dll \
        $src_bin/libvlc.dll \
        $src_bin/postproc-56.dll \
        $src_bin/SDL2.dll \
        $src_bin/swresample-4.dll \
        $src_bin/swscale-6.dll \
        $stagging_directory

    cd $src_lib/vlc
    find plugins -name \*.dll -exec cp --parents {} $stagging_directory_realpath \;
    cd -

    # Copy all exe
    cp \
        $src_bin/avserver.exe \
        $src_bin/client.exe \
        $src_bin/controller.exe \
        $src_bin/input_server.exe \
        $src_bin/service.exe \
        $src_bin/txproto.exe \
        $src_bin/libclient.dll \
        $src_bin/libkynput.dll \
        $src_bin/txproto_stream_audio.lua \
        $src_bin/txproto_stream_video.lua \
        $stagging_directory

    # Strip all exe and dll
    find $stagging_directory -name \*.exe -exec x86_64-w64-mingw32-strip {} \;
    find $stagging_directory -name \*.dll -exec x86_64-w64-mingw32-strip {} \;

    cp \
        $BASE_DIR/scripts/windows/install_service.bat \
        $BASE_DIR/scripts/windows/start_service.bat \
        $BASE_DIR/scripts/windows/stop_service.bat \
        $BASE_DIR/scripts/windows/uninstall_service.bat \
        $stagging_directory

    local zipname=$(basename $stagging_directory).zip
    echo "Creating ${zipname}"
    zip -r ${zipname} $stagging_directory
}

FETCH_SOURCES=true
BUILD_DEPS=true
PACKAGE=false
ACTION="build"
OUT_DIR=$(pwd)/rootfs-x86_64-w64-mingw32

while getopts "hnsdctpo:" OPTION
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
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR=$OUT_DIR/lib/pkgconfig

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

# Run action
case "$ACTION" in
    "build")
        cargo build --release --target x86_64-pc-windows-gnu
        cp target/x86_64-pc-windows-gnu/release/client.exe $OUT_DIR/bin
    ;;
    "clippy")
        cargo clippy --target x86_64-pc-windows-gnu -- -D warnings
    ;;
    "test")
        cargo test --target x86_64-pc-windows-gnu
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    package $OUT_DIR
fi
