### Install Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/quick-start.md

Installs the stable channel of Docker on your system using a convenience script. This is a prerequisite for running Neko with Docker.

```shell
curl -sSL https://get.docker.com/ | CHANNEL=stable bash
```

--------------------------------

### Install Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/quick-start.md

Installs the stable version of Docker on your server using a convenience script. This is a prerequisite for running Neko via Docker.

```shell
curl -sSL https://get.docker.com/ | CHANNEL=stable bash
```

--------------------------------

### Download and Start Neko with Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/quick-start.md

Downloads the Neko Docker Compose configuration file and starts the Neko service in detached mode. This command assumes Docker and Docker Compose are already installed.

```shell
wget https://raw.githubusercontent.com/m1k1o/neko/master/docker-compose.yaml
sudo docker compose up -d
```

--------------------------------

### Install Docker Compose Plugin

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/quick-start.md

Installs the Docker Compose plugin for Linux systems, which is necessary to manage multi-container Docker applications like Neko.

```shell
sudo apt-get update
sudo apt-get install docker-compose-plugin
```

--------------------------------

### Install Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/quick-start.md

Downloads and installs Docker Compose version 1.29.2, making it executable. Docker Compose is used to manage multi-container Docker applications like Neko.

```shell
sudo curl -L "https://github.com/docker/compose/releases/download/1.29.2/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose
```

--------------------------------

### Build and Install xf86-input-neko

Source: https://github.com/m1k1o/neko/blob/master/utils/xorg-deps/xf86-input-neko/README.md

Steps to configure, compile, and install the xf86-input-neko driver. This process involves running standard build commands common in Unix-like systems.

```shell
./configure --prefix=/usr
make
sudo make install
```

--------------------------------

### Deploy Neko with Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/quick-start.md

Downloads the Neko Docker Compose configuration file from the official repository and starts the Neko service in detached mode.

```shell
wget https://raw.githubusercontent.com/m1k1o/neko/master/docker-compose.yaml
sudo docker-compose up -d
```

--------------------------------

### Manage Neko with Docker Compose Commands

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/README.md

Provides essential commands for managing the Neko service when using Docker Compose. This includes starting, stopping, and updating the Neko container.

```sh
# Start Neko in detached mode
docker compose up -d

# Stop Neko
docker compose down

# Update Neko to the latest version
docker compose pull
docker compose up -d
```

--------------------------------

### Neko Configuration File Examples

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Provides example configuration files for the neko server in multiple formats, illustrating the structure and options for server setup, including capture settings, server details, desktop resolution, member providers, session management, and WebRTC configurations.

```YAML
capture:
  screencast:
    enabled: false

server:
  pprof: true

desktop:
  screen: "1920x1080@60"

member:
  provider: "multiuser"
  multiuser:
  admin_password: "admin"
  user_password: "neko"

session:
  merciful_reconnect: true
  implicit_hosting: false
  inactive_cursors: true
  cookie:
    enabled: false

webrtc:
  icelite: true
  iceservers:
    backend:
      - urls: [ stun:stun.l.google.com:19302 ]
    frontend:
      - urls: [ stun:stun.l.google.com:19305 ]
```

```JSON
{
  "capture": {
    "screencast": {
      "enabled": false
    }
  },
  "server": {
    "pprof": true
  },
  "desktop": {
    "screen": "1920x1080@60"
  },
  "member": {
    "provider": "multiuser",
    "multiuser": {
      "admin_password": "admin",
      "user_password": "neko"
    }
  },
  "session": {
    "merciful_reconnect": true,
    "implicit_hosting": false,
    "inactive_cursors": true,
    "cookie": {
      "enabled": false
    }
  },
  "webrtc": {
    "icelite": true,
    "iceservers": {
      "backend": [
        {
          "urls": [ "stun:stun.l.google.com:19302" ]
        }
      ],
      "frontend": [
        {
          "urls": [ "stun:stun.l.google.com:19305" ]
        }
      ]
    }
  }
}
```

```TOML
[capture.screencast]
enabled = false

[server]
pprof = true

[desktop]
screen = "1920x1080@60"

[member]
provider = "multiuser"

[member.multiuser]
admin_password = "admin"
user_password = "neko"

[session]
merciful_reconnect = true
implicit_hosting = false
inactive_cursors = true

[session.cookie]
enabled = false

[webrtc]
icelite = true

[[webrtc.iceservers.backend]]
urls = [ "stun:stun.l.google.com:19302" ]

[[webrtc.iceservers.frontend]]
urls = [ "stun:stun.l.google.com:19305" ]
```

```HCL
capture {
  screencast {
    enabled = false
  }
}

server {
  pprof = true
}

desktop {
  screen = "1920x1080@60"
}

member {
  provider = "multiuser"

  multiuser {
    admin_password = "admin"
    user_password = "neko"
  }
}

session {
  merciful_reconnect = true
  implicit_hosting = false
  inactive_cursors = true

  cookie {
    enabled = false
  }
}

webrtc {
  icelite = true

  iceservers {
    backend {
      urls = [ "stun:stun.l.google.com:19302" ]
    }

    frontend {
      urls = [ "stun:stun.l.google.com:19305" ]
    }
  }
}
```

```Envfile
CAPTURE_SCREENCAST_ENABLED=false

SERVER_PPROF=true

DESKTOP_SCREEN=1920x1080@60

MEMBER_PROVIDER=multiuser
MEMBER_MULTIUSER_ADMIN_PASSWORD=admin
MEMBER_MULTIUSER_USER_PASSWORD=neko

SESSION_MERCIFUL_RECONNECT=true
SESSION_IMPLICIT_HOSTING=false
SESSION_INACTIVE_CURSORS=true
SESSION_COOKIE_ENABLED=false

WEBRTC_ICELITE=true

WEBRTC_ICESERVERS_BACKEND="[{"urls":["stun:stun.l.google.com:19302"]}]"
WEBRTC_ICESERVERS_FRONTEND="[{"urls":["stun:stun.l.google.com:19305"]}]"
```

```Java Properties
capture.screencast.enabled = false

server.pprof = true

desktop.screen = 1920x1080@60

member.provider = multiuser
member.multiuser.admin_password = admin
member.multiuser.user_password = neko

session.merciful_reconnect = true
session.implicit_hosting = false
session.inactive_cursors = true
session.cookie.enabled = false

webrtc.icelite = true

webrtc.iceservers.backend[0].urls = stun:stun.l.google.com:19302
webrtc.iceservers.frontend[0].urls = stun:stun.l.google.com:19305
```

--------------------------------

### Run Neko with Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/README.md

Starts a Neko container in detached mode, mapping necessary ports and setting environment variables for WebRTC and authentication. The container is automatically removed upon exit.

```sh
docker run -d --rm \
  -p 8080:8080 \
  -p 56000-56100:56000-56100/udp \
  -e NEKO_WEBRTC_EPR=56000-56100 \
  -e NEKO_WEBRTC_NAT1TO1=127.0.0.1 \
  -e NEKO_MEMBER_MULTIUSER_USER_PASSWORD=neko \
  -e NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD=admin \
  ghcr.io/m1k1o/neko/firefox:latest
```

--------------------------------

### Install X.org Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/README.md

Installs X11 development libraries and utilities required for X.org server interaction.

```shell
sudo apt-get install libx11-dev libxrandr-dev libxtst-dev libxcvt-dev xorg;
```

--------------------------------

### Install PulseAudio

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/README.md

Installs PulseAudio for audio support on Debian/Ubuntu-based systems.

```shell
sudo apt-get install pulseaudio;
```

--------------------------------

### Webcam Capture Setup (v4l2loopback)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Instructions for installing and loading the v4l2loopback kernel module, which is required for Neko's webcam capture feature on Linux systems.

```bash
# Install the required packages (Debian/Ubuntu)
sudo apt install v4l2loopback-dkms v4l2loopback-utils linux-headers-`uname -r` linux-modules-extra-`uname -r`
# Load the module with exclusive_caps=1 to allow multiple applications to access the virtual webcam
sudo modprobe v4l2loopback exclusive_caps=1
```

--------------------------------

### Install Project Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/README.md

Installs the necessary dependencies for the project using npm. This is typically the first step before running any other commands.

```shell
$ npm run install
```

--------------------------------

### Install GStreamer Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/README.md

Installs necessary GStreamer libraries and plugins for video processing on Debian/Ubuntu-based systems.

```shell
sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
    gstreamer1.0-pulseaudio;
```

--------------------------------

### Neko Configuration File Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

An example of how to configure Neko server settings using a YAML file. This demonstrates setting audio and video bitrates, which correspond to command-line arguments.

```YAML
# audio bitrate in kbit/s
audio_bitrate: 128

# video bitrate in kbit/s
video_bitrate: 3072

```

--------------------------------

### Install Other System Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/README.md

Installs additional system libraries and utilities like xdotool, xclip, and GTK for graphical applications.

```shell
sudo apt-get install xdotool xclip libgtk-3-0 libgtk-3-dev libopus0 libvpx6;
```

--------------------------------

### Start Local Development Server

Source: https://github.com/m1k1o/neko/blob/master/webpage/README.md

Starts a local development server for live previewing changes. The server automatically reloads the browser when files are modified.

```shell
$ npm run start
```

--------------------------------

### Configure Neko with Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/README.md

Defines the Neko service configuration in a docker-compose.yml file, specifying the image, restart policy, port mappings, and environment variables for WebRTC and authentication.

```yaml
services:
  neko:
    image: ghcr.io/m1k1o/neko/firefox:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "56000-56100:56000-56100/udp"
    environment:
      NEKO_WEBRTC_EPR: "56000-56100"
      NEKO_WEBRTC_NAT1TO1: "127.0.0.1"
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: "neko"
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: "admin"
```

--------------------------------

### Neko VLC Docker Compose Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

This Docker Compose setup runs Neko with the VLC media player. It requires SYS_ADMIN capabilities, mounts a volume for video files, and configures ports and environment variables for screen resolution, user authentication, and WebRTC, similar to other browser configurations.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/vlc:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    cap_add:
      - SYS_ADMIN
    volumes:
      - "<your-video-folder>:/video" # mount your video folder
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    environment:
      NEKO_DESKTOP_SCREEN: '1920x1080@30'
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      NEKO_WEBRTC_NAT1TO1: <your-IP>
```

--------------------------------

### Configuration Methods Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

Demonstrates the different methods for setting configuration values, including environment variables, command-line arguments, and YAML files. The example shows how to set the 'nat1to1' variable.

```bash
NEKO_NAT1TO1=<ip>
```

```bash
--nat1to1=<ip>
```

```yaml
nat1to1: <ip>
```

--------------------------------

### Build Neko Client

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/ui.md

Steps to clone the Neko repository, navigate to the client directory, install dependencies, and build the project.

```bash
# Clone the repository
git clone https://github.com/m1k1o/neko
# Change to the client directory
cd neko/client
# Install the dependencies
npm install
# Build the project
npm run build
```

--------------------------------

### Install Neko Server Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/build.md

Installs essential system libraries required for building the Neko server on Debian/Ubuntu-based systems.

```bash
sudo apt-get install -y --no-install-recommends libx11-dev libxrandr-dev libxtst-dev libgtk-3-dev libxcvt-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

--------------------------------

### Screencast Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Example configuration for enabling and customizing the screencast feature in Neko, including pipeline settings for JPEG encoding.

```yaml
capture:
  screencast:
    enabled: true
    pipeline: |
      ximagesrc display-name={display} show-pointer=true use-damage=false
        ! video/x-raw,framerate=10/1
        ! videoconvert
        ! queue
        ! jpegenc quality=60
        ! appsink name=appsink
```

--------------------------------

### Example Pipeline Configurations (VP8 and H264)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Provides example configurations for video pipelines using VP8 and H264 codecs. Demonstrates setting dynamic parameters like bitrate, threads, and GStreamer elements.

```yaml
capture:
  video:
    codec: vp8
    ids: [ hq, lq ]
    pipelines:
      hq:
        fps: 25
        gst_encoder: vp8enc
        gst_params:
          target-bitrate: round(3072 * 650)
          cpu-used: 4
          end-usage: cbr
          threads: 4
          deadline: 1
          undershoot: 95
          buffer-size: (3072 * 4)
          buffer-initial-size: (3072 * 2)
          buffer-optimal-size: (3072 * 3)
          keyframe-max-dist: 25
          min-quantizer: 4
          max-quantizer: 20
      lq:
        fps: 25
        gst_encoder: vp8enc
        gst_params:
          target-bitrate: round(1024 * 650)
          cpu-used: 4
          end-usage: cbr
          threads: 4
          deadline: 1
          undershoot: 95
          buffer-size: (1024 * 4)
          buffer-initial-size: (1024 * 2)
          buffer-optimal-size: (1024 * 3)
          keyframe-max-dist: 25
          min-quantizer: 4
          max-quantizer: 20
```

```yaml
capture:
  video:
    codec: h264
    ids: [ main ]
    pipelines:
      main:
        width: (width / 3) * 2
        height: (height / 3) * 2
        fps: 20
        gst_prefix: "! video/x-raw,format=I420"
        gst_encoder: "x264enc"
        gst_params:
          threads: 4
          bitrate: 4096
          key-int-max: 15
          byte-stream: true
          tune: zerolatency
          speed-preset: veryfast 
        gst_suffix: "! video/x-h264,stream-format=byte-stream"
```

--------------------------------

### Object Provider Example Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

An example YAML configuration for the object provider, demonstrating how to define users, their passwords, and associated profiles directly within the configuration file.

```yaml
member:
  provider: object
  object:
    users:
    - username: "admin"
      password: "admin"
      profile:
        name: "Administrator"
        is_admin: true
        can_login: true
        can_connect: true
        can_watch: true
        can_host: true
        can_share_media: true
        can_access_clipboard: true
        sends_inactive_cursor: true
        can_see_inactive_cursors: true
    - username: "user"
      password: "neko"
      profile:
        name: "User"
        is_admin: false
        can_login: true
        can_connect: true
        can_watch: true
        can_host: true
        can_share_media: true
        can_access_clipboard: true
        sends_inactive_cursor: true
        can_see_inactive_cursors: false
```

--------------------------------

### Build Neko Frontend

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/build.md

Installs frontend dependencies using npm and builds a production-ready frontend for Neko, outputting to the client/build directory.

```bash
cd client;
npm install;
npm run build;
```

--------------------------------

### WebRTC Audio Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Provides an example configuration in YAML format for setting up the audio pipeline for WebRTC streaming. It demonstrates how to specify the audio codec and the Gstreamer pipeline for capturing and encoding audio.

```yaml
capture:
  audio:
    codec: opus
    pipeline: |
      pulsesrc device={device}
      ! audioconvert
      ! opusenc
        bitrate=320000
      ! appsink name=appsink
```

--------------------------------

### Multi-User Provider Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Example configuration file (config.yaml) demonstrating how to set up the Multi-User Provider, including passwords and profile settings for both regular users and administrators.

```yaml
member:
  provider: multiuser
  multiuser:
    admin_password: "admin"
    admin_profile:
      name: ""
      is_admin: true
      can_login: true
      can_connect: true
      can_watch: true
      can_host: true
      can_share_media: true
      can_access_clipboard: true
      sends_inactive_cursor: true
      can_see_inactive_cursors: true
    user_password: "neko"
    user_profile:
      name: ""
      is_admin: false
      can_login: true
      can_connect: true
      can_watch: true
      can_host: true
      can_share_media: true
      can_access_clipboard: true
      sends_inactive_cursor: true
      can_see_inactive_cursors: false
```

--------------------------------

### Neko Docker Compose with Nvidia GPU Acceleration (Image)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

Configures Neko to use Nvidia GPU acceleration by specifying a dedicated image. This setup accelerates video encoding and browser rendering. Requires Nvidia Container Toolkit and the 'nvidia-firefox' image.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/nvidia-firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    environment:
      NEKO_CAPTURE_VIDEO_PIPELINE: |
        ximagesrc display-name={display} show-pointer=true use-damage=false
          ! video/x-raw,framerate=25/1
          ! videoconvert ! queue
          ! video/x-raw,format=NV12
          ! nvh264enc
            name=encoder
            preset=2
            gop-size=25
            spatial-aq=true
            temporal-aq=true
            bitrate=4096
            vbv-buffer-size=4096
            rc-mode=6
          ! h264parse config-interval=-1
          ! video/x-h264,stream-format=byte-stream
          ! appsink name=appsink
      NEKO_CAPTURE_VIDEO_CODEC: "h264"
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
    deploy:
      resources:
        reservations:
          devices:
          - driver: nvidia
            count: 1
            capabilities: [gpu]
```

--------------------------------

### Broadcast Pipeline Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

An example of a custom GStreamer pipeline for broadcasting streams. This pipeline can utilize placeholders like {url}, {device}, and {display} which will be replaced by Neko.

```bash
nvarguscamerasrc ! video/x-raw(memory:NVMM),width=1920,height=1080,format=NV12,framerate=30/1 ! nvvidconv ! nvv4l2h264enc bitrate=4000000 ! rtph264pay config-interval=1 pt=96 ! udpsink host={url} port=5000
```

--------------------------------

### Example ICE Servers in YAML

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Demonstrates how to define multiple ICE servers using YAML format, including both TURN and STUN server configurations with authentication details.

```yaml
- urls: "turn:<MY-COTURN-SERVER>:3478"
  username: "neko"
  credential: "neko"
- urls: "stun:stun.l.google.com:19302"
```

--------------------------------

### Traefik v2 docker-compose.yml

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

Configuration example for Traefik v2 using docker-compose.yml to set up Neko behind a reverse proxy. It defines service ports, routing rules, and TLS settings.

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.services.neko-frontend.loadbalancer.server.port=8080"
  - "traefik.http.routers.neko.rule=${TRAEFIK_RULE}"
  - "traefik.http.routers.neko.entrypoints=${TRAEFIK_ENTRYPOINTS}"
  - "traefik.http.routers.neko.tls.certresolver=${TRAEFIK_CERTRESOLVER}"
```

--------------------------------

### Multi-User Provider Environment Variables

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Example docker-compose.yaml snippet showing how to configure the Multi-User Provider's passwords using environment variables for easier setup.

```yaml
environment:
  NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: "admin"
  NEKO_MEMBER_MULTIUSER_USER_PASSWORD: "neko"
```

--------------------------------

### Uninstall xf86-input-neko

Source: https://github.com/m1k1o/neko/blob/master/utils/xorg-deps/xf86-input-neko/README.md

Instructions for removing the xf86-input-neko driver from the system after it has been installed.

```shell
sudo make uninstall
```

--------------------------------

### Neko CLI Usage

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/examples.md

Information on how to execute the Neko command-line interface (CLI) to view available arguments. It also suggests checking the Dockerfile for required dependencies if not using Docker.

```bash
neko --help
```

--------------------------------

### GStreamer Video Pipeline Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

An example of a custom GStreamer video pipeline for Neko. This allows users to define their own video processing chain for optimal quality and performance, specifying encoders, formats, and framerates.

```bash
ximagesrc display-name=%s show-pointer=true use-damage=false ! video/x-raw,framerate=30/1 ! videoconvert ! queue ! video/x-raw,format=NV12 ! x264enc threads=4 bitrate=3500 key-int-max=60 vbv-buf-capacity=4000 byte-stream=true tune=zerolatency speed-preset=veryfast ! video/x-h264,stream-format=byte-stream,profile=constrained-baseline
```

--------------------------------

### Install Netcat (netcat-openbsd)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

Provides instructions on how to install the netcat utility on Debian/Ubuntu-based systems if the 'nc' command is not found. This is a prerequisite for performing UDP port reachability tests.

```shell
sudo apt-get install netcat
```

--------------------------------

### Example ICE Servers in JSON

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Illustrates the configuration of multiple ICE servers using JSON format, showcasing both TURN and STUN server entries with optional username and credential fields.

```json
[
  {
    "urls": "turn:<MY-COTURN-SERVER>:3478",
    "username": "neko",
    "credential": "neko"
  },
  {
    "urls": "stun:stun.l.google.com:19302"
  }
]
```

--------------------------------

### Configuration Merging Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Demonstrates the order of precedence for Neko configuration values, showing how default values are overridden by configuration files, environment variables, and command-line arguments.

```bash
# Default Value: 127.0.0.1:8080

# Config File
cat config.yaml <<EOF
server:
  bind: "127.0.0.1:8081"
EOF

# Environment Variable
export NEKO_SERVER_BIND=127.0.0.1:8082

# Command-line Argument
./neko -config=config.yaml -server.bind=127.0.0.1:8083
```

```yaml
server:
  bind: "127.0.0.1:8081"
```

--------------------------------

### GStreamer Audio Pipeline Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

An example of a custom GStreamer audio pipeline for Neko. This enables users to configure audio input devices and encoding for streaming, similar to the video pipeline customization.

```bash
pulsesrc device=%s ! audio/x-raw,channels=2 ! audioconvert ! opusenc bitrate=128000
```

--------------------------------

### Install Netcat Package

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Instructions for installing the `netcat` utility on Debian/Ubuntu-based systems if the `nc` command is not found, which is required for UDP port reachability testing.

```shell
sudo apt-get install netcat
```

--------------------------------

### Neko Plugin Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Settings for loading and managing Neko plugins. This includes specifying the directory for plugins, enabling runtime loading, and defining whether plugins are required for Neko to start.

```config
--plugins.dir string
  path to neko plugins to load (default "./bin/plugins")
--plugins.enabled
  load plugins in runtime
--plugins.required
  if true, neko will exit if there is an error when loading a plugin
```

--------------------------------

### Docker Compose Coturn Server Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Provides a Docker Compose configuration for deploying a Coturn server, a popular TURN/STUN server implementation. It includes essential command-line arguments for setup and configuration.

```yaml
services:
  coturn:
    image: 'coturn/coturn:latest'
    network_mode: "host"
    command: |
      -n
      --realm=localhost
      --fingerprint
      --listening-ip=0.0.0.0
      --external-ip=<MY-COTURN-SERVER>
      --listening-port=3478
      --min-port=49160
      --max-port=49200
      --log-file=stdout
      --user=neko:neko
      --lt-cred-mech
```

--------------------------------

### Broadcast Pipeline Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

An example of the `NEKO_CAPTURE_BROADCAST_PIPELINE` configuration for RTMP ingest servers. This pipeline uses `flvmux` and `rtmpsink` with `pulseaudio` for audio and `ximagesrc` with `x264enc` for video, demonstrating how to include parameters like `live=1`.

```yaml
NEKO_CAPTURE_BROADCAST_PIPELINE: "flvmux name=mux ! rtmpsink location={url} pulsesrc device={device} ! audio/x-raw,channels=2 ! audioconvert ! voaacenc ! mux. ximagesrc display-name={display} show-pointer=false use-damage=false ! video/x-raw,framerate=28/1 ! videoconvert ! queue ! x264enc bframes=0 key-int-max=0 byte-stream=true tune=zerolatency speed-preset=veryfast ! mux."
```

--------------------------------

### VLC Media Player Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Shows how to configure the VLC Docker image to access local media files by mounting a directory or setting a specific path via an environment variable.

```shell
# Mount local media directory to /media inside the container
docker run --cap-add=SYS_ADMIN -v /path/to/your/media:/media m1k1o/neko:vlc

# Set VLC_MEDIA environment variable to point to media files
docker run --cap-add=SYS_ADMIN -e VLC_MEDIA=/mnt/myvideos m1k1o/neko:vlc
```

--------------------------------

### MemberProfile Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/schemas/memberprofile.schema.mdx

An example JSON object conforming to the MemberProfile schema, illustrating typical values for member properties.

```json
{
  "name": "string",
  "is_admin": true,
  "can_login": true,
  "can_connect": true,
  "can_watch": true,
  "can_host": true,
  "can_share_media": true,
  "can_access_clipboard": true,
  "sends_inactive_cursor": true,
  "can_see_inactive_cursors": true,
  "plugins": {}
}
```

--------------------------------

### Remmina Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Demonstrates how to configure Remmina within the m1k1o/neko container using environment variables for connection details or by mounting a custom profile.

```shell
# Using environment variable for connection details
docker run --cap-add=SYS_ADMIN -e REMMINA_URL="rdp://user:password@server:port" m1k1o/neko:remmina

# Using a custom profile (saved locally as ~/.local/share/remmina/myprofile.remmina)
docker run --cap-add=SYS_ADMIN -v "~/.local/share/remmina/myprofile.remmina:/root/.local/share/remmina/myprofile.remmina" -e REMMINA_PROFILE="myprofile.remmina" m1k1o/neko:remmina
```

--------------------------------

### Get Screen Configurations API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/screen-configurations-list.api.mdx

Retrieves a list of all available screen configurations. This endpoint allows fetching screen details including width, height, and refresh rate.

```APIDOC
GET /api/room/screen/configurations

Description:
  Retrieve a list of all available screen configurations.

Parameters:
  None

Responses:
  200 OK:
    Description: List of screen configurations retrieved successfully.
    Content:
      application/json:
        Schema:
          type: array
          items:
            type: object
            properties:
              width:
                type: integer
                description: The width of the screen.
                example: 1280
              height:
                type: integer
                description: The height of the screen.
                example: 720
              rate:
                type: integer
                description: The refresh rate of the screen.
                example: 30
            title: ScreenConfiguration

  401 Unauthorized:
    Description: The request requires user authentication.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  403 Forbidden:
    Description: The server understood the request, but refuses to authorize it.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Manage Extensions in Chromium Browsers (JSON)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Manages browser extensions for Chromium-based browsers, specifying which can be installed or blocked. It uses `ExtensionInstallForcelist` for mandatory installations, `ExtensionInstallAllowlist` for user-installable extensions, and `ExtensionInstallBlocklist` to prevent installations.

```json
{
  "ExtensionInstallForcelist": [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm;https://clients2.google.com/service/update2/crx",
    "mnjggcdmjocbbbhaepdhchncahnbgone;https://clients2.google.com/service/update2/crx"
  ],
  "ExtensionInstallAllowlist": [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm",
    "mnjggcdmjocbbbhaepdhchncahnbgone"
  ],
  "ExtensionInstallBlocklist": [
    "*"
  ]
}
```

--------------------------------

### Neko Network and ICE Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags related to network configuration, NAT traversal, and ICE server setup.

```APIDOC
--nat1to1 strings
  sets a list of external IP addresses of 1:1 (D)NAT and a candidate type for which the external IP address is used

