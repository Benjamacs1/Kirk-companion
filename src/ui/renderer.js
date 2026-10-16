const $ = (id) => document.getElementById(id);
let detected = [],
  busy = false,
  profileSignature = "";
const labels = {
  prism: "Prism Launcher",
  multimc: "MultiMC",
  shortcut: "Instance shortcut",
  open: "Open launcher",
};
function notice(text, success = false) {
  $("notice").textContent = text;
  $("notice").hidden = !text;
  $("notice").classList.toggle("success", success);
}
async function act(fn, message) {
  if (busy) return;
  busy = true;
  document.querySelectorAll("button").forEach((b) => {
    b.disabled = true;
  });
  try {
    const data = await fn();
    if (message) notice(message, true);
    return data;
  } catch (error) {
    notice(error.message);
  } finally {
    busy = false;
    document.querySelectorAll("button").forEach((b) => {
      b.disabled = false;
    });
  }
}
function render(state) {
  document.documentElement.dataset.platform = state.platform;
  $("status").textContent = state.status;
  $("status").classList.toggle("connected", !!state.connected);
  $("sidebar-dot").classList.toggle("on", !!state.connected);
  $("sidebar-status").textContent = state.connected
    ? "Connected"
    : state.paired
      ? "Connecting…"
      : "Not paired";
  $("pair-form").hidden = state.paired;
  $("paired").hidden = !state.paired;
  $("paired-name").textContent = state.name;
  $("paired-server").textContent = state.backend;
  // Polling/reconnect events must not replace what the user is typing.
  if (!$("device-name").value) $("device-name").value = state.name;
  if (!$("backend").value) $("backend").value = state.backend;
  $("startup").checked = state.startup;
  $("startup").disabled = !state.startupSupported;
  if (!state.startupSupported)
    $("startup-help").textContent =
      "Start on login is available in the installed Windows and macOS apps. Close this window to keep the companion in the tray.";
  $("storage").textContent = state.storageWarning;
  $("storage").hidden = !state.storageWarning;
  $("version").textContent =
    "Kirk Companion " + state.version + " · MIT license";
  const signature = JSON.stringify(state.profiles);
  if (signature === profileSignature) return;
  profileSignature = signature;
  const list = $("profiles");
  list.replaceChildren();
  if (!state.profiles.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    const title = document.createElement("strong");
    title.textContent = "Give Kirk a place to start.";
    empty.append(
      title,
      "Add your existing instance, or choose a shortcut from your launcher.",
    );
    list.append(empty);
  }
  for (const p of state.profiles) {
    const row = document.createElement("div");
    row.className = "profile";
    const icon = document.createElement("span");
    icon.className = "profile-icon";
    icon.textContent = "↗";
    icon.setAttribute("aria-hidden", "true");
    const text = document.createElement("div");
    text.className = "description";
    const name = document.createElement("strong");
    name.textContent = p.name;
    const detail = document.createElement("p");
    detail.textContent =
      labels[p.kind] +
      (p.instance ? " · " + p.instance : "") +
      (p.kind === "open" ? " · Press Play in your launcher" : "");
    text.append(name, detail);
    const test = document.createElement("button");
    test.className = "secondary";
    test.textContent = "Test launch";
    test.disabled = busy;
    test.setAttribute("aria-label", "Test launch " + p.name);
    test.onclick = () =>
      act(
        () => window.kirk.test(p.id),
        p.kind === "open"
          ? "Launcher opened. Choose your instance and press Play."
          : "Launch sent. Your launcher may ask you to sign in.",
      );
    const remove = document.createElement("button");
    remove.className = "text-button danger";
    remove.textContent = "Remove";
    remove.disabled = busy;
    remove.setAttribute("aria-label", "Remove " + p.name);
    remove.onclick = () =>
      act(() => window.kirk.remove(p.id), "Profile removed.");
    row.append(icon, text, test, remove);
    list.append(row);
  }
}
function method() {
  const direct = ["prism", "multimc"].includes($("kind").value);
  $("instance-fields").hidden = !direct;
  $("instance").required = direct;
  $("method-help").textContent = direct
    ? "Choose the launcher executable and the instance folder name. Portable installs can specify their data folder."
    : $("kind").value === "shortcut"
      ? "Use an instance shortcut created by your launcher, including Modrinth or CurseForge if supported. Test it before using the website."
      : "Opens your launcher. You will still need to choose the instance and press Play.";
}
$("pair-form").onsubmit = (event) => {
  event.preventDefault();
  act(async () => {
    await window.kirk.pair({
      code: $("code").value,
      name: $("device-name").value,
      backend: $("backend").value,
    });
    $("code").value = "";
  }, "Computer paired. Keep the companion running to start your game from the website.");
};
$("unpair").onclick = () =>
  act(() => window.kirk.unpair(), "Computer unpaired.");
$("website").onclick = () => act(() => window.kirk.website());
$("show-add").onclick = () => {
  $("profile-form").hidden = false;
  $("profile-name").focus();
};
$("cancel-add").onclick = () => {
  $("profile-form").hidden = true;
};
$("choose-target").onclick = () =>
  act(async () => {
    const file = await window.kirk.choose("target");
    if (file) $("target").value = file;
  });
$("choose-root").onclick = () =>
  act(async () => {
    const file = await window.kirk.choose("root");
    if (file) $("root").value = file;
  });
$("clear-root").onclick = () => {
  $("root").value = "";
};
$("kind").onchange = method;
$("detected").onchange = () => {
  const p = detected[Number($("detected").value)];
  if (!p || $("detected").value === "") return;
  $("instance").value = p.instance;
  $("root").value = p.root;
  $("kind").value = p.kind;
  if (!$("profile-name").value) $("profile-name").value = p.name;
  method();
};
$("profile-form").onsubmit = (event) => {
  event.preventDefault();
  act(async () => {
    await window.kirk.add({
      name: $("profile-name").value,
      kind: $("kind").value,
      target: $("target").value,
      instance: $("instance").value,
      root: $("root").value,
    });
    $("profile-form").reset();
    $("profile-form").hidden = true;
    method();
  }, "Profile saved. Use Test launch to check it, then start it from your dashboard.");
};
$("startup").onchange = () =>
  act(async () => {
    render(await window.kirk.startup($("startup").checked));
  });
window.kirk.onState(render);
window.kirk
  .state()
  .then(render)
  .catch((error) => notice(error.message));
window.kirk
  .discover()
  .then((items) => {
    detected = items;
    items.forEach((p, i) => {
      const option = document.createElement("option");
      option.value = String(i);
      option.textContent = p.name + " · " + labels[p.kind];
      $("detected").append(option);
    });
  })
  .catch(() => {});
method();
try {
  $("appearance").value =
    localStorage.getItem("kirk-companion-theme") === "kirk" ? "kirk" : "ghost";
} catch {
  /* This visit only if unavailable. */
}
document.documentElement.dataset.theme = $("appearance").value;
$("appearance").onchange = () => {
  document.documentElement.dataset.theme = $("appearance").value;
  try {
    localStorage.setItem("kirk-companion-theme", $("appearance").value);
  } catch {
    /* This visit only. */
  }
};
document.querySelectorAll(".nav-item").forEach((link) =>
  link.addEventListener("click", () => {
    document
      .querySelectorAll(".nav-item")
      .forEach((item) => item.classList.toggle("active", item === link));
  }),
);
