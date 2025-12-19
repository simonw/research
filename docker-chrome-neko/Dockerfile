# Build container that extends neko chromium with CDP agent capabilities
FROM ghcr.io/m1k1o/neko/chromium:latest

USER root

# Install Node.js for CDP agent
RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Create CDP agent directory
WORKDIR /opt/cdp-agent

# Copy package.json first for better layer caching
COPY cdp-agent/package.json ./
RUN npm install

# Copy agent code
COPY cdp-agent/index.js ./

# Create entrypoint script that starts both neko and CDP agent
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# neko defaults (can be overridden via env vars)
# Note: NEKO_DESKTOP_SCREEN will be dynamically adjusted via CDP agent
ENV NEKO_MEMBER_MULTIUSER_USER_PASSWORD=neko
ENV NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD=admin
ENV NEKO_WEBRTC_ICELITE=true
ENV NEKO_SESSION_IMPLICIT_HOSTING=true
ENV NEKO_CAPTURE_AUDIO_ENABLED=false

# CDP agent port
ENV CDP_AGENT_PORT=3001

# Chromium flags for CDP exposure
ENV NEKO_BROWSER_FLAGS="--remote-debugging-address=0.0.0.0 --remote-debugging-port=9222 --no-first-run --no-default-browser-check"

# Expose ports:
# 8080 - neko WebRTC UI
# 9222 - Chrome DevTools Protocol
# 3001 - CDP agent API
EXPOSE 8080 9222 3001

# Use custom entrypoint that starts both services
ENTRYPOINT ["/entrypoint.sh"]
