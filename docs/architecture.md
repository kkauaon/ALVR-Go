# Architecture: ALVR 18.2.3

> Stack and build commands: [project.md](./project.md).

> **Fork scope.** Upstream 18.2.3 supports Quest, Quest 2 and Go. This fork targets the Oculus Go only. Quest-specific code paths remain in the tree but are not part of the fork's support surface; no fork patches exist yet.

## System Overview

ALVR has two halves. The **server** runs as a SteamVR driver on the PC: SteamVR's `vrserver` loads `driver_alvr_server`, which owns streaming, encoding, the web dashboard and the audio bridge. The **client** runs as an Android VR application on the headset: it advertises itself, receives the encoded stream, decodes it with MediaCodec, and submits frames through Oculus VrApi.

```text
                     Windows or Linux PC
 +---------------------------------------------------------------+
 |  ALVR Launcher (druid)                                        |
 |    registers driver -> starts SteamVR -> waits for :8082       |
 |                                                               |
 |  SteamVR vrserver                                             |
 |    +-- driver_alvr_server  (Rust cdylib + C++ OpenVR driver)   |
 |          |- session.json, client trust list, settings          |
 |          |- hyper HTTP + WebSocket server on 0.0.0.0:8082      |
 |          |- dashboard window (embedded Chromium, alcro)        |
 |          |- UDP discovery on :9943                             |
 |          |- TCP control socket on server<->client :9943        |
 |          |- stream sockets on :9944 (UDP or TCP)               |
 |          |- game audio capture, microphone playback           |
 |          |- C++ encoder -> NVENC / AMF / VAAPI / FFmpeg        |
 +-------------------------------+-------------------------------+
                                 | Wi-Fi (802.11ac 5 GHz)
 +-------------------------------v-------------------------------+
 |  Android headset                                              |
 |    ALVR client app (Java + Rust + C++/VrApi)                   |
 |      |- VrApi session, TimeWarp, GLES rendering                |
 |      |- MediaCodec H.264/HEVC decoder threads                  |
 |      |- NAL queue, Reed-Solomon FEC decode                     |
 |      |- control/stream sockets, input and haptics              |
 +---------------------------------------------------------------+
```

## Components

### Launcher (`alvr/launcher`)

A small Druid window with two states: a requirements check and a "waiting for server" view. It enforces a single instance, rejects installation paths containing non-ASCII characters, registers the ALVR driver in `openvrpaths.vrpath` if necessary, and opens SteamVR once automatically. If registration fails while SteamVR is running, it kills SteamVR and relaunches it; the "Reset drivers and retry" button performs the more destructive repair (delete `openvrpaths.vrpath`, relaunch, kill, clear the ALVR entry from `steamvr.vrsettings`). It then polls `http://127.0.0.1:8082/index.html` every 500 ms and closes when the dashboard answers. `--restart-steamvr` and `--update` are non-GUI modes used by the driver and the updater.

### Server driver (`alvr/server`)

The crate builds a `cdylib` that exports `HmdDriverFactory`, the OpenVR entry point. The first call runs `init()` once:

1. Initializes the Tokio runtime, logging (fern) and the session manager.
2. Starts the web server and the dashboard window thread.
3. Applies GPU-specific patches: on NVIDIA, `linux_async_reprojection` is forced off.
4. Clears the client list when the stored `server_version` differs from the running version.
5. Publishes the session path and driver root to the C++ side.

The C++ side is a conventional OpenVR `IServerTrackedDeviceProvider`. `DriverProvider` creates `OvrHmd`, `OvrController` for each hand, and optional Vive tracker proxies. `OvrHmd` implements the display and pose interfaces, including `GetWindowBounds` and distortion; `GetComponent` hands out the Windows-only direct mode component, `OvrDirectModeComponent`, which receives submitted eye textures from SteamVR and passes them to the encoder. On Linux there is no direct mode; the Vulkan layer provides the display instead. Supporting pieces include `PoseHistory` (pose extrapolation), `VSyncThread`, `ChaperoneUpdater`, `IDRScheduler`, `Statistics` (bitrate control and counters), and `Settings`, which reads the `openvr_config` section of `session.json` directly with `picojson`.

`DriverReadyIdle` from SteamVR starts the Rust connection lifecycle loop. The loop repeatedly runs a connection pipeline and sleeps at least one second between attempts, so a client disconnect never terminates the driver.

### Web server and dashboard (`alvr/server/src/web_server.rs`, `alvr/dashboard`)

