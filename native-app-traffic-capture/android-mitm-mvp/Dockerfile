# Android MITM MVP Dockerfile
FROM budtmo/docker-android:emulator_13.0

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8

# Elevate for package installation
USER root

# Install dependencies
RUN mkdir -p /var/lib/apt/lists/partial \
    && chmod 755 /var/lib/apt/lists /var/lib/apt/lists/partial \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        python3 \
        python3-pip \
        wget \
        curl \
        xz-utils \
        openssl \
    && rm -rf /var/lib/apt/lists/*

# Python tooling
RUN PIP_BREAK_SYSTEM_PACKAGES=1 pip3 install --no-cache-dir mitmproxy frida-tools

# Download Frida server
ARG FRIDA_VERSION=16.1.10
RUN wget https://github.com/frida/frida/releases/download/${FRIDA_VERSION}/frida-server-${FRIDA_VERSION}-android-x86_64.xz \
    && unxz frida-server-${FRIDA_VERSION}-android-x86_64.xz \
    && mv frida-server-${FRIDA_VERSION}-android-x86_64 /frida-server \
    && chmod +x /frida-server

# Copy Frida scripts
COPY frida-scripts/ /frida-scripts/

# Prime mitmproxy certificate directory
RUN mkdir -p /root/.mitmproxy \
    && mitmproxy --version

# Copy entrypoint
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Remain as root for runtime compatibility with base image tooling
USER root

EXPOSE 5555 6080 8080 8081

ENTRYPOINT ["/entrypoint.sh"]
