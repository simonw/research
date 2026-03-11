/**
 * SharedWorker polyfill for browsers that don't support it (mobile browsers).
 * Uses a regular Worker with multiplexed connections to simulate SharedWorker.
 */
if (typeof SharedWorker === "undefined") {
  const workerInstances = new Map();

  globalThis.SharedWorker = class SharedWorkerPolyfill {
    constructor(scriptURL, name) {
      const channel = new MessageChannel();
      this.port = channel.port1;

      const absoluteURL = new URL(scriptURL, location.href).href;
      const key = absoluteURL + "::" + (name || "");

      let worker = workerInstances.get(key);
      if (!worker) {
        const workerCode =
          "let _onconnectHandler = null;\n" +
          "Object.defineProperty(self, 'onconnect', {\n" +
          "  get() { return _onconnectHandler; },\n" +
          "  set(fn) { _onconnectHandler = fn; }\n" +
          "});\n" +
          "importScripts(" + JSON.stringify(absoluteURL) + ");\n" +
          "self.addEventListener('message', (e) => {\n" +
          "  if (e.data && e.data.__sharedworker_connect__ && e.ports.length > 0) {\n" +
          "    if (_onconnectHandler) {\n" +
          "      _onconnectHandler({ ports: e.ports });\n" +
          "    }\n" +
          "  }\n" +
          "});\n";
        const blob = new Blob([workerCode], { type: "application/javascript" });
        worker = new Worker(URL.createObjectURL(blob));
        workerInstances.set(key, worker);
      }

      worker.postMessage({ __sharedworker_connect__: true }, [channel.port2]);
    }
  };

  console.log("[polyfill] SharedWorker polyfill installed (using regular Worker)");
}
