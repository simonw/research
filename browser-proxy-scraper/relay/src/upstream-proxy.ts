import net from "node:net";

/**
 * Creates a TCP connection through a SOCKS5 proxy.
 * Implements SOCKS5 handshake (RFC 1928) with no-auth method.
 */
export function createSocks5Connection(
  host: string,
  port: number,
  proxyUrl: string
): Promise<net.Socket> {
  return new Promise((resolve, reject) => {
    const url = new URL(proxyUrl);
    const proxyHost = url.hostname;
    const proxyPort = parseInt(url.port || "1080", 10);

    const socket = net.connect(proxyPort, proxyHost, () => {
      // SOCKS5 greeting: version 5, 1 auth method (no auth)
      socket.write(Buffer.from([0x05, 0x01, 0x00]));
    });

    let state: "greeting" | "connect" | "done" = "greeting";

    socket.once("error", reject);

    socket.on("data", (data) => {
      if (state === "greeting") {
        // Server response: version, chosen method
        if (data[0] !== 0x05 || data[1] !== 0x00) {
          socket.destroy();
          reject(new Error("SOCKS5 auth negotiation failed"));
          return;
        }
        state = "connect";

        // SOCKS5 connect request: version, connect cmd, reserved, domain type
        const hostBuf = Buffer.from(host, "utf8");
        const portBuf = Buffer.alloc(2);
        portBuf.writeUInt16BE(port);
        const req = Buffer.concat([
          Buffer.from([0x05, 0x01, 0x00, 0x03, hostBuf.length]),
          hostBuf,
          portBuf,
        ]);
        socket.write(req);
      } else if (state === "connect") {
        // Server response: version, status, reserved, addr type, addr, port
        if (data[0] !== 0x05 || data[1] !== 0x00) {
          socket.destroy();
          reject(new Error(`SOCKS5 connect failed with status ${data[1]}`));
          return;
        }
        state = "done";
        socket.removeAllListeners("data");
        resolve(socket);
      }
    });
  });
}

/**
 * Creates a TCP connection through an HTTP CONNECT proxy.
 */
export function createHttpProxyConnection(
  host: string,
  port: number,
  proxyUrl: string
): Promise<net.Socket> {
  return new Promise((resolve, reject) => {
    const url = new URL(proxyUrl);
    const proxyHost = url.hostname;
    const proxyPort = parseInt(url.port || "8080", 10);

    const socket = net.connect(proxyPort, proxyHost, () => {
      const connectReq = `CONNECT ${host}:${port} HTTP/1.1\r\nHost: ${host}:${port}\r\n\r\n`;
      socket.write(connectReq);
    });

    socket.once("error", reject);

    let buffer = "";
    const onData = (data: Buffer) => {
      buffer += data.toString();
      if (buffer.includes("\r\n\r\n")) {
        socket.removeListener("data", onData);
        if (buffer.startsWith("HTTP/1.1 200") || buffer.startsWith("HTTP/1.0 200")) {
          resolve(socket);
        } else {
          socket.destroy();
          reject(new Error(`HTTP CONNECT failed: ${buffer.split("\r\n")[0]}`));
        }
      }
    };
    socket.on("data", onData);
  });
}

/**
 * Creates a proxied TCP connection based on the proxy URL scheme.
 */
export function createProxiedConnection(
  host: string,
  port: number,
  proxyUrl: string
): Promise<net.Socket> {
  if (proxyUrl.startsWith("socks5://") || proxyUrl.startsWith("socks://")) {
    return createSocks5Connection(host, port, proxyUrl);
  }
  if (proxyUrl.startsWith("http://") || proxyUrl.startsWith("https://")) {
    return createHttpProxyConnection(host, port, proxyUrl);
  }
  throw new Error(`Unsupported proxy scheme: ${proxyUrl}`);
}

/**
 * A TCPSocket class compatible with wisp-js that routes connections
 * through an upstream proxy. Matches the NodeTCPSocket interface.
 */
export class ProxiedTCPSocket {
  hostname: string;
  port: number;
  proxyUrl: string;
  socket: net.Socket | null = null;
  paused = false;
  connected = false;
  private dataQueue: (Buffer | null)[] = [];
  private dataResolve: ((value: Buffer | null) => void) | null = null;

  constructor(hostname: string, port: number, proxyUrl: string) {
    this.hostname = hostname;
    this.port = port;
    this.proxyUrl = proxyUrl;
  }

  async connect(): Promise<void> {
    this.socket = await createProxiedConnection(
      this.hostname,
      this.port,
      this.proxyUrl
    );
    this.connected = true;
    this.socket.setNoDelay(true);

    this.socket.on("data", (data: Buffer) => {
      if (this.dataResolve) {
        const resolve = this.dataResolve;
        this.dataResolve = null;
        resolve(data);
      } else {
        this.dataQueue.push(data);
      }
    });

    this.socket.on("close", () => {
      this.enqueueNull();
      this.socket = null;
    });

    this.socket.on("end", () => {
      this.enqueueNull();
      if (this.socket) {
        this.socket.destroy();
        this.socket = null;
      }
    });

    this.socket.on("error", () => {
      // Errors are handled by close/end
    });
  }

  private enqueueNull(): void {
    if (this.dataResolve) {
      const resolve = this.dataResolve;
      this.dataResolve = null;
      resolve(null);
    } else {
      this.dataQueue.push(null);
    }
  }

  async recv(): Promise<Buffer | null> {
    if (this.dataQueue.length > 0) {
      return this.dataQueue.shift()!;
    }
    return new Promise((resolve) => {
      this.dataResolve = resolve;
    });
  }

  async send(data: Uint8Array): Promise<void> {
    if (!this.socket) return;
    await new Promise<void>((resolve) => {
      this.socket!.write(data, () => resolve());
    });
  }

  close(): void {
    if (!this.socket) return;
    this.socket.end();
    this.socket = null;
  }

  pause(): void {
    if (this.socket && this.dataQueue.length > 64) {
      this.socket.pause();
      this.paused = true;
    }
  }

  resume(): void {
    if (this.socket && this.paused) {
      this.socket.resume();
      this.paused = false;
    }
  }
}
