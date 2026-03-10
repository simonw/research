import { IncomingMessage } from "node:http";
import { Duplex } from "node:stream";
import { config } from "./config.js";
import { ProxiedTCPSocket } from "./upstream-proxy.js";

// Access the wisp-js server module
// @ts-expect-error - wisp-js has no type declarations
import { server, logging } from "@mercuryworkshop/wisp-js/server";

const { routeRequest, options } = server;

export function configureWisp(): void {
  // Configure hostname whitelist
  if (config.allowedHosts.length > 0) {
    options.hostname_whitelist = config.allowedHosts;
  }

  // Configure stream limits
  if (config.maxStreams > 0) {
    options.stream_limit_total = config.maxStreams;
  }

  // Disable private/loopback for security
  options.allow_private_ips = false;
  options.allow_loopback_ips = false;

  // Set wisp version to 2 for extensions support
  options.wisp_version = 2;

  logging.set_level(logging.INFO);

  console.log("[wisp] configured:", {
    allowedHosts: config.allowedHosts.length > 0 ? config.allowedHosts : "all",
    maxStreams: config.maxStreams,
    upstreamProxy: config.upstreamProxy ? "enabled" : "direct",
  });
}

export function handleUpgrade(
  request: IncomingMessage,
  socket: Duplex,
  head: Buffer
): void {
  const connOptions: Record<string, unknown> = {};

  // If upstream proxy is configured, inject custom TCPSocket class
  if (config.upstreamProxy) {
    connOptions.TCPSocket = createProxiedSocketClass(config.upstreamProxy);
  }

  routeRequest(request, socket, head, connOptions);
}

/**
 * Creates a TCPSocket class compatible with wisp-js that routes
 * connections through an upstream proxy.
 */
function createProxiedSocketClass(proxyUrl: string) {
  return class extends ProxiedTCPSocket {
    constructor(hostname: string, port: number) {
      super(hostname, port, proxyUrl);
    }
  };
}
