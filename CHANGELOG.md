# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

* controller: Update actix-web to 4.2.1
* controller/libclient/libkynput: Update tokio to 1.21.2
* macOS client: Fix Videotoolbox performances issue
* macOS client: Add custom cursor support
* macOS client: Experimental support of IOKit keyboard/mouse grab.
                Enabled when envvar KYBER_USE_IOKIT is defined.
* macOS client: Fullscreen now hides Status bar, dock
* macOS client: Cursor location assistance is disabled
* macOS client: Fullscreen behavior also applied when fullscreen is triggered with the Status bar fullscreen button
* Windows txproto: Fix the cursor position sent. The cursor wasn't warped at the correct position after a window resize
* macOS client: Bind middle mouse button when not using IOKit

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

## [0.1.0] - 2022-05-06

### Added
* Windows Host and Client
* Linux Host and Client
