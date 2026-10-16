# Kirk Companion 0.1.0 preview

First MIT-licensed desktop companion for launching existing Minecraft instances from the Kirk dashboard.

- Setup window matches the in-game Ghost and Kirk palettes, with sidebar navigation and compact controls.
- Direct Prism/MultiMC instance launching, portable data folders and instance discovery.
- Locally selected instance shortcuts for other launchers, plus an explicit Open launcher fallback.
- Account-scoped PC pairing, outbound WebSocket connection, expiring profile-only requests and revocation.
- System tray, optional start on login, and OS-encrypted pairing credentials where available.
- Source archive and SHA256 checksums included with the Windows installer.

The hosted Kirk dashboard/backend update is pending deployment. Until then, use local Test launch or a compatible local server. The full dashboard/backend/desktop flow has passed locally with a temporary database. No live Minecraft account login or actual game launch was used in automated testing.

Windows x64 installer and packaged app have been built and checked locally. The installer is unsigned. macOS/Linux builds and launcher-specific generated shortcuts need platform/integration verification. A successful dispatch means the launcher was requested; Minecraft authentication and Kirk readiness are separate states.
