An investigation into [Guidepup](https://github.com/guidepup/guidepup) reveals that its core package does not support Linux—only macOS (VoiceOver) and Windows (NVDA). However, two practical methods were proven for generating audio screen reader sessions on Linux: one uses the AT-SPI accessibility stack and Orca to walk a real browser's accessibility tree and synthesize narration; the other employs the [virtual screen reader](https://www.npmjs.com/package/@guidepup/virtual-screen-reader) (pure JS, fast) to simulate navigation, then builds audio from spoken phrases. Approach A offers higher fidelity by testing browser-specific accessibility infrastructure, while Approach B is simpler and ideal for automated testing. Both approaches produce usable audio narration, although neither captures Orca's live speech output directly.

**Key findings:**
- Guidepup cannot currently automate or record Orca sessions on Linux.
- AT-SPI + Orca approach yields realistic accessibility testing but requires complex setup.
- Virtual-screen-reader is fast and deterministic but only simulates behavior.
- Integrating Orca into Guidepup would require significant AT-SPI2 D-Bus work.