--tcpmux int
  single TCP mux port for all peers

--udpmux int
  single UDP mux port for all peers

--icelite
  configures whether or not the ice agent should be a lite agent

--iceserver strings
  describes a single STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer

--iceservers string
  describes a single STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer

--ipfetch string
  automatically fetch IP address from given URL when nat1to1 is not present

--epr string
  limits the pool of ephemeral ports that ICE UDP connections can allocate from
```

--------------------------------

### Configure NAT 1-to-1 IP Address

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Specifies a static IP address for the server when it's behind a NAT or in a local network. This IP is sent to clients in ICE candidates. The example shows configuration via a filter in the Neko setup.

```APIDOC
webrtc.nat1to1: "<ip_address>"
  - Description: Manually specify the server's IP address for NAT scenarios.
  - Example: "10.10.0.5"
  - Notes: Only one address can be specified. If accessing from both local and public networks, ensure your router supports NAT loopback (hairpinning).
```

--------------------------------

### Configure VLC with Local Media and Playlist

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This example demonstrates configuring Neko's VLC container by bind-mounting local media files and specifying a playlist file via the VLC_MEDIA environment variable. This allows playback of local content or custom playlists.

```bash
docker run \
  -v /path/to/media:/media \
  -e VLC_MEDIA=/media/playlist.xspf \
  ghcr.io/m1k1o/neko/vlc
```

--------------------------------

### Error Getting Server Reflexive Address (IPv6)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

This error occurs when the system cannot obtain a server reflexive address, often related to IPv6 connectivity or DNS setup. Verify DNS configuration and IPv6 network functionality.

```log
WRN could not get server reflexive address udp6 stun:stun.l.google.com:19302: write udp6 [::]:52042->[2607:f8b0:4001:c1a::7f]:19302: sendto: cannot assign requested address
```

--------------------------------

### HAProxy Service Management and Troubleshooting

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

These commands show how to restart the HAProxy service and check its status. It also includes instructions for monitoring logs in real-time and adjusting timeout settings for better stability.

```sh
service haproxy restart
```

```sh
service haproxy status
```

```sh
tail -f /var/log/haproxy.log
```

```sh
global
  stats timeout 60s
```

```sh
defaults
  option forwardfor
  timeout connect 30000
  timeout client  65000
  timeout server  65000
```

--------------------------------

### Neko Broadcast Pipeline Configuration Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

An example of a NEKO_BROADCAST_PIPELINE environment variable configuration for Neko. This pipeline definition specifies how video and audio streams are processed and sent to an RTMP ingest server, including specific codecs and parameters for low latency.

```yaml
NEKO_BROADCAST_PIPELINE: "flvmux name=mux ! rtmpsink location={url} pulsesrc device={device} ! audio/x-raw,channels=2 ! audioconvert ! voaacenc ! mux. ximagesrc display-name={display} show-pointer=false use-damage=false ! video/x-raw,framerate=28/1 ! videoconvert ! queue ! x264enc bframes=0 key-int-max=0 byte-stream=true tune=zerolatency speed-preset=veryfast ! mux."
```

--------------------------------

### Neko Server Error: UDP6 Address Assignment Failed

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This log message indicates a failure to get a server reflexive address via UDP6, often related to DNS setup or IPv6 connectivity. It suggests checking DNS configuration and IPv6 functionality.

```log
WRN could not get server reflexive address udp6 stun:stun.l.google.com:19302: write udp6 [::]:52042->[2607:f8b0:4001:c1a::7f]:19302: sendto: cannot assign requested address
```

--------------------------------

### Run Neko with VLC Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/examples.md

This Docker Compose configuration deploys Neko with VLC, enabling video playback. It requires the SYS_ADMIN capability and mounts a local folder for videos. Configuration includes ports, shared memory, and environment variables for screen, passwords, and network.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:vlc"
    restart: "unless-stopped"
    shm_size: "2gb"
    volumes:
      - "<your-video-folder>:/video" 
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    cap_add:
      - SYS_ADMIN
    environment:
      NEKO_SCREEN: '1920x1080@30'
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_NAT1TO1: <your-IP>
```

--------------------------------

### Neko Chromium Docker Compose Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

This Docker Compose configuration deploys Neko using the Chromium browser image. It includes necessary capabilities like SYS_ADMIN, defines ports, volume mounts for Chromium settings persistence, and environment variables for screen resolution, user passwords, and WebRTC configuration.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/chromium:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    cap_add:
      - SYS_ADMIN
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - <your-host-path>:/home/neko/.config/chromium # persist chromium settings
    environment:
      NEKO_DESKTOP_SCREEN: '1920x1080@30'
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      NEKO_WEBRTC_NAT1TO1: <your-IP>
```

--------------------------------

### HAProxy Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

This snippet demonstrates how to configure HAProxy to route traffic to a Neko instance based on the host header. It includes ACL definition and backend server setup.

```sh
frontend http-in
  #/********
  #* NEKO *
  acl neko_rule_http hdr(host) neko.domain.com # Adapt the domain
  use_backend neko_srv if neko_rule_http
  #********/

backend neko_srv
  mode http
  option httpchk
      server neko 172.16.0.0:8080 # Adapt the IP
```

--------------------------------

### Caddyfile Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

This snippet shows a basic Caddyfile configuration to proxy requests to a Neko instance. It sets up HTTPS for a domain and forwards traffic to a local backend server.

```conf
https://example.com {
  reverse_proxy localhost:8080
}
```

--------------------------------

### Docker Compose Webcam Device Mapping

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Example configuration snippet for a docker-compose.yaml file to map the host's virtual webcam device (/dev/video0) into a Neko container.

```yaml
services:
  neko:
    ...
    devices:
      - /dev/video0:/dev/video0
    ...

```

--------------------------------

### Get Screen Configuration API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/screen-configuration.api.mdx

Retrieves the current screen configuration of a room. This GET request to \"/api/room/screen\" returns the screen's width, height, and refresh rate upon successful execution. It also defines error responses for unauthorized access (401), forbidden access (403), and internal server errors (500).

```APIDOC
GET /api/room/screen
  description: Retrieve the current screen configuration of the room.
  responses:
    200:
      description: Screen configuration retrieved successfully.
      content:
        application/json:
          schema:
            type: object
            properties:
              width:
                type: integer
                example: 1280
                description: The width of the screen.
              height:
                type: integer
                example: 720
                description: The height of the screen.
              rate:
                type: integer
                example: 30
                description: The refresh rate of the screen.
            title: ScreenConfiguration
    401:
      description: The request requires user authentication.
      content:
        application/json:
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    403:
      description: The server understood the request, but refuses to authorize it.
      content:
        application/json:
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    500:
      description: Unable to get screen configuration.
      content:
        application/json:
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
```

--------------------------------

### Neko Firefox Docker Compose Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

This Docker Compose configuration sets up Neko to run with the Firefox browser. It specifies the image, restart policy, shared memory size, ports, volumes for persistence, and environment variables for user authentication and WebRTC settings.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - <your-host-path>:/home/neko/.mozilla/firefox # persist firexfox settings
    environment:
      NEKO_DESKTOP_SCREEN: '1920x1080@30'
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      NEKO_WEBRTC_NAT1TO1: <your-IP>
```

--------------------------------

### Docker Compose Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

Example docker-compose.yaml file to configure and run the Neko service. It specifies the image, ports, and environment variables, including enabling debug mode.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      # highlight-start
      NEKO_DEBUG: 1
      # highlight-end
```

--------------------------------

### Run Neko with Firefox Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/examples.md

This Docker Compose configuration deploys Neko using the Firefox browser. It specifies ports, shared memory size, and persistent storage for Firefox settings. Environment variables configure screen resolution, passwords, and network settings.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - <your-host-path>:/home/neko/.mozilla/firefox # persist firexfox settings
    environment:
      NEKO_SCREEN: '1920x1080@30'
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_NAT1TO1: <your-IP>
```

--------------------------------

### GET /api/room/control - Get Room Control Status

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-status.api.mdx

Retrieves the current control status of the room. This endpoint returns information about whether a host is present and its ID. It handles authentication and authorization errors.

```APIDOC
GET /api/room/control

Retrieve the current control status of the room.

Parameters:
  None

Responses:
  200 OK:
    description: Control status retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            has_host:
              type: boolean
              description: Indicates if there is a host currently.
            host_id:
              type: string
              description: The ID of the current host, if any.
          title: ControlStatus
  401 Unauthorized:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403 Forbidden:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Supervisord Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/README.md

Example supervisord configuration file (`.conf`) for managing processes within the Neko container. It defines program settings like command, restart behavior, and logging, using environment variables for flexibility.

```config
[program:app]
environment=HOME="/home/%(ENV_USER)s",USER="%(ENV_USER)s",DISPLAY="%(ENV_DISPLAY)s"
command=/opt/path/to/my-app
stopsignal=INT
autorestart=true
priority=800
user=%(ENV_USER)s
stdout_logfile=/var/log/neko/app.log
stdout_logfile_maxbytes=100MB
stdout_logfile_backups=10
redirect_stderr=true
```

--------------------------------

### GET /api/room/clipboard/image.png - Get Clipboard Image

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/clipboard-get-image.api.mdx

Retrieves the current image content from the clipboard. This endpoint returns the image as binary data (image/png) on success. Authentication is required for access, and specific error codes are returned for unauthorized access or server errors.

```APIDOC
GET /api/room/clipboard/image.png
  description: Retrieve the current image content of the clipboard.
  parameters: []
  requestBody: null
  responses:
    "200":
      description: Clipboard image retrieved successfully.
      content:
        "image/png":
          schema:
            type: string
            format: binary
    "401":
      description: The request requires user authentication.
      content:
        "application/json":
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    "403":
      description: The server understood the request, but refuses to authorize it.
      content:
        "application/json":
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    "500":
      description: Unable to get clipboard content.
      content:
        "application/json":
          schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
```

--------------------------------

### GET /api/room/screen/cast.jpg - Get Screencast Image

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/screen-cast-image.api.mdx

Retrieves the current screencast image from the room. This endpoint returns a JPEG image on success. Error responses include details about why the screencast could not be retrieved, such as it not being enabled, authentication issues, or server errors.

```APIDOC
GET /api/room/screen/cast.jpg

Retrieve the current screencast image.

Parameters:
  None

Responses:
  200 OK:
    description: Screencast image retrieved successfully.
    content:
      image/jpeg:
        schema:
          type: string
          format: binary
  400 Bad Request:
    description: Screencast is not enabled.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  401 Unauthorized:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403 Forbidden:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  500 Internal Server Error:
    description: Unable to fetch image.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### GET /api/room/broadcast - Get Broadcast Status

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/broadcast-status.api.mdx

Retrieves the current broadcast status of the room. This endpoint returns the broadcast URL and an indicator of whether the broadcast is active. It handles authentication and authorization errors.

```APIDOC
GET /api/room/broadcast

Retrieve the current broadcast status of the room.

Parameters:
  None

Responses:
  200 OK:
    description: Broadcast status retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            url:
              type: string
              example: "rtmp://localhost/live"
              description: The URL of the broadcast.
            is_active:
              type: boolean
              description: Indicates if the broadcast is active.
          title: BroadcastStatus
  401 Unauthorized:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403 Forbidden:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### GET /api/room/settings - Get Room Settings

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/settings-get.api.mdx

Retrieves the current settings for a room. This endpoint returns a JSON object containing various room configuration parameters. It handles successful retrieval (200 OK) and authentication/authorization errors (401 Unauthorized, 403 Forbidden).

```APIDOC
GET /api/room/settings

Retrieve the current settings of the room.

Responses:
  200:
    description: Room settings retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            private_mode:
              type: boolean
              description: Indicates if the room is in private mode.
            locked_controls:
              type: boolean
              description: Indicates if the room controls are locked.
            implicit_hosting:
              type: boolean
              description: Indicates if implicit hosting is enabled.
            inactive_cursors:
              type: boolean
              description: Indicates if inactive cursors are shown.
            merciful_reconnect:
              type: boolean
              description: Indicates if merciful reconnect is enabled.
            plugins:
              type: object
              additionalProperties: true
              description: Additional plugin settings.
          title: Settings
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Get Current User API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/whoami.api.mdx

Retrieves information about the current user session via the /api/whoami endpoint.

```APIDOC
GET /api/whoami

Description:
  Retrieve information about the current user session.

Parameters:
  (None)

Responses:
  200 OK:
    description: User information retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            id:
              type: string
              description: The unique identifier for the user.
            name:
              type: string
              description: The display name of the user.
            email:
              type: string
              description: The email address of the user.
            roles:
              type: array
              items:
                type: string
              description: A list of roles assigned to the user.

Example:
  {
    "id": "user-123",
    "name": "John Doe",
    "email": "john.doe@example.com",
    "roles": ["admin", "editor"]
  }
```

--------------------------------

### WebRTC ICE Server Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Example configuration for WebRTC ICE servers, specifying STUN server URLs for backend and frontend connections.

```config
webrtc.iceservers.backend[0].urls[0] = stun:stun.l.google.com:19302
webrtc.iceservers.frontend[0].urls[0] = stun:stun.l.google.com:19305
```

--------------------------------

### Docker Run: Neko Chromium Browser Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This command demonstrates how to run a Neko Docker image for Chromium-based browsers. It requires specific capabilities (`--cap-add=SYS_ADMIN`) and an increased shared memory size (`--shm-size=2g`) for proper functionality.

```bash
docker run \
  --cap-add=SYS_ADMIN \
  --shm-size=2g \
  ghcr.io/m1k1o/neko/chromium
```

--------------------------------

### Get Server Statistics

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/stats.api.mdx

Retrieves statistics about the server and active user sessions. Includes host status, server uptime, and user/admin counts. Requires authentication.

```APIDOC
GET /api/stats

Description:
  Retrieve statistics about the server and user sessions.

Responses:
  200 OK:
    description: Statistics retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            has_host: { type: boolean, description: "Indicates if there is a host currently." }
            host_id: { type: string, description: "The ID of the current host, if any." }
            server_started_at: { type: string, format: "date-time", description: "The timestamp when the server started." }
            total_users: { type: integer, description: "The total number of users connected." }
            last_user_left_at: { type: string, format: "date-time", description: "The timestamp when the last user left, if any." }
            total_admins: { type: integer, description: "The total number of admins connected." }
            last_admin_left_at: { type: string, format: "date-time", description: "The timestamp when the last admin left, if any." }
          title: Stats
  401 Unauthorized:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
  403 Forbidden:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
```

--------------------------------

### Neko ConfigurationTab Component Usage

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/plugins.md

Example of using the ConfigurationTab component to display and filter configuration options, likely within a documentation or UI framework.

```javascript
import { ConfigurationTab } from '@site/src/components/Configuration';
import configOptions from './help.json';

<ConfigurationTab options={configOptions} filter={[
  'plugins.enabled',
  'plugins.required',
  'plugins.dir',
]} comments={false} />
```

--------------------------------

### Run Neko with Chromium Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/examples.md

This Docker Compose configuration deploys Neko using the Chromium browser. It requires the SYS_ADMIN capability and configures ports, shared memory, and environment variables for screen resolution, passwords, and network settings.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:chromium"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    cap_add:
      - SYS_ADMIN
    environment:
      NEKO_SCREEN: '1920x1080@30'
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_NAT1TO1: <your-IP>
```

--------------------------------

### Docker Compose with Custom Supervisord Config

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/README.md

Example docker-compose.yaml file demonstrating how to mount a custom supervisord configuration file into the Neko container. It also shows common Neko environment variables for screen resolution, user passwords, and WebRTC configuration.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - "./firefox.conf:/etc/neko/supervisord/firefox.conf"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### Start Broadcast API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/broadcast-start.api.mdx

Initiates the broadcasting of a room's content. This POST request requires a JSON body specifying the broadcast URL and its active status. It returns a 204 on success or various error codes for invalid requests, authentication issues, or server-side problems.

```APIDOC
POST /api/room/broadcast/start

Description:
  Start broadcasting the room's content.

Request Body:
  content:
    application/json:
      schema:
        type: object
        properties:
          url:
            type: string
            example: "rtmp://localhost/live"
            description: "The URL of the broadcast."
          is_active:
            type: boolean
            description: "Indicates if the broadcast is active."
        title: "BroadcastStatus"
      required: true

Responses:
  204:
    description: "Broadcast started successfully."
  400:
    description: "Missing broadcast URL."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  401:
    description: "The request requires user authentication."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  403:
    description: "The server understood the request, but refuses to authorize it."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  422:
    description: "Server is already broadcasting."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  500:
    description: "Unable to start broadcast."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
```

--------------------------------

### Neko Docker Image Applications (GHCR)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Lists common applications available as Neko Docker images on the GitHub Container Registry. Examples include Firefox, Tor Browser, and Waterfox.

```shell
ghcr.io/m1k1o/neko/firefox
```

```shell
ghcr.io/m1k1o/neko/tor-browser
```

```shell
ghcr.io/m1k1o/neko/waterfox
```

--------------------------------

### Configuration Tab Component Usage

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Example of using the ConfigurationTab component to display configuration options. It imports the component and passes configuration data, likely from a JSON file, to render a configuration table.

```javascript
import { ConfigurationTab } from '@site/src/components/Configuration';
import configOptions from './help.json';

<ConfigurationTab options={configOptions} heading={true} />
```

--------------------------------

### Get Session API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/session-get.api.mdx

Retrieves information about a specific session using its unique identifier. This endpoint requires a session ID in the path.

```APIDOC
GET /api/sessions/{sessionId}

Retrieve information about a specific session.

Parameters:
  - in: path
    name: sessionId
    description: The identifier of the session.
    required: true
    schema:
      type: string

Returns:
  - 200 OK: Session details retrieved successfully.
  - 404 Not Found: Session with the specified ID not found.
  - 500 Internal Server Error: An unexpected error occurred.
```

--------------------------------

### Docker Compose Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Example docker-compose.yaml file demonstrating how to expose necessary UDP ports for WebRTC and set environment variables like NEKO_WEBRTC_EPR. This configuration is crucial for establishing peer-to-peer connections.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    # highlight-start
    - "52000-52100:52000-52100/udp"
    # highlight-end
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      # highlight-start
      NEKO_WEBRTC_EPR: 52000-52100
      # highlight-end
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### Get Clipboard Content API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/clipboard-get-text.api.mdx

Retrieves the current content of the clipboard. Supports both plain text and rich text (HTML) formats. Handles authentication and error scenarios.

```APIDOC
GET /api/room/clipboard

Retrieve the current content of the clipboard (rich-text or plain-text).

Responses:
  200 OK
    description: Clipboard content retrieved successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            text:
              type: string
              example: "Copied Content 123"
              description: The plain text content of the clipboard.
            html:
              type: string
              example: "<b>Copied Content 123</b>"
              description: The HTML content of the clipboard.
          title: ClipboardText

  401 Unauthorized
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  403 Forbidden
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  500 Internal Server Error
    description: Unable to get clipboard content.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Get Keyboard Map API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/keyboard-map-get.api.mdx

Retrieves the current keyboard map configuration. This API endpoint returns the keyboard layout and variant. It handles authentication, authorization, and server errors.

```APIDOC
GET /api/room/keyboard/map

Description: Retrieve the current keyboard map configuration.

Responses:
  200 OK:
    Description: Keyboard map retrieved successfully.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            layout:
              type: string
              example: sk
              description: The keyboard layout.
            variant:
              type: string
              example: qwerty
              description: The keyboard variant.
          title: KeyboardMap
  401 Unauthorized:
    Description: The request requires user authentication.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403 Forbidden:
    Description: The server understood the request, but refuses to authorize it.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  500 Internal Server Error:
    Description: Unable to get keyboard map.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Neko Docker Compose with Nvidia GPU Acceleration (Env Vars)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

Configures Neko for Nvidia GPU acceleration using environment variables with the default Firefox image. This method enables GPU-accelerated video encoding. Requires Nvidia Container Toolkit and setting NVIDIA_VISIBLE_DEVICES and NVIDIA_DRIVER_CAPABILITIES.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    environment:
      NVIDIA_VISIBLE_DEVICES: all
      NVIDIA_DRIVER_CAPABILITIES: all
      NEKO_CAPTURE_VIDEO_PIPELINE: |
        ximagesrc display-name={display} show-pointer=true use-damage=false
          ! video/x-raw,framerate=25/1
          ! videoconvert ! queue
          ! video/x-raw,format=NV12
          ! nvh264enc
            name=encoder
            preset=2
            gop-size=25
            spatial-aq=true
            temporal-aq=true
            bitrate=4096
            vbv-buffer-size=4096
            rc-mode=6
          ! h264parse config-interval=-1
          ! video/x-h264,stream-format=byte-stream
          ! appsink name=appsink
      NEKO_CAPTURE_VIDEO_CODEC: "h264"
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
    deploy:
      resources:
        reservations:
          devices:
          - driver: nvidia
            count: 1
            capabilities: [gpu]
```

--------------------------------

### Mount Custom UI Files with Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/ui.md

Example docker-compose.yaml configuration to mount custom UI files from the local client/dist directory into the Neko container, overwriting default files.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - "./client/dist:/var/www"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### Build Neko Server

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/build.md

Navigates to the server directory and executes the build script to compile the Neko server binary and its plugins.

```bash
cd server;
./build;
```

--------------------------------

### Apache configuration for Neko proxy

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

Apache VirtualHost configuration for proxying Neko traffic. Includes settings for SSL, HTTP/2, WebSocket upgrades, and header forwarding, along with module loading.

```xml
<VirtualHost *:80>
  # The ServerName directive sets the request scheme, hostname, and port that
  # the server uses to identify itself. This is used when creating
  # redirection URLs. In the context of virtual hosts, the ServerName
  # specifies what hostname must appear in the request's Host: header to
  # match this virtual host. For the default virtual host (this file), this
  # value is not decisive as it is used as a last resort host regardless.
  # However, you must set it explicitly for any further virtual host.

  # Paths of these modules might vary across different distros.
  LoadModule proxy_module /usr/lib/apache2/modules/mod_proxy.so
  LoadModule proxy_http_module /usr/lib/apache2/modules/mod_proxy_http.so
  LoadModule proxy_wstunnel_module /usr/lib/apache2/modules/mod_proxy_wstunnel.so

  ServerName example.com
  ServerAlias www.example.com

  ProxyRequests Off
  ProxyPass / http://localhost:8080/
  ProxyPassReverse / http://localhost:8080/

  RewriteEngine on
  RewriteCond %{HTTP:Upgrade} websocket [NC]
  RewriteCond %{HTTP:Connection} upgrade [NC]
  RewriteRule /ws(.*) "ws://localhost:8080/ws$1" [P,L]

  # Available log levels: trace8, ..., trace1, debug, info, notice, warn,
  # error, crit, alert, emerg.
  # It is also possible to configure the log level for particular
  # modules, e.g.
  #LogLevel info ssl:warn

  ErrorLog ${APACHE_LOG_DIR}/error.log
  CustomLog ${APACHE_LOG_DIR}/access.log combined

  # For most configuration files from conf-available/, which are
  # enabled or disabled at a global level, it is possible to
  # include a line for only one particular virtual host. For example, the
  # following line enables the CGI configuration for this host only
  # after it has been globally disabled with "a2disconf".
  #Include conf-available/serve-cgi-bin.conf
</VirtualHost>
```

--------------------------------

### DTLS Transport Not Started Error

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

This warning suggests that the DTLS transport has not yet started when an undeclaredMediaProcessor attempts to open an SrtcpSession. Ensure UDP ports are correctly exposed and reachable.

```log
WRN undeclaredMediaProcessor failed to open SrtcpSession: the DTLS transport has not started yet module=webrtc subsystem=
```

--------------------------------

### Gstreamer Pipeline String Example

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Illustrates a typical Gstreamer pipeline string format. It includes placeholders like `{display}` for the display name and requires the sink element to be named `appsink` so that Neko can capture the video frames.

```gstreamer
ximagesrc display-name={display} show-pointer=true use-damage=false ! <your_elements> ! appsink name=appsink
```

--------------------------------

### Get Member Profile API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-get-profile.api.mdx

Retrieves the profile details for a specific member using their unique identifier. This endpoint requires the member's ID in the path.

```APIDOC
GET /api/members/{memberId}

Retrieve the profile of a specific member.

Parameters:
  - in: path
    name: memberId
    description: The identifier of the member.
    required: true
    schema:
      type: string

Returns:
  - 200 OK: Member profile retrieved successfully.
  - 404 Not Found: Member with the specified ID not found.
  - 500 Internal Server Error: An unexpected error occurred.
```

--------------------------------

### Run Neko on Raspberry Pi Docker

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/examples.md

This Docker Compose configuration is tailored for Raspberry Pi, using a specific ARM image. It adjusts shared memory size and uses privileged mode for GPU access. Environment variables configure screen resolution, passwords, and video encoding pipeline for hardware acceleration.

```yaml
version: "3.4"
services:
  neko:
    # see docs for more variants
    image: "ghcr.io/m1k1o/neko/arm-chromium:latest"
    restart: "unless-stopped"
    # increase on rpi's with more then 1gb ram.
    shm_size: "520mb"
    ports:
      - "8088:8080"
      - "52000-52100:52000-52100/udp"
    # note: this is important since we need a GPU for hardware acceleration alternatively
    #       mount the devices into the docker.
    privileged: true
    environment:
      NEKO_SCREEN: '1280x720@30'
      NEKO_PASSWORD: 'neko'
      NEKO_PASSWORD_ADMIN: 'admin'
      NEKO_EPR: 52000-52100
      # note: when setting NEKO_VIDEO, then variables NEKO_MAX_FPS and NEKO_VIDEO_BITRATE
      #       are not being used, you can adjust them in this variable.
      NEKO_VIDEO: |
        ximagesrc display-name=%s use-damage=0 show-pointer=true use-damage=false
          ! video/x-raw,framerate=30/1
          ! videoconvert
          ! queue
          ! video/x-raw,framerate=30/1,format=NV12
          ! v4l2h264enc extra-controls="controls,h264_profile=1,video_bitrate=1250000;"
          ! h264parse config-interval=3
          ! video/x-h264,stream-format=byte-stream,profile=constrained-baseline
      NEKO_VIDEO_CODEC: h264
```

