# Guidepup Screen Reader Investigation - Notes

## Investigation Goal
Evaluate options for using [guidepup](https://github.com/guidepup/guidepup) in a headless Linux server environment to record audio sessions of a simulated screen reader user navigating a web application.

## What is Guidepup?

Guidepup is a screen reader driver for test automation. It provides a unified API to control screen readers programmatically.

### Core guidepup package
- Supports **VoiceOver** (macOS via AppleScript) and **NVDA** (Windows via TLS socket on port 6837)
- **No Linux support** - no Orca, no AT-SPI integration
- Captures **text** of what the screen reader speaks, not audio
- ~3,576 lines of TypeScript, 98 source files

### @guidepup/virtual-screen-reader
- Separate package: screen reader **simulator** for unit tests
- Runs in **pure JavaScript** - no real screen reader needed
- Parses DOM into accessibility tree, simulates navigation
- Produces spoken phrase logs matching real screen reader output
- Works with jsdom (no browser needed) or real browsers
- **Works perfectly in this environment**

### @guidepup/record
- Screen **video** recording (not audio)
- macOS only uses screencapture, Windows uses ffmpeg + gdigrab
- **No Linux support at all**
- No audio recording capability

## Environment Assessment

Ubuntu 24.04.3 LTS, headless server:
- **No display**: `$DISPLAY` empty, no Wayland
- **No audio**: No PulseAudio, no ALSA, no /dev/snd/
- **No screen readers**: No Orca, no espeak
- **Available**: Xvfb, Node.js 22, Playwright 1.56.1, Chromium (bundled), dbus-daemon, AT-SPI2 libraries (partial)
- **Installable via apt**: Everything needed (orca, espeak-ng, pulseaudio, at-spi2-core, ffmpeg)

## What I Tried

### 1. Installing the Linux accessibility stack
```
apt-get install at-spi2-core libatk-adaptor orca espeak-ng speech-dispatcher speech-dispatcher-espeak-ng pulseaudio pulseaudio-utils dbus-x11 ffmpeg
```
All packages installed successfully.

### 2. Starting the infrastructure
- **Xvfb**: `Xvfb :99 -screen 0 1280x1024x24 -ac &` - worked, non-fatal xkbcomp warnings only
- **D-Bus**: `eval $(dbus-launch --sh-syntax)` - worked
- **PulseAudio**: `pulseaudio --start --exit-idle-time=-1` - worked (warning about running as root, but functional)
- **Virtual audio sinks**: `pactl load-module module-null-sink sink_name=virtual_speaker` - worked
- **AT-SPI registry**: `/usr/libexec/at-spi2-registryd &` - started, registered on D-Bus

### 3. Orca screen reader
- **First attempt failed**: Python GI module compiled for Python 3.12 but system `python3` points to 3.11
- **Fix**: Run Orca explicitly with `python3.12 /usr/bin/orca --replace` - worked
- Orca registered in AT-SPI as an application

### 4. Chromium + AT-SPI integration
- Launched with `--force-renderer-accessibility --no-sandbox --disable-gpu`
- After ~5 seconds, appeared in AT-SPI desktop as "Chromium"
- Full accessibility tree visible through AT-SPI:
  - Page structure (frames, panels, toolbars)
  - Web content: headings, paragraphs, buttons, links, forms
  - All ARIA attributes and roles preserved

### 5. AT-SPI tree walking
- Used Python gi.repository.Atspi to walk the tree
- Successfully extracted all semantic content from web pages
- Roles, names, text content, descriptions all available

### 6. TTS audio generation
- **espeak-ng direct**: `espeak-ng -w output.wav "text"` - generates WAV files
- **espeak-ng via PulseAudio**: `espeak-ng "text"` plays through virtual_speaker sink
- **Recording from PulseAudio**: `parecord --device=virtual_speaker.monitor` captures audio
- **ffmpeg recording**: `ffmpeg -f pulse -i virtual_speaker.monitor output.wav` also works

### 7. Virtual screen reader approach
- Installed `@guidepup/virtual-screen-reader` and `jsdom`
- Fed HTML to jsdom, ran virtual screen reader navigation
- Collected all spoken phrases (matching real screen reader output format)
- Piped phrases through espeak-ng to generate WAV files
- **This works completely without any display, audio, or screen reader infrastructure**

## Key Findings

### What works NOW (proven with code):

1. **Approach A: AT-SPI + Real Browser** (full fidelity, more complex)
   - Xvfb + D-Bus + PulseAudio + AT-SPI + Orca + Chromium
   - Walk real accessibility tree via AT-SPI API
   - Generate audio from extracted phrases via espeak-ng
   - Record PulseAudio output with ffmpeg/parecord
   - Produces: text transcript + WAV audio

2. **Approach B: Virtual Screen Reader** (simpler, no infrastructure)
   - jsdom + @guidepup/virtual-screen-reader + espeak-ng
   - No display, no D-Bus, no PulseAudio, no screen reader needed
   - Simulates screen reader navigation through DOM
   - Generates spoken phrase logs matching real screen reader behavior
   - Pipes phrases through espeak-ng for audio
   - Produces: text transcript + WAV audio

### What doesn't work:
- **Core guidepup package**: macOS/Windows only, no Linux support
- **@guidepup/record**: No Linux support, video-only anyway
- **Orca with Python 3.11**: GI module version mismatch (fixable with python3.12)
- **PulseAudio audio playback**: Audio "plays" to null sink but there's no actual speaker output (by design - we record from the monitor)

### Guidepup Linux support gap:
The guidepup project has no Linux screen reader integration. Adding it would require:
- AT-SPI2 D-Bus protocol implementation (or python-atspi bindings via subprocess)
- Orca process management
- Speech-dispatcher or espeak-ng integration for text capture
- This would be a significant contribution (estimate: new `src/linux/Orca/` directory with 15+ files)

## Audio Recording Pipeline Details

### Direct espeak-ng WAV generation (simplest)
```bash
espeak-ng -w output.wav "heading Welcome. paragraph Some text. button Click Me."
```
Produces standard PCM WAV, 22050 Hz, mono, 16-bit.

### PulseAudio capture (more realistic)
```bash
# Start recording
ffmpeg -f pulse -i virtual_speaker.monitor -ac 1 -ar 22050 output.wav &
# Play TTS through PulseAudio
espeak-ng "Hello world"
# Stop recording
kill $RECORDER_PID
```
Captures exactly what would go to speakers, including timing and gaps.

### File sizes observed
- Simple page (7 phrases): ~600KB WAV
- Form page (18 phrases): ~1.6MB WAV
- Navigation page (42 phrases): ~3.6MB WAV
- PulseAudio capture: ~600KB WAV
