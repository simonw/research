FROM selkies-project/selkies-gstreamer-ubuntu20.04:latest

USER root
RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    unzip \
    xvfb \
    supervisor \
    fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://dl.google.com/linux/linux_signing_key.pub | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] http://dl.google.com/linux/chrome/deb/ stable main" > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update && apt-get install -y google-chrome-stable \
    && rm -rf /var/lib/apt/lists/*

COPY extension /opt/extension

COPY server /opt/server
WORKDIR /opt/server
RUN npm install

COPY scripts/supervisord.conf /etc/supervisor/conf.d/supervisord.conf

ENV SELKIES_ENABLE_AUDIO=true
ENV SELKIES_ENABLE_RESIZE=true
ENV DISPLAY=:0

EXPOSE 8080 9222

CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf"]
