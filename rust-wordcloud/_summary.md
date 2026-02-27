Leveraging Rust’s performance and safety, this CLI tool generates PNG word clouds directly from text input using a custom spiral layout algorithm and efficient grid-based collision detection. It supports flexible options for image size, font scaling, color schemes, and background colors, with all core features—such as stopword filtering, spatial indexing, and layout—implemented from scratch without any external word cloud library. Designed for usability, it reads from files or stdin and auto-increments output filenames to prevent overwrites. Key image rendering is powered by Rust crates `image` and `ab_glyph` for font handling and PNG output. For further inspiration and algorithmic details, see Max Woolf’s write-up and example project: [Max Woolf’s AI Agent Coding](https://minimaxir.com/2026/02/ai-agent-coding/).

**Key findings:**
- Grid-based spatial indexing enables real-time placement of 200+ densely packed words (O(1) collision checks).
- Logarithmic font scaling avoids overcrowding by dominant words and improves visual balance.
- Spiral placement with random angular offsets yields more natural, less rigid word clouds.