A Hyper server bound to `0.0.0.0:<web_server_port>` (default 8082). It serves the static dashboard from the installation's `dashboard` directory, with a `..` check before opening files. The JSON API is:

| Route | Method | Purpose |
|---|---|---|
| `/api/settings-schema` | GET | Serialized schema of the settings tree, used to render the UI. |
| `/api/session/load` | GET | Current `SessionDesc`. |
| `/api/session/store-settings` | POST | Merge a settings subtree into the session. |
| `/api/session/store` | POST | Merge a full session object into the session. |
| `/api/log` | WebSocket | Live log lines. |
| `/api/events` | WebSocket | Structured `ServerEvent` objects and raw log events. |
| `/api/driver/register`, `/api/driver/unregister`, `/api/driver/list` | GET/POST | OpenVR driver registration management. |
| `/api/firewall-rules/add`, `/api/firewall-rules/remove` | GET | Add or remove OS firewall rules (elevation may be requested). |
| `/api/audio-devices`, `/api/graphics-devices` | GET | Device enumeration for the settings UI. |
| `/api/client/add`, `/api/client/trust`, `/api/client/remove` | POST | Manage the client trust list. |
| `/api/version`, `/api/server-os` | GET | Version and host OS. |
| `/api/open` | POST | Open a URL in the host browser. |
| `/api/update` | POST | Download an installer from a caller-supplied URL. |
| `/restart-steamvr` | GET | Restart the driver and SteamVR. |

The dashboard is a RequireJS single-page app with four sections: Monitor, Settings, Installation, About. A setup wizard runs on first launch (`setupWizard` in the session). Settings are rendered generically from the schema, with an "advanced" toggle; presets can be uploaded as JSON. The monitor opens `/api/log` and `/api/events`, plots latency and frame-rate graphs with uPlot, and lists discovered clients with Trust/Remove actions.

The driver opens its own dashboard window at `http://127.0.0.1:8082` through `alcro`, using a temporary Chromium profile. That URL is hardcoded, while the web server port is configurable, so a non-default port only works in an external browser.

### Client (`alvr/client`)

The Android app is a thin Java shell around a Rust `cdylib` and a C++ VrApi renderer.

- **Java**: `OvrActivity` owns the VrApi lifecycle and native calls; `DecoderThread`, `NalQueue`, `OutputFrameQueue` and `FrameMap` move encoded frames through MediaCodec.
- **Rust**: JNI entry points for logging, identity creation, activity create/resume/pause/destroy, render, battery, stream start, IDR requests, and connection state. It also owns the connection pipeline, input assembly, audio and control packets.
- **C++**: `ServerConnectionNative.cpp` handles the legacy socket path; `nal.cpp` and `fec.cpp` parse and repair the video stream; `render.cpp`, `ffr.cpp` and `gl_render_utils` draw the eye textures with fixed foveated rendering; `vr_gui.cpp` draws the loading/message panel; `latency_collector.cpp` measures decode and transport latency.

On resume, the client creates a private identity (`NNNN.client.alvr` plus a self-signed certificate), builds a `HeadsetInfoPacket` from the headset's recommended eye resolution and refresh rates, and starts the connection loop. On pause it stops the runtime and native session.

The Android build has `OculusGo` and `OculusQuest` flavors; this fork ships the Go flavor (`applicationIdSuffix ".go"`, `minSdk 21`). At resume the native layer reads `VRAPI_SYS_PROP_DEVICE_TYPE` and maps `VRAPI_DEVICE_TYPE_OCULUSGO` to the Go device type, which Rust reports as "Oculus Go". The Go controller is handled by its own branch in `ovr_context.cpp` (trackpad, trigger, back/enter), separate from the Quest touch-controller branch. The Go has 3DoF head and controller tracking, so no positional controller pose is available; `headset.force_3dof` and `headset.tracking_ref_only` exist for constrained tracking setups.

### Protocol layer (`alvr/sockets`)

- **Discovery**: the client broadcasts a bincode `HandshakePacket::Client` to `255.255.255.255:9943` every second. The server binds UDP 9943, validates the ALVR name and version, checks trust, and answers with `ServerHandshakePacket` only to reject (`ClientUntrusted`, `IncompatibleVersions`). Silence means "not trusted or not seen".
- **Control**: TCP, server connects to the client's listener on port 9943, `LengthDelimitedCodec` framing, bincode payloads. The client sends `HeadsetInfoPacket` and its observed server IP; the server answers with `ClientConfigPacket`. After the split, each side sends `ServerControlPacket` / `ClientControlPacket` messages.
- **Streams**: the client listens on `stream_port` (9944) and the server connects. UDP, throttled UDP, or TCP is chosen by the `stream_protocol` setting. Every packet is `stream_id: u16`, `packet_index: u32`, bincode header, then payload. Stream IDs are `INPUT=0`, `HAPTICS=1`, `AUDIO=2`, `VIDEO=3`. Packet indices allow loss detection. FEC is implemented in the C++ video path and enabled by `enable_fec`.

