# Security

Report a suspected vulnerability privately to the repository owner before sharing working exploit details. Do not include your pairing token, Minecraft credentials, or other people's data in an issue.

The renderer is local, sandboxed and isolated. Only explicit IPC operations are exposed. Executable selection requires a native local file dialog; remote requests contain a saved profile ID, never a command or a URL to download. Direct launcher execution uses argument arrays without a shell. URI activation only opens the setup window. It does not trust query parameters or execute commands.

The paired server can request any profile you explicitly saved. Use a server you trust and remove unused profiles. Pairing is revocable from either end. The companion does not attempt to bypass launcher login or manage Minecraft access tokens. OS-level account compromise is outside this boundary.

Dependency versions are pinned by `package-lock.json`. Production dependency audit is checked separately from installer/dev tooling. Keep Electron and WebSocket dependencies updated and run the automated tests before shipping a new build.
