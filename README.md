# Remote PC client/host

Tool that uses ffmpeg and libvlc to remotey control a computer desktop.

## Build

### Get submodules

The project has some submodules to get
```bash
git submodule update --init
```

### Linux

#### Debian: Install required system packages

```bash
sudo apt-get install -y \
    build-essential cmake meson ninja-build pkg-config \
    llvm-dev libclang-dev clang libssl-dev \
    autoconf libtool curl git wget unzip \
    python3 python3-setuptools python3-mako python3-venv \
    lua5.4 liblua5.4-dev \
    autopoint gettext bison flex nasm ant ragel help2man \
    libdrm-dev libgbm-dev glslang-dev liblcms2-dev libpulse-dev \
    libxcb-xkb-dev libxkbcommon-dev libxcb-xtest0-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libwayland-dev wayland-protocols \
    libssl-dev libevdev-dev libudev-dev libinput-dev libidn11

sudo apt-get build-dep -y vlc
```

#### Arch: Install required system packages

```bash
pacman -S base-devel cmake meson ninja pkg-config llvm clang openssl-1.1 \
    autoconf libtool curl git wget unzip python3 python-setuptools lua gettext \
    bison flex nasm ant ragel help2man libdrm mesa glslang lcms2 pulseaudio  \
    libxcb libxkbcommon xcb-util xcb-util-renderutil wayland wayland-protocols \
    libevdev libinput libidn11 ffmpeg a52dec vulkan-headers
```

#### Install Rust and required Rust tools

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-c
```

#### Build

```bash
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

```bash
sudo apt-get install -y \
    build-essential cmake meson ninja-build pkg-config \
    llvm-dev libclang-dev clang libssl-dev \
    mingw-w64 mingw-w64-tools \
    autoconf libtool curl git wget zip unzip \
    python3 python3-setuptools python3-mako python3-venv \
    lua5.4 gettext bison flex ant ragel help2man nasm
```

#### Install Rust and required Rust tools

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-pc-windows-gnu
cargo install cargo-c
```

#### Build

```bash
./build-win32.sh
```

### macOS

#### Install required system packages

```bash
brew install \
    m4 nasm openssl@1.1 pkg-config \
    autoconf automake libtool \
    cmake gettext meson ragel ant help2man
```

#### Install Rust and required Rust tools

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo install cargo-c
```

#### Build: dev mode

```bash
./build-macos.sh -a <arch>
```

`<arch>` can be:
* `x86_64` (The default value)
* `arm64`

#### Build: create a macOS bundle

A `.app` bundle can be generated:
```bash
./build-macos.sh -p -a <arch>
```

The command outputs:
* `kyber-client.app`
* `kyber-client.app.zip`

The bundle can be runned by another Mac, but it must be allowed at first:
```bash
xattr -r -d com.apple.quarantine kyber-client.app
```
