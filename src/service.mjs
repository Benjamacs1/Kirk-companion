import { randomUUID } from "node:crypto";
import os from "node:os";
import { EventEmitter } from "node:events";
import WebSocket from "ws";
import { ID, publicProfiles, validateProfile } from "./profiles.mjs";

export const DEFAULT_BACKEND = "https://api.charlie-kirk.uk";
export function backendOrigin(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Enter a valid server URL.");
  }
  const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    (url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== "/"
  ) {
    throw new Error(
      "Use an HTTPS server origin, or HTTP on localhost for development.",
    );
  }
  return url.origin;
}

export class Companion extends EventEmitter {
  constructor({
    initial = {},
    save = async () => {},
    launch,
    fetchImpl = fetch,
    Socket = WebSocket,
    now = Date.now,
  } = {}) {
    super();
    this.save = save;
    this.launch = launch;
    this.fetch = fetchImpl;
    this.Socket = Socket;
    this.now = now;
    this.config = {
      machine: ID.test(initial.machine || "") ? initial.machine : randomUUID(),
      backend: backendOrigin(initial.backend || DEFAULT_BACKEND),
      name: String(initial.name || os.hostname()).slice(0, 64),
      token: typeof initial.token === "string" ? initial.token : "",
      profiles: [],
    };
    for (const p of (Array.isArray(initial.profiles)
      ? initial.profiles
      : []
    ).slice(0, 32)) {
      try {
        this.config.profiles.push(validateProfile(p, p.id));
      } catch {
        /* Invalid saved entries cannot become executable profiles. */
      }
    }
    this.status = "Not paired";
    this.lastLaunch = null;
    this.socket = null;
    this.timer = null;
    this.backoff = 2000;
    this.stopped = false;
    this.seen = new Set();
    this.busy = new Set();
    this.lastStarted = new Map();
    this.revision = 0;
    this.pairing = false;
  }
  snapshot() {
    return {
      ...this.config,
      token: undefined,
      paired: !!this.config.token,
      connected: this.socket?.readyState === 1,
      status: this.status,
      lastLaunch: this.lastLaunch,
    };
  }
  changed() {
    this.emit("state", this.snapshot());
  }
  async persist() {
    await this.save(structuredClone(this.config));
    this.publish();
    this.changed();
  }
  async addProfile(input) {
    if (this.config.profiles.length >= 32)
      throw new Error("You can save up to 32 launch profiles.");
    const p = validateProfile(input);
    this.config.profiles.push(p);
    await this.persist();
    return p;
  }
  async removeProfile(id) {
    this.config.profiles = this.config.profiles.filter((p) => p.id !== id);
    await this.persist();
  }
  async pair({ backend, code, name }) {
    if (this.pairing) throw new Error("Pairing is already in progress.");
    if (this.config.token)
      throw new Error("Unpair this computer before pairing it again.");
    const origin = backendOrigin(backend);
    const normalized =
      typeof code === "string" ? code.toUpperCase().replace(/[\s-]/g, "") : "";
    if (!/^[A-Z0-9]{8}$/.test(normalized))
      throw new Error("Enter the eight-character code from the website.");
    const deviceName = String(name || os.hostname())
      .trim()
      .slice(0, 64);
    if (!deviceName) throw new Error("Give this computer a name.");
    this.pairing = true;
    const revision = this.revision;
    try {
      const response = await this.fetch(origin + "/companion/claim", {
        method: "POST",
        headers: { "content-type": "application/json" },
        redirect: "error",
        signal: AbortSignal.timeout(15000),
        body: JSON.stringify({
          code: normalized,
          machine: this.config.machine,
          name: deviceName,
        }),
      });
      const body = await response.json().catch(() => ({}));
      if (
        !response.ok ||
        typeof body.token !== "string" ||
        !/^[A-Za-z0-9_-]{43}$/.test(body.token)
      ) {
        throw new Error(
          body.error === "invalid_or_expired_code"
            ? "That code expired or was already used. Get a new code on the website."
            : body.error === "too_many_computers"
              ? "Remove an old computer from the website before pairing another."
              : response.status === 404
                ? "This server needs the companion backend update."
                : "Pairing failed. Check the server and try again.",
        );
      }
      if (revision !== this.revision) return;
      this.config.backend = origin;
      this.config.name = deviceName;
      this.config.token = body.token;
      await this.persist();
      this.start();
    } catch (error) {
      if (error.name === "TimeoutError" || error instanceof TypeError)
        throw new Error(
          "Could not reach the server. Check your connection and server URL.",
        );
      throw error;
    } finally {
      this.pairing = false;
    }
  }
  async unpair() {
    this.revision++;
    this.stop();
    const { backend, token } = this.config;
    this.config.token = "";
    this.status = "Not paired";
    await this.persist();
    if (!token) return;
    try {
      const res = await this.fetch(backend + "/companion/revoke", {
        method: "POST",
        headers: { authorization: "Bearer " + token },
        redirect: "error",
        signal: AbortSignal.timeout(5000),
      });
      if (!res.ok && res.status !== 401) throw new Error("unreachable");
    } catch {
      throw new Error(
        "Removed from this app. The server could not be reached; remove this computer on the website too.",
      );
    }
  }
  start() {
    this.stopped = false;
    this.connect();
  }
  stop() {
    this.stopped = true;
    clearTimeout(this.timer);
    this.timer = null;
    const ws = this.socket;
    this.socket = null;
    if (ws) ws.close();
  }
  connect() {
    if (this.stopped || !this.config.token || this.socket) return;
    this.status = "Connecting…";
    this.changed();
    const url = this.config.backend.replace(/^http/, "ws") + "/companion/ws";
    const ws = new this.Socket(url, {
      headers: { authorization: "Bearer " + this.config.token },
      handshakeTimeout: 10000,
      maxPayload: 16384,
    });
    this.socket = ws;
    const rejectToken = () => {
      if (this.socket !== ws) return;
      this.config.token = "";
      this.status = "Pairing was removed. Pair this computer again.";
      this.persist().catch(() => {
        this.status = "Could not update saved pairing.";
        this.changed();
      });
    };
    ws.on("open", () => {
      if (this.socket !== ws) {
        ws.close();
        return;
      }
      this.backoff = 2000;
      this.status = "Ready for the website";
      this.publish();
      this.changed();
    });
    let heartbeat;
    const healthy = () => {
      clearTimeout(heartbeat);
      heartbeat = setTimeout(() => ws.terminate(), 75000);
      heartbeat.unref?.();
    };
    ws.on("open", healthy);
    ws.on("ping", healthy);
    ws.on("unexpected-response", (_req, res) => {
      if (res.statusCode === 401) rejectToken();
      res.resume();
      ws.terminate();
    });
    ws.on("message", (raw) => {
      if (this.socket === ws) void this.receive(raw, ws);
    });
    ws.on("error", () => {
      /* close schedules a bounded reconnect, with no tokens in logs. */
    });
    ws.on("close", (code) => {
      clearTimeout(heartbeat);
      if (this.socket !== ws) return;
      if (code === 4001) rejectToken();
      this.socket = null;
      if (this.stopped) return;
      if (this.config.token) {
        this.status = "Server unavailable. Reconnecting…";
        this.timer = setTimeout(() => this.connect(), this.backoff);
        this.backoff = Math.min(60000, this.backoff * 2);
      }
      this.changed();
    });
  }
  publish() {
    if (this.socket?.readyState === 1)
      this.socket.send(
        JSON.stringify({
          type: "hello",
          version: 1,
          profiles: publicProfiles(this.config.profiles),
        }),
      );
  }
  async receive(raw, ws) {
    let request;
    try {
      request = JSON.parse(String(raw));
    } catch {
      return;
    }
    if (
      !request ||
      typeof request.id !== "string" ||
      !/^[A-Za-z0-9_-]{16,64}$/.test(request.id)
    )
      return;
    let result;
    try {
      if (
        request.op !== "launch_game" ||
        !ID.test(request.profile || "") ||
        Object.keys(request).some(
          (key) => !["id", "op", "profile", "expires_at"].includes(key),
        )
      )
        throw new Error("Invalid launch request.");
      if (
        !Number.isSafeInteger(request.expires_at) ||
        request.expires_at <= this.now() ||
        request.expires_at > this.now() + 30000
      )
        throw new Error("Launch request expired. Try again.");
      if (this.seen.has(request.id))
        throw new Error("This request was already handled.");
      this.seen.add(request.id);
      if (this.seen.size > 512)
        this.seen.delete(this.seen.values().next().value);
      result = { ok: true, data: await this.runProfile(request.profile) };
    } catch (error) {
      result = { ok: false, error: error.message };
    }
    if (this.socket === ws && ws.readyState === 1)
      ws.send(JSON.stringify({ id: request.id, ...result }));
  }
  async runProfile(id) {
    const profile = this.config.profiles.find((p) => p.id === id);
    if (!profile)
      throw new Error("This launch profile was removed. Refresh the website.");
    if (
      this.busy.has(id) ||
      this.now() - (this.lastStarted.get(id) ?? -Infinity) < 30000
    )
      throw new Error(
        "A launch was just requested. Wait 30 seconds before trying again.",
      );
    this.busy.add(id);
    try {
      const result = await this.launch(profile);
      this.lastStarted.set(id, this.now());
      this.lastLaunch = {
        name: profile.name,
        at: this.now(),
        status: "dispatched",
      };
      this.changed();
      return result;
    } finally {
      this.busy.delete(id);
    }
  }
}
