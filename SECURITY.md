# Security

Report vulnerabilities privately to the repository owner. Never include pairing tokens, Minecraft credentials, private screenshots, or other people's data in issues.

The native UI loads no remote scripts. Remote launches contain only a saved profile UUID, a unique request ID, and short expiry. They never supply executable paths, commands, downloads, or arbitrary URLs. Direct launch uses argument arrays without a shell. URI activation only opens the app.

Pairing authorizes the owner's statistics, game status/screenshots, assistant conversation, and allowlisted macro/warp controls. It also lets the trusted server request profiles explicitly saved on this PC. Every API checks ownership. Companion credentials must never grant staff access or authenticate directly to game/license endpoints.

Credentials use the OS credential store, or memory if unavailable. Settings have no tokens. Unpair/revocation clears account data and cancels pending API work. Screenshots and conversations are never saved by normal app use; developer screenshot export is limited to explicit sample-data preview mode.

Use a trusted server and remove unused profiles. A compromised OS account or malicious locally selected executable is outside this boundary. Cargo.lock pins dependencies. Run fmt, Clippy, tests, and the backend/native integration harness before release.
