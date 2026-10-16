const { contextBridge, ipcRenderer } = require("electron");
const call =
  (channel) =>
  async (...args) => {
    const reply = await ipcRenderer.invoke("companion:" + channel, ...args);
    if (!reply.ok) throw new Error(reply.error);
    return reply.data;
  };
contextBridge.exposeInMainWorld("kirk", {
  state: call("state"),
  pair: call("pair"),
  unpair: call("unpair"),
  choose: call("choose"),
  discover: call("discover"),
  add: call("add"),
  remove: call("remove"),
  test: call("test"),
  website: call("website"),
  startup: call("startup"),
  onState: (fn) => {
    const handler = (_event, state) => fn(state);
    ipcRenderer.on("companion:state", handler);
    return () => ipcRenderer.removeListener("companion:state", handler);
  },
});
