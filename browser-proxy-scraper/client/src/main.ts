import { initTransport } from "./transport";
import { setupUI, enableUI, setStatus } from "./ui";
import { setupDataPanel } from "./data-panel";
import { setupPuppet } from "./puppet";

async function bootstrap(): Promise<void> {
  setStatus("Registering service worker...");

  // Register SW with root scope
  try {
    const reg = await navigator.serviceWorker.register("/sw.js", { scope: "/" });
    console.log("[main] SW registered, scope:", reg.scope);
  } catch (err) {
    setStatus(`SW registration failed: ${err}`, "error");
    throw err;
  }

  // Wait for SW to be ready
  setStatus("Waiting for service worker...");
  await navigator.serviceWorker.ready;
  console.log("[main] SW ready");

  // Initialize transport (bare-mux + epoxy)
  setStatus("Initializing encrypted transport...");
  try {
    await initTransport();
    console.log("[main] Transport initialized");
  } catch (err) {
    setStatus(`Transport init failed: ${err}`, "error");
    throw err;
  }

  // Set up UI components
  setupUI();
  setupDataPanel();
  setupPuppet();
  enableUI();
}

bootstrap().catch((err) => {
  console.error("[main] Bootstrap failed:", err);
});
