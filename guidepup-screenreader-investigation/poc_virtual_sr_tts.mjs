/**
 * Proof-of-Concept: Virtual Screen Reader + TTS Audio Recording
 *
 * This script demonstrates using @guidepup/virtual-screen-reader to:
 * 1. Parse HTML and build an accessibility tree (no browser needed)
 * 2. Navigate the page as a screen reader user would
 * 3. Collect all spoken phrases
 * 4. Generate TTS audio via espeak-ng
 * 5. Record the complete session to a WAV file
 *
 * This approach works WITHOUT a real screen reader (no Orca/VoiceOver/NVDA)
 * and WITHOUT a display server. It runs in pure headless Node.js + jsdom.
 */

import { virtual } from "@guidepup/virtual-screen-reader";
import { JSDOM } from "jsdom";
import { execSync, spawn } from "child_process";
import { writeFileSync, statSync } from "fs";
import { join } from "path";

const OUTPUT_DIR = "/home/user/research/guidepup-screenreader-investigation";

// ---- Test HTML Pages ----

const PAGES = {
  simple: `
    <html>
    <head><title>Simple Test Page</title></head>
    <body>
      <h1>Welcome to the Test Page</h1>
      <p>This is a simple paragraph of text.</p>
      <button>Click Me</button>
      <a href="#">Learn More</a>
    </body>
    </html>
  `,

  form: `
    <html>
    <head><title>Registration Form</title></head>
    <body>
      <main>
        <h1>Create an Account</h1>
        <form aria-label="Registration form">
          <div>
            <label for="username">Username:</label>
            <input id="username" type="text" required aria-required="true" placeholder="Choose a username">
          </div>
          <div>
            <label for="email">Email:</label>
            <input id="email" type="email" required aria-required="true" placeholder="you@example.com">
          </div>
          <div>
            <label for="password">Password:</label>
            <input id="password" type="password" required aria-required="true">
          </div>
          <div role="group" aria-labelledby="terms-label">
            <span id="terms-label">Terms and Conditions</span>
            <input id="terms" type="checkbox" aria-labelledby="terms-label">
            <label for="terms">I agree to the terms of service</label>
          </div>
          <button type="submit">Create Account</button>
        </form>
      </main>
    </body>
    </html>
  `,

  navigation: `
    <html>
    <head><title>Navigation Example</title></head>
    <body>
      <nav aria-label="Primary">
        <ul>
          <li><a href="#" aria-current="page">Home</a></li>
          <li><a href="#">Products</a></li>
          <li><a href="#">About Us</a></li>
          <li><a href="#">Contact</a></li>
        </ul>
      </nav>
      <main>
        <h1>Our Products</h1>
        <section aria-labelledby="featured-heading">
          <h2 id="featured-heading">Featured Items</h2>
          <article aria-label="Product 1">
            <h3>Widget Pro</h3>
            <p>The best widget for professionals. Rating: 4.5 out of 5 stars.</p>
            <button>Add to Cart</button>
          </article>
          <article aria-label="Product 2">
            <h3>Widget Basic</h3>
            <p>An affordable widget for everyone. Rating: 4.0 out of 5 stars.</p>
            <button>Add to Cart</button>
          </article>
        </section>
      </main>
      <footer>
        <p>Copyright 2025 Widget Corp. All rights reserved.</p>
      </footer>
    </body>
    </html>
  `,
};

// ---- Screen Reader Session ----

async function runScreenReaderSession(pageName, html) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`Screen Reader Session: ${pageName}`);
  console.log("=".repeat(60));

  const dom = new JSDOM(html);
  const document = dom.window.document;

  await virtual.start({ container: document.body, window: dom.window });

  const spokenPhrases = [];
  let keepGoing = true;
  let maxSteps = 50; // Safety limit

  // Navigate through the entire page
  while (keepGoing && maxSteps-- > 0) {
    try {
      await virtual.next();
      const phrase = await virtual.lastSpokenPhrase();

      // Check for end of document
      if (phrase === "end of document" || phrase === "document") {
        spokenPhrases.push(phrase);
        keepGoing = false;
      } else {
        spokenPhrases.push(phrase);
      }
    } catch (e) {
      // Reached end of navigable content
      keepGoing = false;
    }
  }

  console.log(`\nSpoken phrases (${spokenPhrases.length}):`);
  spokenPhrases.forEach((phrase, i) => {
    console.log(`  ${String(i + 1).padStart(2)}. ${phrase}`);
  });

  // Get the full log from the virtual screen reader
  const fullLog = await virtual.spokenPhraseLog();

  await virtual.stop();

  return { pageName, spokenPhrases, fullLog };
}

// ---- TTS Audio Generation ----