--------------------------------

### Remmina: Neko Configuration via Environment Variable

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This example shows how to configure Remmina within a Neko Docker container using the `REMMINA_URL` environment variable. The variable specifies the connection protocol (e.g., `vnc`, `rdp`, `spice`) and server details.

```bash
docker run \
  -e REMMINA_URL=vnc://server:5900 \
  ghcr.io/m1k1o/neko/remmina
```

--------------------------------

### Configure Extension Management in Neko Browser

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Sets policies to manage browser extensions. By default, extension installation is blocked for all extensions. Specific extensions can be force-installed by providing their installation URL and ID, preventing user removal.

```json
{
  "policies": {
    "ExtensionSettings": {
      "*": {
        "installation_mode": "blocked"
      },
      "sponsorBlocker@ajay.app": {
        "install_url": "https://addons.mozilla.org/firefox/downloads/latest/sponsorblock/latest.xpi",
        "installation_mode": "force_installed"
      },
      "uBlock0@raymondhill.net": {
        "install_url": "https://addons.mozilla.org/firefox/downloads/latest/ublock-origin/latest.xpi",
        "installation_mode": "force_installed"
      }
    }
  }
}
```

--------------------------------

### Neko Raspberry Pi GPU Acceleration Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/examples.md

This Docker Compose configuration is tailored for Raspberry Pi, enabling GPU acceleration for Neko. It uses the Chromium image, adjusts SHM size, grants privileged access, and configures a specific video pipeline for H.264 encoding via v4l2h264enc.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/chromium:latest"
    restart: "unless-stopped"
    # increase on rpi's with more then 1gb ram.
    shm_size: "520mb"
    ports:
      - "8088:8080"
      - "52000-52100:52000-52100/udp"
    # note: this is important since we need a GPU for hardware acceleration alternatively
    #       mount the devices into the docker.
    privileged: true
    environment:
      NEKO_CAPTURE_VIDEO_PIPELINE: |
        ximagesrc display-name={display} show-pointer=true use-damage=false
          ! video/x-raw,framerate=25/1
          ! videoconvert ! queue
          ! video/x-raw,format=NV12
          ! v4l2h264enc
            name=encoder
            extra-controls="controls,h264_profile=1,video_bitrate=1250000;"
          ! h264parse config-interval=-1
          ! video/x-h264,stream-format=byte-stream
          ! appsink name=appsink
      NEKO_CAPTURE_VIDEO_CODEC: "h264"
      NEKO_DESKTOP_SCREEN: '1280x720@30'
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### Caddy Reverse Proxy Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Caddy configuration to serve Neko over HTTPS. It sets up a reverse proxy to localhost:8080, forwarding necessary headers like Host, X-Real-IP, X-Forwarded-For, and X-Forwarded-Proto.

```caddy
https://example.com {
  reverse_proxy localhost:8080 {
    header_up Host {host}
    header_up X-Real-IP {remote_host}
    header_up X-Forwarded-For {remote_host}
    header_up X-Forwarded-Proto {scheme}
  }
}
```

--------------------------------

### VP8 Encoder Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Provides example Gstreamer pipeline configurations for VP8 encoding in Neko. It includes settings for both high-quality (hq) and low-quality (lq) streams, specifying parameters like target bitrate, CPU usage, and buffer sizes for the `vp8enc` encoder.

```yaml
capture:
  video:
    codec: vp8
    ids: [ hq, lq ]
    pipelines:
      hq:
        gst_pipeline: |
          ximagesrc display-name={display} show-pointer=true use-damage=false
          ! videoconvert ! queue
          ! vp8enc
            name=encoder
            target-bitrate=3072000
            cpu-used=4
            end-usage=cbr
            threads=4
            deadline=1
            undershoot=95
            buffer-size=12288
            buffer-initial-size=6144
            buffer-optimal-size=9216
            keyframe-max-dist=25
            min-quantizer=4
            max-quantizer=20
          ! appsink name=appsink
      lq:
        gst_pipeline: |
          ximagesrc display-name={display} show-pointer=true use-damage=false
          ! videoconvert ! queue
          ! vp8enc
            name=encoder
            target-bitrate=1024000
            cpu-used=4
            end-usage=cbr
            threads=4
            deadline=1
            undershoot=95
            buffer-size=4096
            buffer-initial-size=2048
            buffer-optimal-size=3072
            keyframe-max-dist=25
            min-quantizer=4
            max-quantizer=20
          ! appsink name=appsink
```

--------------------------------

### Download Default Firefox Policy

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Downloads the default policy configuration file for Firefox browsers from a GitHub repository. This file serves as a starting point for customizing browser policies.

```curl
curl -o ./policy.json https://raw.githubusercontent.com/m1k1o/neko/refs/heads/main/apps/firefox/policies.json
```

--------------------------------

### H264 Encoder Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Provides example Gstreamer pipeline configurations for H264 encoding in Neko. It details settings for the `x264enc` encoder, including threads, bitrate, keyframe interval, and speed presets for optimal latency.

```yaml
capture:
  video:
    codec: h264
    ids: [ main ]
    pipelines:
      main:
        gst_pipeline: |
          ximagesrc display-name={display} show-pointer=true use-damage=false
          ! videoconvert ! queue
          ! x264enc
            name=encoder
            threads=4
            bitrate=4096
            key-int-max=15
            byte-stream=true
            tune=zerolatency
            speed-preset=veryfast
          ! video/x-h264,stream-format=byte-stream
          ! appsink name=appsink
```

--------------------------------

### Nginx configuration for Neko proxy

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/reverse-proxy-setup.md

Nginx server block configuration for proxying Neko traffic. Includes settings for SSL, HTTP/2, WebSocket upgrades, and header forwarding.

```conf
server {
  listen 443 ssl http2;
  server_name example.com;

  location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_cache_bypass $http_upgrade;
  }
}
```

--------------------------------

### Neko WebRTC Error: DTLS Transport Not Started

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

A WebRTC error indicating that the DTLS transport has not started yet, which can happen if UDP ports are not correctly exposed or reachable. Ensure UDP ports are open and accessible.

```log
WRN undeclaredMediaProcessor failed to open SrtcpSession: the DTLS transport has not started yet module=webrtc subsystem=
```

--------------------------------

### Get Screenshot Image API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/screen-shot-image.api.mdx

Retrieves the current screenshot image from the room. Supports specifying image quality via a query parameter. Returns the image as JPEG or an error message in JSON format for authentication, authorization, or server issues.

```APIDOC
GET /api/room/screen/shot.jpg

Description: Retrieve the current screenshot image.

Parameters:
  - name: quality
    in: query
    description: Image quality (0-100).
    required: false
    schema:
      type: integer

Responses:
  - status: 200
    description: Screenshot image retrieved successfully.
    content:
      image/jpeg:
        schema:
          type: string
          format: binary
  - status: 401
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  - status: 403
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  - status: 500
    description: Unable to create image.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Traefik2 Reverse Proxy Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Configuration for Traefik2 to proxy requests to the Neko service running on port 8080. Includes service port, router rules, entrypoints, and TLS certificate resolver.

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.services.neko-frontend.loadbalancer.server.port=8080"
  - "traefik.http.routers.neko.rule=${TRAEFIK_RULE}"
  - "traefik.http.routers.neko.entrypoints=${TRAEFIK_ENTRYPOINTS}"
  - "traefik.http.routers.neko.tls.certresolver=${TRAEFIK_CERTRESOLVER}"
```

--------------------------------

### Apache Reverse Proxy Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Apache configuration for reverse proxying to Neko. It forwards HTTP requests to port 8080 and handles WebSocket upgrades using mod_proxy_wstunnel. Includes instructions for enabling the site and reloading Apache, as well as guidance on enabling SSL with Certbot.

```apache
<VirtualHost *:80>
  LoadModule proxy_module /usr/lib/apache2/modules/mod_proxy.so
  LoadModule proxy_http_module /usr/lib/apache2/modules/mod_proxy_http.so
  LoadModule proxy_wstunnel_module /usr/lib/apache2/modules/mod_proxy_wstunnel.so

  ServerName example.com
  ServerAlias www.example.com

  ProxyRequests Off
  ProxyPass / http://localhost:8080/
  ProxyPassReverse / http://localhost:8080/

  RewriteEngine on
  RewriteCond %{HTTP:Upgrade} websocket [NC]
  RewriteCond %{HTTP:Connection} upgrade [NC]
  RewriteRule /ws(.*) "ws://localhost:8080/ws$1" [P,L]

  ErrorLog ${APACHE_LOG_DIR}/error.log
  CustomLog ${APACHE_LOG_DIR}/access.log combined
</VirtualHost>

# To enable SSL, install certbot and run 'sudo certbot', selecting your domains.
```

--------------------------------

### Neko Docker Compose with TURN Server

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Example Docker Compose configuration for Neko, integrating a local coturn server for enhanced NAT traversal. It specifies Neko image, ports, and environment variables including ICE server configuration.

```yaml
version: "3.4"

services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_ICESERVERS: |
        [{
          "urls": [ "turn:<MY-COTURN-SERVER>:3478" ],
          "username": "neko",
          "credential": "neko"
        },{
          "urls": [ "stun:stun.nextcloud.com:3478" ]
        }]
  coturn:
    image: 'coturn/coturn:latest'
    network_mode: "host"
    command: |
      -n
      --realm=localhost
      --fingerprint
      --listening-ip=0.0.0.0
      --external-ip=<MY-COTURN-SERVER>
      --listening-port=3478
      --min-port=49160
      --max-port=49200
      --log-file=stdout
      --user=neko:neko
      --lt-cred-mech
```

--------------------------------

### Configure Static External IP Address

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Example docker-compose.yaml snippet demonstrating how to manually set the server's external IP address using the NEKO_WEBRTC_NAT1TO1 environment variable, useful for static IP configurations.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      # highlight-start
      NEKO_WEBRTC_NAT1TO1: <your-IP>
      # highlight-end
```

--------------------------------

### GET /api/members - List Members

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-list.api.mdx

Retrieves a list of all members from the system. Supports pagination via 'limit' and 'offset' query parameters.

```APIDOC
GET /api/members

Retrieve a list of all members.

Parameters:
  - limit (number, optional): The maximum number of members to return. Defaults to a system-defined value if not provided.
  - offset (number, optional): The number of members to skip before starting to collect the result set. Defaults to 0 if not provided.

Returns:
  - A list of member objects. The exact structure of member objects is not detailed here but typically includes identifiers and associated data.

Example Usage:
  GET /api/members?limit=10&offset=20
```

--------------------------------

### GET /api/sessions - List Sessions

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/sessions-get.api.mdx

Retrieves a list of all active sessions. This endpoint does not require any parameters or a request body.

```APIDOC
GET /api/sessions
  Description: Retrieve a list of all active sessions.
  Parameters:
    None
  Request Body:
    None
  Responses:
    200 OK: List of sessions
    401 Unauthorized: Authentication failed
    500 Internal Server Error: Server error
```

--------------------------------

### Neko Display and Video Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags for configuring screen capture, video codecs, and streaming parameters.

```APIDOC
--display string
  XDisplay to capture

--video_codec string
  video codec to be used

--av1
  DEPRECATED: use video_codec

--h264
  DEPRECATED: use video_codec

--vp8
  DEPRECATED: use video_codec

--vp9
  DEPRECATED: use video_codec

--video string
  video codec parameters to use for streaming

--video_bitrate int
  video bitrate in kbit/s

--hwenc string
  use hardware accelerated encoding

--max_fps int
  maximum fps delivered via WebRTC, 0 is for no maximum
```

--------------------------------

### Neko Docker Compose with TCP/UDP Mux

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Demonstrates a Docker Compose setup for Neko utilizing TCP and UDP multiplexing (mux) for WebRTC communication. This approach reduces the number of required UDP ports.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "8081:8081/tcp"
    - "8082:8082/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_TCPMUX: 8081
      NEKO_UDPMUX: 8082
      NEKO_ICELITE: 1
```

--------------------------------

### GET /metrics API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/metrics.api.mdx

Retrieves metrics for the API. Returns a 200 status code upon successful retrieval.

```APIDOC
GET /metrics
  Description: Retrieve metrics for the API.
  Responses:
    200:
      description: Metrics retrieved successfully.
```

--------------------------------

### GET /api/room/keyboard/modifiers - Keyboard Modifiers API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/keyboard-modifiers-get.api.mdx

Retrieves the current status of keyboard modifier keys (Shift, Caps Lock, Control, Alt, Num Lock, Meta, Super, AltGr). The endpoint returns a JSON object detailing the state of each key. Authentication is required, and unauthorized or forbidden access will result in specific error responses.

```APIDOC
GET /api/room/keyboard/modifiers

Description:
  Retrieve the current keyboard modifiers status.

Parameters:
  (None)

Responses:
  200 OK:
    Description: Keyboard modifiers retrieved successfully.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            shift:
              type: boolean
              description: Indicates if the shift key is pressed.
            capslock:
              type: boolean
              description: Indicates if the caps lock key is active.
            control:
              type: boolean
              description: Indicates if the control key is pressed.
            alt:
              type: boolean
              description: Indicates if the alt key is pressed.
            numlock:
              type: boolean
              description: Indicates if the num lock key is active.
            meta:
              type: boolean
              description: Indicates if the meta key is pressed.
            super:
              type: boolean
              description: Indicates if the super key is pressed.
            altgr:
              type: boolean
              description: Indicates if the altgr key is pressed.
          title: KeyboardModifiers

  401 Unauthorized:
    Description: The request requires user authentication.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  403 Forbidden:
    Description: The server understood the request, but refuses to authorize it.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Configure Custom IP Retrieval URL

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Example docker-compose.yaml snippet showing how to set the NEKO_WEBRTC_IP_RETRIEVAL_URL environment variable to specify a custom service for determining the server's public IP address.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      # highlight-start
      NEKO_WEBRTC_IP_RETRIEVAL_URL: https://ifconfig.co/ip
      # highlight-end
```

--------------------------------

### Nginx Reverse Proxy Setup

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Nginx configuration to act as a reverse proxy for Neko. It handles SSL, HTTP/2, proxies requests to localhost:8080, and manages WebSocket upgrades with appropriate headers.

```nginx
server {
  listen 443 ssl http2;
  server_name example.com;

  location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 86400;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Host $host;
    proxy_set_header X-Forwarded-Port $server_port;
    proxy_set_header X-Forwarded-Protocol $scheme;
  }
}
```

--------------------------------

### Clone Neko Repository

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/developer-guide/build.md

Clones the Neko Git repository to your local machine and navigates into the project directory.

```bash
git clone https://github.com/m1k1o/neko.git
cd neko
```

--------------------------------

### NVENC H264 Encoder Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Provides example Gstreamer pipeline configurations for H264 encoding using NVIDIA's NVENC hardware accelerator in Neko. This configuration requires an NVIDIA GPU and NVENC support, specifying parameters like preset, GOP size, bitrate, and VBV buffer size for `nvh264enc`.

```yaml
capture:
  video:
    codec: h264
    ids: [ main ]
    pipelines:
      main:
        gst_pipeline: |
          ximagesrc display-name={display} show-pointer=true use-damage=false
          ! videoconvert ! queue
          ! video/x-raw,format=NV12
          ! nvh264enc
            name=encoder
            preset=2
            gop-size=25
            spatial-aq=true
            temporal-aq=true
            bitrate=4096
            vbv-buffer-size=4096
            rc-mode=6
          ! h264parse config-interval=-1
          ! video/x-h264,stream-format=byte-stream
          ! appsink name=appsink
```

--------------------------------

### Health Check Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/healthcheck.api.mdx

Provides a GET request to check the health status of the API. It returns a 200 OK status if the API is healthy. No parameters are required for this endpoint.

```APIDOC
GET /health
  - Checks the health status of the API.
  - Returns: 200 OK (The API is healthy.)
```

--------------------------------

### API Response Schemas and Status Codes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/session-get.api.mdx

Defines the structure of successful and error responses for the API. Includes details on session data, member profiles, and common error messages for authentication and resource not found scenarios.

```APIDOC
API Responses:

200 OK
  Description: Session retrieved successfully.
  Content: application/json
  Schema:
    type: object
    properties:
      id:
        type: string
        description: The unique identifier of the session.
      profile:
        type: object
        description: The profile information of the user.
        x-tags: ["members"]
        properties:
          name:
            type: string
            description: The name of the member.
          is_admin:
            type: boolean
            description: Indicates if the member is an admin.
          can_login:
            type: boolean
            description: Indicates if the member can log in.
          can_connect:
            type: boolean
            description: Indicates if the member can connect.
          can_watch:
            type: boolean
            description: Indicates if the member can watch.
          can_host:
            type: boolean
            description: Indicates if the member can host.
          can_share_media:
            type: boolean
            description: Indicates if the member can share media.
          can_access_clipboard:
            type: boolean
            description: Indicates if the member can access the clipboard.
          sends_inactive_cursor:
            type: boolean
            description: Indicates if the member sends inactive cursor.
          can_see_inactive_cursors:
            type: boolean
            description: Indicates if the member can see inactive cursors.
          plugins:
            type: object
            additionalProperties: true
            description: Additional plugin settings.
        title: MemberProfile
      state:
        type: object
        description: The current state of the session.
        properties:
          is_connected:
            type: boolean
            description: Indicates if the user is connected.
          is_watching:
            type: boolean
            description: Indicates if the user is watching.
        title: SessionState
    title: SessionData

401 Unauthorized
  Description: The request requires user authentication.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
    title: ErrorMessage

403 Forbidden
  Description: The server understood the request, but refuses to authorize it.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
    title: ErrorMessage

404 Not Found
  Description: The specified resource was not found.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
    title: ErrorMessage
```

--------------------------------

### Neko Server Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags related to server operation, logging, security, and client serving.

```APIDOC
--legacy
  enable legacy mode (default true)

--logs
  save logs to file

--cert string
  path to the SSL cert used to secure the neko server

--key string
  path to the SSL key used to secure the neko server

--bind string
  address/port/socket to serve neko

--proxy
  enable reverse proxy mode

--static string
  path to neko client files to serve

--path_prefix string
  path prefix for HTTP requests

--cors strings
  list of allowed origins for CORS

--locks strings
  resources, that will be locked when starting (control, login)

--implicit_control
  if enabled members can gain control implicitly

--control_protection
  control protection means, users can gain control only if at least one admin is in the room

--heartbeat_interval int
  heartbeat interval in seconds (default 120)

--file_transfer_enabled
  enable file transfer feature

--file_transfer_path string
  path to use for file transfer
```

--------------------------------

### Neko Broadcasting Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags for setting up and managing broadcasting streams.

```APIDOC
--broadcast_pipeline string
  custom gst pipeline used for broadcasting, strings {url} {device} {display} will be replaced

--broadcast_url string
  a default default URL for broadcast streams, can be disabled/changed later by admins in the GUI

--broadcast_autostart
  automatically start broadcasting when neko starts and broadcast_url is set
```

--------------------------------

### Neko Project API Responses (200, 401, 403)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/whoami.api.mdx

Defines the structure of API responses for the Neko project, including successful retrieval of user session data (200 OK) and error responses for authentication (401 Unauthorized) and authorization (403 Forbidden).

```APIDOC
API Responses for Neko Project:

Status Code 200 (OK):
  Description: Current user information retrieved successfully.
  Content:
    application/json:
      Schema: SessionData
        type: object
        properties:
          id: { type: string, description: "The unique identifier of the session." }
          profile: { type: object, description: "The profile information of the user.", x-tags: ["members"], title: MemberProfile, properties: {
            name: { type: string, description: "The name of the member." },
            is_admin: { type: boolean, description: "Indicates if the member is an admin." },
            can_login: { type: boolean, description: "Indicates if the member can log in." },
            can_connect: { type: boolean, description: "Indicates if the member can connect." },
            can_watch: { type: boolean, description: "Indicates if the member can watch." },
            can_host: { type: boolean, description: "Indicates if the member can host." },
            can_share_media: { type: boolean, description: "Indicates if the member can share media." },
            can_access_clipboard: { type: boolean, description: "Indicates if the member can access the clipboard." },
            sends_inactive_cursor: { type: boolean, description: "Indicates if the member sends inactive cursor." },
            can_see_inactive_cursors: { type: boolean, description: "Indicates if the member can see inactive cursors." },
            plugins: { type: object, additionalProperties: true, description: "Additional plugin settings." }
          }}
          state: { type: object, description: "The current state of the session.", title: SessionState, properties: {
            is_connected: { type: boolean, description: "Indicates if the user is connected." },
            is_watching: { type: boolean, description: "Indicates if the user is watching." }
          }}
        title: SessionData

Status Code 401 (Unauthorized):
  Description: The request requires user authentication.
  Content:
    application/json:
      Schema: ErrorMessage
        type: object
        properties:
          message: { type: string, description: "Detailed error message." }
        title: ErrorMessage

Status Code 403 (Forbidden):
  Description: The server understood the request, but refuses to authorize it.
  Content:
    application/json:
      Schema: ErrorMessage
        type: object
        properties:
          message: { type: string, description: "Detailed error message." }
        title: ErrorMessage
```

--------------------------------

### Configure UDP/TCP Multiplexing Ports

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Configures the single port used for multiplexing multiple UDP and TCP connections. This port must be accessible on the server's firewall. The example demonstrates setting these via environment variables in docker-compose.yaml.

```APIDOC
webrtc.udpmux: <port>
  - Description: The port used for multiplexing UDP connections.
  - Example: 59000

webrtc.tcpmux: <port>
  - Description: The port used for multiplexing TCP connections.
  - Example: 59000
  - Notes: UDP is preferred for latency, but TCP can be used as a fallback. Ensure the specified port is open on the server's firewall.
```

```yaml
environment:
  NEKO_WEBRTC_UDPMUX: "59000"
  NEKO_WEBRTC_TCPMUX: "59000"
ports:
  - "59000:59000/udp"
  - "59000:59000/tcp"
```

--------------------------------

### Configure IP Retrieval URL

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Sets a URL from which the server will retrieve its public IP address if it cannot be automatically resolved. This is useful for servers behind complex NAT configurations. The example shows configuration via a filter.

```APIDOC
webrtc.ip_retrieval_url: "<url>"
  - Description: A URL to fetch the server's public IP address from.
  - Example: "https://api.ipify.org"
  - Notes: The server performs an HTTP GET request to this URL to determine its public IP address.
```

--------------------------------

### Configure Ephemeral UDP Port Range (EPR)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Sets the range of UDP ports used by the server for establishing peer-to-peer connections with clients. This range must be open on the server's firewall. The example shows how to configure this via environment variables in docker-compose.yaml.

```APIDOC
webrtc.epr: "<start_port>-<end_port>"
  - Description: The range of UDP ports for ephemeral connections.
  - Example: "59000-59100"
  - Notes: Ensure this range is open on the server's firewall and matches the port mapping in your deployment configuration.
```

```yaml
environment:
  NEKO_WEBRTC_EPR: "59000-59100"
ports:
  - "59000-59100:59000-59100/udp"
```

--------------------------------

### Member Profile Retrieval API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-get-profile.api.mdx

This API documentation describes the responses for retrieving a member's profile. It includes the schema for a successful retrieval (200 OK) and common error scenarios like unauthorized (401), forbidden (403), and not found (404) responses.

```APIDOC
GET /members/{memberId} (Example Endpoint)

Responses:

200 OK - MemberProfile
  Description: Member profile retrieved successfully.
  Content: application/json
  Schema:
    type: object
    properties:
      name:
        type: string
        description: The name of the member.
      is_admin:
        type: boolean
        description: Indicates if the member is an admin.
      can_login:
        type: boolean
        description: Indicates if the member can log in.
      can_connect:
        type: boolean
        description: Indicates if the member can connect.
      can_watch:
        type: boolean
        description: Indicates if the member can watch.
      can_host:
        type: boolean
        description: Indicates if the member can host.
      can_share_media:
        type: boolean
        description: Indicates if the member can share media.
      can_access_clipboard:
        type: boolean
        description: Indicates if the member can access the clipboard.
      sends_inactive_cursor:
        type: boolean
        description: Indicates if the member sends inactive cursor.
      can_see_inactive_cursors:
        type: boolean
        description: Indicates if the member can see inactive cursors.
      plugins:
        type: object
        additionalProperties: true
        description: Additional plugin settings.

401 Unauthorized
  Description: The request requires user authentication.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.

403 Forbidden
  Description: The server understood the request, but refuses to authorize it.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.

404 Not Found
  Description: The specified resource was not found.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
```

--------------------------------

### Neko Screen Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flag for setting the default screen resolution and framerate.

```APIDOC
--screen string
  default screen resolution and framerate
```

--------------------------------

### Server Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Configures the Neko server, including SSL, metrics, path prefixes, and static file serving.

```config
--server.key string
  path to the SSL key used to secure the neko server
--server.metrics
  enable prometheus metrics available at /metrics (default true)
--server.path_prefix string
  path prefix for HTTP requests (default "/")
--server.pprof
  enable pprof endpoint available at /debug/pprof
--server.proxy
  trust reverse proxy headers
--server.static string
  path to neko client files to serve
```

--------------------------------

### Neko Audio Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags for configuring audio capture devices, audio codecs, and streaming parameters.

```APIDOC
--device string
  audio device to capture

--audio_codec string
  audio codec to be used

--g722
  DEPRECATED: use audio_codec

--opus
  DEPRECATED: use audio_codec

--pcma
  DEPRECATED: use audio_codec

--pcmu
  DEPRECATED: use audio_codec

--audio string
  audio codec parameters to use for streaming

--audio_bitrate int
  audio bitrate in kbit/s
```

--------------------------------

### Deploy Website to GitHub Pages

Source: https://github.com/m1k1o/neko/blob/master/webpage/README.md

Deploys the built website to GitHub Pages. Supports deployment with or without SSH, pushing to the 'gh-pages' branch.

