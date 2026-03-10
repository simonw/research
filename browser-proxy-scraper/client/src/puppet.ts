const puppetInput = document.getElementById("puppet-input") as HTMLInputElement;
const proxyFrame = document.getElementById("proxy-frame") as HTMLIFrameElement;

type PuppetCommand = "query" | "queryAll" | "click" | "type" | "scroll" | "getPageInfo" | "evaluate";

const pendingCommands = new Map<string, {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
}>();

export function sendCommand(
  command: PuppetCommand,
  args: Record<string, unknown> = {}
): Promise<unknown> {
  const id = crypto.randomUUID();

  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      pendingCommands.delete(id);
      reject(new Error(`Puppet command '${command}' timed out after 10s`));
    }, 10000);

    pendingCommands.set(id, { resolve, reject, timeout });

    proxyFrame.contentWindow?.postMessage(
      { type: "PUPPET_CMD", id, command, args },
      "*"
    );
  });
}

function parseCommand(input: string): { command: PuppetCommand; args: Record<string, unknown> } {
  const parts = input.trim().split(/\s+/);
  const command = parts[0] as PuppetCommand;

  switch (command) {
    case "query":
    case "queryAll":
      return { command, args: { selector: parts.slice(1).join(" ") } };
    case "click":
      return { command, args: { selector: parts.slice(1).join(" ") } };
    case "type": {
      const selector = parts[1];
      const text = parts.slice(2).join(" ");
      return { command, args: { selector, text } };
    }
    case "scroll":
      return { command, args: { y: parseInt(parts[1] || "500", 10) } };
    case "getPageInfo":
      return { command, args: {} };
    case "evaluate":
      return { command, args: { code: parts.slice(1).join(" ") } };
    default:
      return { command: "query", args: { selector: input } };
  }
}

export function setupPuppet(): void {
  // Listen for responses from puppet agent
  window.addEventListener("message", (event) => {
    if (event.data?.type === "PUPPET_RESULT") {
      const { id, result, error } = event.data;
      const pending = pendingCommands.get(id);
      if (!pending) return;

      clearTimeout(pending.timeout);
      pendingCommands.delete(id);

      if (error) {
        pending.reject(new Error(error));
      } else {
        pending.resolve(result);
      }
    }

    if (event.data?.type === "PUPPET_READY") {
      console.log("[puppet] Agent ready in proxied page");
      puppetInput.disabled = false;
      puppetInput.placeholder = "Puppet ready — try: query h1 | click .btn | getPageInfo";
    }
  });

  puppetInput.addEventListener("keydown", async (e) => {
    if (e.key !== "Enter") return;
    const input = puppetInput.value.trim();
    if (!input) return;

    const { command, args } = parseCommand(input);
    console.log("[puppet] sending:", command, args);

    try {
      const result = await sendCommand(command, args);
      console.log("[puppet] result:", result);
    } catch (err) {
      console.error("[puppet] error:", err);
    }
  });

  // Expose sendCommand globally for console access
  (window as unknown as Record<string, unknown>).puppet = { sendCommand };
}
