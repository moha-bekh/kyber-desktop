# Remote PC client/host

Tool that uses ffmpeg and libvlc to remotey control a computer desktop.

## Build

### Get submodules

The project has some submodules to get
```bash
git submodule update --init --recursive
```

### Linux

#### Debian: Install required system packages

```bash
sudo apt-get install -y \
    build-essential cmake meson ninja-build pkg-config \
    llvm-dev libclang-dev clang libssl-dev \
    autoconf libtool curl git wget unzip \
    python3 python3-setuptools python3-mako python3-venv \
    python3-jinja2 \
    lua5.4 liblua5.4-dev \
    autopoint gettext bison flex nasm ant ragel help2man \
    libdrm-dev libgbm-dev glslang-dev liblcms2-dev libpulse-dev \
    libxcb-xkb-dev libxkbcommon-dev libxcb-xtest0-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libwayland-dev wayland-protocols libvulkan-dev \
    libssl-dev libevdev-dev libudev-dev libinput-dev \

sudo apt-get build-dep -y vlc
```

The kysdk/kymedia meson build requires meson **1.10.0+**. Debian/Ubuntu apt
(including Trixie) ship an older meson, so install a recent one with `uv` if the
build complains:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
uv tool install meson==1.10.0 --with jinja2
export PATH="$HOME/.local/bin:$PATH"
```

Tested distros
* Debian Bookworm
* Ubuntu 24.04

#### Arch: Install required system packages

```bash
pacman -Sy base-devel clang cmake meson ninja pkg-config \
    git wget unzip nasm python-jinja lua \
    libxcb libxrandr libxkbcommon libva wayland-protocols vulkan-headers \
    opus libpulse alsa-lib \
    libinput
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

The script outputs files in folder `rootfs-x86_64-linux-gnu`, including helper
scripts to setup all the required envvars to run the client and the server.

Note that there are some options in the build script to avoid some build steps:
* `-s`: Skip the dependencies build
* `-o`: Output directory (must be absolute path)

#### Run client

```bash
./rootfs-x86_64-linux-gnu/run_kyclient.sh <host_ip>
```

### Input notes for client

* The input part has two backend for Keyboard and Mouse acquisition: `winit` and
  `libinput`. `winit` uses a standard X11 connection to get inputs, while `libinput`
  reads directly `/dev/input/` devices, but it requires for the user to be in the `input` group.
  To have access to `/dev/input/`, use one of the tricks describes above.

#### Run host

```bash
./rootfs-x86_64-linux-gnu/run_kycontroller.sh
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
    python3-jinja2 \
    lua5.4 gettext bison flex ant ragel help2man nasm
```

The kysdk/kymedia meson build requires meson **1.10.0+**. Debian/Ubuntu apt
(including Trixie) ship an older meson, so install a recent one with `uv` if the
build complains:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
uv tool install meson==1.10.0 --with jinja2
export PATH="$HOME/.local/bin:$PATH"
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
    m4 nasm openssl@3 pkg-config \
    autoconf automake libtool \
    cmake gettext meson ragel ant help2man \
    bison
```

The bison bundled with macOS is too old, so the homebrew one must be installed
and put ahead on PATH:

```bash
echo 'export PATH="/opt/homebrew/opt/bison/bin:$PATH"' >> ~/.zshrc

# or for the current shell only

export PATH="/opt/homebrew/opt/bison/bin:$PATH"
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

## Connecting via a `kyber://` link

The client can open a connection from a `kyber://<host>:<port>/?<param>=<value>` URL, so a browser or the kyhub portal can launch a session with one click. Each query parameter is a CLI flag spelled with underscores (`tls_tofu` for `--tls-tofu`); `yuv444` and `display_index` map to aliases of `--444` and `--display-idx`. Boolean parameters take a value in the URL (`tls_tofu=true`, accepts `true`/`1`/empty or `false`/`0`) but map to a bare flag on the CLI. Value parameters that carry a boolean (`audio`, `inputs`, ...) accept only `true` or `false`, since clap validates them. Values are form decoded, so `+` means a space and a literal `+` must be `%2B`. A `port` parameter overrides the port in the authority. For example `kyber://streamer.example?tls_tofu=true&auth_oidc_token=<token>` launches the same session as `kyclient streamer.example --tls-tofu --auth-oidc-token <token>`.