```shell
$ USE_SSH=true npm run deploy
```

```shell
$ GIT_USER=<Your GitHub username> npm run deploy
```

--------------------------------

### Neko Configuration Environment Variables

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details environment variables used to configure Neko's server-side behavior, including path prefixes, codecs, and control settings.

```bash
NEKO_PATH_PREFIX
  Description: Sets a path prefix for Neko's routes.

NEKO_AUDIO_CODEC=
NEKO_VIDEO_CODEC=
  Description: Specifies the audio and video codecs to be used by Neko.

NEKO_IMPLICITCONTROL=1
  Description: Enables implicit control, allowing users to gain control without explicit requests.

NEKO_BROADCAST_URL=rtmp://your-rtmp-endpoint/live
  Description: Automatically starts broadcasting the Neko stream to the specified RTMP endpoint.

NEKO_LOCKS=control login
  Description: Configures lock settings, such as preventing users from taking control or logging in.

NEKO_CONTROL_PROTECTION=true
  Description: Enables control protection, requiring at least one admin to be present in the room for users to gain control.
```

--------------------------------

### Neko WebRTC and Audio Settings

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Highlights specific WebRTC and audio configurations and fixes implemented in Neko.

```APIDOC
WebRTC & Audio Configurations:

Opus SDP Configuration:
  - `stereo=1`: Added to opus SDP to clients answer to fix stereo problems in chromium-based browsers.

WebRTC ICE Gathering:
  - Fixed client-side WebRTC ICE gathering to allow Neko usage with only STUN and TURN servers, without exposed ports.

WebRTC Connection:
  - UDP and TCP mux for WebRTC connection to handle multiple peers.
  - Automatic WebRTC SDP negotiation using onnegotiationneeded handlers for adding/removing tracks on demand.

Opus FEC:
  - Opus now uses `useinbandfec=1` to potentially fix minor audio loss issues.
```

--------------------------------

### Logging Configuration Options

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Sets up the logging system, including log directory, format, level, colorization, and timestamp format.

```APIDOC
log.dir: string
  Description: Directory to store logs. If empty, logs are written to stdout. Useful for containerized Neko.

log.json: boolean
  Description: When true, logs are written in JSON format.

log.level: string
  Description: Log level. Available levels: 'trace', 'debug', 'info', 'warn', 'error', 'fatal', 'panic', 'disabled'.

log.nocolor: boolean
  Description: When true, ANSI colors are disabled in non-JSON output. Also accepts 'NO_COLOR' environment variable.

log.time: string
  Description: Time format used in logs. Available formats: 'unix', 'unixms', 'unixmicro'.
```

--------------------------------

### Desktop Capture Configuration (V2 vs V3)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Compares V2 and V3 environment variables related to desktop capture, specifically focusing on screen selection.

```env
V2 Configuration: NEKO_SCREEN
V3 Configuration: NEKO_DESKTOP_SCREEN
```

--------------------------------

### Supported CPU Architectures

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Details the CPU architectures for which Neko Docker images are built and supported. This information is crucial for selecting the correct image for your specific hardware, ensuring compatibility and optimal performance.

```bash
# Supported architectures for Neko Docker images:
# linux/amd64 - 64-bit Intel/AMD (most common)
# linux/arm64 - 64-bit ARM (e.g., Raspberry Pi 4, Apple M1/M2)
# linux/arm/v7 - 32-bit ARM (e.g., Raspberry Pi 3, Raspberry Pi Zero)
```

--------------------------------

### Copy Default Supervisord Config

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/README.md

Bash commands to create a temporary Docker container, copy the default supervisord configuration file for a specific application (e.g., Firefox) from the container to the local machine, and then remove the temporary container.

```bash
# Create a container without starting it
docker create --name neko ghcr.io/m1k1o/neko/firefox:latest
# Copy the default configuration file to your local machine
docker cp neko:/etc/neko/supervisord/firefox.conf ./firefox.conf
# Remove the container
docker rm -f neko
```

--------------------------------

### Neko Docker Image Tags

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Lists available Docker image tags for Neko, providing specific browser or desktop environment configurations.

```dockerfile
m1k1o/neko:kde
m1k1o/neko:xfce
m1k1o/neko:vivaldi
m1k1o/neko:opera
m1k1o/neko:microsoft-edge
m1k1o/neko:remmina
```

--------------------------------

### Neko Plugin Configuration Options

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/plugins.md

Defines general configuration settings for Neko's plugin system. It covers enabling plugins, making them required for startup, and specifying the directory for plugin files.

```yaml
plugins:
  enabled: true
  required: false
  dir: "./plugins"
```

--------------------------------

### Neko Serve Command Line Arguments

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

Details the various command-line flags available for the 'neko serve' command. These flags control audio/video codecs, network binding, broadcasting, security, file transfer, and display capture settings.

```APIDOC
Neko Serve Command:
  neko serve [flags]

Description:
  Starts the Neko server to stream desktop audio and video, with options for file transfer and broadcasting.

Flags:
  --audio string
    Audio codec parameters to use for streaming.
  --audio_bitrate int
    Audio bitrate in kbit/s (default 128).
  --audio_codec string
    Audio codec to be used (default "opus").
  --bind string
    Address/port/socket to serve Neko (default "127.0.0.1:8080").
  --broadcast_pipeline string
    Custom GST pipeline used for broadcasting. Strings {url}, {device}, {display} will be replaced.
  --broadcast_url string
    URL for broadcasting. Setting this value automatically enables broadcasting.
  --cert string
    Path to the SSL cert used to secure the Neko server.
  --control_protection
    Control protection means users can gain control only if at least one admin is in the room.
  --cors strings
    List of allowed origins for CORS (default [*]).
  --device string
    Audio device to capture (default "audio_output.monitor").
  --display string
    XDisplay to capture (default ":99.0").
  --epr string
    Limits the pool of ephemeral ports that ICE UDP connections can allocate from (default "59000-59100").
  --file_transfer_enabled
    Enable file transfer feature (default false).
  --file_transfer_path string
    Path to use for file transfer (default "/home/neko/Downloads").
  --g722
    DEPRECATED: use --audio_codec.
  -h, --help
    Help for serve.
  --hwenc string
    Use hardware accelerated encoding.
  --icelite
    Configures whether or not the ICE agent should be a lite agent.
  --iceserver strings
    Describes a single STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer (default [stun:stun.l.google.com:19302]).
  --iceservers string
    Describes a single STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer.
  --implicit_control
    If enabled, members can gain control implicitly.
  --ipfetch string
    Automatically fetch IP address from given URL when nat1to1 is not present (default "http://checkip.amazonaws.com").
  --key string
    Path to the SSL key used to secure the Neko server.
  --locks strings
    Resources that will be locked when starting (control, login).
  --max_fps int
    Maximum FPS delivered via WebRTC, 0 is for no maximum (default 25).
  --nat1to1 strings
    Sets a list of external IP addresses of 1:1 (D)NAT and a candidate type for which the external IP address is used.
  --opus
    DEPRECATED: use --audio_codec.
  --password string
    Password for connecting to stream (default "neko").
  --password_admin string
    Admin password for connecting to stream (default "admin").
  --path_prefix string
    Path prefix for HTTP requests (default "/").
  --pcma
    DEPRECATED: use --audio_codec.
  --pcmu
    DEPRECATED: use --audio_codec.
  --proxy
    Enable reverse proxy mode.
  --screen string
    Default screen resolution and framerate (default "1280x720@30").
  --static string
    Path to Neko client files to serve (default "./www").
  --tcpmux int
    Single TCP mux port for all peers.
  --udpmux int
    Single UDP mux port for all peers.
  --video string
    Video codec parameters to use for streaming.
  --video_bitrate int
    Video bitrate in kbit/s (default 3072).
  --video_codec string
    Video codec to be used (default "vp8").
  --vp8
    DEPRECATED: use --video_codec.
  --vp9
    DEPRECATED: use --video_codec.

Global Flags:
  --config string
    Configuration file path.
  -d, --debug
    Enable debug mode.
  -l, --logs
    Save logs to file.
```

--------------------------------

### Broadcast Configuration (V2 vs V3)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Details the mapping of V2 broadcast configuration environment variables to their V3 equivalents. Covers pipeline settings, broadcast URLs, and autostart functionality.

```env
V2 Configuration: NEKO_BROADCAST_PIPELINE
V3 Configuration: NEKO_CAPTURE_BROADCAST_PIPELINE

V2 Configuration: NEKO_BROADCAST_URL
V3 Configuration: NEKO_CAPTURE_BROADCAST_URL

V2 Configuration: NEKO_BROADCAST_AUTOSTART
V3 Configuration: NEKO_CAPTURE_BROADCAST_AUTOSTART
```

--------------------------------

### Remmina: Neko Configuration via Bind-Mount

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This method configures Remmina by bind-mounting a custom configuration file into the container and setting the `REMMINA_PROFILE` environment variable. The `.remmina` file contains the connection details.

```ini
[remmina]
name=Default
protocol=VNC
server=server.local
port=5900
```

```bash
docker run \
  -v /path/to/default.remmina:/root/.local/share/remmina/default.remmina \
  -e REMMINA_PROFILE=/root/.local/share/remmina/default.remmina \
  ghcr.io/m1k1o/neko/remmina
```

--------------------------------

### Neko Media Streaming Backends

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/roadmap.md

Outlines the different backends available for media streaming in Neko, detailing their functionalities and potential features. This abstraction allows for flexible media handling based on device, network, and server capabilities.

```APIDOC
NekoMediaStreaming:
  Backends:
    - none: No media streaming is currently streamed.
    - m3u8: Media is streamed using HLS (HTTP Live Streaming).
    - webrtc: Media is streamed using WebRTC.
    - quic: Media is streamed using QUIC.
    - ... others (e.g. RTSP, DASH...)
  Features:
    - WebRTC can send media to the server.
    - HTTP can only receive media from the server.
  Interface:
    All streaming backends must satisfy a single interface for communication with the rest of the system.
```

--------------------------------

### Build Static Website Content

Source: https://github.com/m1k1o/neko/blob/master/webpage/README.md

Generates the static website files, typically into a 'build' directory. These files can then be deployed to any hosting service.

```shell
$ npm run build
```

--------------------------------

### Neko Password Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/help.txt

Flags for setting passwords for connecting to the stream and for administrative access.

```APIDOC
--password string
  password for connecting to stream

--password_admin string
  admin password for connecting to stream
```

--------------------------------

### WebRTC Video Configuration (V2 vs V3)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Compares V2 and V3 environment variables for configuring WebRTC video capture. Highlights new variable names, deprecated settings, and changes like the removal of fixed bitrate and hardware encoding options in favor of custom pipelines.

```env
V2 Configuration: NEKO_DISPLAY
V3 Configuration: NEKO_CAPTURE_VIDEO_DISPLAY and NEKO_DESKTOP_DISPLAY (consider using DISPLAY env variable if both should be the same)

V2 Configuration: NEKO_VIDEO_CODEC
V3 Configuration: NEKO_CAPTURE_VIDEO_CODEC

V2 Configuration: NEKO_AV1=true (deprecated)
V3 Configuration: NEKO_CAPTURE_VIDEO_CODEC=av1

V2 Configuration: NEKO_H264=true (deprecated)
V3 Configuration: NEKO_CAPTURE_VIDEO_CODEC=h264

V2 Configuration: NEKO_VP8=true (deprecated)
V3 Configuration: NEKO_CAPTURE_VIDEO_CODEC=vp8

V2 Configuration: NEKO_VP9=true (deprecated)
V3 Configuration: NEKO_CAPTURE_VIDEO_CODEC=vp9

V2 Configuration: NEKO_VIDEO
V3 Configuration: NEKO_CAPTURE_VIDEO_PIPELINE (V3 allows multiple video pipelines)

V2 Configuration: NEKO_VIDEO_BITRATE (removed)
V3 Configuration: Use custom pipeline instead

V2 Configuration: NEKO_HWENC (removed)
V3 Configuration: Use custom pipeline instead

V2 Configuration: NEKO_MAX_FPS (removed)
V3 Configuration: Use custom pipeline instead
```

--------------------------------

### Embed Neko Desktop in Web Page (URL/HTML)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/faq.md

This entry provides instructions on how to embed the Neko desktop into a web page without triggering a login prompt for viewers. It includes a specific URL format and required iframe attributes for fullscreen embedding.

```text
URL Format:
http://<your-neko-server-ip>:8080/?usr=neko&pwd=neko

Iframe Attributes:
allowfullscreen="true" webkitallowfullscreen="true" mozallowfullscreen="true"
or
allow="fullscreen *"
(Replace '*' with your iframe origin if needed for security).
```

--------------------------------

### Neko Project CLI Configuration and Logging Flags

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

This section details the command-line flags available for configuring the Neko project. It covers options for specifying configuration files, enabling debug mode, and controlling various aspects of the logging system, such as directory, format, level, colorization, and timestamp format.

```APIDOC
CLI Flags:

-c, --config string
  description: configuration file path
  type: string

-d, --debug
  description: enable debug mode
  type: boolean

--log.dir string
  description: logging directory to store logs
  type: string

--log.json
  description: logs in JSON format
  type: boolean

--log.level string
  description: set log level (trace, debug, info, warn, error, fatal, panic, disabled) (default "info")
  type: string
  constraints: one of [trace, debug, info, warn, error, fatal, panic, disabled]
  default: "info"

--log.nocolor
  description: no ANSI colors in non-JSON output
  type: boolean

--log.time string
  description: time format used in logs (unix, unixms, unixmicro) (default "unix")
  type: string
  constraints: one of [unix, unixms, unixmicro]
  default: "unix"
```

--------------------------------

### Neko Desktop Interaction Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Manages desktop interaction settings for Neko, including display selection, file chooser dialog handling, custom input drivers for touchscreens, default screen resolution, window unminimization, and upload drop functionality.

```config
--desktop.display string
  X display to use for desktop sharing
--desktop.file_chooser_dialog
  whether to handle file chooser dialog externally
--desktop.input.enabled
  whether custom xf86 input driver should be used to handle touchscreen (default true)
--desktop.input.socket string
  socket path for custom xf86 input driver connection (default "/tmp/xf86-input-neko.sock")
--desktop.screen string
  default screen size and framerate (default "1280x720@30")
--desktop.unminimize
  automatically unminimize window when it is minimized (default true)
--desktop.upload_drop
  whether drop upload is enabled (default true)
```

--------------------------------

### Troubleshoot Black Screen with Neko (YAML)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/faq.md

This YAML snippet addresses the issue of a black screen in Chromium-based browsers while the remote cursor is still moving. It shows the necessary `cap_add` directive to include `SYS_ADMIN` in the Docker Compose configuration, which is often required for proper rendering.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:chromium"
    # highlight-start
    cap_add:
    - SYS_ADMIN
    # highlight-end
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: '1920x1080@30'
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
```

--------------------------------

### X264 Gstreamer Pipeline for Broadcast

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

This YAML configuration defines a Gstreamer pipeline for broadcasting audio and video using the x264 encoder. It specifies audio input from pulseaudio, video input from X display, and output via RTMP. Placeholders like {device}, {display}, and {url} are used for dynamic configuration.

```yaml
capture:
  broadcast:
    pipeline: |
      flvmux name=mux
        ! rtmpsink location={url}
      pulsesrc device={device}
        ! audio/x-raw,channels=2
        ! audioconvert
        ! voaacenc
        ! mux.
      ximagesrc display-name={display} show-pointer=false use-damage=false
        ! video/x-raw,framerate=28/1
        ! videoconvert
        ! queue
        ! x264enc bframes=0 key-int-max=0 byte-stream=true tune=zerolatency speed-preset=veryfast
        ! mux.

```

--------------------------------

### WebRTC Audio Configuration (V2 vs V3)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Compares V2 and V3 environment variables for configuring WebRTC audio capture. Details changes in audio device and codec settings, including deprecated codec flags and the shift towards custom audio pipelines.

```env
V2 Configuration: NEKO_DEVICE
V3 Configuration: NEKO_CAPTURE_AUDIO_DEVICE

V2 Configuration: NEKO_AUDIO_CODEC
V3 Configuration: NEKO_CAPTURE_AUDIO_CODEC

V2 Configuration: NEKO_G722=true (deprecated)
V3 Configuration: NEKO_CAPTURE_AUDIO_CODEC=g722

V2 Configuration: NEKO_OPUS=true (deprecated)
V3 Configuration: NEKO_CAPTURE_AUDIO_CODEC=opus

V2 Configuration: NEKO_PCMA=true (deprecated)
V3 Configuration: NEKO_CAPTURE_AUDIO_CODEC=pcma

V2 Configuration: NEKO_PCMU=true (deprecated)
V3 Configuration: NEKO_CAPTURE_AUDIO_CODEC=pcmu

V2 Configuration: NEKO_AUDIO
V3 Configuration: NEKO_CAPTURE_AUDIO_PIPELINE

V2 Configuration: NEKO_AUDIO_BITRATE (removed)
V3 Configuration: Use custom pipeline instead
```

--------------------------------

### Intel GPU Flavors (VAAPI)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Lists Neko Docker image flavors that support Intel GPU hardware acceleration using VAAPI. These images are specifically for the linux/amd64 architecture and provide enhanced graphics performance for compatible Intel hardware.

```bash
# Intel GPU accelerated applications
# Only for architecture linux/amd64
# Base image: ghcr.io/m1k1o/neko/intel-base

# Browsers:
# ghcr.io/m1k1o/neko/intel-firefox
# ghcr.io/m1k1o/neko/intel-waterfox
# ghcr.io/m1k1o/neko/intel-chromium
# ghcr.io/m1k1o/neko/intel-google-chrome
# ghcr.io/m1k1o/neko/intel-ungoogled-chromium
# ghcr.io/m1k1o/neko/intel-microsoft-edge
# ghcr.io/m1k1o/neko/intel-brave
# ghcr.io/m1k1o/neko/intel-vivaldi
# ghcr.io/m1k1o/neko/intel-opera
# ghcr.io/m1k1o/neko/intel-tor-browser

# Utilities:
# ghcr.io/m1k1o/neko/intel-remmina
# ghcr.io/m1k1o/neko/intel-vlc
# ghcr.io/m1k1o/neko/intel-xfce
# ghcr.io/m1k1o/neko/intel-kde
```

--------------------------------

### Basic Gstreamer Pipeline Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Defines the structure for setting a Gstreamer pipeline description in `config.yaml`. It shows where to place the `gst_pipeline` parameter for a specific pipeline ID, allowing custom Gstreamer pipelines for video capture.

```yaml
capture:
  video:
    ...
    pipelines:
      <pipeline_id>:
        gst_pipeline: "<gstreamer_pipeline>"
```

--------------------------------

### Expression-Driven Configuration Structure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Defines the structure for expression-driven video pipeline configuration, including dynamic resolution, framerate, and GStreamer parameters. Expressions are evaluated using the gval library.

```yaml
capture:
  video:
    ...
    pipelines:
      <pipeline_id>:
        width: "<expression>"
        height: "<expression>"
        fps: "<expression>"
        gst_prefix: "<gst_pipeline>"
        gst_encoder: "<gst_encoder_name>"
        gst_params:
          <param_name>: "<expression>"
        gst_suffix: "<gst_pipeline>"
        show_pointer: true
