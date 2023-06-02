# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.11.0] - 2023-06-02

* Windows host: If Kidd is installed, a virtual monitor is plugged
                if the PC has no monitor

## [0.10.1] - 2023-04-17

* Update Rust dependencies
* Windows host: Correctly inject Ctrl+Alt+Del
* Windows host: Correctly inject Win+L

## [0.10.0] - 2023-04-07

* kymux: The implementation is now compatible with 32 bits targets.
         Breaks compatibility with previous versions
* client: Add --display_idx option to select the display to stream
* Handle any Host display resolution
* Handle Host dynamic resolution update
* All components are now logging in "log/" folder
* Windows service:
    * No cmd.exe are spawned anymore
    * install_service.bat now configure the service as autostart
    * install_service.bat correctly setups firewall rules

## [0.9.0] - 2023-02-23

* client: Update to winit 0.28.1
* libkynput: Update to xcb 1.2.0. Used on Linux Host to get
             cursor position and shape
* libkynput: Update to SDL 2.26.3
* kymux: A controller and a client can now be started on the same machine
* macOS client: Relative mouse moves can now be received without enabling IOKit
* macOS client: Correctly scale window size on Retina displays
* client: Hostname are now resolved when passed in the commandline
* Client isn't stuck anymore if an error occurs at early startup
* client: Rework cursor handling. "SetCursor: Reject" log spam should not happen
          anymore

## [0.8.1] - 2023-02-20

* client: Avoid to send Gamepad events when window focus is lost
* controller: Fix keyboard flush on Window focus loss/Application exit
* client: When the cursor is hidden, it is now impossible to go out of the window (0.7 regression)
* Windows: If MouseMove is supported, avoid to send MousePosition when cursor is hidden (0.7.0 regression)
* macOS M1 client: Fix a random crash at startup, during Kymux URI parsing

## [0.8.0] - 2023-02-15

* controller/libkynput: windows_sys is now used and replaces winapi
* Rust dependencies updated
* Rust toolchain updated to 1.65.0
* Add avserver that now uses libtxproto
* controller: Use libkypc to spawn and communicate inputserver and avserver
* controller: Switch to HTTPS
* client: Kymux (QUIC) replaces RTP as the default protocol
* client: Add --protocol option to choose between Kymux/RTP
* avserver: Add support of AMD and Intel GPUs on Windows. Not supported when using RTP
* client: Add --video_codec option to choose between H264/HEVC/AV1. Not supported when using RTP
* client: Add --port to choose the port to use to connect to a controller

## [0.7.0] - 2022-10-28

* controller: Update actix-web to 4.2.1
* controller/libclient/libkynput: Update tokio to 1.21.2
* macOS client: Fix Videotoolbox performances issue
* macOS client: Add custom cursor support
* macOS client: Experimental support of IOKit KeyboardKey/MouseMove grab.
                Enabled when envvar KYBER_USE_IOKIT is defined.
* macOS client: Fullscreen now hides Status bar, dock
* macOS client: Cursor location assistance is disabled
* macOS client: Fullscreen behavior also applied when fullscreen is triggered with the Status bar fullscreen button
* Windows txproto: Fix the cursor position sent. The cursor wasn't warped at the correct position after a window resize
* macOS client: Bind middle mouse button when not using IOKit
* All clients: Fullscreen now locks the cursor in the video. This also allows to use macOS client on multiple screen setups
* Windows/Linux: libvlc build has been reduced
* Windows/Linux: Final packages have cleaned. Useless files aren't copied, and bins/libs are stripped

## [0.6.0] - 2022-10-12

* client: --inputs=false can now be passed to disable inputs
* Audio grab (Windows/Linux) and playback (All) is now available. It is disabled by default, and can be enabled with --audio=true
* Update libvlc, based on ba9b27e1ae +  0latency.41
* macOS client: Minimal implementation. Video Toolbox performances are bad, but software decoding is working correctly. Basic Keyboard/Mouse/Gamepad support
* Linux client: Fix a mouse left click issue that works once
* Windows: Fix loading issue for both Client/Host on Windows 11

## [0.5.0] - 2022-06-10

* Switch to Rust 1.61.0
* Run `cargo update`
* Windows: Switch to [vigem-client](https://github.com/CasualX/vigem-client) crate (MIT license).
* Update SDL2 to 2.0.22
* Windows client: Fix video artifact issue

## [0.4.0] - 2022-06-02

* client: Input/Window interaction improvements
* Host cursor is now sent to the client on connection
* Gamepad plug/unplug are sent even if the window isn't focused
* client: The cursor is now grabbed within the window in relative mode

## [0.3.0] - 2022-05-17

* client: Add --444 flag to enable YUV 4:4:4
* Windows client: Fix 444 decoding

## [0.2.0] - 2022-05-13

* Improve client robustness when a process crashes on host side (#71)
* Assassin's Creed Odyssey is not playable with the mouse
* Linux client: Add support of libinput 1.19+
* Linux client: Extended scancodes are now handled correctly when the user isn't in input group
* Linux client: App can now be closed with CTRL+C
* vlc: Fix a one-frame latency issue on RTP access

## [0.1.0] - 2022-05-06

### Added
* Windows Host and Client
* Linux Host and Client
