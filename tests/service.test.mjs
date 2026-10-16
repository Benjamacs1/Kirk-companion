import test from "node:test";
import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";
import { WebSocketServer } from "ws";
import { Companion, backendOrigin } from "../src/service.mjs";

async function until(fn) {
  const deadline = Date.now() + 3000;
  while (!fn()) {
    if (Date.now() > deadline) throw new Error("State did not arrive.");
    await new Promise((r) => setTimeout(r, 10));
  }
}
test("server configuration requires HTTPS, with explicit loopback development support", () => {
  assert.equal(backendOrigin("https://example.com/"), "https://example.com");
  assert.equal(backendOrigin("http://127.0.0.1:1234"), "http://127.0.0.1:1234");
  for (const url of [
    "http://example.com",
    "file:///tmp/x",
    "https://user:pass@example.com",
    "https://example.com/path",
    "https://example.com?token=a",
  ])
    assert.throws(() => backendOrigin(url));
});
test("pair, publish profiles, dispatch exactly once, reject unsafe requests and revoke", async () => {
  const token = "A".repeat(43);
  let claim,
    revoked = false,
    peer,
    hello,
    launches = 0;
  const server = createServer(async (req, res) => {
    if (req.url === "/companion/claim") {
      let body = "";
      for await (const chunk of req) body += chunk;
      claim = JSON.parse(body);
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify({ token }));
    } else if (req.url === "/companion/revoke") {
      revoked = req.headers.authorization === "Bearer " + token;
      res.end("{}");
    } else {
      res.statusCode = 404;
      res.end("{}");
    }
  });
  const sockets = new WebSocketServer({ server });
  sockets.on("connection", (ws, req) => {
    assert.equal(req.headers.authorization, "Bearer " + token);
    peer = ws;
    ws.on("message", (raw) => {
      const msg = JSON.parse(String(raw));
      if (msg.type === "hello") hello = msg;
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const backend = `http://127.0.0.1:${server.address().port}`;
  const saved = [];
  const service = new Companion({
    save: async (data) => saved.push(data),
    launch: async (p) => {
      launches++;
      return { status: "dispatched", profile: p.id };
    },
  });
  try {
    const p = await service.addProfile({
      name: "Main",
      target: process.execPath,
      kind: "prism",
      instance: "26.2 Kirk",
    });
    await service.pair({ code: "ACDE-FHJK", name: "Gaming PC", backend });
    await until(() => hello);
    assert.equal(claim.code, "ACDEFHJK");
    assert.equal(claim.name, "Gaming PC");
    assert.equal(service.snapshot().token, undefined);
    assert.equal(service.snapshot().connected, true);
    assert.equal(saved.at(-1).token, token);
    assert.deepEqual(Object.keys(hello.profiles[0]).sort(), [
      "capability",
      "id",
      "launcher",
      "name",
    ]);
    const send = async (overrides = {}) => {
      const waiting = new Promise((resolve) => {
        const listener = (raw) => {
          const msg = JSON.parse(String(raw));
          if (msg.id) {
            peer.off("message", listener);
            resolve(msg);
          }
        };
        peer.on("message", listener);
      });
      peer.send(
        JSON.stringify({
          id: "request_1234567890123456",
          op: "launch_game",
          profile: p.id,
          expires_at: Date.now() + 5000,
          ...overrides,
        }),
      );
      return waiting;
    };
    assert.equal(
      (await send({ target: "evil.exe", id: "invalid_1234567890123456" })).ok,
      false,
    );
    assert.equal((await send({ expires_at: Date.now() - 1 })).ok, false);
    assert.equal((await send({ op: "shell" })).ok, false);
    assert.equal(
      (await send({ profile: randomUUID(), id: "unknown_1234567890123456" }))
        .ok,
      false,
    );
    assert.equal((await send()).ok, true);
    assert.equal(launches, 1);
    assert.match((await send()).error, /already handled/);
    assert.match(
      (await send({ id: "another_1234567890123456" })).error,
      /30 seconds/,
    );
    assert.equal(launches, 1);
    await service.removeProfile(p.id);
    await until(() => hello.profiles.length === 0);
    await service.unpair();
    assert.equal(revoked, true);
    assert.equal(service.snapshot().paired, false);
  } finally {
    service.stop();
    for (const ws of sockets.clients) ws.terminate();
    await new Promise((r) => sockets.close(r));
    await new Promise((r) => server.close(r));
  }
});
test("revoked pairing clears its token and does not reconnect forever", async () => {
  const server = createServer();
  const sockets = new WebSocketServer({ server });
  sockets.on("connection", (ws) => ws.close(4001, "unpaired"));
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const service = new Companion({
    initial: {
      token: "A".repeat(43),
      backend: `http://127.0.0.1:${server.address().port}`,
    },
    launch: async () => {},
  });
  try {
    service.start();
    await until(() => !service.config.token);
    assert.equal(service.snapshot().paired, false);
    assert.equal(service.timer, null);
  } finally {
    service.stop();
    for (const ws of sockets.clients) ws.terminate();
    await new Promise((r) => sockets.close(r));
    await new Promise((r) => server.close(r));
  }
});
test("failed launches can be retried and concurrent requests do not spawn twice", async () => {
  let release,
    launches = 0;
  const service = new Companion({
    launch: async () => {
      launches++;
      await new Promise((r) => {
        release = r;
      });
      throw new Error("Login needed");
    },
  });
  const profile = await service.addProfile({
    name: "Main",
    kind: "open",
    target: process.execPath,
  });
  const first = assert.rejects(service.runProfile(profile.id), /Login needed/);
  await assert.rejects(service.runProfile(profile.id), /30 seconds/);
  release();
  await first;
  assert.equal(launches, 1);
  assert.equal(service.busy.size, 0);
});
