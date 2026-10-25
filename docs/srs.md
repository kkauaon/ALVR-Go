# SRS: ALVR 18.2.3 (derived from existing code)

> Status: derived from a source audit, with a Go-only fork scope added on 2026-10-24. Requires review and approval before use in planning. The requirement bodies describe behavior inferred from code paths, defaults or the repository README; the scope section records the fork owner's decision.

## Context

ALVR is a PC-to-headset VR streaming system. A Windows or Linux PC runs an OpenVR driver inside SteamVR, encodes the composited eye images, and sends them over Wi-Fi together with game audio and haptics. An Android app on the headset decodes the stream, renders it through VrApi with Asynchronous Timewarp, and sends tracking, input, microphone audio and battery data back. Upstream 18.2.3 supports Quest, Quest 2 and Go; this fork targets Go only. Configuration and live monitoring happen through a native egui dashboard window opened by the driver (no browser, embedded Chromium or network-reachable UI).

The requirements are grouped by domain. "Priority" is uniformly TBD because the source tree contains no requirement backlog. Each behavior is marked Draft until a maintainer confirms it is intended, not merely implemented.

## Fork scope

This fork starts from ALVR 18.2.3 and targets the Oculus Go only. The requirements below describe upstream behavior; the fork keeps them except where a clause is explicitly Quest-only.

- Supported headset: Oculus Go, arm64, Android API 21 or newer.
- Reference client build: the `OculusGo` product flavor, shipped as `alvr_client_oculus_go.apk`.
- Server behavior stays as upstream unless a Go limitation requires a patch.
- Quest and Quest 2 support is inherited in the source but out of scope for fork releases.
- Requirements that depend on 6DoF controllers or hand tracking do not apply to the Go and may be dropped.

# 1. Functional Requirements

## RF-100: Connection and Discovery

### RF-100.1: Client announcement
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* The client broadcasts a bincode handshake packet over UDP to `255.255.255.255:9943` and repeats it every second while no server answers.
* The server listens on UDP 9943 and ignores unrelated datagrams.
* Handshakes from v11-v13 clients are recognized by their leading bytes and reported as "too old".
* A packet with a different ALVR name is reported as an invalid client.
* <!-- TBD: whether broadcast discovery must work across subnets or routed networks -->

### RF-100.2: Version compatibility
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-100.1
* Client and server are compatible when their major versions match.
* For prerelease versions, the comparison includes major, minor, patch and prerelease tag; build metadata is ignored.
* An incompatible client receives an `IncompatibleVersions` response and displays a message instead of connecting.

### RF-100.3: Client trust
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-100.1
* A previously unseen hostname is added to the session as an untrusted client with its reported device name.
* A client is accepted only if its stored entry is trusted, or if `client_discovery.auto_trust_clients` is enabled.
* An untrusted client receives a `ClientUntrusted` response and the headset shows "On the PC, click Trust next to the client entry".
* The dashboard can trust a client (optionally recording its IP), remove a single manual IP, or remove the client entry.
* Untrusted entries are removed at driver startup.

### RF-100.4: Direct connection by IP
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-100.3
* Trusted clients can be reached by the manual IPs stored in their session entry, without relying on discovery.
* When client discovery is disabled, the server connects only to stored manual IPs.

## RF-200: Session Establishment

### RF-200.1: Headset capability exchange
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-100.2
* The client reports the headset's recommended eye resolution, available refresh rates, preferred refresh rate, device name and client version.
* The server selects the supported refresh rate closest to the configured `preferred_fps` and logs a warning when it differs from the request.

### RF-200.2: Resolution negotiation
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.1
* Render and target resolutions are configured either as a scale of the headset's recommendation or as an absolute frame size.
* Both eye width and height are rounded down to multiples of 32 pixels before being sent to the client.
* The absolute frame width is split in half to obtain the per-eye width.

### RF-200.3: Client configuration
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.2
* The server sends the full session as JSON, a legacy dashboard URL field (`http://<server-ip>:<web_server_port>/`, no longer served by anything), eye resolution, FPS, game audio sample rate and server version.
* The client re-parses the session locally to obtain stream port, protocol and client-side display options.

### RF-200.4: OpenVR configuration consistency
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.3
* The server derives an `OpenvrConfig` from the current settings at each connection.
* If it differs from the stored configuration, the server sends `Restarting`, saves the session and restarts SteamVR before accepting the stream.
* <!-- TBD: which settings are intended to apply without a restart; the source comment calls this transitional -->

