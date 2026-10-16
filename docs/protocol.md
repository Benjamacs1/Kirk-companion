# Companion protocol v1 and desktop dashboard API

Choose a server origin locally in Connection. HTTPS is required, except literal loopback HTTP for development. Credential-bearing HTTP requests refuse redirects. Never place tokens in URLs.

The matching private Kirk-backend changes are migration `027_companion.sql`, companion/download routes, web session routes, and deployment docs. They still need production deployment. No mod changes are required.

## Pairing and credential scope

The signed-in website issues `{code,expires_at}` with POST `/web/api/computers/pair-code`. Codes are single-use, account-specific, rate-limited, and valid ten minutes. POST `/companion/claim` with `{code,machine,name}` exchanges the code for `{token,computer}`. Machine is a persistent UUID; name is 1–64 characters. Token is 32 random bytes encoded as 43 base64url characters. Store only token/code hashes on the server.

The credential authorizes only its owner's desktop APIs: own dashboard, selected own game controls, this PC's launch socket, and revocation. It never authorizes staff routes, license administration, Discord tokens, or game credential export.

## Desktop account API

All routes use `Authorization: Bearer <token>`, check revocation, return `Cache-Control: no-store`, and must enforce server-side owner scope.

- GET `/companion/dashboard?days=7&license=1`: days must be 1/7/30, optional license must belong to this account (otherwise use combined). Returns `{account:{name},device:{paired,online,clients:[{id,name,online,since,last_seen_at}]},computers,stats,month,trackers}`. Stats includes `{licensed,license,licenses,days,totals,today,macros,series,skills,items}`. Totals/day/macro figures use seconds, coins, items_value, xp. Series rows add UTC day; macro rows add macro. Skills are `{skill,xp}`. Trackers includes totals powder/counters/profit/uptime_ms, trackers, items, all_time and updated_at. Month is the 30-day stats series. No account tokens or executable paths may be included.
- POST `/companion/command` with `{op,args,client?}`. Client is a numeric game device ID, checked among the credential owner's games. Returns `{ok,data,error}`. Allowlist: status/screenshot (empty args), start/stop `{macro}`, lobby `{place}`, ask `{text}` up to 500 chars, conversation `{do?:"forget"}`. Macro IDs: mithril, commission, fishing, foraging, worm, revenant, tarantula, sven, hunting; stop also accepts all. No arbitrary operations or extra arguments. Status includes player, in_world, area, zone, purse, macros, recent chat, and optional profit. Screenshot data is `{mime:"image/png",base64,width,height}`. Conversation data is `{lines:[{kirk:boolean,text}],busy,enabled}`. Ask returns `{reply}`. A successful HTTP response may still have ok=false; surface that error.
- POST `/companion/trackers/refresh`: optional rate-limited request to synchronize the owner's connected clients in the background.

The native app polls about every 15 seconds with one refresh in flight. API work runs independently of the launch socket; a long assistant response must not delay launch dispatch. Bound responses (8 MiB maximum) and screenshot decoding. Unpair clears account data and cancels pending work; ignore stale completions after revocation or client switches.

## Launch socket

Connect `/companion/ws` with the Bearer header. On connect and profile change send `{type:"hello",version:1,profiles:[{id,name,launcher,capability}]}`. Up to 32 profiles; IDs are UUIDs, names at most 80 chars. Launcher: prism/multimc/shortcut/open. Capability: launch/open. Never transmit filesystem paths. Use ping/pong, 16 KiB message/frame limits, bounded reconnect backoff. Close 4001 revokes pairing; other disconnects reconnect without queued launches.

Website POST `/web/api/computers/launch` with `{computer,profile}` checks the signed-in owner, online readiness, and advertised profile. Send `{id,op:"launch_game",profile,expires_at}`: only these four fields. ID is 16–64 base64url chars, profile is UUID, expiry Unix ms normally ten seconds ahead (reject expired or more than 30 sec ahead). Reject replays, unknown profiles, and recent duplicate dispatches. Clock synchronization matters. Reply `{id,ok:true,data:{status:"dispatched"}}` or `{id,ok:false,error}`. Server timeout ten seconds. Dispatch does not guarantee a loaded game. Macro start is a separate operation.

## Revocation and distribution

POST `/companion/revoke` deletes this credential and closes 4001. Website GET `/web/api/computers` lists own PCs; POST `/web/api/computers/remove` with `{computer}` revokes one. All mutations require appropriate session/Origin checks or the desktop Bearer token.

Matching installer/source files under COMPANION_RELEASE_DIR are served publicly by `/web/companion/download`, `/web/companion/source`, and advertised by `/web/api/companion-release`. Source must match the binary version.

Local integration: build the sibling app's examples, then run `node --import tsx tools/companion-rust-e2e.ts` in Kirk-backend. It uses a temporary database, a fake game socket, and a recording launcher to verify native pairing, statistics, controls, screenshot/chat, concurrent website launch, cooldown, and revocation.