function generateAudio(session, outputFile) {
  const { pageName, spokenPhrases } = session;

  // Build narration with natural pauses
  const narrationParts = [];
  narrationParts.push(`Screen reader session for: ${pageName}.`);
  narrationParts.push("Beginning of page.");

  for (const phrase of spokenPhrases) {
    // Add the phrase as-is (espeak will handle pronunciation)
    narrationParts.push(phrase);
  }

  narrationParts.push("End of screen reader session.");

  const fullNarration = narrationParts.join(". ");
  const outputPath = join(OUTPUT_DIR, outputFile);

  // Generate WAV using espeak-ng
  try {
    execSync(`espeak-ng -w "${outputPath}" "${fullNarration.replace(/"/g, '\\"')}"`, {
      timeout: 30000,
    });

    const stats = statSync(outputPath);
    console.log(`\nAudio generated: ${outputPath} (${stats.size} bytes)`);
    return outputPath;
  } catch (err) {
    // If the text is too long for command line, write to a temp file
    const textFile = "/tmp/narration.txt";
    writeFileSync(textFile, fullNarration);
    execSync(`espeak-ng -w "${outputPath}" -f "${textFile}"`, {
      timeout: 30000,
    });
    const stats = statSync(outputPath);
    console.log(`\nAudio generated: ${outputPath} (${stats.size} bytes)`);
    return outputPath;
  }
}

// ---- PulseAudio Recording (optional) ----

async function generateAudioViaPulseAudio(session, outputFile) {
  const { spokenPhrases, pageName } = session;
  const outputPath = join(OUTPUT_DIR, outputFile);

  // Check if PulseAudio is available
  try {
    execSync("pactl info", { stdio: "pipe" });
  } catch {
    console.log("\nPulseAudio not available, skipping live recording.");
    return null;
  }

  const narration = [
    `Screen reader session for ${pageName}.`,
    ...spokenPhrases,
    "End of session.",
  ].join(". ");

  // Start ffmpeg recording from PulseAudio monitor
  const recorder = spawn(
    "ffmpeg",
    [
      "-y",
      "-f", "pulse",
      "-i", "virtual_speaker.monitor",
      "-ac", "1",
      "-ar", "22050",
      "-t", "60", // Max 60 seconds
      outputPath,
    ],
    { stdio: "pipe" }
  );

  // Wait for recorder to initialize
  await new Promise((r) => setTimeout(r, 500));

  // Speak through PulseAudio
  const speaker = spawn("espeak-ng", [narration], { stdio: "pipe" });
  await new Promise((resolve) => speaker.on("close", resolve));

  // Wait for audio to finish
  await new Promise((r) => setTimeout(r, 1000));

  // Stop recorder
  recorder.kill("SIGTERM");
  await new Promise((resolve) => recorder.on("close", resolve));

  try {
    const stats = statSync(outputPath);
    console.log(`PulseAudio recording: ${outputPath} (${stats.size} bytes)`);
    return outputPath;
  } catch {
    console.log("PulseAudio recording failed.");
    return null;
  }
}

// ---- Main ----

async function main() {
  console.log("=" .repeat(60));
  console.log("Virtual Screen Reader + TTS Audio Recording POC");
  console.log("=" .repeat(60));
  console.log("\nThis demo uses @guidepup/virtual-screen-reader to simulate");
  console.log("screen reader navigation and generates audio via espeak-ng.\n");

  const sessions = [];

  // Run sessions for each test page
  for (const [name, html] of Object.entries(PAGES)) {
    const session = await runScreenReaderSession(name, html);
    sessions.push(session);
  }

  // Generate audio for each session
  console.log(`\n${"=".repeat(60)}`);
  console.log("Generating TTS Audio Files");
  console.log("=".repeat(60));

  for (const session of sessions) {
    generateAudio(session, `vsr_session_${session.pageName}.wav`);
  }

  // Try PulseAudio recording for the first session
  console.log(`\n${"=".repeat(60)}`);
  console.log("PulseAudio Live Recording (if available)");
  console.log("=".repeat(60));
  await generateAudioViaPulseAudio(sessions[0], "vsr_session_pulseaudio.wav");

  // Save session data as JSON
  const jsonPath = join(OUTPUT_DIR, "virtual_sr_sessions.json");
  const sessionData = sessions.map((s) => ({
    page: s.pageName,
    phraseCount: s.spokenPhrases.length,
    phrases: s.spokenPhrases,
    fullLog: s.fullLog,
  }));
  writeFileSync(jsonPath, JSON.stringify(sessionData, null, 2));
  console.log(`\nSession data saved to: ${jsonPath}`);

  console.log(`\n${"=".repeat(60)}`);
  console.log("All sessions complete!");
  console.log("=".repeat(60));
}

main().catch(console.error);
