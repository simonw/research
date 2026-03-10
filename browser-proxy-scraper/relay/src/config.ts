import "dotenv/config";

export const config = {
  port: parseInt(process.env.PORT || "3000", 10),
  upstreamProxy: process.env.UPSTREAM_PROXY || "", // socks5://... or http://...
  allowedHosts: process.env.ALLOWED_HOSTS
    ? process.env.ALLOWED_HOSTS.split(",").map((h) => h.trim())
    : [], // empty = allow all
  maxStreams: parseInt(process.env.MAX_STREAMS || "-1", 10), // -1 = unlimited
};