### RF-200.5: Stream transport
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.3
* The client listens on the configured `stream_port` (default 9944) and the server connects to it.
* Supported transports are UDP, throttled UDP and TCP. Defaults are UDP on Windows and TCP on Linux; throttled UDP carries a bitrate multiplier.
* Control traffic uses TCP port 9943 with length-delimited, bincode-encoded packets.
* Stream packets carry a 16-bit stream ID, a 32-bit packet index, a bincode header and a payload.

### RF-200.6: Start and keepalive
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.5
* The server sends `StartStream`; the client replies `StreamReady`; the server must establish the stream socket within 5 seconds.
* Both sides exchange keepalive packets every second.
* A socket error or missing keepalive ends the session, is reported as `ClientDisconnected`, and returns the driver to the discovery loop.

## RF-300: Media and Tracking

### RF-300.1: Video streaming
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* Video is encoded as H.264 or HEVC with a configurable bitrate in Mbit/s.
* Hardware encoding uses NVENC, AMF VCE or VAAPI where available; software encoding uses FFmpeg/x264/x265 when built with the corresponding feature.
* `use_10bit_encoder` selects a 10-bit encoder where supported.
* Forward error correction can be enabled or disabled; the client repairs packet loss and reports FEC failures.
* The client can request an IDR frame; the server's IDR scheduler and `aggressive_keyframe_resend` setting control resend behavior.
* The client requests a realtime decoder priority by default.

### RF-300.2: Adaptive bitrate
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.1
* Adaptive bitrate is enabled by default with a 200 Mbit/s ceiling and a 12 ms latency target.
* The encoder bitrate moves up or down according to measured latency, a threshold, up/down rates, and a light-load threshold.
* An optional frametime-based mode uses a maximum latency and an offset instead of a fixed target.
* <!-- TBD: approved latency bounds and the measurement definition used for "latency" -->

### RF-300.3: Visual processing
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.1
* Fixed foveated rendering can be enabled with center size, center shift, and edge ratios per axis; it is disabled by default on Linux.
* Color correction can be enabled with brightness, contrast, saturation, gamma and sharpening values.
* On Linux, `linux_async_reprojection` is a patch flag that controls asynchronous reprojection; it is forced off on NVIDIA GPUs.

### RF-300.4: Game audio
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* The server captures the selected output device in stereo at its shared-mode sample rate and streams it to the client.
* The selected device can be default, matched by name, or matched by index.
* `mute_when_streaming` mutes the local device while the stream is active.
* Average buffering and batch duration are configurable.
* On Windows the driver also points OpenVR's default playback device at the captured device.

### RF-300.5: Microphone
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.4
* Microphone streaming is optional and off by default.
* The client's microphone is played into a configurable virtual microphone output on the PC at a configurable sample rate.
* Game audio and microphone must not resolve to the same device; the connection is rejected if they do.

### RF-300.6: Tracking and input
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* The client sends head, left hand and right hand poses with target timestamps, plus controller buttons, trigger, grip, trackpad and optional hand-tracking bones.
* On Oculus Go, the controller is 3DoF and reports trackpad, trigger, back and enter; no 6DoF controller pose or hand-tracking data is available.
* The server converts these into OpenVR poses, applies configured position/rotation offsets, and can predict poses on the server side.
* Linear and angular velocity cutoffs, a tracking frame offset, a pose time offset and a controller pose offset are configurable.
* Force-3DoF and tracking-reference-only modes are available for constrained setups.
* Playspace size updates from the client are applied to the chaperone.

### RF-300.7: Haptics
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.6
* OpenVR haptic events are forwarded to the client with path, duration, frequency and amplitude.
* Intensity, amplitude curve, minimum duration, and compensation for short vibrations are configurable.

### RF-300.8: Latency and statistics
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* The client reports send, transport and decode latency, packet loss, FEC failures, FPS and headset frame index.
* The dashboard monitor displays latency, framerate and packet statistics and plots them over time.

### RF-300.9: Client-reported state
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* The client reports battery level and charging state, and the server exposes it to OpenVR.
* The client reports its per-eye field of view and IPD; the server applies them to the emulated headset.
* The client applies the configured dark-mode preference to its UI.

## RF-400: Headset and Controller Emulation

### RF-400.1: Device identity
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* Headset serial number, tracking system, model, driver version, manufacturer, render model and registered device type are configurable and exposed to OpenVR.
* Controller identity strings, render models, input profile path, and device type are configurable, with presets for Oculus and Valve controllers.
* Upstream ships Rift S and Valve Index controller presets; no Oculus Go controller preset exists, so the Go controller identity is an open question for this fork.
* A controller emulation mode index selects the input mapping.

