# Dispatch: Milestone M6 — Canonical Windows Release Pipeline & Packaging

## Target Worker
- Archetype: teamwork_preview_worker
- Role: Release & Packaging Worker

## Mission Scope
1. Verify and align workspace release build configuration for `crates/desktop_ui` (`OSMOO.exe`), `apps/daemon` (`voxy-daemon.exe`), and `apps/overlay` (`voxy-overlay.exe`).
2. Verify NSIS installer script in `installer/nsis/osmoo.nsi`:
   - Validates installation path, desktop shortcuts, start menu entries, and registry uninstall keys.
   - Packages `installer/OSMOO/OSMOO-Setup-x64.exe` or validates existing artifact.
   - Computes and documents SHA-256 integrity checksum.
3. Test silent install (`/S`) and uninstallation flows without orphaned files or registry residue.
4. Verify acceptance criteria A5 (packaging component).
5. Document build and verification steps in `handoff.md`.
