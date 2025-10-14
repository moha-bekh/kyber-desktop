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

Tested distros
* Debian Bookworm
* Ubuntu 24.04

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
rustup install 1.89.0 && rustup default 1.89.0
cargo install --locked cargo-c@0.10.15
```

#### Build

```bash
./build-linux.sh
```

The script outputs files in folder `rootfs-x86_64-linux-gnu`.

There are helper scripts in `scripts/linux` to setup all the required envvars to
run the client and the server:

```bash
cp scripts/linux/*.sh rootfs-x86_64-linux-gnu
```

Note that there are some options in the build script to avoid some build steps:
* `-n`: Skip git fetch/checkout (preserves your local changes)
* `-s`: Skip the dependencies build
* `-o`: Output directory (must be absolute path)

#### Run client

```bash
# Force X11 Compatibility Mode
export WAYLAND_DISPLAY=""
./rootfs-x86_64-linux-gnu/run_client.sh <host_ip>`
```

### Input notes for client

* The input part has two backend for Keyboard and Mouse acquisition: `winit` and
  `libinput`. `winit` uses a standard X11 connection to get inputs, while `libinput`
  reads directly `/dev/input/` devices, but it requires for the user to be in the `input` group.
  To have access to `/dev/input/`, use one of the tricks describes above.

#### Run host

```bash
./rootfs-x86_64-linux-gnu/run_controller.sh
```

#### Grant Input Device Permissions for Controller

The host (`controller`) needs permission to create virtual input devices. There are two options for that.

##### Use `chmod`

This won't persist across reboot.

```bash
sudo chmod +w /dev/uinput
```

##### Add user to the `input` group

This is a persistent solution (more intrusive).

```bash
# Add current user to the 'input' group to allow access to /dev/uinput
sudo usermod -aG input $USER
sudo usermod -a -G input $USER
```

**IMPORTANT:** This permission change only applies after  **log out of session and log back in**.
make sure it work with `groups | grep input`

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
rustup install 1.89.0 && rustup default 1.89.0
rustup target add x86_64-pc-windows-gnu
cargo install --locked cargo-c@0.10.15
```

#### Build

```bash
./build-win32.sh
```

Note that there are some options in the build script to avoid some build steps:
* `-n`: Skip git fetch/checkout (preserves your local changes)
* `-s`: Skip the dependencies build
* `-o`: Output directory (must be absolute path)

#### Generate the final package

The build scripts generate a Unix-like folder structure. For Windows, the `-p`
option packages the output into a flat structure.

The command outputs:
* `kyber-windows-x86_64`: The "flattened" build output
* `kyber-windows-x86_64.zip`: The content of `kyber-windows-x86_64` in a zip

#### Host notes

* To have access to UAC, install and start the Windows service
  (`install_service.bat`, `start_service.bat`). Starting the service from an
  SSH session will work without any trick
* `controller.exe` can be started manually from an interactive session. If started
  from an SSH session, it requires some additional tricks

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
rustup install 1.89.0 && rustup default 1.89.0
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo install --locked cargo-c@0.10.15
```

#### Build: dev mode

```bash
./build-macos.sh -a <arch>
```

`<arch>` can be:
* `x86_64` (The default value)
* `arm64`

Note that there are some options in the build script to avoid some build steps:
* `-n`: Skip git fetch/checkout (preserves your local changes)
* `-s`: Skip the dependencies build
* `-o`: Output directory (must be absolute path)

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
