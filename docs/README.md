# ALVR documentation

Audited source tree: ALVR 18.2.3 (all workspace crates and the Android client report 18.2.3). The tree is the base for a fork aimed only at the Oculus Go; the fork direction is recorded in the docs. No Go-specific behavior changes exist yet, though the build toolchain was upgraded on 2026-10-24.

Before this audit the repository had no project-level documentation. The only Markdown files were the top-level README and short per-crate READMEs under `alvr/`. Those files remain the source of truth for crate-level notes; they are current but very thin.

| Doc | Status | Derived from |
|---|---|---|
| [project.md](./project.md) | Current; upstream 18.2.3 with an upgraded build toolchain | Workspace manifests, packaging, xtask, filesystem layout, pinned tool versions. |
| [architecture.md](./architecture.md) | Current, upstream 18.2.3 | Rust and C++ sources on both sides, protocol definitions, dashboard, session model. Go scope noted at the top. |
| [srs.md](./srs.md) | Draft, needs approval | Observable behavior and defaults in the code, plus the README for platform and network requirements. Go scope added. |
| [product/vision.md](./product/vision.md) | Direction set, needs ratification | Fork owner's Go-only goal, plus upstream README, platform-specific code, settings surface and license choices. |

## Known gaps

- No Go-specific patches: the tree is upstream 18.2.3 with no Go-specific behavior changes applied yet. The build toolchain was upgraded on 2026-10-24, and CI workflows were removed; builds are local.
- No `docs/product/discovery.md`: there is no market or competitive research in the tree, and this audit cannot invent it.
- The SRS has uniformly TBD priorities. No backlog, roadmap or issue export is present in this copy.
- The copy has no `.git` directory, so release history, ownership and the rationale behind past decisions are not derivable. The 2022-05-11 release date comes from GitHub release metadata, not from this copy.
- Server and client behavior is documented from static reading. No end-to-end run was performed against SteamVR or an Oculus Go; the Go debug and release APKs build, but were not installed on a headset.

## Suggested pipeline

1. **brainstorming** (product scope): ratify `docs/product/vision.md`, especially the Go-only audience, controller mapping and upstream merge policy.
2. **product-discovery** if external positioning is needed: produce `docs/product/discovery.md` before committing to a direction.
3. **srs**: review `docs/srs.md`, replace TBD priorities, and resolve the Go-specific open questions.
4. **architecture-design**: confirm or revise `docs/architecture.md` for the client changes the SRS implies.
5. **implementation-plan**: turn approved requirements into tracked work items.

Until the SRS is approved, treat it as a description of the current system rather than a specification of the intended one.
