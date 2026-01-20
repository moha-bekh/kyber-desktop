#!/bin/bash

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

build_deps() {
    cd kysdk/kyctl
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
        $src_bin/avcodec-61.dll \
        $src_bin/avdevice-61.dll \
        $src_bin/avfilter-10.dll \
        $src_bin/avformat-61.dll \
        $src_bin/avutil-59.dll \
        $src_bin/kyaudioreg.dll \
        $src_bin/liblua.dll \
        $src_bin/libplacebo-351.dll \
        $src_bin/libspirv-cross-c-shared.dll \
        $src_bin/libSPIRV-Tools-shared.dll \
        $src_bin/libtxproto-0.dll \
        $src_bin/libvlccore.dll \
        $src_bin/libvlc.dll \
        $src_bin/postproc-58.dll \
        $src_bin/SDL2.dll \
        $src_bin/swresample-5.dll \
        $src_bin/swscale-8.dll \
        $src_bin/libstdc++-6.dll \
        $src_bin/libgcc_s_seh-1.dll \
        $src_bin/libwinpthread-1.dll \
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

BUILD_DEPS=true
PACKAGE=false
ACTION="build"
CONSOLE_FLAG=""
HIDE_CONSOLE_FEATURE=""
OUT_DIR=$(pwd)/rootfs-x86_64-w64-mingw32

while getopts "hfsdctpo:w" OPTION
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
            -p client \
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

        COMPILER="x86_64-w64-mingw32-g++"
        MINGW_RUNTIME="
        libstdc++-6.dll
        libgcc_s_seh-1.dll
        libgcc_s_dw2-1.dll
        libwinpthread-1.dll
        "

        for dll in $MINGW_RUNTIME; do
        dll_path=$("$COMPILER" -print-file-name="$dll" 2>/dev/null | tr -d '\r' | tr -d '\n' || true)
        if [ -n "$dll_path" ] && [ "$dll_path" != "$dll" ] && [ -f "$dll_path" ]; then
            if command -v install >/dev/null 2>&1; then
            install -m 0755 "$dll_path" "$OUT_DIR/bin/"
            else
            cp -f "$dll_path" "$OUT_DIR/bin/"
            fi
        else
            echo "MinGW: $dll not found via $COMPILER -print-file-name (skipping)."
        fi
        done
    ;;
    "clippy")
        cargo clippy -p client --target x86_64-pc-windows-gnu -- -D warnings
    ;;
    "test")
        cargo test -p client --target x86_64-pc-windows-gnu
    ;;
    "none")
        echo "No action requested"
    ;;
esac

# Package if requested
if [ "$PACKAGE" = true ]; then
    package $OUT_DIR
fi