The full mapping:

| URL parameter | CLI flag | Type |
|---|---|---|
| `port` | `--port` (or the port in `<host>:<port>`) | value |
| `protocol` | `--protocol` | value |
| `kymux_video` | `--kymux-video` | value |
| `kymux_audio` | `--kymux-audio` | value |
| `video_codec` | `--video-codec` | value |
| `bitrate` | `--bitrate` | value |
| `audio` | `--audio` | value |
| `inputs` | `--inputs` | value |
| `intra_refresh` | `--intra-refresh` | value |
| `gamepad_force_xbox360` | `--gamepad-force-xbox360` | value |
| `clipboard` | `--clipboard` | value |
| `keyboard_grab` | `--keyboard-grab` | value |
| `metrics` | `--metrics` | value |
| `audio_buffer` | `--audio-buffer` | value |
| `video_buffer` | `--video-buffer` | value |
| `display_index` | `--display-index` (alias of `--display-idx`) | value |
| `display_count` | `--display-count` | value |
| `auth_username` | `--auth-username` | value |
| `auth_token` | `--auth-token` | value |
| `auth_oidc_token` | `--auth-oidc-token` | value |
| `auth_oidc_refresh_token` | `--auth-oidc-refresh-token` | value |
| `kyhub_machine_id` | `--kyhub-machine-id` | value |
| `yuv444` | `--yuv444` (alias of `--444`) | boolean |
| `software_decode` | `--software-decode` | boolean |
| `tls_tofu` | `--tls-tofu` | boolean |
| `tls_known_hosts` | `--tls-known-hosts` | boolean |

A `kyber://` link is web-triggerable, so the parameters that would change how the server is trusted, start a sign-in or carry a password are rejected: `tls_skip_verification`, `tls_fingerprint`, `tls_root_cert`, `tls_host`, `tls_tofu_interactive`, `auth_oidc` and `auth_password`. `tls_tofu` stays: it trusts the first certificate of the link's own host only. Any other unknown parameter is an error, and so is anything else on the command line next to a link: the client logs the error and exits instead of connecting with a setting silently dropped.

A link with `auth_oidc_token` logs in with that token and never opens a browser. With `kyhub_machine_id` and `auth_oidc_refresh_token` next to it, the client renews the token at kyhub's `:refresh-connection`; without them the session ends when the token expires. `kyhub_machine_id` alone is rejected, and a challenge from the link's host ends the session instead of signing in to kyhub: a link must not start a sign-in. The token is visible to the other processes of the same user for as long as the client runs, in the command line the OS passes and in any launcher journal, so a portal should mint a short-lived, single-use token for a link.

To register the scheme on Linux, build the package and run the install script from it:

```bash
./build-linux.sh -p
tar xjf kyber-linux-x86_64.tar.bz2
./kyber-linux-x86_64/install_kyclient.sh
```

This sets the client as the handler for `x-scheme-handler/kyber`. To undo, remove `~/.local/share/applications/kyber.desktop`. On Windows, run `register_url_scheme.bat` from the extracted package to register the scheme for the current user (no admin rights needed); `unregister_url_scheme.bat` removes it.

## Some tips

### My video decoder crashes

Especially on Linux with an Nvidia driver, the video decoding part can crash.

Software decode can be enable with `--software-decode` to workaround the issue.

### I want to run the controller and the client on the same machine

It is possible to connect to `localhost`. However, the client must be started
with `--audio=false --inputs=false` options.
