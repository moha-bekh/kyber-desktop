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
   -n            Skip source fetch
   -f            Force reset the dependencies to their pinned version
   -s            Skip native dependencies build
   -d            Build only native dependencies
   -c            Run clippy
   -t            Run tests
   -p            Generate a package of the app
   -w            Don't spawn cmd.exe when running processes
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
    # Checkout kymux
    if [[ ! -d kymux ]]; then
        git_clone "kyber-labs/kyber-streamer/kymux.git"
    else
        git -C kymux fetch
    fi

    KYMUX_PATH=$(realpath kymux)
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

    LIBKYPC_PATH=$(realpath libkypc)

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

    LIBKYNPUT_PATH=$(realpath libkynput)

    cd libkynput
    git checkout $FORCE_OPT $LIBKYNPUT_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "kymux_client = { path = \"$KYMUX_PATH/kymux_client\" }" >> .cargo/config.toml
    echo "libkypc = { path = \"$LIBKYPC_PATH\" }" >> .cargo/config.toml

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
    echo "kymux = { path = \"$KYMUX_PATH/kymux\" }" >> .cargo/config.toml
    echo "kymux_client = { path = \"$KYMUX_PATH/kymux_client\" }" >> .cargo/config.toml
    echo "kycom = { path = \"$KYMUX_PATH/kyproto/kycom\" }" >> .cargo/config.toml
    echo "kynet = { path = \"$KYMUX_PATH/kyproto/kynet\" }" >> .cargo/config.toml
    echo "kyproto = { path = \"$KYMUX_PATH/kyproto/kyproto\" }" >> .cargo/config.toml
    echo "libkynput = { path = \"$LIBKYNPUT_PATH/libkynput\" }" >> .cargo/config.toml
    echo "libkynput-rs = { path = \"$LIBKYNPUT_PATH/libkynput-rs\" }" >> .cargo/config.toml
    echo "libkynput-sys = { path = \"$LIBKYNPUT_PATH/libkynput-rs/libkynput-sys\" }" >> .cargo/config.toml

    cd -

    # Checkout avserver
    if [[ ! -d avserver ]]; then
        git_clone "kyber-labs/kyber-streamer/avserver.git"
    else
        git -C avserver fetch
    fi

    AVSERVER_PATH=$(realpath avserver)

    cd avserver
    git checkout $FORCE_OPT $AVSERVER_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "libkypc = { path = \"$LIBKYPC_PATH\" }" >> .cargo/config.toml
    echo "kymux_client = { path = \"$KYMUX_PATH/kymux_client\" }" >> .cargo/config.toml

    cd -

    # Checkout controller
    if [[ ! -d controller ]]; then
        git_clone "kyber-labs/kyber-streamer/controller.git"
    else
        git -C controller fetch
    fi

    cd controller
    git checkout $FORCE_OPT $CONTROLLER_SHA1 || exit
    git submodule update $FORCE_OPT --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "avservice-types = { path = \"$AVSERVER_PATH/avservice-types\" }" >> .cargo/config.toml
    echo "inputservice-types = { path = \"$LIBKYNPUT_PATH/inputservice-types\" }" >> .cargo/config.toml
    echo "kymux = { path = \"$KYMUX_PATH/kymux\" }" >> .cargo/config.toml
    echo "libkypc = { path = \"$LIBKYPC_PATH\" }" >> .cargo/config.toml

    cd -
}

build_deps() {
    # Build libkynput
    cd libkynput
    ./build-win32.sh $CONSOLE_FLAG -o $OUT_DIR
    cd -

    # Build libclient
    cd libclient
    ./build-win32.sh -o $OUT_DIR
    cd -

    # Build avserver
    cd avserver
    ./build-win32.sh $CONSOLE_FLAG-o $OUT_DIR
    cd -

    # Build controller
    cd controller
    ./build-win32.sh $CONSOLE_FLAG -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1

    if [[ -z "${KYBER_STAGING_DIRECTORY}" ]]; then
        local staging_directory="kyber-windows-x86_64"
    else
        local staging_directory="${KYBER_STAGING_DIRECTORY}"
    fi

    local staging_directory_realpath=$(realpath $staging_directory)

    local src_bin=$source_directory/bin
    local src_lib=$source_directory/lib

    mkdir -p "$staging_directory"

    # Copy all dll
    cp \
        $src_bin/avcodec-60.dll \
        $src_bin/avdevice-60.dll \
        $src_bin/avfilter-9.dll \
        $src_bin/avformat-60.dll \
        $src_bin/avutil-58.dll \
        $src_bin/kyaudioreg.dll \
        $src_bin/liblua.dll \
        $src_bin/libtxproto-0.dll \
        $src_bin/libvlccore.dll \
        $src_bin/libvlc.dll \
        $src_bin/postproc-57.dll \
        $src_bin/SDL2.dll \
        $src_bin/swresample-4.dll \
        $src_bin/swscale-7.dll \
        $staging_directory

    cd $src_lib/vlc
    find plugins -name \*.dll -exec cp --parents {} $staging_directory_realpath \;
    cd -

    # Copy all exe
    cp \
        $src_bin/avserver.exe \
        $src_bin/client.exe \
        $src_bin/controller.exe \
        $src_bin/ffmpeg.exe \
        $src_bin/inputserver.exe \
        $src_bin/service.exe \
        $src_bin/txproto.exe \
        $src_bin/libclient.dll \
        $src_bin/libkynput.dll \
        $src_bin/kybertest_cert.pem \
        $src_bin/kybertest_key.pem \
        $src_bin/kybertest_rootca_cert.pem \
        $src_bin/kyber_config.toml \
        $staging_directory

    # Strip all exe and dll
    find $staging_directory -name \*.exe -exec x86_64-w64-mingw32-strip {} \;
    find $staging_directory -name \*.dll -exec x86_64-w64-mingw32-strip {} \;

    cp \
        $BASE_DIR/scripts/windows/install_service.bat \
        $BASE_DIR/scripts/windows/start_service.bat \
        $BASE_DIR/scripts/windows/stop_service.bat \
        $BASE_DIR/scripts/windows/uninstall_service.bat \
        $staging_directory

    # Copy resources
    cp \
        desktop/res/kyber.ico \
        $staging_directory

    local zipname=$(basename $staging_directory).zip
    echo "Creating ${zipname}"
    zip -r ${zipname} $staging_directory
}

FETCH_SOURCES=true
FORCE=false
BUILD_DEPS=true
PACKAGE=false
ACTION="build"
CONSOLE_FLAG=""
HIDE_CONSOLE_FEATURE=""
OUT_DIR=$(pwd)/rootfs-x86_64-w64-mingw32

while getopts "hnfsdctpo:w" OPTION
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
        w)
            CONSOLE_FLAG="-w"
            HIDE_CONSOLE_FEATURE="--features win32-hide-console"
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
        cargo build \
            --release \
            --target x86_64-pc-windows-gnu \
            $HIDE_CONSOLE_FEATURE

        cp target/x86_64-pc-windows-gnu/release/client.exe $OUT_DIR/bin

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
