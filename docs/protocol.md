# Companion protocol v1

The companion can connect to a compatible self-hosted server: expand **Use a different Kirk server** before pairing. HTTPS is required except HTTP on literal loopback hosts for development. The server origin is chosen locally and the app refuses redirects for credential-bearing requests.

The matching Kirk backend changes are in the separate Kirk-backend project: migration `027_companion.sql`, `src/routes/companion.ts`, `src/routes/companionDownload.ts`, and the web/session routes. They need deployment before the public website can pair companions. They do not require changes to the Minecraft mod.

For the full local integration check, install dependencies in both sibling projects, then run `node --import tsx tools/companion-e2e.ts` from Kirk-backend. It drives the real dashboard and Electron setup window against a temporary database, verifies launch routing and revocation, and writes desktop/phone screenshots to the companion's ignored `artifacts/` folder.

## Pairing

The signed-in dashboard obtains `{ code, expires_at }` via `POST /web/api/computers/pair-code`. Codes last ten minutes, are single-use, rate-limited and specific to desktop pairing. The desktop POSTs JSON `{ code, machine, name }` to `/companion/claim`. `machine` is a persistent locally generated UUID, `name` is at most 64 characters. The response contains a random 32-byte base64url `token` and numeric `computer`. Servers should store only token/code hashes and limit linked computers per account. A companion token must never authorize game or license endpoints.

## Connection

Connect to `/companion/ws` using `Authorization: Bearer <token>`. The desktop sends this after connecting and whenever saved profiles change:

```json
{
  "type": "hello",
  "version": 1,
  "profiles": [
    {
      "id": "a0ebd95a-3351-4da5-ad55-6240bc0b5b29",
      "name": "Main",
      "launcher": "prism",
      "capability": "launch"
    }
  ]
}
```

Up to 32 profiles; IDs are UUIDs, names at most 80 characters. Launcher kinds: `prism`, `multimc`, `shortcut`, `open`. Capabilities: `launch` or `open`. Do not transmit filesystem paths or account credentials. Use WebSocket ping/pong heartbeats and bounded reconnect backoff. Close code `4001` means the pairing was revoked; the desktop discards its token. Other disconnects reconnect without queueing launches.

## Launching

The authenticated website POSTs `{ computer, profile }` to `/web/api/computers/launch`. Check session origin and account ownership, then require that computer to be online with that profile advertised. Send:

```json
{
  "id": "random_unique_request_123",
  "op": "launch_game",
  "profile": "a0ebd95a-3351-4da5-ad55-6240bc0b5b29",
  "expires_at": 1792112400000
}
```

`id` contains 16–64 base64url characters. `expires_at` is Unix milliseconds, normally ten seconds ahead; the desktop rejects expired requests or expiry over 30 seconds ahead. The example timestamp is illustrative. The only permitted keys are the four shown. The desktop rejects replayed IDs, unknown local profiles, other operations, and concurrent/recent launches of the same profile. It replies `{ id, ok: true, data: { status: "dispatched", ... } }` or `{ id, ok: false, error: "Human-readable message" }`. The server times out after ten seconds, never queues for offline PCs, and records only safe audit metadata. Clock synchronization is needed on both sides.

## Revocation and state

`POST /companion/revoke` with the companion Bearer token deletes that device and closes its socket with `4001`. The website can list its own PCs with `GET /web/api/computers` and remove one with `POST /web/api/computers/remove` and `{ computer }`. The pairing channel is separate from the existing in-game remote channel; launching is `launch_game`, never macro `start`.

## Distribution

Deploy matching installer/source files under `COMPANION_RELEASE_DIR` on the backend (defaults to sibling `../Kirk-companion/dist`). Public routes `/web/companion/download` and `/web/companion/source` serve fixed allowlisted filenames. `/web/api/companion-release` advertises availability. Build and provide a source ZIP for the same version as the installer. These downloads do not require a Kirk license.
