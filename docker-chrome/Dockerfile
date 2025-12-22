FROM linuxserver/chromium:latest

RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    nodejs \
    socat \
    unzip \
    libnss3-tools \
    && rm -rf /var/lib/apt/lists/*

# Download and install Brightdata CA certificate for system trust store
RUN curl -L -o /tmp/brightdata_ca.zip https://brightdata.com/static/brightdata_proxy_ca.zip && \
    unzip /tmp/brightdata_ca.zip -d /tmp/brightdata_ca && \
    cp "/tmp/brightdata_ca/New SSL certifcate - MUST BE USED WITH PORT 33335/BrightData SSL certificate (port 33335).crt" /usr/local/share/ca-certificates/brightdata.crt && \
    update-ca-certificates && \
    rm -rf /tmp/brightdata_ca.zip /tmp/brightdata_ca

RUN npx playwright install-deps chromium

COPY server /opt/server
WORKDIR /opt/server
RUN npm install

COPY scripts/install-brightdata-cert.sh /custom-cont-init.d/01-install-brightdata-cert.sh
COPY scripts/start-server.sh /custom-cont-init.d/99-start-server.sh
RUN chmod +x /custom-cont-init.d/01-install-brightdata-cert.sh /custom-cont-init.d/99-start-server.sh

ENV TITLE=DockerChrome
ENV PUID=1000
ENV PGID=1000
ENV TZ=Etc/UTC
ENV CHROME_CLI="--remote-debugging-port=9222"

EXPOSE 8080