```

--------------------------------

### Gstreamer Video Encoder Plugins

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

A table outlining common video codecs and their associated Gstreamer encoder plugins. It includes links to documentation for C implementations of these encoders, such as vp8enc, vp9enc, av1enc, x264enc, and x265enc, along with hardware-accelerated versions like vaapivp8enc and nvh264enc.

```text
| VP8   | [vp8enc](https://gstreamer.freedesktop.org/documentation/vpx/vp8enc.html?gi-language=c) | [vaapivp8enc](https://github.com/GStreamer/gstreamer-vaapi/blob/master/gst/vaapi/gstvaapiencode_vp8.c) | ? |
| VP9   | [vp9enc](https://gstreamer.freedesktop.org/documentation/vpx/vp9enc.html?gi-language=c) | [vaapivp9enc](https://github.com/GStreamer/gstreamer-vaapi/blob/master/gst/vaapi/gstvaapiencode_vp9.c) | ? |
| AV1   | [av1enc](https://gstreamer.freedesktop.org/documentation/aom/av1enc.html?gi-language=c) | ? | [nvav1enc](https://gstreamer.freedesktop.org/documentation/nvcodec/nvav1enc.html?gi-language=c) |
| H264  | [x264enc](https://gstreamer.freedesktop.org/documentation/x264/index.html?gi-language=c) | [vaapih264enc](https://gstreamer.freedesktop.org/documentation/vaapi/vaapih264enc.html?gi-language=c) | [nvh264enc](https://gstreamer.freedesktop.org/documentation/nvcodec/nvh264enc.html?gi-language=c) |
| H265  | [x265enc](https://gstreamer.freedesktop.org/documentation/x265/index.html?gi-language=c) | [vaapih265enc](https://gstreamer.freedesktop.org/documentation/vaapi/vaapih265enc.html?gi-language=c) | [nvh265enc](https://gstreamer.freedesktop.org/documentation/nvcodec/nvh265enc.html?gi-language=c) |
```

--------------------------------

### Neko Configuration Environment Variables

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details environment variables used to configure Neko's behavior, including WebRTC settings, VNC connections, and stream quality. These variables allow for customization of the Neko server's operational parameters.

```APIDOC
Environment Variables:

NEKO_ICESERVERS
  Description: Configuration for ICE servers, including support for password-protected entries.
  Type: string

NEKO_DEBUG
  Description: Enables debug logging for the Neko server.
  Type: boolean

VIDEO_BITRATE
  Description: Sets the video stream bitrate in kbit/s to control quality.
  Type: integer

AUDIO_BITRATE
  Description: Sets the audio stream bitrate in kbit/s to control quality.
  Type: integer

MAX_FPS
  Description: Specifies the maximum WebRTC frame rate. Set to 0 to disable capping (e.g., for 60fps).
  Type: integer

NEKO_VNC_URL
  Description: Specifies the target URL for VNC connections when using neko as a bridge.
  Type: string
```

--------------------------------

### WebRTC Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Configures WebRTC settings, including ICE, bandwidth estimation, and NAT traversal.

```config
--webrtc.epr string
  limits the pool of ephemeral ports that ICE UDP connections can allocate from
--webrtc.estimator.debug
  enables debug logging for the bandwidth estimator
--webrtc.estimator.diff_threshold float
  how bigger the difference between estimated and stream bitrate must be to trigger upgrade/downgrade (default 0.15)
--webrtc.estimator.downgrade_backoff duration
  how long to wait before downgrading again after previous downgrade (default 10s)
--webrtc.estimator.enabled
  enables the bandwidth estimator
--webrtc.estimator.initial_bitrate int
  initial bitrate for the bandwidth estimator (default 1000000)
--webrtc.estimator.passive
  passive estimator mode, when it does not switch pipelines, only estimates
--webrtc.estimator.read_interval duration
  how often to read and process bandwidth estimation reports (default 2s)
--webrtc.estimator.stable_duration duration
  how long to wait for stable connection (upward or neutral trend) before upgrading (default 12s)
--webrtc.estimator.stalled_duration duration
  how long to wait for stalled bandwidth estimation before downgrading (default 24s)
--webrtc.estimator.unstable_duration duration
  how long to wait for stalled connection (neutral trend with low bandwidth) before downgrading (default 6s)
--webrtc.estimator.upgrade_backoff duration
  how long to wait before upgrading again after previous upgrade (default 5s)
--webrtc.icelite
  configures whether or not the ICE agent should be a lite agent
--webrtc.iceservers.backend string
  STUN and TURN servers used by the backend (default "[]")
--webrtc.iceservers.frontend string
  STUN and TURN servers used by the frontend (default "[]")
--webrtc.icetrickle
  configures whether cadidates should be sent asynchronously using Trickle ICE (default true)
--webrtc.ip_retrieval_url string
  URL address used for retrieval of the external IP address (default "https://checkip.amazonaws.com")
--webrtc.nat1to1 strings
  sets a list of external IP addresses of 1:1 (D)NAT and a candidate type for which the external IP address is used
--webrtc.tcpmux int
  single TCP mux port for all peers
--webrtc.udpmux int
  single UDP mux port for all peers, replaces EPR
```

--------------------------------

### Neko Docker Image Tags

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Lists available Docker image tags for the Neko project, indicating specific configurations or base software used. These tags facilitate deployment with different underlying applications or browser versions.

```docker
m1k1o/neko:brave
m1k1o/neko:google-chrome
m1k1o/neko:vlc
m1k1o/neko:xfce
m1k1o/neko:vncviewer
```

--------------------------------

### Neko Internal Settings and Fixes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details internal settings, bug fixes, and improvements related to pipeline management, key mapping, and state synchronization.

```APIDOC
Internal Settings & Fixes:

Pipeline Management:
  - Fixed an issue where `max_fps=0` would lead to an invalid pipeline.

Keysym Mapping:
  - Fixed keysym mapping for unknown keycodes, resolving issues with key combinations on certain keyboards.

Play State Synchronization:
  - Fixed play state synchronization when autoplay is disabled.

TCP Mux:
  - Fixed occasional TCP mux freezes by adding a write buffer.
```

--------------------------------

### NVENC Gstreamer Pipeline for Broadcast

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

This YAML configuration defines a Gstreamer pipeline for broadcasting audio and video using Nvidia's NVENC encoder. It requires Nvidia GPU support and specific Docker images. The pipeline uses pulseaudio for audio and X display for video, outputting via RTMP. Placeholders like {device}, {display}, and {url} are used.

```yaml
capture:
  broadcast:
    pipeline: |
      flvmux name=mux
        ! rtmpsink location={url}
      pulsesrc device={device}
        ! audio/x-raw,channels=2
        ! audioconvert
        ! voaacenc
        ! mux.
      ximagesrc display-name={display} show-pointer=false use-damage=false
        ! video/x-raw,framerate=30/1
        ! videoconvert
        ! queue
        ! video/x-raw,format=NV12
        ! nvh264enc name=encoder preset=low-latency-hq gop-size=25 spatial-aq=true temporal-aq=true bitrate=2800 vbv-buffer-size=2800 rc-mode=6
        ! h264parse config-interval=-1
        ! video/x-h264,stream-format=byte-stream,profile=high
        ! h264parse
        ! mux.

```

--------------------------------

### Configure VLC with Environment Variable

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This snippet shows how to run a Neko VLC container by setting the VLC_MEDIA environment variable to a remote media URL. This is a straightforward method for streaming content directly into the Neko environment.

```bash
docker run \
  -e VLC_MEDIA=http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 \
  ghcr.io/m1k1o/neko/vlc
```

--------------------------------

### Input Device Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/desktop.md

Enables and configures support for input devices, particularly touchscreens, using a custom driver. It specifies whether input support is enabled and the path to the socket file for the custom driver.

```APIDOC
Input Device Configuration:
  input.enabled: boolean
    Description: Enables input device support. Currently supports touchscreens via a custom driver.
    Default: false

  input.socket: string
    Description: Refers to the socket file that the custom driver creates for communication.
    Default: "/tmp/xf86-input-neko.sock"
```

--------------------------------

### Neko API Authentication (V2 Legacy)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Details on how authentication is handled in the legacy API, including query parameters for username (`?usr=`) and password (`?pwd=`). It explains session handling for HTTP and WebSocket requests and supported authentication providers.

```APIDOC
API Authentication (V2 Legacy):

- Query Parameters:
  - `?pwd=` (string): User password. Required for authentication.
  - `?usr=` (string, optional): Username. If not provided, a random username is generated.

- Session Handling:
  - HTTP Requests: A new user session is created for each request and destroyed upon completion.
  - WebSocket Requests: The session is kept alive until the WebSocket connection is closed.

- Supported Providers (for legacy API):
  - `multiuser`: Requires `?usr=` and `?pwd=`.
  - `noauth`: Does not require `?usr=` or `?pwd=`.

- Notes:
  - The legacy API uses a compatibility layer that allows V2 clients to connect to V3.
  - The `?usr=` parameter is optional; if omitted, the API generates a random username.
  - Only `multiuser` or `noauth` providers are supported without specifying the `?usr=` query string.
```

--------------------------------

### Neko Configuration Imports

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Imports necessary components and configuration data for Neko's configuration management system.

```javascript
import { Def, Opt } from '@site/src/components/Anchor';
import { ConfigurationTab } from '@site/src/components/Configuration';
import configOptions from './help.json';
```

--------------------------------

### Neko WebRTC API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Details on changes to the WebRTC API between V2 and V3, specifically regarding data channel creation and byte order.

```APIDOC
WebRTC API (V2 Legacy):

- Data Channel Handling:
  - V2: Created a new data channel on the client side.
  - V3: Creates a new data channel on the server side. The legacy API handler overwrites the existing V3 data channel with the legacy one.

- Byte Order:
  - Changed from Little Endian to Big Endian format for easier client-side manipulation.
```

--------------------------------

### Server Configuration Options

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Configures server network binding, SSL certificates, CORS policies, metrics exposure, path prefixes, profiling, and reverse proxy trust.

```APIDOC
server.bind: string
  Description: Address/port/socket to serve neko. For Docker, bind to '0.0.0.0' to allow external connections.

server.cert: string
  Description: Path to the SSL certificate file. If empty, server runs in plain HTTP.

server.key: string
  Description: Path to the SSL key file. If empty, server runs in plain HTTP.

server.cors: array of strings
  Description: List of allowed origins for CORS.
  - If empty: CORS is disabled, only same-origin requests allowed.
  - If '*': All origins allowed. Neko responds with the requested origin, not '*'.
  - If list of origins: Only specified origins allowed.

server.metrics: boolean
  Description: When true, Prometheus metrics are available at '/metrics'.

server.path_prefix: string
  Description: Prefix for all HTTP requests. Useful for serving Neko under a subpath (e.g., '/neko') behind a reverse proxy.

server.pprof: boolean
  Description: When true, the pprof endpoint is available at '/debug/pprof' for debugging and profiling. Should be disabled in production.

server.proxy: boolean
  Description: When true, Neko trusts 'X-Forwarded-For' and 'X-Real-IP' headers from a reverse proxy. Ensure proxy sets these headers and they are not trusted when not behind a proxy.

server.static: string
  Description: Path to the directory containing Neko client files to serve. Useful for serving client files on the same domain as the server.
```

--------------------------------

### Browser Profile Ownership Management

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Demonstrates bash commands to check and correct the ownership of persistent browser profile directories within a Neko container. This ensures the `neko` user has the necessary permissions to access and manage the profile, preventing startup issues.

```bash
# Check the owner of the profile
docker exec -it <container-id> ls -la /home/neko/.config/google-chrome
# To change the owner
docker exec -it <container-id> chown -R neko:neko /home/neko/.config/google-chrome
```

--------------------------------

### Neko Dependency Replacements

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Notes on dependencies that have been replaced or removed.

```APIDOC
Dependency Changes:

NordVPN Replacement:
  - NordVPN extension replaced with Sponsorblock extension in default configuration.

vncviewer Image Removal:
  - Removed `vncviewer` image as its functionality is replaced and extended by remmina.
```

--------------------------------

### Neko V2 to V3 Configuration Mapping

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

This section details the mapping between Neko V2 configuration options and their corresponding V3 equivalents. It is crucial for users migrating from V2 to understand these changes to ensure smooth operation in V3, especially when legacy support is enabled.

```APIDOC
Configuration Migration:

Neko V3 maintains compatibility with V2 configuration options when legacy support is enabled. The following table outlines the direct mappings:

| V2 Configuration Option      | V3 Configuration Option
```

--------------------------------

### Validate UDP Port Reachability with Netcat

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Demonstrates how to use the `netcat` (nc) command to test UDP connectivity between a server and a client. This helps diagnose issues with port forwarding or ISP blocking.

```shell
# On the server:
nc -ul 52001

# On the local client:
nc -u [server ip] 52001
```

--------------------------------

### x86_64 Docker Images (Docker Hub)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Provides a list of m1k1o/neko Docker images for x86_64 architecture available on Docker Hub. Some images require the `--cap-add=SYS_ADMIN` capability for full functionality.

```shell
docker pull m1k1o/neko:latest
# or
docker pull m1k1o/neko:firefox
# for Firefox.

docker pull m1k1o/neko:chromium
# for Chromium (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:google-chrome
# for Google Chrome (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:ungoogled-chromium
# for Ungoogled Chromium (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:microsoft-edge
# for Microsoft Edge (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:brave
# for Brave Browser (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:vivaldi
# for Vivaldi Browser (needs --cap-add=SYS_ADMIN).

docker pull m1k1o/neko:opera
# for Opera Browser (requires extra steps for DRM).

docker pull m1k1o/neko:tor-browser
# for Tor Browser.

docker pull m1k1o/neko:remmina
# for Remmina (pass REMMINA_URL or REMMINA_PROFILE env var).

docker pull m1k1o/neko:vlc
# for VLC Media Player (mount volume to /media or set VLC_MEDIA).

docker pull m1k1o/neko:xfce
# or
docker pull m1k1o/neko:kde
# for a shared desktop environment.
```

--------------------------------

### Unminimize Feature Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/desktop.md

Configures the 'unminimize' feature, which automatically restores windows that have been minimized. This is useful in minimal desktop environments where accidental minimization can make applications inaccessible.

```APIDOC
Unminimize Feature:
  unminimize: boolean
    Description: Enables the unminimize feature that listens for minimize events and restores the window back to its original state.
```

--------------------------------

### Neko Adaptive Framerate and Autoplay

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Explains improvements to adaptive framerate streaming and autoplay behavior.

```APIDOC
Framerate & Autoplay:

Adaptive Framerate:
  - Streams in the framerate selected from the dropdown.

Autoplay Refactoring:
  - Refactored autoplay to start playing audio if allowed by the browser.
```

--------------------------------

### Docusaurus Imports

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/introduction.md

Imports necessary components and hooks from Docusaurus for rendering documentation cards and accessing sidebar category information.

```javascript
import DocCardList from '@theme/DocCardList';
import {useCurrentSidebarCategory} from '@docusaurus/theme-common';
```

--------------------------------

### Configuration Tab Component Usage

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Demonstrates the usage of the ConfigurationTab component, likely for displaying configuration options. It takes `configOptions` and a `heading` flag as props, typically within a JSX environment.

```javascript
<ConfigurationTab options={configOptions} heading={true} />
```

--------------------------------

### Run Neko with Nvidia GPU Acceleration (Docker)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

This command demonstrates how to run the Neko container with Nvidia GPU acceleration enabled. It specifies ports, environment variables for screen resolution, passwords, network settings, video codec, and hardware encoding, along with necessary system capabilities and GPU allocation.

```bash
docker run -d --gpus all \
  -p 8080:8080 \
  -p 56000-56100:56000-56100/udp \
  -e NEKO_SCREEN=1920x1080@30 \
  -e NEKO_PASSWORD=neko \
  -e NEKO_PASSWORD_ADMIN=admin \
  -e NEKO_EPR=56000-56100 \
  -e NEKO_NAT1TO1=192.168.1.10 \
  -e NEKO_ICELITE=1 \
  -e NEKO_VIDEO_CODEC=h264 \
  -e NEKO_HWENC=nvenc \
  --shm-size=2gb \
  --cap-add=SYS_ADMIN \
  --name neko \
  ghcr.io/m1k1o/neko/nvidia-google-chrome:latest
```

--------------------------------

### Upload Drop Feature Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/desktop.md

Configures the 'upload drop' feature, allowing users to upload files by dragging and dropping them into the application window. This involves client-side event catching, server-side overlay, and simulated mouse events.

```APIDOC
Upload Drop Feature:
  upload_drop: boolean
    Description: Enables the upload drop feature, allowing file uploads via drag and drop into the application window.
```

--------------------------------

### Desktop Environment Display and Screen Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/desktop.md

Configures the display server and screen resolution for the Neko desktop environment. The 'display' setting refers to the X server, defaulting to the DISPLAY environment variable. The 'screen' setting defines resolution and refresh rate, with a default of '1280x720@30'.

```APIDOC
Desktop Environment Configuration:
  display: string
    Description: Refers to the X server running on the system. If not specified, the environment variable DISPLAY is used. This display is also used in the Capture configuration to capture the screen.
    Default: Uses the DISPLAY environment variable.

  screen: string
    Description: Refers to the screen resolution and refresh rate.
    Format: "<width>x<height>@<refresh rate>"
    Default: "1280x720@30"
```

--------------------------------

### Neko UI Upgrades

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details upgrades to UI libraries used in Neko.

```APIDOC
UI Library Upgrades:

Font Awesome & Sweetalert2:
  - Upgraded to newest major versions.
```

--------------------------------

### Neko Server Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Configures the Neko server, including the bind address/port/socket, SSL certificate path for secure connections, and Cross-Origin Resource Sharing (CORS) settings.

```config
--server.bind string
  address/port/socket to serve neko (default "127.0.0.1:8080")
--server.cert string
  path to the SSL cert used to secure the neko server
--server.cors strings
  list of allowed origins for CORS, if empty CORS is disabled, if '*' is present all origins are allowed
```

--------------------------------

### Neko Capture Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Configures various capture settings for Neko, including audio codecs, devices, broadcast parameters, microphone, screencast, and video/webcam streams. It specifies pipelines, bitrates, quality, and device selections.

```config
--capture.audio.codec string
  audio codec to be used (default "opus")
--capture.audio.device string
  pulseaudio device to capture (default "audio_output.monitor")
--capture.audio.pipeline string
  gstreamer pipeline used for audio streaming
--capture.broadcast.audio_bitrate int
  broadcast audio bitrate in KB/s (default 128)
--capture.broadcast.autostart
  automatically start broadcasting when neko starts and broadcast_url is set (default true)
--capture.broadcast.pipeline string
  gstreamer pipeline used for broadcasting
--capture.broadcast.preset string
  broadcast speed preset for h264 encoding (default "veryfast")
--capture.broadcast.url string
  initial URL for broadcasting, setting this value will automatically start broadcasting
--capture.broadcast.video_bitrate int
  broadcast video bitrate in KB/s (default 4096)
--capture.microphone.device string
  pulseaudio device used for microphone (default "audio_input")
--capture.microphone.enabled
  enable microphone stream (default true)
--capture.screencast.enabled
  enable screencast
--capture.screencast.pipeline string
  gstreamer pipeline used for screencasting
--capture.screencast.quality string
  screencast JPEG quality (default "60")
--capture.screencast.rate string
  screencast frame rate (default "10/1")
--capture.video.codec string
  video codec to be used (default "vp8")
--capture.video.display string
  X display to capture
--capture.video.ids strings
  ordered list of video ids
--capture.video.pipeline string
  shortcut for configuring only a single gstreamer pipeline, ignored if pipelines is set
--capture.video.pipelines string
  pipelines config used for video streaming (default "{}")
--capture.webcam.device string
  v4l2sink device used for webcam (default "/dev/video0")
--capture.webcam.enabled
  enable webcam stream
--capture.webcam.height int
  webcam stream height (default 720)
--capture.webcam.width int
  webcam stream width (default 1280)
```

--------------------------------

### Neko NVENC Support

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Highlights the addition of NVENC support for hardware H.264 encoding on Nvidia GPUs.

```APIDOC
NVENC Support:

Hardware Encoding:
  - Added NVENC support for hardware H.264 encoding on Nvidia GPUs.
```

--------------------------------

### Neko Docker Images

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Lists available Neko Docker images, categorized by underlying browser/application and hardware acceleration support (Intel/Nvidia). GHCR images are built via GitHub Actions.

```dockerfile
ghcr.io/m1k1o/neko/intel-ungoogled-chromium:latest
ghcr.io/m1k1o/neko/intel-microsoft-edge:latest
ghcr.io/m1k1o/neko/intel-brave:latest
ghcr.io/m1k1o/neko/intel-vivaldi:latest
ghcr.io/m1k1o/neko/intel-opera:latest
ghcr.io/m1k1o/neko/intel-tor-browser:latest
ghcr.io/m1k1o/neko/intel-remmina:latest
ghcr.io/m1k1o/neko/intel-vlc:latest
ghcr.io/m1k1o/neko/intel-xfce:latest
ghcr.io/m1k1o/neko/intel-kde:latest
ghcr.io/m1k1o/neko/nvidia-firefox:latest
ghcr.io/m1k1o/neko/nvidia-chromium:latest
ghcr.io/m1k1o/neko/nvidia-google-chrome:latest
ghcr.io/m1k1o/neko/nvidia-microsoft-edge:latest
ghcr.io/m1k1o/neko/nvidia-brave:latest
```

--------------------------------

### Mount Firefox Browser Profile

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Explains how to mount the Firefox user profile directory as a volume. This preserves browser data, such as history, cookies, and settings, across container restarts.

```bash
-v '${PWD}/data:/home/neko/.mozilla/firefox/profile.default'
```

--------------------------------

### POST /api/members - Create Member

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-create.api.mdx

Creates a new member with the provided username, password, and profile details. The request body must be a JSON object conforming to the MemberCreate schema.

```APIDOC
POST /api/members

---

**Description:**
Create a new member.

**Request Body:**
```json
{
  "username": "string",
  "password": "string",
  "profile": {
    "name": "string",
    "is_admin": "boolean",
    "can_login": "boolean",
    "can_connect": "boolean",
    "can_watch": "boolean",
    "can_host": "boolean",
    "can_share_media": "boolean",
    "can_access_clipboard": "boolean",
    "sends_inactive_cursor": "boolean",
    "can_see_inactive_cursors": "boolean",
    "plugins": { ... }
  }
}
```

**Schema Details:**
- `username` (string): The username of the new member.
- `password` (string): The password of the new member.
- `profile` (object): The profile information of the new member.
  - `name` (string): The name of the member.
  - `is_admin` (boolean): Indicates if the member is an admin.
  - `can_login` (boolean): Indicates if the member can log in.
  - `can_connect` (boolean): Indicates if the member can connect.
  - `can_watch` (boolean): Indicates if the member can watch.
  - `can_host` (boolean): Indicates if the member can host.
  - `can_share_media` (boolean): Indicates if the member can share media.
  - `can_access_clipboard` (boolean): Indicates if the member can access the clipboard.
  - `sends_inactive_cursor` (boolean): Indicates if the member sends inactive cursor.
  - `can_see_inactive_cursors` (boolean): Indicates if the member can see inactive cursors.
  - `plugins` (object): Additional plugin settings.

**Required Parameters:**
- `username`
- `password`
- `profile`
```

--------------------------------

### Intel VAAPI GPU Acceleration Docker Images (GHCR)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Provides m1k1o/neko Docker images optimized for Intel GPUs using VAAPI for hardware acceleration, available on GitHub Container Registry.

```shell
docker pull ghcr.io/m1k1o/neko/intel-firefox:latest

docker pull ghcr.io/m1k1o/neko/intel-chromium:latest

docker pull ghcr.io/m1k1o/neko/intel-google-chrome:latest
```

--------------------------------

### Neko Clipboard Synchronization

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Addresses issues and improvements related to clipboard synchronization across different browsers.

```APIDOC
Clipboard Synchronization:

Chromium-based Browsers:
  - Fixed clipboard sync in chromium-based browsers.
```

--------------------------------

### File Provider Configuration Schema

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Defines the configuration parameters for the file-based member provider. It specifies the path to the members file and whether passwords are hashed.

```APIDOC
File Provider Configuration:
  member.provider: 'file'
    - Description: Specifies the file provider.
  member.file.path: string
    - Description: Absolute path to the file containing users and passwords.
    - Default: "/opt/neko/members.json"
  member.file.hash: boolean
    - Description: Whether passwords are hashed using sha256.
    - Default: false
```

--------------------------------

### Docker Compose: Neko Browser Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

This YAML snippet shows the equivalent configuration for Docker Compose to run Neko Docker images for Chromium-based browsers. It specifies the necessary `cap_add` and `shm_size` parameters.

```yaml
cap_add:
- SYS_ADMIN
shm_size: 2g
```

--------------------------------

### Embed Neko URL Without Login Prompt

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/faq.md

Provides the URL format to embed the Neko application into a web page, bypassing the login prompt for viewers. Replace `<your-neko-server-ip>` with the actual IP address of your Neko server.

```text
http://<your-neko-server-ip>:8080/?usr=neko&pwd=neko
```

--------------------------------

### Neko WebSocket API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Information on WebSocket message handling and connection behavior in the legacy API. It covers ping intervals and heartbeat mechanisms compared to V3.

```APIDOC
WebSocket API (V2 Legacy):

- Endpoint:
  - Legacy API: `/ws` (handled by the compatibility layer).
  - V3 API: `/api/ws`.

- Ping/Heartbeat:
  - V2: Sent ping messages every 60 seconds.
  - V3: Sends ping messages every 10 seconds and uses an additional heartbeat mechanism to verify connection activity.

- Compatibility:
  - When the legacy API is enabled, users connect to the `/ws` endpoint and are handled by the V2 API compatibility layer.
```

--------------------------------

### Mount Chromium Policies JSON

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Illustrates mounting a custom policies.json file into a Chromium container. This enables granular control over Chromium's settings, including security policies and user preferences, by providing a custom configuration file.

```bash
-v '${PWD}/policies.json:/etc/chromium/policies/managed/policies.json'
```

--------------------------------

### Configure Docker Compose for UDP Port Exposure

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This snippet demonstrates how to configure a docker-compose.yaml file to expose the necessary UDP ports for Neko. It includes setting the ephemeral port range for Neko and mapping these ports in the Docker service definition.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_ICELITE: 1
```

--------------------------------

### Neko Broadcast Pipeline for Hardware Accelerated Encoding

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

This GStreamer pipeline configuration is designed for Neko's broadcast functionality, utilizing Nvidia hardware acceleration for H.264 encoding. It specifies audio input, video capture, and the NVENC encoder with optimized presets for low latency and quality.

```yaml
NEKO_BROADCAST_PIPELINE: |
  flvmux name=mux 
    ! rtmpsink location={url} pulsesrc device={device} 
    ! audio/x-raw,channels=2 
    ! audioconvert 
    ! voaacenc 
    ! mux. 
  ximagesrc display-name={display} show-pointer=false use-damage=false 
    ! video/x-raw,framerate=30/1 
    ! videoconvert 
    ! queue 
    ! video/x-raw,format=NV12 
    ! nvh264enc name=encoder preset=low-latency-hq gop-size=25 spatial-aq=true temporal-aq=true bitrate=2800 vbv-buffer-size=2800 rc-mode=6 
    ! h264parse config-interval=-1 
    ! video/x-h264,stream-format=byte-stream,profile=high 
    ! h264parse 
    ! mux.
```

--------------------------------

### Neko Refactoring and Dependencies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details refactoring efforts, dependency updates, and changes in internal component naming.

```APIDOC
Refactoring & Dependencies:

Go and Node Updates:
  - Updated to Go 1.19 and Node 18.
  - Removed `go-events` as a dependency.

Server Refactoring:
  - Split `remote` into `desktop` and `capture`.
  - Refactored `xorg` to include `xevent` and handle clipboard as an event, removing looped polling.

Pulseaudio Sink Renaming:
  - Renamed pulseaudio sink from `auto_null` to `audio_output` to be recognized by KDE.

Pulseaudio Configuration:
  - Pulseaudio is now configured using environment variables to preserve settings when mounting `/home/neko`.
```

--------------------------------

### Enable Debug Mode in Neko (YAML)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/faq.md

This snippet demonstrates how to enable debug mode for the Neko server by setting the `NEKO_DEBUG` environment variable in a `docker-compose.yaml` file. This setting provides verbose server information. Ensure client-side debug mode is also enabled in the JavaScript console for complete visibility.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      # highlight-start
      NEKO_DEBUG: 1
      # highlight-end
```

--------------------------------

### Nvidia GPU Flavors (CUDA)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Lists Neko Docker image flavors that support Nvidia GPU hardware acceleration using CUDA and EGL. These images are also for the linux/amd64 architecture and are designed to leverage Nvidia graphics cards for improved performance.

```bash
# Nvidia GPU accelerated applications
# Only for architecture linux/amd64
# Base image: ghcr.io/m1k1o/neko/nvidia-base

# Browsers:
# ghcr.io/m1k1o/neko/nvidia-firefox
# ghcr.io/m1k1o/neko/nvidia-chromium
# ghcr.io/m1k1o/neko/nvidia-google-chrome
# ghcr.io/m1k1o/neko/nvidia-microsoft-edge
# ghcr.io/m1k1o/neko/nvidia-brave
```

--------------------------------

### Neko VirtualGL Version Update

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Notes the release of VirtualGL version 3.1 and its implications for Nvidia GPU acceleration in Chromium.

```APIDOC
VirtualGL Update:

Version 3.1:
  - Added support for Chromium browsers to use Nvidia GPU acceleration.
```

--------------------------------

### Neko Web Interface Query Parameters

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/ui.md

Customize the Neko web interface by appending query parameters to the URL. These parameters control aspects like user login, video display, volume, and language.

```APIDOC
Neko Web Interface Query Parameters:

Parameters:
  usr=<username>
    - Description: Prefills the username field.
    - Type: string
  pwd=<password>
    - Description: Prefills the password field.
    - Type: string
  cast=1
    - Description: Hides all controls and shows only the video.
    - Type: boolean (1 for true)
  embed=1
    - Description: Hides most additional components and shows only the video.
    - Type: boolean (1 for true)
  volume=<0-1>
    - Description: Sets the volume to the given value.
    - Type: float (between 0 and 1)
  lang=<language>
    - Description: Sets the language for the interface.
    - Type: string
  show_side=1
    - Description: Shows the sidebar on startup.
    - Type: boolean (1 for true)
  mute_chat=1
    - Description: Mutes the chat on startup.
    - Type: boolean (1 for true)

Example Usage:
  http(s)://<URL:Port>/?pwd=neko&usr=guest&cast=1
```

--------------------------------

### Configure TURN Servers with NEKO_ICESERVERS

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Configures Neko to use TURN servers for peer-to-peer connections, bypassing port forwarding. This involves setting the NEKO_ICESERVERS environment variable with a JSON array of server configurations, including TURN and STUN URLs and credentials. Ensure credentials are correctly escaped.

```yaml
NEKO_ICESERVERS: '[{"urls": ["turn:<MY-COTURN-SERVER>:443?transport=udp", "turn:<MY-COTURN-SERVER>:443?transport=tcp", "turns:<MY-COTURN-SERVER>:443?transport=udp", "turns:<MY-COTURN-SERVER>:443?transport=tcp"], "credential": "<MY-COTURN-CREDENTIAL>"}, {"urls": ["stun:stun.nextcloud.com:443"]}]'
```

--------------------------------

### File Chooser Dialog Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/desktop.md

Configures the handling of external file chooser dialogs. This experimental feature allows the Neko client to handle file selection requests, bypassing the display of the dialog within the Neko desktop environment.

```APIDOC
File Chooser Dialog Configuration:
  file_chooser_dialog: boolean
    Description: Enables external handling of file chooser dialogs. When enabled, the Neko client is prompted to upload files from the local filesystem instead of displaying the dialog internally. This feature is experimental and may be error-prone.
```

--------------------------------

### Copy Default Browser Policy File

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Provides bash commands to copy the default policy file from a Neko container to the local machine. This is useful for inspecting or modifying the default policies before mounting a custom one.

```bash
# Create a container without starting it
docker create --name neko ghcr.io/m1k1o/neko/firefox:latest
# Copy the policy file from the container to your local machine
docker cp neko:/usr/lib/firefox/distribution/policies.json ./policy.json
# Remove the container
docker rm neko
```

--------------------------------

### Neko HTTP API Endpoints

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Overview of the Neko project's HTTP API endpoints, detailing their purpose and migration status from V2 to V3. Specific endpoints like /health, /stats, /screenshot.jpg, and /file are covered.

```APIDOC
GET /health
  Description: Checks the health status of the server.
  Migration: Migrated to the /docs/v3/api/healthcheck endpoint.
  Returns: 200 OK if the server is running.

GET /stats
  Description: Retrieves server statistics and session information.
  Migration: Migrated to /docs/v3/api/stats and /docs/v3/api/sessions-get endpoints.
  Returns: A JSON object containing connection status, session members, banned IPs, locked resources, server uptime, and control settings.

GET /screenshot.jpg
  Description: Captures and returns a screenshot of the desktop.
  Migration: Migrated to the /docs/v3/api/screen-shot-image endpoint.
  Returns: A JPEG image of the desktop.

GET /file
  Description: Handles file transfer operations.
  Migration: Functionality moved to a dedicated File Transfer Plugin (/docs/v3/configuration/plugins#filetransfer).
```

--------------------------------

### Object Provider Configuration Schema

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Defines the configuration parameters for the in-memory object-based member provider. It allows specifying users directly in the configuration.

```APIDOC
Object Provider Configuration:
  member.provider: 'object'
    - Description: Specifies the object provider.
  member.object.users: array
    - Description: List of users with their passwords and profiles.
    - Default: []
    - User Object:
      - username: string
      - password: string
      - profile: object
        - name: string
        - is_admin: boolean
        - can_login: boolean
        - can_connect: boolean
        - can_watch: boolean
        - can_host: boolean
        - can_share_media: boolean
        - can_access_clipboard: boolean
        - sends_inactive_cursor: boolean
        - can_see_inactive_cursors: boolean
```

--------------------------------

### Enable Neko Debug Mode with Docker Compose

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/faq.md

This configuration snippet shows how to enable debug mode for the Neko service within a docker-compose.yaml file by setting the NEKO_DEBUG environment variable.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      # highlight-start
      NEKO_DEBUG: 1
      # highlight-end
```

--------------------------------

### ICE Server Configuration Object Structure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Defines the structure and properties of a single ICE server configuration object. This includes the list of server URLs, and optional authentication credentials.

```APIDOC
ICE_Server_Object:
  urls: string[]
    - List of URLs for the ICE server. Can include multiple URLs for redundancy or different protocols (e.g., "stun:stun.l.google.com:19302", "turn:my.turn.server:3478").
  username: string (optional)
    - Username for authenticating with the ICE server if required.
  credential: string (optional)
    - Credential (password) for authenticating with the ICE server if required.
```

--------------------------------

### Mount Firefox Policies JSON

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Demonstrates how to mount a custom policies.json file into a Firefox container. This allows for extensive customization of browser behavior, such as security settings and feature enablement, by overriding default policies.

```bash
-v '${PWD}/policies.json:/usr/lib/firefox/distribution/policies.json'
```

--------------------------------

### Docker Compose ICE Servers Environment Variable

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Shows how to set ICE server configurations as a JSON string for the `NEKO_WEBRTC_ICESERVERS_FRONTEND` environment variable within a docker-compose.yaml file.

```yaml
NEKO_WEBRTC_ICESERVERS_FRONTEND: |
  [{
    "urls": [ "turn:<MY-COTURN-SERVER>:3478" ],
    "username": "neko",
    "credential": "neko"
  },{
    "urls": [ "stun:stun.nextcloud.com:3478" ]
  }]
```

--------------------------------

### Room Configuration Options

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Defines settings for room behavior, such as private mode, login/control locking, implicit hosting, and reconnection policies.

```APIDOC
session.private_mode: boolean
  Description: Whether private mode is enabled. Users do not receive the room video or audio.

session.locked_logins: boolean
  Description: Whether logins are locked for users. Admins can still login.

session.locked_controls: boolean
  Description: Whether controls are locked for users. Admins can still control.

session.control_protection: boolean
  Description: Users can gain control only if at least one admin is in the room.

session.implicit_hosting: boolean
  Description: Automatically grants control to a user when they click on the screen, unless an admin has locked the controls.

session.inactive_cursors: boolean
  Description: Whether to show inactive cursors server-wide (only for users that have it enabled in their profile).

session.merciful_reconnect: boolean
  Description: Whether to allow reconnecting to the websocket even if the previous connection was not closed. This means that a new login can kick out the previous one.

session.heartbeat_interval: integer
  Description: Interval in seconds for sending a heartbeat message to the server. Used to keep the connection alive and detect connection loss.
```

--------------------------------

### Enable File Upload/Download in Browser Policies

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Configures browser policies to allow file uploading and downloading, and whitelists specific file URLs. This is a security measure to grant the browser access to local file system operations.

```json
{
  "DownloadRestrictions": 0,
  "AllowFileSelectionDialogs": true,
  "URLAllowlist": [
      "file:///home/neko/Downloads"
  ]
}
```

--------------------------------

### Neko Environment Variables

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/configuration.md

A comprehensive list of environment variables used to configure Neko. Each variable controls a specific aspect of the server's behavior, from screen resolution and security to networking and media encoding.

```text
NEKO_SCREEN: Resolution after startup. Only Admins can change this later. e.g. 1920x1080@30
NEKO_PASSWORD: Password for the user login. e.g. user_password
NEKO_PASSWORD_ADMIN: Password for the admin login. e.g. admin_password
NEKO_CONTROL_PROTECTION: Control protection means, users can gain control only if at least one admin is in the room. e.g. false
NEKO_IMPLICIT_CONTROL: If enabled members can gain control implicitly, they don't need to request control. e.g. false
NEKO_LOCKS: Resources, that will be locked when starting, separated by whitespace. Currently supported: control, login, file_transfer. e.g. control
NEKO_EPR: For WebRTC needed range of UDP ports. e.g. 52000-52099
NEKO_UDPMUX: Alternative to epr with only one UDP port. e.g. 52100
NEKO_TCPMUX: Use TCP connection, meant as fallback for UDP. e.g. 52100
NEKO_NAT1TO1: IP of the server that will be sent to client, if not specified, public IP is automatically resolved. e.g. 10.0.0.1
NEKO_IPFETCH: Automatically fetch IP address from given URL when nat1to1 is not specified. e.g. http://checkip.amazonaws.com
NEKO_ICELITE: Use the ice lite protocol. e.g. false
NEKO_ICESERVER: Describes a single STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer (simple usage for server without authentication). e.g. stun:stun.l.google.com:19302
NEKO_ICESERVERS: Describes multiple STUN and TURN server that can be used by the ICEAgent to establish a connection with a peer. e.g. [{"urls": ["turn:turn.example.com:19302", "stun:stun.example.com:19302"], "username": "name", "credential": "password"}, {"urls": ["stun:stun.example2.com:19302"]}] [More information](https://developer.mozilla.org/en-US/docs/Web/API/RTCIceServer)
NEKO_VIDEO_CODEC: vp8 (default encoder), vp9 (parameter not optimized yet), h264 (second best option)
NEKO_VIDEO_BITRATE: Bitrate of the video stream in kb/s. e.g. 3500
NEKO_VIDEO: Makes it possible to create custom gstreamer video pipeline. With this you could find the best quality for your CPU. Installed are gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly. e.g. [GStreamer pipeline example]
NEKO_MAX_FPS: The resulting stream frames per seconds should be capped (0 for uncapped). e.g. 0
NEKO_HWENC: none (default CPU encoding), vaapi, nvenc
NEKO_AUDIO_CODEC: opus (default encoder), g722, pcmu, pcma
NEKO_AUDIO_BITRATE: Bitrate of the audio stream in kb/s. e.g. 196
NEKO_AUDIO: Makes it possible to create custom gstreamer audio pipeline, same as for video. e.g. [GStreamer pipeline example]
NEKO_BROADCAST_PIPELINE: Makes it possible to create custom gstreamer pipeline used for broadcasting, strings {url}, {device} and {display} will be replaced.
NEKO_BROADCAST_URL: Set a default URL for broadcast streams. It can be disabled/changed later by admins in the GUI. e.g. rtmp://<your-server>:1935/ingest/<stream-key>
NEKO_BROADCAST_AUTOSTART: Automatically start broadcasting when neko starts and broadcast_url is set. e.g. true
NEKO_BIND: Address/port/socket where neko binds to (default 127.0.0.1:8080). e.g. :8080
NEKO_CERT: Path to the SSL-Certificate. e.g. /certs/cert.pem
NEKO_KEY: Path to the SSL-Certificate private key. e.g. /certs/key.pem
NEKO_PROXY: Enable reverse proxy mode, so that neko trusts X-Forwarded-For headers. e.g. false
NEKO_PATH_PREFIX: Path prefix for HTTP requests. e.g. /neko/
NEKO_CORS: Cross origin request sharing, whitespace separated list of allowed hosts, * for all. e.g. 127.0.0.1 neko.example.com
NEKO_FILE_TRANSFER_ENABLED: Enable file transfer feature. e.g. true
```

--------------------------------

### Session Authentication API Responses

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/login.api.mdx

Details the possible API responses for user authentication and session login. Includes successful authentication (200 OK) with session data and token, as well as error responses for unauthorized (401) and forbidden (403) access, both returning error messages.

```APIDOC
responses={
  "200": {
    "description": "User authenticated successfully.",
    "content": {
      "application/json": {
        "schema": {
          "allOf": [
            {
              "type": "object",
              "properties": {
                "id": {
                  "type": "string",
                  "description": "The unique identifier of the session."
                },
                "profile": {
                  "description": "The profile information of the user.",
                  "x-tags": [
                    "members"
                  ],
                  "type": "object",
                  "properties": {
                    "name": {
                      "type": "string",
                      "description": "The name of the member."
                    },
                    "is_admin": {
                      "type": "boolean",
                      "description": "Indicates if the member is an admin."
                    },
                    "can_login": {
                      "type": "boolean",
                      "description": "Indicates if the member can log in."
                    },
                    "can_connect": {
                      "type": "boolean",
                      "description": "Indicates if the member can connect."
                    },
                    "can_watch": {
                      "type": "boolean",
                      "description": "Indicates if the member can watch."
                    },
                    "can_host": {
                      "type": "boolean",
                      "description": "Indicates if the member can host."
                    },
                    "can_share_media": {
                      "type": "boolean",
                      "description": "Indicates if the member can share media."
                    },
                    "can_access_clipboard": {
                      "type": "boolean",
                      "description": "Indicates if the member can access the clipboard."
                    },
                    "sends_inactive_cursor": {
                      "type": "boolean",
                      "description": "Indicates if the member sends inactive cursor."
                    },
                    "can_see_inactive_cursors": {
                      "type": "boolean",
                      "description": "Indicates if the member can see inactive cursors."
                    },
                    "plugins": {
                      "type": "object",
                      "additionalProperties": true,
                      "description": "Additional plugin settings."
                    }
                  },
                  "title": "MemberProfile"
                },
                "state": {
                  "description": "The current state of the session.",
                  "type": "object",
                  "properties": {
                    "is_connected": {
                      "type": "boolean",
                      "description": "Indicates if the user is connected."
                    },
                    "is_watching": {
                      "type": "boolean",
                      "description": "Indicates if the user is watching."
                    }
                  },
                  "title": "SessionState"
                }
              },
              "title": "SessionData"
            },
            {
              "type": "object",
              "properties": {
                "token": {
                  "type": "string",
                  "description": "The session token, only if cookie authentication is disabled."
                }
              }
            }
          ],
          "title": "SessionLoginResponse"
        }
      }
    }
  },
  "401": {
    "description": "The request requires user authentication.",
    "content": {
      "application/json": {
        "schema": {
          "type": "object",
          "properties": {
            "message": {
              "type": "string",
              "description": "Detailed error message."
            }
          },
          "title": "ErrorMessage"
        }
      }
    }
  },
  "403": {
    "description": "The server understood the request, but refuses to authorize it.",
    "content": {
      "application/json": {
        "schema": {
          "type": "object",
          "properties": {
            "message": {
              "type": "string",
              "description": "Detailed error message."
            }
          },
          "title": "ErrorMessage"
        }
      }
    }
  }
}
```

--------------------------------

### Neko Connection States and Types

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/roadmap.md

Defines the possible connection states for a Neko client and the various connection types supported for backend communication. This allows API users to manage connection status without needing to understand underlying protocols.

```APIDOC
NekoConnection:
  States:
    - connected: User is connected to the server.
    - connecting: Client is attempting to establish a connection to the server.
    - disconnected: User is disconnected from the server with no attempts to reconnect. Notified with a reason.
  Types:
    - none: No connection is currently used.
    - short_polling: Client requests server updates every X ms.
    - long_polling: HTTP request is kept open until server sends updates, then client sends another request.
    - sse: Server sends updates using Server-Sent Events.
    - websocket: Server sends updates using WebSockets.
    - ... others (e.g. MQTT...)
```

--------------------------------

### Neko Autolock Setting

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Describes the change in autolock behavior for KDE sessions.

```APIDOC
Autolock Setting:

KDE Sessions:
  - Disabled autolock for KDE to prevent screen locking when inactive.
```

--------------------------------

### Validate UDP Port Reachability with Netcat

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This section provides commands to test UDP port reachability between a server and a client using netcat (nc). It involves running a listener on the server and a sender on the client to verify connectivity.

```shell
# On your server:
nc -ul 52101

# On your local client:
nc -u [server ip] 52101
```

--------------------------------

### Mount Chromium Browser Profile

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Details mounting the Chromium user profile directory as a volume. This ensures that browser data, including settings and session information, is persisted between container restarts.

```bash
-v '${PWD}/data:/home/neko/.config/chromium'
```

--------------------------------

### Member Profile Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Illustrates the structure of a member profile, which defines a user's attributes and permissions within the Neko system. This profile can be configured in YAML or JSON format.

```yaml
name: User Name
is_admin: false
can_login: true
can_connect: true
can_watch: true
can_host: true
can_share_media: true
can_access_clipboard: true
sends_inactive_cursor: true
can_see_inactive_cursors: true
plugins:
  <key>: <value>
```

```json
{
  "name": "User Name",
  "is_admin": false,
  "can_login": true,
  "can_connect": true,
  "can_watch": true,
  "can_host": true,
  "can_share_media": true,
  "can_access_clipboard": true,
  "sends_inactive_cursor": true,
  "can_see_inactive_cursors": true,
  "plugins": {
    "<key>": "<value>"
  }
}
```

--------------------------------

### GHCR Neko Docker Image Naming Convention

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Defines the naming convention for Neko Docker images hosted on the GitHub Container Registry (GHCR). It specifies the structure including optional flavor, application, and version.

```shell
ghcr.io/m1k1o/neko/[<flavor>-]<application>:<version>
```

--------------------------------

### WebRTC Video Capture Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/capture.md

Configuration options for capturing and encoding video for WebRTC clients using Gstreamer. Supports multiple video pipelines, each with customizable Gstreamer descriptions or dynamic generation. All pipelines must use the same video codec.

```APIDOC
WebRTC Video Capture Configuration:

- capture.video.display: The name of the X display to capture. Defaults to the environment variable DISPLAY.
  - Type: string
  - Default: environment variable DISPLAY

- capture.video.codec: The video codec to use for encoding. Supported codecs include vp8, vp9, av1, h264. Client WebRTC implementation compatibility may vary; vp8 and h264 are widely supported.
  - Type: string
  - Allowed values: "vp8", "vp9", "av1", "h264"

- capture.video.ids: A list of pipeline IDs. The first ID in the list serves as the default pipeline.
  - Type: array of strings

- capture.video.pipeline: A shorthand for defining a single Gstreamer pipeline description. This option is ignored if 'capture.video.pipelines' is defined.
  - Type: string (Gstreamer pipeline description)

- capture.video.pipelines: A dictionary where keys are unique pipeline IDs and values are pipeline configurations. Pipelines can be defined dynamically via Expression-Driven Configuration or using a Gstreamer Pipeline Description.
  - Type: object
  - Structure: { "pipeline_id": { "description": "Gstreamer pipeline description" | "expression": "expression string" } }
```

--------------------------------

### Neko Member Management Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Configures how Neko manages members, supporting different providers like file-based or multi-user. Options include password hashing, file paths, admin/user passwords, profile templates, and user lists.

```config
--member.file.hash
  member file provider: whether the passwords are hashed using sha256 or not (recommended) (default true)
--member.file.path string
  member file provider: path to the file containing the users and their passwords
--member.multiuser.admin_password string
  member multiuser provider: password for admin users (default "admin")
--member.multiuser.admin_profile string
  member multiuser provider: profile template for admin users (default "{}")
--member.multiuser.user_password string
  member multiuser provider: password for regular users (default "neko")
--member.multiuser.user_profile string
  member multiuser provider: profile template for regular users (default "{}")
--member.object.users string
  member object provider: list of users with their passwords and profiles (default "[]")
--member.provider string
  selected member provider (default "multiuser")
```

--------------------------------

### GHCR Neko Docker Image Versioning

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Details the versioning tags available for Neko Docker images on GHCR, following Semantic Versioning 2.0.0. Includes 'latest', major, minor, and specific patch versions.

```shell
latest
MAJOR
MAJOR.MINOR
MAJOR.MINOR.PATCH
```

--------------------------------

### Member API Responses

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-create.api.mdx

This section outlines the structure of responses returned by member-related API operations. It covers successful member creation (200 OK) with member details and various error conditions such as unauthorized access (401), forbidden access (403), and pre-condition failures like duplicate member IDs (422).

```APIDOC
Member API Responses:
  200 OK:
    description: Member created successfully.
    content:
      application/json:
        schema:
          type: object
          properties:
            id:
              type: string
              description: The unique identifier of the member.
            profile:
              type: object
              description: The profile information of the member.
              x-tags: ["members"]
              properties:
                name:
                  type: string
                  description: The name of the member.
                is_admin:
                  type: boolean
                  description: Indicates if the member is an admin.
                can_login:
                  type: boolean
                  description: Indicates if the member can log in.
                can_connect:
                  type: boolean
                  description: Indicates if the member can connect.
                can_watch:
                  type: boolean
                  description: Indicates if the member can watch.
                can_host:
                  type: boolean
                  description: Indicates if the member can host.
                can_share_media:
                  type: boolean
                  description: Indicates if the member can share media.
                can_access_clipboard:
                  type: boolean
                  description: Indicates if the member can access the clipboard.
                sends_inactive_cursor:
                  type: boolean
                  description: Indicates if the member sends inactive cursor.
                can_see_inactive_cursors:
                  type: boolean
                  description: Indicates if the member can see inactive cursors.
                plugins:
                  type: object
                  additionalProperties: true
                  description: Additional plugin settings.
          title: MemberData

  401 Unauthorized:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  403 Forbidden:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage

  422 Unprocessable Entity:
    description: Member with chosen ID already exists.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Neko Session Provider Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Configures the session provider for the Neko project. Sessions can be stored in memory (default) or in a file by specifying the 'session.file' path. Future providers like Redis and PostgreSQL are planned.

```APIDOC
session.file: string
  Description: Path to the file where sessions will be stored. If not set, sessions are stored in memory and lost on server restart.
  Example: '/opt/neko/sessions.json'
```

--------------------------------

### Docker Hub Neko Image Naming Convention

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/installation/docker-images.md

Specifies the naming convention for Neko Docker images available on Docker Hub. This registry hosts development versions for AMD64 architecture without flavors.

```shell
m1k1o/neko:<application>
```

--------------------------------

### Generate API Token

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Command to generate a random hexadecimal token suitable for API authentication.

```bash
openssl rand -hex 32
```

--------------------------------

### x86_64 Docker Images (GitHub Container Registry)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Provides a list of m1k1o/neko Docker images for x86_64 architecture available on GitHub Container Registry (GHCR). These are often faster to pull than Docker Hub images.

```shell
docker pull ghcr.io/m1k1o/neko/firefox:latest

docker pull ghcr.io/m1k1o/neko/chromium:latest

docker pull ghcr.io/m1k1o/neko/google-chrome:latest

docker pull ghcr.io/m1k1o/neko/ungoogled-chromium:latest

docker pull ghcr.io/m1k1o/neko/microsoft-edge:latest

docker pull ghcr.io/m1k1o/neko/brave:latest

docker pull ghcr.io/m1k1o/neko/vivaldi:latest

docker pull ghcr.io/m1k1o/neko/opera:latest

docker pull ghcr.io/m1k1o/neko/tor-browser:latest

docker pull ghcr.io/m1k1o/neko/remmina:latest

docker pull ghcr.io/m1k1o/neko/vlc:latest

docker pull ghcr.io/m1k1o/neko/xfce:latest

docker pull ghcr.io/m1k1o/neko/kde:latest
```

--------------------------------

### Neko Fullscreen Incompatibility Fix

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details a fix for fullscreen incompatibility issues.

```APIDOC
Fullscreen Fix:

Safari Incompatibility:
  - Fixed fullscreen incompatibility for Safari [#121].
```

--------------------------------

### HAProxy Defaults Section Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Configures default options for HAProxy, including enabling the 'forwardfor' option to preserve client IP addresses and setting various connection timeouts for client and server interactions.

```haproxy
defaults
    option forwardfor
    timeout connect 30000
    timeout client  65000
    timeout server  65000
```

--------------------------------

### POST /api/login - User Authentication

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/login.api.mdx

This API endpoint handles user authentication. It accepts a JSON payload containing the user's username and password to establish a session. The request body defines the schema for authentication credentials.

```APIDOC
POST /api/login

Description:
  Authenticate a user and start a new session.

Request Body:
  content:
    application/json:
      schema:
        type: object
        properties:
          username:
            type: string
            description: The username of the user.
          password:
            type: string
            description: The password of the user.
        title: SessionLoginRequest
  required: true

Response Status Codes:
  (Details not provided in the source snippet)
```

--------------------------------

### Docker Daemon Configuration for Subnets

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Illustrates the structure of `/etc/docker/daemon.json` to configure Docker's default IP address pools. This is used to change Docker network subnets, resolving conflicts with existing host networks and ensuring proper internet connectivity.

```json
{
  "default-address-pools": [
    {
      "base" : "10.10.0.0/16",
      "size" : 24
    }
  ]
}
```

--------------------------------

### Client Error: Autoplay Policy Block

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

The 'NotAllowedError' occurs when the browser prevents video playback because the user has not interacted with the document first. A manual click on the play button is required to resolve this.

```log
NotAllowedError: play() failed because the user didn't interact with the document first
```

--------------------------------

### Neko Chat Plugin Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/plugins.md

Configuration for the Neko chat plugin, enabling chat support and defining user/room permissions for sending and receiving messages.

```yaml
plugins:
  chat.enabled: true
  chat.can_send: true
  chat.can_receive: true
```

--------------------------------

### Restart HAProxy Service

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Command to restart the HAProxy service. This action is necessary after making any modifications to the HAProxy configuration file to apply the changes.

```shell
service haproxy restart
```

--------------------------------

### Neko NVH264ENC SPS/PPS Fix

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details a fix for the `nvh264enc` component not sending SPS and PPS NAL units.

```APIDOC
NVH264ENC Fix:

SPS/PPS NAL Units:
  - Fixed an issue where `nvh264enc` did not send SPS and PPS NAL units.
```

--------------------------------

### SSH Configuration for Port Forwarding

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/networking.md

Sets up SSH client configuration to forward Neko's ports from the remote server to the local machine, enabling secure access without exposing Neko directly to the internet.

```shell
Host PC-Work
    HostName work.example.com
    User xxx
    Port xyy
    RemoteForward 8080 localhost:8080
    RemoteForward 52000 localhost:52000
```

--------------------------------

### Neko Client Error: Black Screen with Cursor

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This client error manifests as a black screen with a cursor but no browser window. It typically occurs when using chromium-based browsers without the necessary SYS_ADMIN capability flag (`-cap-add=SYS_ADMIN`) during container startup.

```log
Getting black screen with a cursor, but no browser.
```

--------------------------------

### WebRTC ICE Configuration Options

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/webrtc.md

Configuration options for WebRTC ICE (Interactive Connectivity Establishment) settings in Neko. This includes enabling or disabling features like ICE Trickle and ICE Lite, which affect how peer-to-peer connections are established.

```APIDOC
Configuration Option: webrtc.icetrickle
  Description: Controls whether ICE candidates are sent as they are discovered, allowing for faster connection establishment.
  Type: Boolean
  Default: (not specified)
  Related: webrtc.icelite

Configuration Option: webrtc.icelite
  Description: Enables a minimal ICE protocol implementation for servers on public IPs. Must be disabled when using ICE Servers.
  Type: Boolean
  Default: false
  Related: webrtc.icetrickle
  Note: When using ICE Servers, ICE Lite must be disabled.
```

--------------------------------

### Check Neko Server External IP Log

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

Demonstrates how to view Neko server logs to verify the detected external IP address used for client connections. It uses 'docker-compose logs' and 'grep' to filter relevant log lines.

```shell
docker-compose logs neko | grep nat_ips
```

--------------------------------

### Configure Neko with Nvidia GPU Acceleration (Docker Compose)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

This docker-compose configuration sets up Neko for Nvidia GPU acceleration. It defines the image, restart policy, shared memory size, port mappings, system capabilities, and environment variables for screen resolution, passwords, network, video codec, and hardware encoding, including GPU resource reservations.

```yaml
version: "3.4"
services:
  neko:
    image: "ghcr.io/m1k1o/neko/nvidia-google-chrome:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "56000-56100:56000-56100/udp"
    cap_add:
    - SYS_ADMIN
    environment:
      NEKO_SCREEN: '1920x1080@30'
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 56000-56100
      NEKO_NAT1TO1: 192.168.1.10
      NEKO_VIDEO_CODEC: h264
      NEKO_HWENC: nvenc
    deploy:
      resources:
        reservations:
          devices:
          - driver: nvidia
            count: 1
            capabilities: [gpu]
```

--------------------------------

### File Provider members.json Structure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Illustrates the JSON structure for storing user credentials and profiles when using the file provider. Passwords can be plain text or SHA256 hashed.

```json
{
  "<user_login>": {
    "password": "<user_password>",
    "profile": /* Member Profile */
  }
}
```

--------------------------------

### POST /api/room/control/take

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-take.api.mdx

Initiates the process to take control of the room. This endpoint is used to assert control over a shared room resource. It does not require any specific request body parameters but relies on authentication and authorization.

```APIDOC
POST /api/room/control/take

---

**Description:**
Take control of the room.

**Parameters:**
(No parameters defined)

**Request Body:**
(No request body defined)

**Responses:**
- **204 No Content:**
  Control taken successfully.

- **401 Unauthorized:**
  The request requires user authentication.
  **Content:** `application/json`
  ```json
  {
    "message": "Detailed error message."
  }
  ```

- **403 Forbidden:**
  The server understood the request, but refuses to authorize it.
  **Content:** `application/json`
  ```json
  {
    "message": "Detailed error message."
  }
  ```
```

--------------------------------

### Persistent Browser Profile Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Configures a docker-compose service to mount a local directory as a volume for the browser's profile. This ensures that browser settings, bookmarks, extensions, and history persist across container restarts.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - "./profile:/home/neko/.mozilla/firefox/profile.default"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### ARM-based Docker Images (GHCR)

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Provides a list of m1k1o/neko Docker images for ARM-based architectures (like Raspberry Pi) available on GitHub Container Registry. Note that Docker Hub ARM images are not maintained.

```shell
docker pull ghcr.io/m1k1o/neko/arm-firefox:latest

docker pull ghcr.io/m1k1o/neko/arm-chromium:latest

docker pull ghcr.io/m1k1o/neko/arm-ungoogled-chromium:latest

docker pull ghcr.io/m1k1o/neko/arm-vlc:latest

docker pull ghcr.io/m1k1o/neko/arm-xfce:latest
```

--------------------------------

### Neko Character Support

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Information on improved character set support.

```APIDOC
Character Support:

Improved Support:
  - Improved Chinese and Korean characters support.
```

--------------------------------

### No-Auth Provider Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Configuration for the no-authentication provider, which allows any user to log in without credentials. This is intended for testing and development only.

```APIDOC
No-Auth Provider Configuration:
  member.provider: 'noauth'
    - Description: Specifies the no-authentication provider.
    - Warning: Do not use in production environments.
```

--------------------------------

### Room Upload Documentation Structure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/room-upload.tag.mdx

This MDX code snippet uses Docusaurus components to dynamically render a list of documentation cards based on the current sidebar category. It's designed to organize and display related room upload endpoint documentation.

```mdx
import DocCardList from '@theme/DocCardList';
import {useCurrentSidebarCategory} from '@docusaurus/theme-common';

<DocCardList items={useCurrentSidebarCategory().items}/>
```

--------------------------------

### Docker Compose: Chromium Cap Add and Shm Size

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Configures the Neko Chromium Docker image with necessary capabilities (`SYS_ADMIN`) and shared memory size (`shm_size`) to prevent black screens with cursors. This is crucial for proper rendering and operation of Chromium-based browsers within the container.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/chromium:latest"
    # highlight-start
    cap_add:
    - SYS_ADMIN
    # highlight-end
    restart: "unless-stopped"
    # highlight-start
    shm_size: "2gb"
    # highlight-end
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
```

--------------------------------

### Neko URL Parameters

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Describes URL parameters that can be used to control Neko's behavior when embedded or accessed remotely.

```APIDOC
URL Parameters:

?embed=1
  Description: Hides the sidebar and top bar for embedding Neko in other websites.

?volume=<0-1>
  Description: Sets the initial volume of the player. Accepts a float between 0.0 and 1.0.

/screenshot.jpg?pwd=<admin>
  Description: Generates a screenshot of the current desktop session. Requires admin privileges and the room to be unlocked.
```

--------------------------------

### Neko API Endpoint for Statistics

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Provides access to real-time statistics about active connections and host/member counts within the Neko server. Requires administrator privileges via a password parameter.

```APIDOC
GET /stats
  Description: Retrieves server statistics.
  Parameters:
    - pwd (string): Administrator password for authentication.
  Returns:
    - Object: Contains total active connections, host count, and member count.
  Example:
    GET /stats?pwd=admin
```

--------------------------------

### Pulseaudio Log Entries

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

These are informational log messages from the Pulseaudio sound server. Unless audio playback issues are present, these logs can be ignored.

```log
I: [pulseaudio] client.c: Created 0 "Native client (UNIX socket client)"
I: [pulseaudio] protocol-native.c: Client authenticated anonymously.
I: [pulseaudio] source-output.c: Trying to change sample spec
I: [pulseaudio] sink.c: Reconfigured successfully
I: [pulseaudio] source.c: Reconfigured successfully
I: [pulseaudio] client.c: Freed 0 "neko"
I: [pulseaudio] protocol-native.c: Connection died.
```

--------------------------------

### Browser Policy File Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Configures a docker-compose service to mount a local JSON file as the browser's policy file. This allows for programmatic customization of browser settings, such as disabling extensions or setting default configurations.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    volumes:
      - "./policy.json:/usr/lib/firefox/distribution/policies.json"
    environment:
      NEKO_DESKTOP_SCREEN: 1920x1080@30
      NEKO_MEMBER_MULTIUSER_USER_PASSWORD: neko
      NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD: admin
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
```

--------------------------------

### Docker Compose: Custom DNS Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Specifies custom DNS servers (e.g., Cloudflare, Google) within the `docker-compose.yaml` file. This is a solution for the 'No internet in the remote browser' error, ensuring the container can resolve domain names correctly.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/chromium:latest"
    # highlight-start
    dns:
    - 1.1.1.1
    - 8.8.8.8
    # highlight-end
    cap_add:
    - SYS_ADMIN
    restart: "unless-stopped"
    shm_size: "2gb"
    # ...
```

--------------------------------

### Neko API Authentication Schemes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/README.mdx

Details the various authentication methods supported by the Neko API, including API key via cookie, Bearer token, and API key via query parameter. Each scheme specifies its type and relevant parameters for integration.

```APIDOC
Authentication Schemes:

1. CookieAuth:
   - Security Scheme Type: apiKey
   - Parameter Name: NEKO_SESSION (Header)
   - Description: Authentication using a session cookie.

2. Bearer Auth:
   - Security Scheme Type: http
   - HTTP Authorization Scheme: bearer
   - Description: Authentication using a Bearer token.

3. TokenAuth:
   - Security Scheme Type: apiKey
   - Parameter Name: token (Header)
   - Description: Authentication using a token passed as a query parameter.
```

--------------------------------

### Neko Audio on iOS

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Notes on fixing audio playback on iOS devices.

```APIDOC
iOS Audio Fix:

Audio Playback:
  - Audio on iOS now works, specifically for iOS 15+ [#62].
```

--------------------------------

### Neko Touch Event Support

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Confirms the addition of touch event support for mobile devices.

```APIDOC
Touch Event Support:

Mobile Devices:
  - Touch events are now supported on mobile devices.
```

--------------------------------

### Session Management Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/help.txt

Manages session-related settings, including API tokens, control protection, cookie configurations, and heartbeat intervals.

```config
--session.api_token string
  API token for interacting with external services
--session.control_protection
  users can gain control only if at least one admin is in the room
--session.cookie.domain string
  domain of the cookie
--session.cookie.enabled
  whether cookies authentication should be enabled (default true)
--session.cookie.expiration duration
  expiration of the cookie (default 24h0m0s)
--session.cookie.http_only
  use http only cookies (default true)
--session.cookie.name string
  name of the cookie that holds token (default "NEKO_SESSION")
--session.cookie.path string
  path of the cookie
--session.cookie.secure
  use secure cookies (default true)
--session.file string
  if sessions should be stored in a file, otherwise they will be stored only in memory
--session.heartbeat_interval int
  interval in seconds for sending heartbeat messages (default 120)
--session.implicit_hosting
  allow implicit control switching (default true)
--session.inactive_cursors
  show inactive cursors on the screen
--session.locked_controls
  whether controls should be locked for users initially
--session.locked_logins
  whether logins should be locked for users initially
--session.merciful_reconnect
  allow reconnecting to websocket even if previous connection was not closed (default true)
--session.private_mode
  whether private mode should be enabled initially
```

--------------------------------

### Neko File Transfer Plugin Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/plugins.md

Configuration for the Neko file transfer plugin, enabling file transfer functionality, specifying the upload directory, and setting the refresh interval for file lists.

```yaml
plugins:
  filetransfer.enabled: true
  filetransfer.dir: "./uploads"
  filetransfer.refresh_interval: "30s"
```

--------------------------------

### Set Directory Permissions for Persistent Profile

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Sets the ownership of the local profile directory to UID 1000 and GID 1000. This is crucial for the Neko user inside the container to have the necessary read/write permissions for the mounted volume.

```bash
sudo chown -R 1000:1000 ./profile
```

--------------------------------

### Tail HAProxy Log File

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Command to continuously monitor the HAProxy log file in real-time. This is crucial for diagnosing issues by observing incoming requests and server responses.

```shell
tail -f /var/log/haproxy.log
```

--------------------------------

### Set Keyboard Map API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/keyboard-map-set.api.mdx

This API endpoint allows you to update the keyboard map configuration for a room. It accepts a JSON body containing the desired keyboard layout and variant. A successful update returns a 204 No Content status. Authentication is required, and unauthorized or forbidden requests will return appropriate error messages.

```APIDOC
POST /api/room/keyboard/map

---

**Description:**
Update the keyboard map configuration.

**Request Body:**
```json
{
  "layout": "sk",
  "variant": "qwerty"
}
```

**Schema:**
```json
{
  "type": "object",
  "properties": {
    "layout": {
      "type": "string",
      "example": "sk",
      "description": "The keyboard layout."
    },
    "variant": {
      "type": "string",
      "example": "qwerty",
      "description": "The keyboard variant."
    }
  },
  "title": "KeyboardMap"
}
```

**Responses:**

*   **204 No Content:** Keyboard map updated successfully.
*   **401 Unauthorized:** The request requires user authentication.
    ```json
    {
      "message": "Detailed error message."
    }
    ```
*   **403 Forbidden:** The server understood the request, but refuses to authorize it.
    ```json
    {
      "message": "Detailed error message."
    }
    ```
*   **500 Internal Server Error:** Unable to change keyboard map.
    ```json
    {
      "message": "Detailed error message."
    }
    ```
```

--------------------------------

### Neko UnGoogled Chromium Bug Fix

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details a fix for an auto-build bug related to ungoogled-chromium.

```APIDOC
UnGoogled Chromium Fix:

Auto Build Bug:
  - Fixed an auto build bug for ungoogled-chromium.
```

--------------------------------

### Generate SHA256 Hashed Password

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Command to generate a SHA256 hash of a password, suitable for secure storage in the members.json file when hashing is enabled.

```bash
echo -n "password" | openssl sha256 -binary | base64 -
```

--------------------------------

### Iframe Attributes for Fullscreen Embedding

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/faq.md

Specifies the necessary HTML attributes for an iframe element to enable fullscreen functionality when embedding Neko content. This ensures a proper viewing experience.

```html
allowfullscreen="true" webkitallowfullscreen="true" mozallowfullscreen="true"
allow="fullscreen *"
```

--------------------------------

### Import DocCardList Component

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/README.md

Imports the DocCardList component, a common pattern for organizing documentation cards in frameworks like React. This is a standard JavaScript import statement.

```javascript
import DocCardList from '@theme/DocCardList';
```

--------------------------------

### Docker Compose for VPN Access

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/networking.md

Configures Neko for VPN access by exposing necessary ports and setting the NAT1TO1 environment variable to the server's VPN IP address, allowing communication over the VPN.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000-52100:52000-52100/udp"
    environment:
      NEKO_WEBRTC_EPR: 52000-52100
      NEKO_WEBRTC_ICELITE: 1
      NEKO_WEBRTC_NAT1TO1: <your-VPN-IP>
```

--------------------------------

### Check Neko External IP Log

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Command to view Neko server logs and identify the detected external IP address, which is used for client connections. It also shows the ephemeral port range and ICE configuration.

```shell
docker compose logs neko | grep nat_ips
```

--------------------------------

### POST /api/room/clipboard - Set Clipboard Content API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/clipboard-set-text.api.mdx

This API endpoint allows updating the content of the clipboard. It supports both plain text and HTML content. The request body specifies the text and/or HTML to be set.

```APIDOC
POST /api/room/clipboard

Update the content of the clipboard (rich-text or plain-text).

Request Body:
  Content-Type: application/json
  Schema:
    type: object
    properties:
      text: { type: string, example: "Copied Content 123", description: "The plain text content of the clipboard." }
      html: { type: string, example: "<b>Copied Content 123</b>", description: "The HTML content of the clipboard." }
    title: ClipboardText

Responses:
  204: { description: "Clipboard content updated successfully." }
  401: { description: "The request requires user authentication.", content: { application/json: { schema: { type: object, properties: { message: { type: string, description: "Detailed error message." } }, title: ErrorMessage } } }
  403: { description: "The server understood the request, but refuses to authorize it.", content: { application/json: { schema: { type: object, properties: { message: { type: string, description: "Detailed error message." } }, title: ErrorMessage } } }
  500: { description: "Unable to set clipboard content.", content: { application/json: { schema: { type: object, properties: { message: { type: string, description: "Detailed error message." } }, title: ErrorMessage } } }
```

--------------------------------

### HAProxy Frontend and Backend Configuration for Neko

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Configures HAProxy to route HTTP traffic for a specific hostname to the Neko backend service. This involves defining an Access Control List (ACL) based on the host header and directing traffic to the appropriate backend server.

```haproxy
frontend http-in
  #/********
  #* NEKO *
  acl neko_rule_http hdr(host) neko.domain.com # Adapt the domain
  use_backend neko_srv if neko_rule_http
  #********/

backend neko_srv
  mode http
  option httpchk
      server neko 172.16.0.0:8080 # Adapt the IP
```

--------------------------------

### Configure Neko to Use Custom IP Fetcher

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This docker-compose.yaml snippet shows how to configure Neko to use a custom IP fetching service (e.g., an external URL) to determine its public IP address, useful when automatic detection fails.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_ICELITE: 1
      NEKO_IPFETCH: https://ifconfig.co/ip
```

--------------------------------

### Docker Network Conflict Diagnosis

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Provides bash commands to inspect Docker networks and host network configurations. This helps identify and resolve IP subnet conflicts between Docker networks and the host network, which can cause internet connectivity issues for containers.

```bash
$ for n in `docker network ls --format '{{ .ID }}'`; do docker network inspect --format '{{ .IPAM.Config }} {{ .Name }}' $n; done
[] host
[{172.16.0.0/24  172.16.0.1 map[]}] bridge
[{172.17.0.0/24  172.17.0.1 map[]}] neko1-net
[] none
[{172.18.0.0/24  172.18.0.1 map[]}] neko2-net

$ ip route | grep default
default via 172.18.0.1 dev eth0 proto dhcp src 172.18.0.2 metric 100
```

--------------------------------

### POST /api/room/screen - Update Screen Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/screen-configuration-change.api.mdx

Updates the screen configuration of a room. This endpoint accepts a JSON payload specifying the desired screen width, height, and refresh rate. It returns the updated configuration upon success or an error message for invalid requests or authorization issues.

```APIDOC
POST /api/room/screen
  Updates the screen configuration of the room.

  Request Body:
    Content-Type: application/json
    Schema: object
      properties:
        width: integer (example: 1280) - The width of the screen.
        height: integer (example: 720) - The height of the screen.
        rate: integer (example: 30) - The refresh rate of the screen.

  Responses:
    200 OK:
      description: Screen configuration updated successfully.
      content:
        application/json:
          schema:
            type: object
            properties:
              width: integer (example: 1280) - The width of the screen.
              height: integer (example: 720) - The height of the screen.
              rate: integer (example: 30) - The refresh rate of the screen.
    401 Unauthorized:
      description: The request requires user authentication.
      content:
        application/json:
          schema:
            type: object
            properties:
              message: string - Detailed error message.
    403 Forbidden:
      description: The server understood the request, but refuses to authorize it.
      content:
        application/json:
          schema:
            type: object
            properties:
              message: string - Detailed error message.
    422 Unprocessable Entity:
      description: Invalid screen configuration.
      content:
        application/json:
          schema:
            type: object
            properties:
              message: string - Detailed error message.
```

--------------------------------

### Allow File Upload/Download in Chromium Browsers (JSON)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Configures Chromium-based browsers to allow file uploading and downloading. This involves setting `DownloadRestrictions` to 0, enabling `AllowFileSelectionDialogs`, and whitelisting specific file paths.

```json
{
  "DownloadRestrictions": 0,
  "AllowFileSelectionDialogs": true,
  "URLAllowlist": [
    "file:///home/neko/Downloads"
  ]
}
```

--------------------------------

### Neko CSS Styling

Source: https://github.com/m1k1o/neko/blob/master/client/public/index.html

Applies specific styling to the video container, addressing a potential iOS bug where video playback might fail if not configured correctly. This CSS ensures the video container occupies the full width and height.

```css
n.eko        /* weird iOS bug, if this is not set right here, video just does not start */ .video-container { width: 100%; height: 100%; }
```

--------------------------------

### Enable Persistent Data in Browser Policies

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/README.md

Configures browser policies to maintain persistent data, such as cookies and browsing history, when the browser is closed. This overrides the default behavior of clearing data on exit.

```json
{
  "DefaultCookiesSetting": 1,
  "RestoreOnStartup": 1
}
```

--------------------------------

### Batch Request API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/batch.api.mdx

This endpoint allows you to execute multiple API requests in a single batch call. It accepts an array of request objects, each specifying a path, HTTP method, and an optional body. The response is an array of results corresponding to each request.

```APIDOC
POST /api/batch

Description:
  Execute multiple API requests in a single call.

Request Body:
  type: array
  items:
    type: object
    properties:
      path:
        type: string
        description: The API endpoint path.
      method:
        type: string
        enum: ["GET", "POST", "DELETE"]
        description: The HTTP method to be used.
      body:
        description: The request body for the API call.
    required:
      - path
      - method

Responses:
  200 OK:
    description: Batch request executed successfully.
    content:
      application/json:
        schema:
          type: array
          items:
            type: object
            properties:
              path:
                type: string
                description: The API endpoint path.
              method:
                type: string
                enum: ["GET", "POST", "DELETE"]
                description: The HTTP method used.
              body:
                description: The response body from the API call.
              status:
                type: integer
                description: The HTTP status code of the response.

```

--------------------------------

### Release Control API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-release.api.mdx

This endpoint releases control of the room. It handles successful releases and various error conditions like authentication, authorization, and pre-existing host states.

```APIDOC
POST /api/room/control/release

Description:
  Release control of the room.

Parameters:
  None defined.

Request Body:
  None defined.

Responses:
  204:
    description: "Control released successfully."
  401:
    description: "The request requires user authentication."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  403:
    description: "The server understood the request, but refuses to authorize it."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
  422:
    description: "There is already a host."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
```

--------------------------------

### Neko Feature Flags and Control

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Describes features related to user control, locking mechanisms, and data reporting within Neko sessions.

```APIDOC
Control & Feature Flags:

Lock Controls:
  - Global locking of controls for users.
  - Ability to set locks via configuration: `NEKO_LOCKS=control login`.

Control Protection:
  - Users can gain control only if at least one admin is present: `NEKO_CONTROL_PROTECTION=true`.

Emote Sending:
  - Emotes are now sent on mouse down holding.

Stats Data:
  - Included data in stats: `banned`, `locked`, `server_started_at`, `last_admin_left_at`, `last_user_left_at`, `control_protection`.
```

--------------------------------

### Session Creation Error: Invalid NAT IP Mapping

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

Indicates an issue with the 1:1 NAT IP mapping configuration. Check the NEKO_WEBRTC_NAT1TO1 setting or the IP address returned by NEKO_WEBRTC_IP_RETRIEVAL_URL.

```log
WRN session created with an error error="invalid 1:1 NAT IP mapping"
```

--------------------------------

### Check HAProxy Service Status

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Command to check the current operational status of the HAProxy service. Useful for verifying if the service is running correctly after a restart or during troubleshooting.

```shell
service haproxy status
```

--------------------------------

### Update Room Settings POST API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/settings-set.api.mdx

API endpoint to update the settings of a room. It accepts a POST request with room settings in the JSON body and returns status codes indicating success or failure.

```APIDOC
POST /api/room/settings

Description:
  Update the settings of the room.

Request Body:
  content:
    application/json:
      schema:
        type: object
        properties:
          private_mode:
            type: boolean
            description: Indicates if the room is in private mode.
          locked_controls:
            type: boolean
            description: Indicates if the room controls are locked.
          implicit_hosting:
            type: boolean
            description: Indicates if implicit hosting is enabled.
          inactive_cursors:
            type: boolean
            description: Indicates if inactive cursors are shown.
          merciful_reconnect:
            type: boolean
            description: Indicates if merciful reconnect is enabled.
          plugins:
            type: object
            additionalProperties: true
            description: Additional plugin settings.
        title: Settings
  required: true

Responses:
  204:
    description: Room settings updated successfully.
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Disable All Neko Browser Policies

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Creates an empty JSON file to completely disable all predefined policies in Neko browsers. This allows users to customize browser settings freely without policy restrictions.

```json
{}
```

--------------------------------

### Configure Neko with Manual NAT IP Address

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

This docker-compose.yaml snippet illustrates how to manually specify the server's public IP address for Neko using the NEKO_NAT1TO1 environment variable. This is an alternative to automatic IP detection.

```yaml
version: "3.4"
services:
  neko:
    image: "m1k1o/neko:firefox"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
    - "8080:8080"
    - "52000-52100:52000-52100/udp"
    environment:
      NEKO_SCREEN: 1920x1080@30
      NEKO_PASSWORD: neko
      NEKO_PASSWORD_ADMIN: admin
      NEKO_EPR: 52000-52100
      NEKO_ICELITE: 1
      NEKO_NAT1TO1: <your-IP>
```

--------------------------------

### Neko Stats API Response Structure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/migration-from-v2/README.md

Defines the JSON structure returned by the /stats endpoint, providing details on active connections, session members, banned IPs, locked resources, server uptime, and control protection settings.

```json
{
  "connections": 0, // How many connections are currently active
  "host": "<session_id>", // Who is currently having a session (empty if no one)
  "members": [
    {
      "session_id": "<session_id>",
      "displayname": "Name",
      "admin": true,
      "muted": false,
    }
  ], // List of currently connected users
  "banned": {
    "<ip>": "<session_id>" // List of banned IPs and who banned them as a session_id
  },
  "locked": {
    "<resource>": "<session_id>" // List of locked resources and who locked them as a session_id
  },
  "server_started_at": "2021-01-01T00:00:00Z", // Server uptime
  "last_admin_left_at": "2021-01-01T00:00:00Z",
  "last_user_left_at": "2021-01-01T00:00:00Z",
  "control_protection": false, // Whether the control protection is enabled
  "implicit_control": false // Whether implicit control is enabled
}
```

--------------------------------

### RTMP Broadcast Pipeline Connection Error

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

This error indicates a failure to connect to an RTMP stream for broadcasting, often due to ingest server requirements for specific URL parameters like `live=1` or handling of apostrophes. Consult the broadcast pipeline documentation for correct formatting.

```log
Could not connect to RTMP stream "'rtmp://<ingest-url>/live/<stream-key-removed> live=1'" for writing
```

--------------------------------

### API Response Status Codes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-bulk-update.api.mdx

Details the possible HTTP status codes returned by the API, including their meanings and associated error response schemas. This documentation covers successful responses and various error conditions like authentication, authorization, and resource not found.

```APIDOC
API Response Status Codes:

204 No Content:
  description: Members updated successfully.

401 Unauthorized:
  description: The request requires user authentication.
  content:
    application/json:
      schema:
        type: object
        properties:
          message:
            type: string
            description: Detailed error message.
        title: ErrorMessage

403 Forbidden:
  description: The server understood the request, but refuses to authorize it.
  content:
    application/json:
      schema:
        type: object
        properties:
          message:
            type: string
            description: Detailed error message.
        title: ErrorMessage

404 Not Found:
  description: The specified resource was not found.
  content:
    application/json:
      schema:
        type: object
        properties:
          message:
            type: string
            description: Detailed error message.
        title: ErrorMessage
```

--------------------------------

### Neko API User Authentication

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Details the API User, a special user for authenticating HTTP API requests. It uses a token instead of a password and cannot connect to rooms but can perform administrative tasks. The API User is disabled if the token is not set.

```APIDOC
session.api_token: string
  Description: The secret token used to authenticate API requests. If not set, the API User is disabled.
  Example: '<secret_token>'

  Note: This token can be generated using 'openssl rand -hex 32'.
```

--------------------------------

### HAProxy Global Timeout Configuration

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/reverse-proxy.md

Sets the global statistics timeout for HAProxy. This parameter defines how long the stats page will remain valid before requiring a refresh, ensuring up-to-date monitoring information.

```haproxy
global
    stats timeout 60s
```

--------------------------------

### Member Retrieval API Responses

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-list.api.mdx

Details the HTTP responses for retrieving members. Includes successful retrieval of member data (200 OK) and error responses for authentication (401 Unauthorized) and authorization (403 Forbidden) failures.

```APIDOC
GET /members
  Description: Retrieves a list of members.
  Responses:
    200 OK:
      Description: Members retrieved successfully.
      Content:
        application/json:
          Schema:
            type: array
            items:
              type: object
              properties:
                id:
                  type: string
                  description: The unique identifier of the member.
                profile:
                  type: object
                  description: The profile information of the member.
                  properties:
                    name:
                      type: string
                      description: The name of the member.
                    is_admin:
                      type: boolean
                      description: Indicates if the member is an admin.
                    can_login:
                      type: boolean
                      description: Indicates if the member can log in.
                    can_connect:
                      type: boolean
                      description: Indicates if the member can connect.
                    can_watch:
                      type: boolean
                      description: Indicates if the member can watch.
                    can_host:
                      type: boolean
                      description: Indicates if the member can host.
                    can_share_media:
                      type: boolean
                      description: Indicates if the member can share media.
                    can_access_clipboard:
                      type: boolean
                      description: Indicates if the member can access the clipboard.
                    sends_inactive_cursor:
                      type: boolean
                      description: Indicates if the member sends inactive cursor.
                    can_see_inactive_cursors:
                      type: boolean
                      description: Indicates if the member can see inactive cursors.
                    plugins:
                      type: object
                      additionalProperties: true
                      description: Additional plugin settings.
    401 Unauthorized:
      Description: The request requires user authentication.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
    403 Forbidden:
      Description: The server understood the request, but refuses to authorize it.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
```

--------------------------------

### POST /api/room/upload/dialog

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/upload-dialog.api.mdx

Upload a file to a dialog. This endpoint handles file uploads for dialogs, accepting multipart/form-data.

```APIDOC
POST /api/room/upload/dialog

Upload a file to a dialog.

Request Body:
  Content-Type: multipart/form-data
  Schema:
    type: object
    properties:
      files:
        type: array
        description: Files to be uploaded.
        items:
          type: string
          format: binary

Responses:
  204: File uploaded to dialog successfully.
  400: Unable to upload file.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  401: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  422: No upload dialog prompt active.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  500: Unable to process uploaded file.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Neko Emoji and File Transfer

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Details the addition of emoji support and file transfer functionality.

```APIDOC
Emoji & File Transfer:

Emoji Support:
  - Added emoji support.

File Transfer:
  - Added file transfer functionality.
```

--------------------------------

### Neko Broadcast Pipeline RTMP Connection Error

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

An error encountered when the broadcast pipeline cannot connect to an RTMP stream for writing. This often requires adjusting the RTMP URL with specific parameters like `live=1` or removing problematic characters like apostrophes, depending on the ingest server.

```log
Could not connect to RTMP stream "'rtmp://<ingest-url>/live/<stream-key-removed> live=1'" for writing
```

--------------------------------

### API Response Status Codes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-update-profile.api.mdx

This section details the possible HTTP status codes returned by the API, along with their descriptions and potential error response schemas. It covers successful responses and various error conditions like authentication, authorization, and resource not found.

```APIDOC
HTTP Response Status Codes:

204 No Content
  - Description: Member profile updated successfully.

401 Unauthorized
  - Description: The request requires user authentication.
  - Content:
    - application/json:
      - Schema:
        - title: ErrorMessage
        - type: object
        - properties:
          - message:
            - type: string
            - description: Detailed error message.

403 Forbidden
  - Description: The server understood the request, but refuses to authorize it.
  - Content:
    - application/json:
      - Schema:
        - title: ErrorMessage
        - type: object
        - properties:
          - message:
            - type: string
            - description: Detailed error message.

404 Not Found
  - Description: The specified resource was not found.
  - Content:
    - application/json:
      - Schema:
        - title: ErrorMessage
        - type: object
        - properties:
          - message:
            - type: string
            - description: Detailed error message.
```

--------------------------------

### POST /api/room/upload/drop - Upload and Drop File

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/upload-drop.api.mdx

Uploads a file and drops it at a specified location. It handles file uploads via multipart/form-data and specifies drop coordinates (X, Y). Supports various response codes for success and different error scenarios.

```APIDOC
POST /api/room/upload/drop

Upload a file and drop it at a specified location.

Request Body:
  Content-Type: multipart/form-data
  Schema:
    type: object
    properties:
      x: { type: number, description: "X coordinate of drop." }
      y: { type: number, description: "Y coordinate of drop." }
      files: { type: array, description: "Files to be uploaded.", items: { type: string, format: "binary" } }

Responses:
  204: File uploaded and dropped successfully.
  400: Unable to upload file.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
  401: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
  403: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
  500: Unable to process uploaded file.
    content:
      application/json:
        schema:
          type: object
          properties:
            message: { type: string, description: "Detailed error message." }
          title: ErrorMessage
```

--------------------------------

### POST /api/room/control/give/{sessionId} - Give Room Control API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-give.api.mdx

This API endpoint allows a user to transfer control of a room to a specified session. It requires the target session ID as a path parameter. Successful execution returns a 204 status code, while various error conditions like invalid sessions or authentication failures are handled with specific error codes and messages.

```APIDOC
POST /api/room/control/give/{sessionId}

Give control of the room to a specific session.

Parameters:
  - in: path
    name: sessionId
    description: The identifier of the session.
    required: true
    schema:
      type: string

Responses:
  204:
    description: Control given successfully.
  400:
    description: Target session is not allowed to host.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  404:
    description: The specified resource was not found.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### API Response Status Codes

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/sessions-get.api.mdx

Details the HTTP status codes and their corresponding response payloads for the Neko API. This includes successful session retrieval and various error conditions like authentication and authorization failures.

```APIDOC
API Response Status Codes:

200 OK
  Description: Sessions retrieved successfully.
  Content: application/json
  Schema:
    type: array
    items:
      type: object
      properties:
        id:
          type: string
          description: The unique identifier of the session.
        profile:
          type: object
          description: The profile information of the user.
          x-tags: ["members"]
          properties:
            name:
              type: string
              description: The name of the member.
            is_admin:
              type: boolean
              description: Indicates if the member is an admin.
            can_login:
              type: boolean
              description: Indicates if the member can log in.
            can_connect:
              type: boolean
              description: Indicates if the member can connect.
            can_watch:
              type: boolean
              description: Indicates if the user can watch.
            can_host:
              type: boolean
              description: Indicates if the user can host.
            can_share_media:
              type: boolean
              description: Indicates if the user can share media.
            can_access_clipboard:
              type: boolean
              description: Indicates if the user can access the clipboard.
            sends_inactive_cursor:
              type: boolean
              description: Indicates if the member sends inactive cursor.
            can_see_inactive_cursors:
              type: boolean
              description: Indicates if the member can see inactive cursors.
            plugins:
              type: object
              additionalProperties: true
              description: Additional plugin settings.
          title: MemberProfile
        state:
          type: object
          description: The current state of the session.
          properties:
            is_connected:
              type: boolean
              description: Indicates if the user is connected.
            is_watching:
              type: boolean
              description: Indicates if the user is watching.
          title: SessionState
      title: SessionData

401 Unauthorized
  Description: The request requires user authentication.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
    title: ErrorMessage

403 Forbidden
  Description: The server understood the request, but refuses to authorize it.
  Content: application/json
  Schema:
    type: object
    properties:
      message:
        type: string
        description: Detailed error message.
    title: ErrorMessage
```

--------------------------------

### MemberProfile Schema

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/schemas/memberprofile.schema.mdx

Defines the MemberProfile object structure, detailing user permissions and settings. This schema is used for representing member data.

```APIDOC
MemberProfile:
  type: object
  description: Represents a member's profile and permissions.
  properties:
    name: { type: string, description: "The name of the member." }
    is_admin: { type: boolean, description: "Indicates if the member is an admin." }
    can_login: { type: boolean, description: "Indicates if the member can log in." }
    can_connect: { type: boolean, description: "Indicates if the member can connect." }
    can_watch: { type: boolean, description: "Indicates if the member can watch." }
    can_host: { type: boolean, description: "Indicates if the member can host." }
    can_share_media: { type: boolean, description: "Indicates if the member can share media." }
    can_access_clipboard: { type: boolean, description: "Indicates if the member can access the clipboard." }
    sends_inactive_cursor: { type: boolean, description: "Indicates if the member sends inactive cursor." }
    can_see_inactive_cursors: { type: boolean, description: "Indicates if the member can see inactive cursors." }
    plugins: { type: object, additionalProperties: true, description: "Additional plugin settings." }
```

--------------------------------

### Neko Broadcast Status

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Information on how broadcast status changes are communicated to administrators.

```APIDOC
Broadcast Status:

Admin Notifications:
  - Broadcast status changes are sent to all admins.
```

--------------------------------

### POST /api/room/control/reset

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-reset.api.mdx

Resets the control status of the room. This endpoint handles the POST request to reset room controls, returning a 204 on success or appropriate error codes.

```APIDOC
POST /api/room/control/reset
  Description: Reset the control status of the room.
  Parameters:
    (No explicit request body parameters defined in the provided snippet)
  Responses:
    204: Control reset successfully.
    401: The request requires user authentication.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    403: The server understood the request, but refuses to authorize it.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
```

--------------------------------

### Enable Persistent Data in Chromium Browsers (JSON)

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Enables persistent data storage, such as cookies and browsing history, for Chromium-based browsers. It sets `DefaultCookiesSetting` to 1 and `RestoreOnStartup` to 1 to retain data across sessions.

```json
{
  "DefaultCookiesSetting": 1,
  "RestoreOnStartup": 1
}
```

--------------------------------

### Docker Compose for SSH Access

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/networking.md

Configures Neko for secure SSH access by enabling TCP multiplexing and setting NAT1TO1 to the loopback address (127.0.0.1), facilitating access via SSH port forwarding.

```yaml
services:
  neko:
    image: "ghcr.io/m1k1o/neko/nvidia-firefox:latest"
    restart: "unless-stopped"
    shm_size: "2gb"
    ports:
      - "8080:8080"
      - "52000:52000"
    environment:
      NEKO_WEBRTC_TCPMUX: 52000
      NEKO_WEBRTC_ICELITE: 1
      NEKO_WEBRTC_NAT1TO1: 127.0.0.1
```

--------------------------------

### POST /api/room/control/request

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/control-request.api.mdx

Requests control of a room. This endpoint handles the process of a user requesting to take control, detailing successful requests and various error conditions such as authentication failures, authorization issues, or pre-existing host conflicts.

```APIDOC
POST /api/room/control/request
  Description: Request control of the room.
  Responses:
    204:
      Description: Control requested successfully.
    401:
      Description: The request requires user authentication.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    403:
      Description: The server understood the request, but refuses to authorize it.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
    422:
      Description: There is already a host.
      Content:
        application/json:
          Schema:
            type: object
            properties:
              message:
                type: string
                description: Detailed error message.
            title: ErrorMessage
```

--------------------------------

### Client Connection Error: WebSocket

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

A Firefox error indicating a failure to establish a WebSocket connection to the server. Check if the TCP port is exposed, the reverse proxy correctly handles WebSocket connections, and if the browser has not disabled them.

```log
Firefox can’t establish a connection to the server at ws://<your-IP>/ws?password=neko.
```

--------------------------------

### Find Firefox Extension ID

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Provides JavaScript code snippets to help users find the unique identifier (ID) for Firefox extensions. One method involves inspecting the browser's internal state via `about:debugging`, while another uses a console command on the add-ons webpage.

```javascript
Object.keys(JSON.parse(document.getElementById('redux-store-state').textContent).addons.byGUID)[0]
```

--------------------------------

### Neko Client Error: WebSocket Connection Failure

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

A common client-side error where Firefox cannot establish a WebSocket connection to the server. This can be due to incorrect TCP port exposure, reverse proxy misconfiguration, or browser settings disabling WebSockets.

```log
Firefox can’t establish a connection to the server at ws://<your-IP>/ws?password=neko.
```

--------------------------------

### Neko Cookie Authentication Settings

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/configuration/authentication.md

Configuration options for managing cookie-based authentication between the client and server. Cookies are enabled by default but can be disabled. Security implications of disabling cookies are highlighted.

```APIDOC
session.cookie.enabled: boolean
  Description: Whether cookies are enabled for authentication. Defaults to true.

session.cookie.name: string
  Description: The name of the cookie used to store the session.

session.cookie.expiration: string
  Description: Expiration time of the cookie, using Go duration format (e.g., '24h', '1h30m', '60m').

session.cookie.secure: boolean
  Description: Ensures the cookie is only sent over HTTPS. Defaults to true. Recommended to keep true unless using HTTP for testing/development.

session.cookie.http_only: boolean
  Description: Ensures the cookie cannot be accessed by JavaScript. Defaults to true. Recommended to keep true.

session.cookie.domain: string
  Description: Defines the domain for which the cookie is valid.

session.cookie.path: string
  Description: Defines the path for which the cookie is valid.
```

--------------------------------

### Browser Error: D-Bus Connection Failure

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/troubleshooting.md

This error originates from the browser's attempt to connect to the D-Bus system, failing to parse the server address. This message can be safely ignored as it does not affect Neko's functionality.

```log
[ERROR:bus.cc(393)] Failed to connect to the bus: Could not parse server address: Unknown address type (examples of valid types are "tcp" and on UNIX "unix")
```

--------------------------------

### Neko Server Error: Invalid NAT IP Mapping

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

A common server warning indicating an issue with the 1:1 NAT IP mapping configuration. This often occurs when the NEKO_NAT1TO1 environment variable is set to a local IP address, which is unreachable by external clients.

```log
WRN session created with and error error="invalid 1:1 NAT IP mapping"
```

--------------------------------

### Enable Persistent Data in Neko Browser

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/customization/browsers.md

Configures Firefox policies to allow persistent data, such as cookies and browsing history, to be retained when the browser is closed. This is achieved by setting `SanitizeOnShutdown` to false and configuring the `Homepage` to restore the previous session.

```json
{
  "policies": {
    "SanitizeOnShutdown": false,
    "Homepage": {
      "StartPage": "previous-session"
    }
  }
}
```

--------------------------------

### POST /api/members_bulk/update - Bulk Update Members

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-bulk-update.api.mdx

API endpoint to update the profiles of multiple members in bulk. It accepts a JSON body containing member IDs and the profile updates. This operation allows for efficient modification of multiple member profiles in a single request.

```APIDOC
POST /api/members_bulk/update

Update the profiles of multiple members in bulk.

Request Body:
  application/json:
    schema: MemberBulkUpdate

MemberBulkUpdate:
  type: object
  properties:
    ids:
      type: array
      items:
        type: string
      description: The list of member IDs to be updated.
    profile:
      $ref: '#/components/schemas/MemberProfile'
  required:
    - ids
    - profile

MemberProfile:
  type: object
  description: The new profile information for the members.
  properties:
    name:
      type: string
      description: The name of the member.
    is_admin:
      type: boolean
      description: Indicates if the member is an admin.
    can_login:
      type: boolean
      description: Indicates if the member can log in.
    can_connect:
      type: boolean
      description: Indicates if the member can connect.
    can_watch:
      type: boolean
      description: Indicates if the member can watch.
    can_host:
      type: boolean
      description: Indicates if the member can host.
    can_share_media:
      type: boolean
      description: Indicates if the member can share media.
    can_access_clipboard:
      type: boolean
      description: Indicates if the member can access the clipboard.
    sends_inactive_cursor:
      type: boolean
      description: Indicates if the member sends inactive cursor.
    can_see_inactive_cursors:
      type: boolean
      description: Indicates if the member can see inactive cursors.
    plugins:
      type: object
      additionalProperties: true
      description: Additional plugin settings.


```

--------------------------------

### POST /api/profile - Update User Profile

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/profile.api.mdx

Updates the current user's profile without syncing it with the member profile. This feature is experimental and requires user authentication.

```APIDOC
POST /api/profile

Updates the current user's profile without syncing it with the member profile (experimental).

Request Body:
  Type: object (MemberProfile)
  Properties:
    name: string - The name of the member.
    is_admin: boolean - Indicates if the member is an admin.
    can_login: boolean - Indicates if the member can log in.
    can_connect: boolean - Indicates if the member can connect.
    can_watch: boolean - Indicates if the member can watch.
    can_host: boolean - Indicates if the member can host.
    can_share_media: boolean - Indicates if the member can share media.
    can_access_clipboard: boolean - Indicates if the member can access the clipboard.
    sends_inactive_cursor: boolean - Indicates if the member sends inactive cursor.
    can_see_inactive_cursors: boolean - Indicates if the member can see inactive cursors.
    plugins: object - Additional plugin settings.

Responses:
  204: Profile updated successfully.
  401: The request requires user authentication.
    Content:
      application/json:
        Schema:
          type: object
          properties:
            message: string - Detailed error message.
```

--------------------------------

### Neko Browser Error: Failed to Connect to Bus

Source: https://github.com/m1k1o/neko/blob/master/webpage/versioned_docs/version-v2/troubleshooting.md

An error originating from the browser's internal components, specifically related to connecting to the D-Bus system. This error is generally ignorable and does not affect Neko's functionality.

```log
[ERROR:bus.cc(393)] Failed to connect to the bus: Could not parse server address: Unknown address type (examples of valid types are "tcp" and on UNIX "unix")
```

--------------------------------

### POST /api/logout - User Logout

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/logout.api.mdx

Terminates the current user session. This endpoint handles the logout process for authenticated users.

```APIDOC
POST /api/logout

Terminate the current user session.

Responses:
  "200":
    description: "User logged out successfully."
  "401":
    description: "The request requires user authentication."
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: "Detailed error message."
          title: "ErrorMessage"
```

--------------------------------

### POST /api/room/keyboard/modifiers - Set Keyboard Modifiers

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/keyboard-modifiers-set.api.mdx

Updates the status of keyboard modifier keys such as Shift, Caps Lock, Control, Alt, Meta, Super, and AltGr. The request body is a JSON object containing boolean values for each modifier key. A successful update returns a 204 No Content status. Unauthorized or forbidden access returns 401 or 403 respectively, with an error message.

```APIDOC
POST /api/room/keyboard/modifiers

---

**Description:**
Update the keyboard modifiers status.

**Request Body:**
```json
{
  "shift": true, 
  "capslock": false, 
  "control": false, 
  "alt": false, 
  "numlock": false, 
  "meta": false, 
  "super": false, 
  "altgr": false
}
```

**Schema:**
```json
{
  "type": "object",
  "properties": {
    "shift": {
      "type": "boolean",
      "description": "Indicates if the shift key is pressed."
    },
    "capslock": {
      "type": "boolean",
      "description": "Indicates if the caps lock key is active."
    },
    "control": {
      "type": "boolean",
      "description": "Indicates if the control key is pressed."
    },
    "alt": {
      "type": "boolean",
      "description": "Indicates if the alt key is pressed."
    },
    "numlock": {
      "type": "boolean",
      "description": "Indicates if the num lock key is active."
    },
    "meta": {
      "type": "boolean",
      "description": "Indicates if the meta key is pressed."
    },
    "super": {
      "type": "boolean",
      "description": "Indicates if the super key is pressed."
    },
    "altgr": {
      "type": "boolean",
      "description": "Indicates if the altgr key is pressed."
    }
  },
  "title": "KeyboardModifiers"
}
```

**Responses:**

*   **204 No Content:** Keyboard modifiers updated successfully.
*   **401 Unauthorized:** The request requires user authentication.
    ```json
    {
      "message": "Detailed error message."
    }
    ```
*   **403 Forbidden:** The server understood the request, but refuses to authorize it.
    ```json
    {
      "message": "Detailed error message."
    }
    ```
```

--------------------------------

### Neko Emoji Matching Regex

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/release-notes.md

Specifies the regular expression used for emoji matching and a fix applied.

```APIDOC
Emoji Matching:

Regex Update:
  - Fixed bad emoji matching with new regex `/^:([^:\s]+):/` for patterns like `:+1:` and `:100:`.
```

--------------------------------

### POST /api/members_bulk/delete - Bulk Delete Members

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-bulk-delete.api.mdx

Removes multiple members from the system in a single request. It expects a JSON payload containing an array of member IDs to be deleted. Successful deletion returns a 204 No Content status. Authentication and authorization are required.

```APIDOC
POST /api/members_bulk/delete

Description:
  Remove multiple members in bulk.

Request Body:
  content:
    application/json:
      schema:
        type: object
        properties:
          ids:
            type: array
            items:
              type: string
            description: The list of member IDs to be deleted.
        title: MemberBulkDelete
      required: true

Responses:
  204:
    description: Members removed successfully.
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  404:
    description: The specified resource was not found.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Stop Broadcast API Endpoint

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/broadcast-stop.api.mdx

This API endpoint allows you to stop broadcasting the room's content. It requires POST request to the /api/room/broadcast/stop path. The endpoint returns a 204 status code on success, or error codes like 401 (Unauthorized), 403 (Forbidden), or 422 (Server not broadcasting) with detailed error messages.

```APIDOC
POST /api/room/broadcast/stop

Description: Stop broadcasting the room's content.

Responses:
  204:
    description: Broadcast stopped successfully.
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  422:
    description: Server is not broadcasting.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Remove Member API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-remove.api.mdx

This endpoint allows for the removal of a specific member from the system. It requires the member's unique identifier in the path. Successful removal returns a 204 No Content status. Authentication and authorization errors are handled with 401 and 403 status codes respectively, while a 404 indicates the member was not found.

```APIDOC
DELETE /api/members/{memberId}

Description:
  Remove a specific member.

Parameters:
  - in: path
    name: memberId
    description: The identifier of the member.
    required: true
    schema:
      type: string

Responses:
  204:
    description: Member removed successfully.
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  404:
    description: The specified resource was not found.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```

--------------------------------

### Close File Chooser Dialog API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/upload-dialog-close.api.mdx

This API endpoint is used to close the file chooser dialog. It performs a DELETE request to the /api/room/upload/dialog path. The endpoint returns a 204 status code on success. Error conditions include unauthorized access (401), forbidden access (403), or when no upload dialog is active (422).

```APIDOC
OpenAPI:
  paths:
    /api/room/upload/dialog:
      delete:
        summary: Close File Chooser Dialog
        operationId: closeFileChooserDialog
        tags:
          - Room
        responses:
          "204":
            description: File chooser dialog closed successfully.
          "401":
            description: The request requires user authentication.
            content:
              application/json:
                schema:
                  type: object
                  properties:
                    message:
                      type: string
                      description: Detailed error message.
                  title: ErrorMessage
          "403":
            description: The server understood the request, but refuses to authorize it.
            content:
              application/json:
                schema:
                  type: object
                  properties:
                    message:
                      type: string
                      description: Detailed error message.
                  title: ErrorMessage
          "422":
            description: No upload dialog prompt active.
            content:
              application/json:
                schema:
                  type: object
                  properties:
                    message:
                      type: string
                      description: Detailed error message.
                  title: ErrorMessage
```

--------------------------------

### DELETE /api/sessions/{sessionId} - Remove Session

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/session-remove.api.mdx

Terminates a specific session identified by its ID. This endpoint requires the session ID as a path parameter. It does not accept a request body. Successful termination returns a 204 No Content status. Error responses include 401 Unauthorized, 403 Forbidden, and 404 Not Found, each with a descriptive error message.

```APIDOC
DELETE /api/sessions/{sessionId}
  - Terminates a specific session.
  - Parameters:
    - in: path
      name: sessionId
      description: The identifier of the session.
      required: true
      schema:
        type: string
  - Responses:
    - 204:
        description: Session removed successfully.
    - 401:
        description: The request requires user authentication.
        content:
          application/json:
            schema:
              type: object
              properties:
                message:
                  type: string
                  description: Detailed error message.
              title: ErrorMessage
    - 403:
        description: The server understood the request, but refuses to authorize it.
        content:
          application/json:
            schema:
              type: object
              properties:
                message:
                  type: string
                  description: Detailed error message.
              title: ErrorMessage
    - 404:
        description: The specified resource was not found.
        content:
          application/json:
            schema:
              type: object
              properties:
                message:
                  type: string
                  description: Detailed error message.
              title: ErrorMessage
```

--------------------------------

### POST /api/members/{memberId} - Update Member Profile

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-update-profile.api.mdx

Updates the profile of a specific member using their unique identifier. This endpoint accepts a JSON payload containing the updated member details. Ensure the memberId in the path is valid and the request body adheres to the specified schema.

```APIDOC
POST /api/members/{memberId}

Update Member Profile

Updates the profile of a specific member.

Parameters:
  - in: path
    name: memberId
    description: The identifier of the member.
    required: true
    schema:
      type: string

Request Body:
  content:
    application/json:
      schema:
        type: object
        properties:
          name:
            type: string
            description: The name of the member.
          is_admin:
            type: boolean
            description: Indicates if the member is an admin.
          can_login:
            type: boolean
            description: Indicates if the member can log in.
          can_connect:
            type: boolean
            description: Indicates if the member can connect.
          can_watch:
            type: boolean
            description: Indicates if the member can watch.
          can_host:
            type: boolean
            description: Indicates if the member can host.
          can_share_media:
            type: boolean
            description: Indicates if the member can share media.
          can_access_clipboard:
            type: boolean
            description: Indicates if the member can access the clipboard.
          sends_inactive_cursor:
            type: boolean
            description: Indicates if the member sends inactive cursor.
          can_see_inactive_cursors:
            type: boolean
            description: Indicates if the member can see inactive cursors.
          plugins:
            type: object
            additionalProperties: true
            description: Additional plugin settings.
        title: MemberProfile
  required: true


```

--------------------------------

### API: POST /api/sessions/{sessionId}/disconnect

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/session-disconnect.api.mdx

Forcefully disconnect a specific session. This endpoint requires the session's unique identifier as a path parameter. It returns a 204 on successful disconnection or appropriate error codes for authentication, authorization, or not found scenarios.

```APIDOC
POST /api/sessions/{sessionId}/disconnect

---

**Description:**
Forcefully disconnect a specific session.

**Parameters:**

*   **Path Parameters:**
    *   `sessionId` (string, required): The identifier of the session.

**Responses:**

*   **204 No Content:**
    Session disconnected successfully.

*   **401 Unauthorized:**
    The request requires user authentication.
    *   **Content:** `application/json`
        ```json
        {
          "message": "Detailed error message."
        }
        ```

*   **403 Forbidden:**
    The server understood the request, but refuses to authorize it.
    *   **Content:** `application/json`
        ```json
        {
          "message": "Detailed error message."
        }
        ```

*   **404 Not Found:**
    The specified resource was not found.
    *   **Content:** `application/json`
        ```json
        {
          "message": "Detailed error message."
        }
        ```


```

--------------------------------

### Update Member Password API

Source: https://github.com/m1k1o/neko/blob/master/webpage/docs/api/members-update-password.api.mdx

This endpoint allows updating the password for a specific member. It requires the member's ID in the path and the new password in the request body. Successful updates return a 204 status code.

```APIDOC
POST /api/members/{memberId}/password

Description:
  Update the password of a specific member.

Parameters:
  - in: path
    name: memberId
    description: The identifier of the member.
    required: true
    schema:
      type: string

Request Body:
  required: true
  content:
    application/json:
      schema:
        type: object
        properties:
          password:
            type: string
            description: The new password for the member.
        title: MemberPassword

Responses:
  204:
    description: Member password updated successfully.
  401:
    description: The request requires user authentication.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  403:
    description: The server understood the request, but refuses to authorize it.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
  404:
    description: The specified resource was not found.
    content:
      application/json:
        schema:
          type: object
          properties:
            message:
              type: string
              description: Detailed error message.
          title: ErrorMessage
```