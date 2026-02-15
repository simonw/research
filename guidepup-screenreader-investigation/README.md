# Guidepup Screen Reader Investigation

## Summary

This investigation evaluates [guidepup](https://github.com/guidepup/guidepup) for recording audio sessions of simulated screen reader users navigating web applications in a headless Linux environment.

**Key finding:** The core guidepup package only supports macOS (VoiceOver) and Windows (NVDA) -- it has no Linux support. However, two viable approaches exist for achieving the goal in this environment, both proven with working code.

## Two Working Approaches

### Approach A: AT-SPI + Orca + Audio Recording (Real Browser)

Uses the Linux accessibility stack to walk a real browser's accessibility tree, then synthesizes and records audio.

**Infrastructure required:**
- Xvfb (virtual display)
- D-Bus session bus
- PulseAudio with null sink (virtual audio)
- AT-SPI2 registry
- Orca screen reader (run via `python3.12`)
- Chromium with `--force-renderer-accessibility`

**Pipeline:**
1. Start Xvfb, D-Bus, PulseAudio, AT-SPI, and Orca
2. Launch Chromium with accessibility flags, loading target URL
3. Use Python AT-SPI2 bindings (`gi.repository.Atspi`) to walk the accessibility tree
4. Extract roles, names, and text content from all accessible elements
5. Build narration text from the accessibility tree
6. Generate audio via `espeak-ng -w output.wav "narration text"`
7. Alternatively, record live PulseAudio output with `ffmpeg -f pulse -i virtual_speaker.monitor`

**Pros:** Tests real browser accessibility tree; exercises actual AT-SPI protocol; catches browser-specific a11y issues
**Cons:** Complex infrastructure setup; Python version dependency (3.12 needed for GI bindings); slower

**See:** `poc_atspi_orca_recording.py`

### Approach B: Virtual Screen Reader + TTS (No Infrastructure)

Uses `@guidepup/virtual-screen-reader` to simulate screen reader navigation through HTML, then synthesizes audio.

**Requirements:**
- Node.js
- `@guidepup/virtual-screen-reader` (npm)
- `jsdom` (npm)
- `espeak-ng` (system package)

**Pipeline:**
1. Parse HTML with jsdom
2. Start virtual screen reader on the DOM container
3. Navigate through content with `virtual.next()`
4. Collect all spoken phrases via `virtual.spokenPhraseLog()`
5. Generate audio via `espeak-ng -w output.wav "phrases..."`

**Pros:** No display/audio/D-Bus infrastructure needed; fast; deterministic; pure Node.js; excellent spec compliance
**Cons:** Doesn't test real browser accessibility; simulated (not actual screen reader behavior); jsdom may differ from real browser DOM

**See:** `poc_virtual_sr_tts.mjs`

## Guidepup Ecosystem Overview

| Package | Purpose | Linux Support |
|---------|---------|--------------|
| `guidepup` | Screen reader automation driver | **No** (macOS/Windows only) |
| `@guidepup/virtual-screen-reader` | DOM-based screen reader simulator | **Yes** (pure JS) |
| `@guidepup/record` | Screen video recording | **No** (macOS/Windows only) |
| `@guidepup/playwright` | Playwright integration | N/A (wraps core) |
| `@guidepup/jest` | Jest matchers | N/A (wraps virtual) |

## Environment Setup (Ubuntu 24.04)

### Install system packages
```bash
apt-get install -y at-spi2-core libatk-adaptor orca espeak-ng \
  speech-dispatcher speech-dispatcher-espeak-ng \
  pulseaudio pulseaudio-utils dbus-x11 ffmpeg
```

### Start accessibility infrastructure
```bash
# Virtual display
Xvfb :99 -screen 0 1280x1024x24 -ac &
export DISPLAY=:99

# D-Bus session bus
eval $(dbus-launch --sh-syntax)

# PulseAudio with virtual audio sink
pulseaudio --start --exit-idle-time=-1
pactl load-module module-null-sink sink_name=virtual_speaker
pactl set-default-sink virtual_speaker

# AT-SPI registry
/usr/libexec/at-spi2-registryd &

# Orca screen reader (must use python3.12 due to GI binding version)
python3.12 /usr/bin/orca --replace &
```

### Install Node.js packages (for Approach B)
```bash
npm install @guidepup/virtual-screen-reader jsdom
```

## Audio Recording Methods

### Direct WAV generation (simplest)
```bash
espeak-ng -w output.wav "Screen reader narration text here"
```

### PulseAudio live capture (captures real-time playback)
```bash
# Start recording from virtual speaker monitor
ffmpeg -y -f pulse -i virtual_speaker.monitor -ac 1 -ar 22050 recording.wav &
REC_PID=$!

# Play TTS through PulseAudio
espeak-ng "Hello, heading, welcome, paragraph, some text"

# Stop recording
sleep 1 && kill $REC_PID
```

## Limitations and Caveats

1. **No guidepup Linux support:** The core package cannot drive Orca. Adding support would require implementing AT-SPI2 D-Bus communication (a substantial effort).

2. **Python version mismatch:** The Orca screen reader in Ubuntu 24.04 requires Python 3.12's GI bindings, but the system `python3` may point to 3.11. Run Orca explicitly with `python3.12`.

3. **espeak-ng voice quality:** The default espeak-ng voice is robotic. For higher quality, consider installing MBROLA voices or using an alternative TTS engine.

4. **Virtual screen reader fidelity:** The virtual-screen-reader simulates expected behavior per WAI-ARIA specs but may differ from actual VoiceOver/NVDA/Orca behavior in edge cases.

5. **Audio is synthesized, not captured:** In both approaches, we generate audio from text. We're not capturing Orca's actual speech output. True Orca audio capture would require routing speech-dispatcher output through PulseAudio and recording from the monitor source.

## Files in This Investigation

| File | Description |
|------|-------------|
| `README.md` | This report |
| `notes.md` | Detailed investigation notes and findings |
| `poc_atspi_orca_recording.py` | Approach A: AT-SPI tree walk + TTS recording |
| `poc_virtual_sr_tts.mjs` | Approach B: Virtual screen reader + TTS recording |
| `accessibility_tree.json` | Sample AT-SPI tree output from Chromium |
| `virtual_sr_sessions.json` | Sample virtual screen reader session data |
| `screenreader_session.wav` | Audio from Approach A (AT-SPI + espeak-ng) |
| `vsr_session_simple.wav` | Audio from Approach B (simple page) |
| `vsr_session_form.wav` | Audio from Approach B (form page) |
| `vsr_session_pulseaudio.wav` | Audio from PulseAudio live capture |

## Recommendations

For the stated goal of "recording audio sessions of a simulated screen reader user using an application":

1. **For CI/testing:** Use Approach B (virtual-screen-reader). It's fast, deterministic, requires no infrastructure, and produces accurate spoken phrase logs. Add espeak-ng for audio generation.

2. **For realistic simulation:** Use Approach A (AT-SPI + Orca). It tests the real browser accessibility tree and catches real-world a11y issues. More complex setup but higher fidelity.

3. **For best of both:** Run Approach B for speed during development, Approach A for integration/acceptance testing.

4. **Contributing to guidepup:** Adding Linux/Orca support to the core guidepup package would be valuable. It would require implementing AT-SPI2 D-Bus communication in TypeScript, similar to how VoiceOver uses AppleScript and NVDA uses TLS sockets.