### RF-400.2: Extra tracked devices
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-400.1
* A Vive tracker proxy can be enabled to expose additional tracked devices.
* The headset's tracking system can optionally be reused for controllers.

### RF-400.3: Chaperone
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.6
* The driver can set a default 2 m by 2 m chaperone when SteamVR becomes ready.
* Client playspace dimensions smaller than 2 m are clamped to 2 m before being applied.

## RF-500: Settings and Dashboard

### RF-500.1: Schema-driven settings UI
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* The egui dashboard reads the settings schema generated by the settings derive macro in-process (no HTTP endpoint).
* The dashboard renders sections, choices, switches, optional values, arrays, vectors and dictionaries from that schema, including min/max, step and GUI hints.
* An "advanced" toggle hides fields marked advanced.

### RF-500.2: Persistence and migration
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-500.1
* Settings are stored in `session.json` and written when a settings lock is released.
* If the file cannot be deserialized, it is preserved as `session_old.json` and values are extrapolated by field name and type using the schema.
* Defaults fill any field that cannot be recovered; an extrapolation failure is logged as an event.
* A server version change updates `serverVersion` and clears the client list.
* <!-- TBD: retention policy for session_old.json and acceptable data loss on schema changes -->

### RF-500.3: First-run setup wizard
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-500.1
* <!-- TBD: the first-run wizard existed only in the web dashboard and is not ported to egui yet -->
* The wizard opens automatically when `setupWizard` is true.
* It covers hardware requirements and detected GPU, software requirements, firewall setup, tracking mode, a compatibility or visual-quality preset, preset import and completion.
* The compatibility preset sets 0.75 render scale, 15 Mbit/s, FFR on, H.264 and 72 Hz; the quality preset sets 1.0 scale, 40 Mbit/s, FFR off, HEVC and 90 Hz.

### RF-500.4: Presets
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-500.2
* <!-- TBD: preset import is not implemented in the egui dashboard yet; the web dashboard supported uploading a JSON session file -->
* The installation ships presets for Oculus Go and for "The Walking Dead: Saints and Sinners".

### RF-500.5: Localization and theme
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* <!-- TBD: the web dashboard had translations (English, Bulgarian, German, Spanish, French, Italian, Japanese, Korean, Dutch, Russian, Slovenian, Chinese); the egui dashboard is English only for now -->
* The locale can follow the system, and a stored locale overrides it on the next load.
* Themes are system default, classic and darkly; a separate client dark-mode flag affects the headset UI.

### RF-500.6: Confirmation dialogs
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-500.1
* Restart and settings-revert confirmation dialogs can each be disabled.

## RF-600: Operations and Lifecycle

### RF-600.1: Launcher preflight
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* The launcher runs as a single instance.
* It refuses installation paths containing non-ASCII characters.
* It requires SteamVR to have been launched at least once and starts SteamVR once automatically.
* It registers the driver if needed. If registration fails while SteamVR is running, it kills SteamVR and relaunches it.
* It closes once the driver accepts a TCP connection on `127.0.0.1:<web_server_port>` (default 8082), which it does once the dashboard is running.
* It offers "Reset drivers and retry", which deletes `openvrpaths.vrpath`, relaunches and kills SteamVR, clears the ALVR entry from `steamvr.vrsettings`, and restarts SteamVR.

### RF-600.2: Driver registration
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-600.1
* The driver registers and unregisters its path in SteamVR's `openvrpaths.vrpath`, preserving unrelated entries.
* Registration changes are backed up and restored across a SteamVR restart.

