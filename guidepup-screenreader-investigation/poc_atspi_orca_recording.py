#!/usr/bin/env python3.12
"""
Proof-of-Concept: AT-SPI + Orca + Audio Recording

This script demonstrates a complete pipeline for:
1. Starting a virtual display (Xvfb), PulseAudio with null sink, and AT-SPI
2. Launching Orca screen reader with espeak-ng TTS
3. Opening a web page in Chromium with accessibility enabled
4. Walking the AT-SPI accessibility tree
5. Generating TTS audio from the accessibility tree content
6. Recording the TTS audio output to a WAV file

This simulates what a screen reader user would hear when navigating a web page.
"""

import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

import subprocess
import time
import os
import sys
import signal
import json
from pathlib import Path


class AccessibilityEnvironment:
    """Manages the virtual accessibility environment."""

    def __init__(self):
        self.processes = []
        self.display = ":99"
        self.chrome_path = "/root/.cache/ms-playwright/chromium-1194/chrome-linux/chrome"

    def setup(self):
        """Set up the complete accessibility environment."""
        print("=== Setting up accessibility environment ===\n")

        # 1. Start Xvfb (may already be running)
        self._start_xvfb()

        # 2. Start D-Bus session bus
        self._start_dbus()

        # 3. Start PulseAudio with null sink
        self._start_pulseaudio()

        # 4. Start AT-SPI registry
        self._start_atspi()

        # 5. Initialize AT-SPI
        os.environ['DISPLAY'] = self.display
        os.environ['GTK_MODULES'] = 'atk-bridge'
        os.environ['NO_AT_BRIDGE'] = '0'
        Atspi.init()
        print("  AT-SPI initialized")

        print("\n=== Environment ready ===\n")

    def _start_xvfb(self):
        """Start Xvfb virtual display."""
        # Check if already running
        result = subprocess.run(['pgrep', '-f', f'Xvfb {self.display}'],
                                capture_output=True)
        if result.returncode == 0:
            print(f"  Xvfb already running on {self.display}")
            os.environ['DISPLAY'] = self.display
            return

        proc = subprocess.Popen(
            ['Xvfb', self.display, '-screen', '0', '1280x1024x24', '-ac'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        self.processes.append(proc)
        time.sleep(1)
        os.environ['DISPLAY'] = self.display
        print(f"  Xvfb started on {self.display}")

    def _start_dbus(self):
        """Start D-Bus session bus."""
        if os.environ.get('DBUS_SESSION_BUS_ADDRESS'):
            print(f"  D-Bus already running")
            return

        result = subprocess.run(
            ['dbus-launch', '--sh-syntax'],
            capture_output=True, text=True
        )
        for line in result.stdout.strip().split('\n'):
            if '=' in line:
                key, val = line.split('=', 1)
                val = val.rstrip(';').strip("'\"")
                os.environ[key] = val
        print(f"  D-Bus session started")

    def _start_pulseaudio(self):
        """Start PulseAudio with virtual sinks."""
        result = subprocess.run(['pulseaudio', '--check'], capture_output=True)
        if result.returncode == 0:
            print("  PulseAudio already running")
        else:
            subprocess.run(['pulseaudio', '--start', '--exit-idle-time=-1'],
                           capture_output=True)
            time.sleep(1)
            print("  PulseAudio started")

        # Ensure null sink exists
        result = subprocess.run(
            ['pactl', 'list', 'sinks', 'short'],
            capture_output=True, text=True
        )
        if 'virtual_speaker' not in result.stdout:
            subprocess.run([
                'pactl', 'load-module', 'module-null-sink',
                'sink_name=virtual_speaker',
                'sink_properties=device.description=Virtual_Speaker'
            ], capture_output=True)
            subprocess.run(
                ['pactl', 'set-default-sink', 'virtual_speaker'],
                capture_output=True
            )
            print("  Virtual audio sink created")
        else:
            print("  Virtual audio sink already exists")

    def _start_atspi(self):
        """Start AT-SPI registry daemon."""
        result = subprocess.run(['pgrep', '-f', 'at-spi2-registryd'],
                                capture_output=True)
        if result.returncode == 0:
            print("  AT-SPI registry already running")
            return

        proc = subprocess.Popen(
            ['/usr/libexec/at-spi2-registryd'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        self.processes.append(proc)
        time.sleep(1)
        print("  AT-SPI registry started")

    def cleanup(self):
        """Stop all managed processes."""
        for proc in reversed(self.processes):
            try:
                proc.terminate()
                proc.wait(timeout=5)
            except:
                proc.kill()


class AccessibilityTreeWalker:
    """Walks the AT-SPI accessibility tree and extracts content."""

    def __init__(self):
        self.spoken_phrases = []

    def get_applications(self):
        """List all registered AT-SPI applications."""
        desktop = Atspi.get_desktop(0)
        apps = []
        for i in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(i)
            if app:
                apps.append(app.get_name())
        return apps

    def walk_tree(self, obj, depth=0, max_depth=10):
        """Walk the accessibility tree and collect navigable elements."""
        if obj is None or depth > max_depth:
            return

        try:
            role = obj.get_role_name()
            name = obj.get_name() or ''
            description = obj.get_description() or ''

            # Extract text content
            text_content = ''
            try:
                ti = obj.get_text()
                if ti:
                    count = ti.get_character_count()
                    if count > 0:
                        text_content = ti.get_text(0, min(count, 500))
            except:
                pass

            # Build spoken phrase based on role (simulating screen reader output)
            phrase = self._build_phrase(role, name, text_content, description)
            if phrase:
                self.spoken_phrases.append({
                    'role': role,
                    'name': name,
                    'text': text_content,
                    'phrase': phrase,
                    'depth': depth
                })

            # Recurse into children
            for i in range(obj.get_child_count()):
                self.walk_tree(obj.get_child_at_index(i), depth + 1, max_depth)

        except Exception as e:
            pass  # Skip inaccessible nodes

    def _build_phrase(self, role, name, text, description):
        """Build a screen-reader-style spoken phrase for an element."""
        # Skip structural/container roles that don't produce speech
        skip_roles = {'application', 'panel', 'filler', 'redundant object',
                      'section', 'separator', 'unknown', 'tool bar',
                      'page tab list', 'notification', 'alert'}

        if role in skip_roles:
            return None

        parts = []

        if role == 'heading':
            level = ''  # Could extract aria-level
            parts.append(f"heading, {name}" if name else "heading")
        elif role == 'link':
            parts.append(f"link, {name}" if name else "link")
        elif role == 'push button':
            parts.append(f"button, {name}" if name else "button")
        elif role == 'entry':
            parts.append(f"edit text, {name}" if name else "edit text")
        elif role == 'paragraph':
            if text:
                parts.append(text)
        elif role == 'static':
            if text and text != name:
                parts.append(text)
            elif name:
                parts.append(name)
        elif role == 'document web':
            parts.append(f"web content, {name}" if name else "web content")
        elif role == 'frame':
            parts.append(f"window, {name}" if name else "window")
        elif role == 'page tab':
            parts.append(f"tab, {name}" if name else "tab")
        elif role == 'list':
            parts.append("list")
        elif role == 'list item':
            parts.append(f"list item, {name}" if name else "list item")
        elif role == 'navigation':
            parts.append(f"navigation, {name}" if name else "navigation")
        elif role == 'label':
            parts.append(name if name else '')
        elif name:
            parts.append(f"{role}, {name}")

        phrase = ' '.join(parts).strip()
        return phrase if phrase else None

    def get_page_content_phrases(self, app_name="Chromium"):
        """Extract spoken phrases from a web page in the given application."""
        self.spoken_phrases = []
        desktop = Atspi.get_desktop(0)

        for i in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(i)
            if app and app.get_name() == app_name:
                # Find the document web node
                self._find_and_walk_document(app)
                break

        return self.spoken_phrases

    def _find_and_walk_document(self, obj, depth=0):
        """Find the 'document web' node and walk from there."""
        if obj is None or depth > 15:
            return False

        try:
            role = obj.get_role_name()
            if role == 'document web':
                self.walk_tree(obj, 0, 8)
                return True

            for i in range(obj.get_child_count()):
                if self._find_and_walk_document(obj.get_child_at_index(i), depth + 1):
                    return True
        except:
            pass

        return False


class TTSAudioRecorder:
    """Records TTS audio output from screen reader phrases."""

    def __init__(self, output_dir="/tmp"):
        self.output_dir = Path(output_dir)

    def speak_and_record(self, phrases, output_file="screenreader_session.wav"):
        """Speak all phrases through espeak-ng and record the audio."""
        output_path = self.output_dir / output_file

        # Build the full text to speak
        full_text = self._build_narration(phrases)

        print(f"\n=== Generating TTS narration ===")
        print(f"Phrases to speak: {len(phrases)}")
        print(f"Output file: {output_path}")

        # Method 1: Direct espeak-ng to WAV (simplest, most reliable)
        print("\n--- Method 1: Direct espeak-ng WAV output ---")
        result = subprocess.run(
            ['espeak-ng', '-w', str(output_path), full_text],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            size = output_path.stat().st_size
            print(f"  Recorded {size} bytes to {output_path}")
        else:
            print(f"  Error: {result.stderr}")

        # Method 2: Record via PulseAudio (captures what would play through speakers)
        pa_output = self.output_dir / "screenreader_session_pulseaudio.wav"
        print(f"\n--- Method 2: PulseAudio capture ---")
        self._record_via_pulseaudio(full_text, str(pa_output))

        return str(output_path)

    def _build_narration(self, phrases):
        """Build narration text from phrases, adding pauses between elements."""
        parts = []
        for p in phrases:
            phrase = p['phrase']
            # Add SSML-like pauses between different types of content
            parts.append(phrase)
        return '. '.join(parts)

    def _record_via_pulseaudio(self, text, output_path):
        """Record TTS output via PulseAudio null sink monitor."""
        # Start recording from the monitor source
        rec_proc = subprocess.Popen(
            ['ffmpeg', '-y', '-f', 'pulse',
             '-i', 'virtual_speaker.monitor',
             '-ac', '1', '-ar', '22050',
             output_path],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )

        time.sleep(0.5)  # Let recorder initialize

        # Play through PulseAudio
        subprocess.run(
            ['espeak-ng', '--stdout', text],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL
        )
        speak_proc = subprocess.Popen(
            ['espeak-ng', text],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        speak_proc.wait()

        time.sleep(1)  # Capture trailing audio

        # Stop recording
        rec_proc.terminate()
        try:
            rec_proc.wait(timeout=5)
        except:
            rec_proc.kill()

        if Path(output_path).exists():
            size = Path(output_path).stat().st_size
            print(f"  Recorded {size} bytes to {output_path}")
        else:
            print(f"  Recording failed (file not created)")


def main():
    """Run the complete proof-of-concept."""
    import warnings
    warnings.filterwarnings('ignore', category=DeprecationWarning)

    print("=" * 60)
    print("AT-SPI + Orca Screen Reader Audio Recording POC")
    print("=" * 60)

    env = AccessibilityEnvironment()
    env.setup()

    # Give Orca time to register (if not already running)
    result = subprocess.run(['pgrep', '-f', 'orca'], capture_output=True)
    if result.returncode != 0:
        print("Starting Orca screen reader...")
        subprocess.Popen(
            ['python3.12', '/usr/bin/orca', '--replace'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        time.sleep(3)

    # Launch Chromium with a test page
    test_html = (
        "data:text/html,"
        "<html><head><title>Screen Reader Test</title></head>"
        "<body>"
        "<nav aria-label='Main navigation'>"
        "<ul><li><a href='#'>Home</a></li><li><a href='#'>About</a></li></ul>"
        "</nav>"
        "<main>"
        "<h1>Welcome to Accessibility Testing</h1>"
        "<p>This paragraph demonstrates how a screen reader navigates web content.</p>"
        "<form>"
        "<label for='email'>Email Address:</label>"
        "<input id='email' type='email' placeholder='user@example.com'>"
        "<button type='submit'>Subscribe</button>"
        "</form>"
        "<h2>Features</h2>"
        "<ul>"
        "<li>Automated accessibility tree walking</li>"
        "<li>Text-to-speech synthesis</li>"
        "<li>Audio recording of screen reader output</li>"
        "</ul>"
        "</main>"
        "</body></html>"
    )

    chrome_path = "/root/.cache/ms-playwright/chromium-1194/chrome-linux/chrome"
    chrome_proc = subprocess.Popen(
        [chrome_path, '--force-renderer-accessibility', '--no-sandbox',
         '--disable-gpu', '--disable-dev-shm-usage', test_html],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    print(f"Chromium launched (PID: {chrome_proc.pid})")
    time.sleep(5)

    # Walk the accessibility tree
    print("\n=== Walking AT-SPI Accessibility Tree ===\n")
    walker = AccessibilityTreeWalker()

    apps = walker.get_applications()
    print(f"Registered applications: {apps}")

    phrases = walker.get_page_content_phrases("Chromium")
    print(f"\nExtracted {len(phrases)} spoken phrases:\n")
    for i, p in enumerate(phrases):
        indent = "  " * p['depth']
        print(f"  {i + 1:2d}. {indent}[{p['role']}] {p['phrase']}")

    # Record audio
    output_dir = Path("/home/user/research/guidepup-screenreader-investigation")
    output_dir.mkdir(parents=True, exist_ok=True)

    recorder = TTSAudioRecorder(output_dir=str(output_dir))
    output_file = recorder.speak_and_record(phrases)

    # Save the accessibility tree and spoken phrases as JSON
    tree_file = output_dir / "accessibility_tree.json"
    with open(tree_file, 'w') as f:
        json.dump({
            'applications': apps,
            'phrases': phrases,
            'url': 'data:text/html,...',
            'timestamp': time.strftime('%Y-%m-%d %H:%M:%S')
        }, f, indent=2)
    print(f"\nAccessibility tree saved to {tree_file}")

    # Cleanup
    chrome_proc.terminate()
    chrome_proc.wait()
    print("\n=== Done ===")


if __name__ == '__main__':
    main()
