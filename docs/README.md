# ALVR-Go documentation

ALVR-Go is a fork of ALVR 18.2.3 aimed only at the Oculus Go (all workspace crates and the Android client still report 18.2.3). It started as a source audit of upstream; the docs were then updated as the fork changed. The fork so far consists of: a modernized build toolchain and dependency graph; all desktop UI (launcher and dashboard) on egui, with the web dashboard, web server, `druid`, `gtk`, `alcro` and `webbrowser` removed; the update system and CI removed; Oculus Go defaults and a trimmed settings schema; a Go controller mapped to an Oculus Touch layout; and stream robustness fixes (adaptive bitrate, AMD AMF, sockets, audio capture, pause/resume on the headset). The Go debug and release APKs build with Gradle and the fork has been run on a Go with a Windows/AMD server; there is no automated end-to-end test.

The top-level README is the user-facing entry point. The short per-crate READMEs under `alvr/` remain the source of truth for crate-level notes; they are inherited from upstream and thin.

| Doc | Status | Derived from |
|---|---|---|
| [project.md](./project.md) | Current; ALVR-Go with an upgraded build toolchain | Workspace manifests, packaging, xtask, filesystem layout, pinned tool versions. |
| [architecture.md](./architecture.md) | Current | Rust and C++ sources on both sides, protocol definitions, dashboard, session model. Go scope noted at the top. |
| [srs.md](./srs.md) | Draft, needs approval | Observable behavior and defaults in the code, plus the README for platform and network requirements. Go scope added. |
| [product/vision.md](./product/vision.md) | Direction set, needs ratification | Fork owner's Go-only goal, plus upstream README, platform-specific code, settings surface and license choices. |

## Known gaps

- No `docs/product/discovery.md`: there is no market or competitive research in the tree.
- The SRS has uniformly TBD priorities. No backlog or roadmap exists; use the repository issues for tracking.
- No CI: workflows were removed, so builds, tests and packaging are local and `cargo xtask publish-*` does not upload anything. No pre-built releases are published yet.
- Only `alvr_session` has automated tests. Server and client behavior beyond that was verified by running the fork on a Go with a Windows/AMD server; NVIDIA, Intel and Linux servers were not tested in this fork.
- The Go controller mapping has had limited testing on a real controller.

## Suggested pipeline

1. **brainstorming** (product scope): ratify `docs/product/vision.md`, especially the Go-only audience, controller mapping and upstream merge policy.
2. **product-discovery** if external positioning is needed: produce `docs/product/discovery.md` before committing to a direction.
3. **srs**: review `docs/srs.md`, replace TBD priorities, and resolve the Go-specific open questions.
4. **architecture-design**: confirm or revise `docs/architecture.md` for the client changes the SRS implies.
5. **implementation-plan**: turn approved requirements into tracked work items.

Until the SRS is approved, treat it as a description of the current system rather than a specification of the intended one.