### RF-600.3: Firewall configuration
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* The dashboard can add or remove inbound rules for SteamVR's `vrserver.exe` on Windows, requesting elevation through UAC.
* On Linux the packaged script configures `ufw` or `firewalld` and escalates through `pkexec`.
* Firewall errors are shown in the dashboard (rule already set, request dismissed, or the command's exit code).

### RF-600.4: Restart and shutdown
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-600.2
* The dashboard's "Restart SteamVR" button shuts the driver down and restarts SteamVR.
* Closing the dashboard window shuts down the runtime and calls `ShutdownSteamvr`.
* The driver can restart itself after a settings change that affects `OpenvrConfig`.

### RF-600.5: Lifecycle scripts
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* A configured command runs on client connect and on disconnect, with `ACTION=connect` or `ACTION=disconnect` in the environment.
* Script failures are logged as warnings and do not abort the stream.

### RF-600.6: Logging and events
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* Logs and structured events are delivered live to the dashboard in-process; there is no network log endpoint.
* Log-to-disk can be enabled; the session log and a crash log are written to the configured log directory. The crash log always records error-level lines, even when the session log is off.
* The notification level setting exists in the session; the egui dashboard filters its log view by severity instead of raising notifications.
* The log dispatch level is debug in debug builds and info in release builds.

### RF-600.7: Updates
**Priority:** TBD | **Status:** Removed | **Dependencies:** none
* The update system was removed from this fork: no update check, channel setting, update prompt, installer download or launcher `--update` mode. New versions are installed manually.

## RF-700: Packaging and Platform Support

### RF-700.1: Windows distribution
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-600.2
* `publish-server` produces a portable ZIP and, when WiX is available, an MSI and a bootstrapper bundle that wraps the MSI.
* A portable layout keeps executables, presets, config and logs in one directory.

### RF-700.2: Linux distribution
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-600.2
* Linux builds follow an FHS layout by default, or a custom root passed with `--root`.
* `alvr_build_linux.sh` can produce DEB and RPM packages and a portable tarball.
* The Linux install includes the Vulkan capture layer, its manifest, and the `vrcompositor-wrapper`.
* The driver runs under SteamVR's 64-bit Linux runtime only (`x86_64`).

### RF-700.3: Android distribution
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.1
* The fork ships one APK, built from the `OculusGo` flavor as `alvr_client_oculus_go.apk`.
* The Quest and Quest 2 flavor remains buildable in the upstream source but is not part of a fork release.
* Stable and nightly package flavors use different application ID suffixes.
* The release workflow signs the APK with the project signing key before upload.

### RF-700.4: Version management
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* `bump-versions` updates all crate manifests, the Android version name, and the DEB/RPM metadata in one step.
* Stable releases increment the Android version code; nightly builds use a `+nightly.YYYYMMDD` suffix.

# 2. Non-Functional Requirements

## RF-800: Platform and Environment

### RF-800.1: Supported hosts
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* Server support targets Windows 8 and newer, with Windows 10 May 2020 or newer recommended, and 64-bit Linux.
* Headset support is limited to the Oculus Go (arm64, API 21+); Quest and Quest 2 are out of scope for this fork.
* A dedicated NVIDIA GPU with NVENC, or an AMD GPU with AMF VCE, is required for hardware encoding.
* Laptops with hybrid graphics must assign the high-performance GPU to ALVR and SteamVR.

### RF-800.2: Network
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* A 5 GHz 802.11ac wireless link for the headset and a wired PC connection are recommended.
* PC and headset must share a router or a routed connection.

### RF-800.3: Privacy
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* ALVR apps do not directly collect data, per the project README.
* The fork makes no network requests of its own to update servers.

### RF-800.4: Licensing
**Priority:** TBD | **Status:** Draft | **Dependencies:** none
* The project is MIT-licensed.
* Windows builds with `--gpl` and Linux builds with bundled FFmpeg include GPL components; the release ships third-party license notices.

## RF-900: Performance and Reliability

### RF-900.1: Latency
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-300.2
* The default configuration targets 12 ms of latency with adaptive bitrate enabled.
* <!-- TBD: a documented end-to-end latency budget and the measurement method to verify it -->

### RF-900.2: Recovery
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-200.6
* A failed connection never terminates the driver; it retries at least once per second.
* A client decoder error can request a new IDR without restarting the stream.
* Encoder or audio device failures end the session and return to discovery.

### RF-900.3: Diagnostics
**Priority:** TBD | **Status:** Draft | **Dependencies:** RF-600.6
* The dashboard must show live logs, latency, frame rate, packet loss, FEC failures and connection state.
* Crash logs must be written even when the session log is disabled.

# 3. Open Questions

- [ ] Which requirements are contractual for a release, and what priority scheme should replace the uniform TBD?
- [ ] Which Oculus Go firmware versions and VrApi builds must the fork support?
- [ ] Should the Quest and Quest 2 flavors be removed from the fork, or kept buildable but unreleased?
- [ ] Which controller identity should the Go controller emulate, given there is no Go preset?
- [ ] What render resolution, refresh rate and bitrate should a Go-first default use?
- [ ] Is authenticated or encrypted transport planned? This version has neither. (Dashboard access is no longer a network surface: the egui dashboard runs in the driver process.)
- [ ] Which SteamVR versions must the fork support?
- [ ] What latency and frame-rate targets define "working" on the Go?
- [ ] Should `session_old.json` and crash logs have a retention or redaction policy?
- [ ] Should `web_server_port` (now only the loopback readiness beacon port and the legacy port in `ClientConfigPacket`) be renamed, and should the dashboard URL field in `ClientConfigPacket` be dropped?
