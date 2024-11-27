# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

* avserver/win32: Implement monotonic clock
* windows_service: Correctly log CreateProcessAsUserW() error
* kymux: Try to find an available port when starting IPC
* kyaudioreg: Fix compensation
* desktop: Add --audio-buffer parameter
* controller: Remove timeout on avserver kill
* kymux: Try to find an available port when starting IPC
* libclient/rdp: Allow to override the SDP folder with envvar KYBER_CACHEDIR

## [0.18.2] - 2024-10-22

* controller: Log initialization errors in controller.log

## [0.18.1] - 2024-10-21

* avserver: Log when display enumeration has been started
* avserver: Add pid in logs
* txproto/dxgi: Add more error logs
* txproto/wasapi: Reduce some log severity
* controller: Log actix server errors
* controller: Log displays at startup
* libclient: Stop backend on DataPlaneClosed

## [0.18.0] - 2024-10-09

* desktop: Refactor protocol selection and add audio protocol parameter
* desktop: Correctly hide cursor at startup if required
* avserver/win32: Workaround negative PTS generation while NTP goes backward
  in time
* controller/ws: Handle close control frame
* libclient: Update VLC. Kymux branch is now based on 6880921445
* libclient: Allow to rely on builtin OS TLS certificates
* desktop/kymux-audio: Add unreliable parameter
* controller: Correctly load the complete certificate chain
* controller/ws: Make server certificate generation optional
* desktop: Add audio volume keyboard shortcuts
* Update Quic stack (quinn 0.11.5, wtransport 0.4.0, rustls 0.23.13)

## [0.17.1] - 2024-07-11

* desktop: Add --gamepad_force_xbox360
* libclient: Log intra_refresh setting received in StreamingSessionConfig

## [0.17.0] - 2024-06-28

* libclient: Integrate kyaudioreg
* avserver: Allow to overload encoder configuration from kyber_config.toml
* kymux: Fix duplicate datagram handling with unreliable
* kymux: Fix sequencer for unreliable protocols
* desktop/win32: Hide console
* desktop: Add multiscreen support. Can be enabled with option --display_count
* desktop: Allow to configure Intra refresh with option --intra_refresh
* inputserver/win32: Plug PlayStation controllers as PS4 controllers
* controller: Allow to override dataplane port

## [0.16.1] - 2024-05-21

* kymux: Fix unreliable header size
* desktop: Expose kymux_unreliable protocol
* macos: Fix window size sometimes being invalid when resizing on macOS 14 Sonoma
* avserver/win32: Make process DPI aware
* inputserver/win32: Make process DPI aware
* desktop: Move the host cursor position to match the local one at startup
* libclient: Improve start and stop logs

## [0.16.0] - 2024-04-18

* avserver: Support yuv444p for hevc when using qsv
* avserver/win32: Update nv-codec-headers to n12.1.14.0
* avserver/win32: Update vpl to v2.10.1
* avserver/win32: Update amf to 1.4.33
* avserver: Update ffmpeg to 6.1.1
* libclient: Update VLC. Kymux branch is now based on dfd6d83e50
* vlc: Use AudioRegulator on Windows/Linux/macOS
* desktop: Audio is now enabled by default
* Update Rust dependencies
* kymux: Revert timeout detection
* libclient: Stop websocket on ControlPlaneClosed
* libclient: Log PlayerConfig on start_streaming()
* avserver: Improve initialization logs in dxgi and packet_sink
* avserver/amf: Add missing D3D11 flags to support AMD Ryzen 7 PRO 5875U

## [0.15.0] - 2024-03-19

* Rust toolchain updated to 1.76.0
* controller: Update wtransport to 0.11.0
* controller/cors: Allow Access-Control-Allow-Credentials
* controller: Add /stop_streaming endpoint
* controller: Fix racy kymux stop. Should fix the remaining 403 errors
              when starting kymux
* avserver: Integrate libtxproto
* avserver: Encoder selection now applies also to RTP backend
* avserver: Detect Initialization/Acquisition/Encoding errors and forward them
            to the application
* controller: Forward avserver crash to the application
* kymux/kycom: Fix TcpStream read error
* controller/libclient: Implement connection timeout detection (1 minute) on
  Data Plane. The event is only logged for now
* libclient: Stop if Data Plane connection fails, or if streaming start fails
* libclient/macos: Use ffmpeg 5.1.1 (like Windows) instead of master
* avserver: Support yuv444p for hevc when using nvenc
* libkynput: Handle Side/Extra MouseButtonType
* txproto: Avoid to drop encoded frames when network is slow. Drop acquired
  frames instead
* kymux: Correctly handle write during congestion, when using wtransport backend

## [0.14.0] - 2024-02-07

* controller/libclient: Implement connection timeout detection (1 minute) on
  Control Plane
* Add client lifecycle notifications (connection failed, disconnected + reason)
* Connection steps are now splitted
  1. Connect to the Control plane
  2. Start streaming session
* avserver/win32: Correctly patch NVENC H264 SPS to fix H264 latency issue on
  Windows Chromium when using hardware decoding

## [0.13.2] - 2023-12-18

* controller: Make listening port configurable in kyber_config.toml

## [0.13.1] - 2023-12-15

* libclient: Correctly set TLS hostnames for Kymux and Websocket connections
* Update TLS certificates

## [0.13.0] - 2023-11-17

:warning: This release isn't compatible with 0.12.x versions. :warning:

* Rust toolchain updated to 1.73.0
* ci: Debian build now targets Bookworm (12.0)
* linux: Integrate a correct support of libinput 1.19+
* avserver: Update ffmpeg to 6.0 and update its dependencies
* libkynput: Update to SDL 2.28.2
* kymux: Rework the EndpointId size and generation
* controller now kills stucked avserver when streaming session stop is requested
* libkynput: Counter Strike 2 is now playable
* libkynput: Fix black cursor instead of invisible cursor on VLC 3
* controller: Add websocket and webtransport backends

## [0.12.0] - 2023-06-23

:warning: This release isn't compatible with 0.10.x and 0.11.x versions. :warning:

* Update libvlc to kymux.15. Based on upstream 36b1865d95
* kymux: Add new video protocols (Gopstream and UnreliableFec)
* avserver: Enable Intra Refresh when using Nvenc. Can be disabled in kyber_config.toml
* client: Add --bitrate option to configure the connection bitrate
* client: Add new possible values for --protocol
    * kymux_reliable: Same than the existing "kymux" option. Uses a standard Quic stream for video
    * kymux_gopstream: Enable Gopstream protocol for video
    * kymux_unreliable_fec: Enable UnreliableFec protocol for video

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
