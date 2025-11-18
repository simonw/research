FROM shmayro/dockerify-android:latest

# Install mitmproxy and Frida dependencies
RUN apt-get update && apt-get install -y \
    python3-pip \
    curl \
    wget \
    unzip \
    build-essential \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 18+ (required for ws-scrcpy)
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | bash - && \
    apt-get install -y nodejs && \
    node --version && \
    npm --version

# Install mitmproxy binary (Python 3.8-compatible)
RUN wget -q https://snapshots.mitmproxy.org/10.1.1/mitmproxy-10.1.1-linux.tar.gz -O /tmp/mitmproxy.tar.gz \
    && mkdir -p /opt/mitmproxy \
    && tar -xzf /tmp/mitmproxy.tar.gz -C /opt/mitmproxy \
    && ln -s /opt/mitmproxy/mitmproxy /usr/local/bin/mitmproxy \
    && ln -s /opt/mitmproxy/mitmdump /usr/local/bin/mitmdump \
    && ln -s /opt/mitmproxy/mitmweb /usr/local/bin/mitmweb \
    && rm /tmp/mitmproxy.tar.gz

# Install Frida tools
RUN pip3 install --no-cache-dir frida-tools==12.3.0

# Create directories for our setup
RUN mkdir -p /root/.mitmproxy \
    /root/frida-scripts \
    /root/scripts

# Copy Frida scripts from existing implementation
COPY frida-scripts/ /root/frida-scripts/

# Copy APKs to be pre-installed (YouTube, etc.)
# APKs will be installed automatically by entrypoint.sh if present
COPY apks/ /tmp/apks/

# Install ws-scrcpy
RUN cd /root && \
    git clone https://github.com/NetrisTV/ws-scrcpy.git && \
    cd ws-scrcpy && \
    npm install && \
    npm run dist:prod || true

# Copy custom entrypoint and scripts
COPY scripts/entrypoint.sh /root/scripts/entrypoint.sh
COPY scripts/clear-flows.sh /root/scripts/clear-flows.sh
RUN chmod +x /root/scripts/*.sh

# Expose additional ports for mitmproxy
EXPOSE 8080 8081

# Override the default entrypoint to include our MITM setup
ENTRYPOINT ["/root/scripts/entrypoint.sh"]