### Audio (`alvr/audio`)

`AudioDevice` abstracts output, virtual-microphone input, and virtual-microphone output devices over CPAL. On Windows it also resolves WASAPI endpoint IDs so OpenVR can point its default playback/recording devices at the right hardware. The server captures game audio in stereo at the device's shared-mode sample rate, streams it, and optionally plays microphone audio received from the client into a virtual microphone device. Linux uses ALSA or JACK as selected by `linux_backend`. Game audio and the microphone may not be the same device; the handshake rejects that configuration.

### Linux capture path (`alvr/vulkan-layer`, `alvr/vrcompositor-wrapper`)

SteamVR's Linux runtime does not accept software output devices. The ALVR Vulkan layer adds a display to `vkGetPhysicalDeviceDisplayPropertiesKHR` and implements the related display/swapchain functions, so the compositor renders into ALVR-owned images. It then shares those images with the driver process and forwards present calls. `vrcompositor-wrapper` sets `VK_LAYER_PATH` and `VK_INSTANCE_LAYERS=VK_LAYER_ALVR_capture`, disables the Steam Fossilize layer (it breaks the ALVR layer), and execs `vrcompositor.real`. Both pieces are Linux-only and documented as SteamVR-specific in their README.

## Data Model

### Session

```text
SessionDesc
  serverVersion: semver
  setupWizard: bool
  locale: string
  openvrConfig: OpenvrConfig        # everything the C++ driver needs at init
  clientConnections:
    <hostname>: { displayName, manualIps: set<IpAddr>, trusted: bool }
  sessionSettings: SessionSettings  # full settings tree, every branch present
  advanced: bool
```

`OpenvrConfig` is the subset that must be known before a client connects: headset identity strings, eye and target resolutions, refresh rate, codec, bitrate and adaptive-bitrate parameters, controller identity and offsets, prediction and haptics tuning, foveation and color correction, FEC, and the Linux async-reprojection patch.

When a client connects, the server builds a fresh `OpenvrConfig` from the current settings. If it differs from the stored one, the server replies `Restarting`, saves the session, and restarts SteamVR. This is why some settings apply immediately while others require a SteamVR restart.

### Settings tree

`alvr/session/src/settings.rs` defines a `Settings` root with five sections:

| Section | Contents |
|---|---|
| `video` | Adapter, render resolution (scale or absolute), recommended target resolution, preferred FPS, codec (H.264/HEVC), 10-bit encode, software thread count, bitrate, adaptive bitrate switch, vsync-to-photons latency, foveated rendering switch, color correction switch. |
| `audio` | Linux backend (ALSA/JACK), game audio switch (device, mute while streaming, buffering/batch), microphone switch (input/output devices, sample rate, buffering/batch). |
| `headset` | Headset emulation mode, universe ID, identity strings, tracking offsets, 3DoF, tracking-reference mode, Vive tracker proxy, controllers switch (mode, identity, prediction, velocity cutoffs, pose offsets, haptics tuning), extra latency mode. |
| `connection` | Client discovery switch (auto-trust), web server port, stream protocol, stream port, aggressive keyframe resend, on-connect and on-disconnect scripts, FEC. |
| `extra` | Theme, client dark mode, confirmation dialogs, update channel, log-to-disk, notification level, advanced-only patches (`removeSyncPopup`, `linuxAsyncReprojection`). |

The tree is generated by `#[derive(SettingsSchema)]` into two parallel shapes: the concrete `Settings` used by the driver, and a `SettingsDefault` tree that keeps every branch even when a switch is disabled. The UI works on the second shape, the driver on the first. Types include sections, tagged choices, optional values, switches, arrays, vectors and dictionaries, each with defaults, min/max bounds, step sizes and GUI hints.

### Persistence

