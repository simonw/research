FROM linuxserver/chromium:latest

RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    nodejs \
    socat \
    && rm -rf /var/lib/apt/lists/*

RUN npx playwright install-deps chromium

COPY extension /opt/extension

COPY server /opt/server
WORKDIR /opt/server
RUN npm install

COPY scripts/start-server.sh /custom-cont-init.d/start-server.sh
RUN chmod +x /custom-cont-init.d/start-server.sh

ENV TITLE=DockerChrome
ENV PUID=1000
ENV PGID=1000
ENV TZ=Etc/UTC

EXPOSE 8080
