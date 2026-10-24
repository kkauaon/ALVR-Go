# Vision: ALVR for Oculus Go (fork of 18.2.3)

> Status: direction set by the fork owner on 2026-10-24. The code base is upstream ALVR 18.2.3; no Go-specific patches are present yet. This document states where the fork is going, not what the tree currently does.

## Purpose

Keep the Oculus Go usable as a wireless PC VR headset. The PC renders in SteamVR, ALVR encodes and streams the result over the local network, and the Go decodes and displays it. The fork exists because upstream ALVR supports Quest, Quest 2 and Go at once, and the Go is the device that needs the most care: it is 3DoF, it has a single 3DoF controller, and it is fixed hardware that no longer receives platform updates.

The fork keeps upstream's architecture and protocol. The PC side stays an OpenVR driver plus dashboard; the headset side stays an Android app built from the `OculusGo` product flavor. Patches are expected to concentrate on the client, the Go defaults, and the packaging path, not on a rewrite.

## Users

- **Oculus Go owners who still want wireless PC VR.** This is the primary and only supported audience. The Go is discontinued and its store is frozen, so these users depend on sideloaded APKs and community maintenance.
- **People running modest PCs.** The Go's resolution and refresh rate are lower than Quest headsets, so it is a plausible target for hardware that would struggle with a Quest 2.
- **Linux users with a Go.** The server's Linux path is a substantial part of upstream and is expected to keep working, though Go-specific testing on Linux is a fork responsibility.
- **Maintainers of the Go path.** The fork should be small enough for one or two people to keep building against a fixed device.

Quest and Quest 2 users are explicitly not the audience. Their code paths may remain in the tree, but no fork release is promised for them.

## Principles

- **Go is the reference device.** A patch is accepted only if it is validated on a Go, or if it is server-side and cannot change Go behavior.
- **Stay close to upstream 18.2.3.** Keep the wire protocol, session schema and driver interfaces compatible so an upstream 18.2.3 server can talk to the fork client and vice versa. Divergence should be justified by a Go limitation, not by taste.
- **Patch narrowly.** Prefer Go-specific defaults and small client fixes over refactors. Every changed line is a line that has to be re-checked against upstream.
- **Do not pretend the Go is a Quest.** Do not enable hand tracking, 6DoF controller poses, or refresh rates the device does not report. Surface the device's real capabilities instead.
- **Keep it open and permissively licensed.** MIT for the project; GPL components remain opt-in build features with notices.
- **Do not collect data.** Upstream states that ALVR apps do not directly collect data. The GitHub release check remains the only outbound request in the UI.
- **Degrade rather than fail.** Software encoding, UDP/TCP stream choices, optional FEC and adaptive bitrate matter more on a Go-class link than on a Quest 2.

## Tensions visible in the code and the fork

- **Fixed hardware versus upstream drift.** The Go will not gain new firmware or SDK features. Upstream will keep moving toward newer headsets. Each upstream merge risks pulling in assumptions the Go cannot satisfy, so the fork needs an explicit merge policy.
- **Go controller mapping versus emulation.** Upstream ships controller presets for Oculus Rift S and Valve Index, and the client has a separate "GearVR or Oculus Go Controller" input path. The fork has to decide whether the Go controller is exposed as a 3DoF controller, emulated as a Rift controller, or left to the user.
- **Small screen, deep settings.** The dashboard exposes dozens of advanced controls. On a Go, the useful subset is smaller, but removing settings would fork the schema and break session compatibility.
- **Quest code in a Go fork.** Leaving Quest paths in place keeps merges cheap but leaves untested code in the release. Removing them shrinks the surface but makes every upstream merge harder.
- **Openness versus safety.** The dashboard is unauthenticated and reachable on the LAN, and the update endpoint downloads a caller-supplied URL. A Go-focused fork inherits that posture unless it deliberately changes it.

## Open questions

- [ ] Which Oculus Go firmware versions must the fork support, and which VrApi build is the floor?
- [ ] Should the Go controller be exposed as 3DoF, emulated as a Rift S controller, or configurable?
- [ ] Should Quest and Quest 2 flavors be removed from the fork, or kept building but unreleased?
- [ ] What are the default render resolution, refresh rate and bitrate for a Go-first setup?
- [ ] Is the fork Windows-first, or must Linux server parity be maintained release by release?
- [ ] How are upstream 18.2.3 fixes merged after the fork diverges: cherry-pick, periodic rebase, or one-way snapshot?
- [ ] Does the fork keep the ALVR name and update channel, or rebrand to avoid pulling upstream installers?
- [ ] Should the dashboard's unauthenticated LAN exposure and arbitrary-URL update be fixed in this fork?
