# Remote PC client/host

Tool that uses ffmpeg and libvlc to remotey control a computer desktop.

## Build

### Linux

#### Install required system packages

```
sudo apt-get install -y \
    build-essential cmake meson ninja-build pkg-config \
    llvm-dev libclang-dev clang libssl-dev \
    autoconf libtool curl git wget unzip \
    python3 python3-setuptools python3-mako \
    lua5.4 liblua5.4-dev \
    autopoint gettext bison flex nasm ant ragel help2man \
    libdrm-dev libgbm-dev glslang-dev liblcms2-dev libpulse-dev \
    libxcb-xkb-dev libxkbcommon-dev libxcb-xtest0-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libwayland-dev wayland-protocols \
    libssl-dev libevdev-dev libudev-dev libinput-dev libidn11

sudo apt-get build-dep -y vlc
```

#### Install Rust and required Rust tools

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-c
```

#### Build

```
./build-linux.sh
```

The script outputs files in folder `rootfs-x86_64-linux-gnu`.

`LD_LIBRARY_PATH` must be configured to be able to run produced executables:
```
ROOTFS_DIR=$(pwd)/rootfs-x86_64-linux-gnu
export LD_LIBRARY_PATH=$ROOTFS_DIR/lib:$ROOTFS_DIR/lib/x86_64-linux-gnu:$ROOTFS_DIR/lib/vlc
```

#### Run

* Client: `./rootfs-x86_64-linux-gnu/bin/client <host_ip>`
* Host: `./rootfs-x86_64-linux-gnu/bin/controller`

#### Host notes

* User must have write access to `/dev/uinput` to be able to plug virtual devices (keyboard and mouse).

#### Client notes

* The input part has two backend for Keyboard and Mouse acquisition: `winit` and `libinput`. `winit` uses a standard X11 connection to get inputs, while `libinput` reads directly `/dev/input/` devices, but it requires for the user to be in the `input` group.

### Windows

#### Install required system packages

```
sudo apt-get install -y \
    build-essential cmake meson ninja-build pkg-config \
    llvm-dev libclang-dev clang libssl-dev \
    mingw-w64 mingw-w64-tools \
    autoconf libtool curl git wget zip unzip \
    python3 python3-setuptools python3-mako \
    lua5.4 gettext bison flex ant ragel help2man nasm
```

#### Install Rust and required Rust tools

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-pc-windows-gnu
cargo install cargo-c
```

#### Build

```
./build-win32.sh
```
