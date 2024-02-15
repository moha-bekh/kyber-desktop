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
   -n            Skip sources fetch
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

checkout_deps() {
    # Checkout kymux
    if [[ ! -d kymux ]]; then
        git_clone "kyber-labs/kyber-streamer/kymux.git"
    else
        git -C kymux fetch
    fi

    KYMUX_PATH=$(realpath kymux)

    cd kymux
    git reset --hard
    git checkout $KYMUX_SHA1
    git submodule update --init --recursive
    cd -

    # Checkout libkypc
    if [[ ! -d libkypc ]]; then
        git_clone "kyber-labs/kyber-streamer/libkypc.git"
    else
        git -C libkypc fetch
    fi

    LIBKYPC_PATH=$(realpath libkypc)

    cd libkypc
    git reset --hard
    git checkout $LIBKYPC_SHA1
    git submodule update --init --recursive
    cd -

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
    git submodule update --init --recursive

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
    git reset --hard
    git checkout $LIBCLIENT_SHA1
    git submodule update --init --recursive

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
    git reset --hard
    git checkout $AVSERVER_SHA1
    git submodule update --init --recursive

    mkdir -p .cargo
    echo "[patch.crates-io]" > .cargo/config.toml
    echo "libkypc = { path = \"$LIBKYPC_PATH\" }" >> .cargo/config.toml

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
    git submodule update --init --recursive

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
    ./build-linux.sh -o $OUT_DIR
    cd -

    # Build libclient
    cd libclient
    ./build-linux.sh -o $OUT_DIR
    cd -

    # Build avserver
    cd avserver
    ./build-linux.sh -o $OUT_DIR
    cd -

    # Build controller
    cd controller
    ./build-linux.sh -o $OUT_DIR
    cd -
}

package() {
    local source_directory=$1

    if [[ -z "${KYBER_STAGGING_DIRECTORY}" ]]; then
        local stagging_directory="kyber-linux-x86_64"
    else
        local stagging_directory="${KYBER_STAGGING_DIRECTORY}"
    fi

    local stagging_directory_realpath=$(realpath $stagging_directory)

    local src_bin=$source_directory/bin

    mkdir -p "$stagging_directory/bin"
    mkdir -p "$stagging_directory/lib"

    # Copy bin directory
    cp \
        $src_bin/avserver \
        $src_bin/client \
        $src_bin/controller \
        $src_bin/ffmpeg \
        $src_bin/inputserver \
        $src_bin/txproto \
        $src_bin/kyber_config.toml \
        $stagging_directory/bin

    # Copy test TLS cert/key
    cp \
        $src_bin/kybertest_cert.pem \
        $src_bin/kybertest_key.pem \
        $src_bin/kybertest_rootca_cert.pem \
        $stagging_directory/bin

    # Copy libraries
    cd $source_directory
    find lib -name \*.so\* -exec cp -a --parents {} $stagging_directory_realpath \;
    cd -

    # Strip all binaries and libraries
    find $stagging_directory/bin -type f -executable -exec strip {} \;
    find $stagging_directory/lib -name \*.so\* -exec strip {} \;

    # Copy scripts
    cp \
        scripts/linux/run_client.sh \
        scripts/linux/run_controller.sh \
        $stagging_directory

    tar cfvj $(basename $stagging_directory).tar.bz2 $stagging_directory
}

FETCH_SOURCES=true
BUILD_DEPS=true
PACKAGE=false
ACTION="build"
OUT_DIR=$(pwd)/rootfs-x86_64-linux-gnu

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
export LD_LIBRARY_PATH=$OUT_DIR/lib:$OUT_DIR/lib/x86_64-linux-gnu
export PKG_CONFIG_PATH=$OUT_DIR/lib/pkgconfig:$OUT_DIR/lib/x86_64-linux-gnu/pkgconfig

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
        cargo build --release
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

    ;;
    "clippy")
        cargo clippy -- -D warnings
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