| File | Location | Content |
|---|---|---|
| `session.json` | Windows: install root. Linux: `~/.config/alvr/`, unless the build overrides the config directory (`ALVR_CONFIG_DIR`, set by `--root` packaging). | The whole `SessionDesc`. |
| `session_old.json` | Same directory | Copy of a session that failed to deserialize. |
| `session_log.txt` / `alvr_session_log.txt` | Windows: install root. Linux: home directory. | Session log when `log_to_disk` is on; otherwise logs go to stdout. |
| `crash_log.txt` | Same log directory | Error-level log sink. |
| `openvrpaths.vrpath` | SteamVR config directory | External driver registration. |

Loading is tolerant. If `session.json` does not deserialize, it is copied to `session_old.json` and `merge_from_json` walks the schema, keeping old values whose names and JSON types still match and filling the rest from defaults. The server version is then updated and the client list cleared.

### Wire packets

`ClientHandshakePacket` (ALVR name, version, device name, hostname, reserved fields), `HeadsetInfoPacket` (recommended eye size, refresh rates, preferred rate), `ClientConfigPacket` (session JSON, dashboard URL, eye resolution, FPS, game audio sample rate, server version), `ServerControlPacket` (StartStream, Restarting, KeepAlive, legacy time sync), `ClientControlPacket` (PlayspaceSync, RequestIdr, StreamReady, ViewsConfig, Battery, legacy time sync, error report), `Input` (device motions, optional hand tracking, button values, legacy controller state), `Haptics`, and the legacy `VideoFrameHeaderPacket`. Legacy fields remain in the schema for compatibility with older packet layouts.

## Key Flows

### 1. Launch and driver load

`ALVR Launcher` checks that SteamVR is installed, registers the driver path if needed, and opens SteamVR once (killing and relaunching it only when registration fails). `vrserver` loads `driver_alvr_server`, which runs `init()` and creates the web server and dashboard window. When SteamVR signals `DriverReadyIdle`, the async connection lifecycle loop starts.

### 2. Discovery and trust

The client broadcasts its handshake every second. The server creates a session entry for an unknown hostname (untrusted), then accepts the client only if the entry is trusted or `auto_trust_clients` is enabled. If not, it replies `ClientUntrusted`; the client shows "On the PC, click Trust next to the client entry". Trust can be granted from the monitor tab, which adds the client's IP to `manualIps`, or manually by adding an IP.

### 3. Handshake and configuration

After discovery, the server opens TCP 9943 to the client and receives `HeadsetInfoPacket`. It computes the video eye resolution from the render-resolution setting, aligns both dimensions down to 32-pixel multiples, picks the supported refresh rate nearest to the preferred one, resolves the game audio sample rate, and sends a `ClientConfigPacket` containing the session JSON, dashboard URL and server version. The client re-parses the session locally, so both sides agree on stream port and protocol before any stream socket exists. If the newly derived `OpenvrConfig` differs from the stored one, the driver restarts SteamVR instead of streaming.

### 4. Stream start

The server sends `StartStream`; the client binds its stream socket and replies `StreamReady`; the server connects within a 5-second timeout, then calls `InitializeStreaming()` in C++. It also runs the on-connect script, starts game audio capture and microphone playback, and starts the video, time-sync, haptics and input loops. `StreamCloseGuard` reverses this on any exit path and runs the on-disconnect script.

### 5. Tracking, input and haptics

The client converts VrApi tracking into `Input`: head, left hand and right hand motions, optional angular/linear velocities, controller buttons, trigger/grip values, and hand-tracking bone data. The server translates it into the C++ `TrackingInfo` struct and calls `InputReceive`. `OvrHmd::OnPoseUpdated` stores poses and `GetPose` returns the predicted pose for SteamVR. OpenVR haptic events are captured in `RunFrame`, scaled by the haptics settings, and sent back as `Haptics` packets; the client applies them through VrApi. Playspace size changes from the client update the chaperone.

### 6. Video path

SteamVR composites the game frame. On Windows, `OvrDirectModeComponent` receives the eye textures and `FrameRender` applies color correction and fixed foveated rendering before `VideoEncoder` (NVENC, AMF VCE or software) produces H.264/HEVC. On Linux, the Vulkan layer receives the frame and the FFmpeg-based pipelines encode it (NVENC, VAAPI or software). Encoded packets reach Rust through the `VideoSend` callback and are written to the VIDEO stream with sequence numbers and optional FEC. The client parses NAL units, repairs packet loss, feeds MediaCodec, and renders the decoded eye textures through VrApi, which applies Asynchronous TimeWarp.

### 7. Latency, bitrate and statistics

The client reports time sync and latency data on the control socket. The server forwards it to the C++ `TimeSyncReceive`, where `Statistics` uses the reported latency against the adaptive-bitrate target to raise or lower the encoder bitrate within the configured maximum. Legacy statistics events are emitted as log lines and forwarded to the dashboard over `/api/events`; the monitor plots total, encode, transport and decode latency plus client and server FPS.

### 8. Settings, update and shutdown

The dashboard posts settings to `/api/session/store`. `SessionManager` merges them into `SessionDesc`, writes `session.json`, and broadcasts a `Session` event. Changes that affect `OpenvrConfig` take effect on the next connection, which triggers a SteamVR restart. Updates are checked in the browser against the GitHub Releases API; `/api/update` downloads the installer to the temp directory, then `notify_application_update` shuts down the driver and invokes the launcher with `--update`. Closing the dashboard window shuts down the runtime and calls `ShutdownSteamvr`.

## Integration Points

| System | Protocol / API | Direction | Failure handling |
|---|---|---|---|
| SteamVR | OpenVR driver interfaces, `openvrpaths.vrpath` | Server registers and is loaded by SteamVR | Registration errors surface in the dashboard; a failed launch offers "Reset drivers and retry". |
| Oculus runtime | VrApi, `libvrapi.so`, platform loader | Client initializes and renders | VrApi errors abort the native session; the Java activity reports them. |
| Android decoder | MediaCodec via JNI | Client decodes H.264/HEVC | Decoder threads wait for an IDR on error and can request one from the server. |
| Video encoders | NVENC, AMF VCE, VAAPI, FFmpeg, MediaCodec | Server encodes, client decodes | Encoder initialization failures return error strings; software fallback exists when built with FFmpeg support (`--gpl` on Windows, bundled FFmpeg on Linux). |
| Audio devices | WASAPI, ALSA, JACK via CPAL | Bidirectional | Device open failures abort the connection with an error; game audio and microphone cannot share a device. |
| Vulkan loader and compositor | Vulkan explicit layer, `VK_LAYER_PATH` | Linux capture | Layer manifest missing or wrapper symlink broken panics the wrapper; SteamVR then reports a compositor failure. |
| GitHub Releases | HTTPS REST and asset download | Dashboard to GitHub, installer to PC | Download errors are reported to the dashboard; failed downloads do not install. |
| OS firewall | `netsh`, `ufw`, `firewalld` | Server modifies host rules | Non-zero exit codes are returned to the dashboard. |
| Local web UI | HTTP/WebSocket on 8082 | Browser or embedded Chromium | Server binds all interfaces; static files return 404, traversal attempts return 403. |

## Security Model

This section describes what the code enforces, not what it ought to enforce.

- **Client trust is an allowlist.** `session.json` stores each client's hostname, display name, manual IPs and a `trusted` flag. A client is accepted when its hostname entry is trusted or when discovery auto-trust is enabled. Auto-trust defaults to on only in debug builds.
- **Version gate.** The server rejects clients whose major version differs, or whose full version differs for prereleases. Older v11-v13 handshakes are detected and reported as "too old".
- **Transport is unauthenticated and unencrypted.** The client generates a self-signed certificate and key at identity creation, but the control and stream sockets in this version are plain TCP/UDP with bincode payloads; no TLS handshake exists in `alvr/sockets`. Trust therefore rests on the hostname/IP allowlist and on LAN isolation, not on cryptography.
- **The dashboard has no authentication.** It binds `0.0.0.0:8082`, sets `Access-Control-Allow-Origin: *`, and exposes settings changes, driver registration, firewall changes, arbitrary URL opening and the update download. Anyone who can reach the port can exercise those endpoints. The only request-level guard is a `..` check on static file paths.
- **The update endpoint fetches a caller-supplied URL.** `POST /api/update` downloads whatever URL it is given to the temp directory and then triggers the launcher's update path, which runs the downloaded installer. Access to the dashboard is therefore equivalent to code execution on the PC.
- **Firewall rules are scoped to executables.** On Windows the rules allow inbound traffic for `vrserver.exe` in both `win64` and `win32`; adding or removing them requests elevation through UAC. On Linux the packaged scripts use `pkexec`.
- **Logs and config are plain files.** `session.json`, session logs and crash logs contain hostnames, IP addresses, device names and settings. They are not encrypted or access-controlled beyond filesystem permissions.
- **Path handling.** The dashboard server rejects paths containing `..`, and the launcher refuses installation paths containing non-ASCII characters, asking the user to move the folder somewhere else.
