# Rust Word Cloud CLI: A Code Walkthrough

*2026-02-27T19:09:12Z by Showboat 0.6.1*
<!-- showboat-id: 37f14287-5f1f-458a-a808-1b033893e80f -->

This walkthrough traces through a 444-line Rust program that generates word cloud PNG images from text input. The entire project lives in a single file — `src/main.rs` — and leans on a handful of well-chosen crates for font rendering, image generation, and argument parsing. We'll follow the code top-to-bottom, which roughly mirrors the execution path: parse CLI args, read input, count words, then place them one by one on a canvas using an Archimedean spiral and grid-based collision detection.

## Dependencies (Cargo.toml)

Let's start with what the project pulls in. The dependency list is compact — five crates, each with a clear role:

```bash
cat rust-wordcloud/Cargo.toml
```

```output
[package]
name = "wordcloud"
version = "0.1.0"
edition = "2024"

[dependencies]
ab_glyph = "0.2.32"
clap = { version = "4.5.60", features = ["derive"] }
image = "0.25.9"
imageproc = "0.26.0"
rand = "0.10.0"
```

- **ab_glyph** — Pure-Rust font loading and glyph metrics. Used to measure how wide each word will be at a given font size, including kerning between character pairs.
- **clap** (with the `derive` feature) — Declarative CLI argument parsing. A single `#[derive(Parser)]` on a struct gives you typed flags, defaults, and help text for free.
- **image** — Creates and saves the PNG canvas. Provides the `RgbImage` type and pixel manipulation.
- **imageproc** — Extends `image` with drawing primitives. This project uses its `draw_text_mut` to stamp words onto the canvas.
- **rand** — Random number generation. Each word gets a randomized starting angle on the placement spiral so the layout doesn't look mechanical.

## Imports (lines 1–9)

The imports mirror the dependency list. Here's the opening of `main.rs`:

```bash
sed -n '1,9p' rust-wordcloud/src/main.rs
```

```output
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use clap::Parser;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
```

From `ab_glyph`, four items are pulled in: `Font` and `ScaleFont` (traits for querying glyph metrics), `FontRef` (a borrowed font parsed from a byte slice), and `PxScale` (a pixel-size wrapper). From the standard library, `HashMap` drives word frequency counting, `fs` and `io::Read` handle file/stdin input, and `Path` is used for output filename logic.

## CLI Argument Parsing (lines 11–53)

The program's interface is defined entirely through clap's derive macro. A single struct captures every knob the user can turn:

```bash
sed -n '11,53p' rust-wordcloud/src/main.rs
```

```output
/// A CLI tool to generate word cloud PNG images from text input.
#[derive(Parser)]
#[command(name = "wordcloud", version, about)]
struct Cli {
    /// Input text file (reads from stdin if not provided)
    input: Option<String>,

    /// Output PNG filename (auto-generates wordcloud.png, wordcloud-2.png, etc. if not set)
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Image width in pixels
    #[arg(long, default_value_t = 1200)]
    width: u32,

    /// Image height in pixels
    #[arg(long, default_value_t = 800)]
    height: u32,

    /// Maximum number of words to display
    #[arg(long, default_value_t = 200)]
    max_words: usize,

    /// Minimum font size in pixels
    #[arg(long, default_value_t = 12.0)]
    min_font_size: f32,

    /// Maximum font size in pixels
    #[arg(long, default_value_t = 90.0)]
    max_font_size: f32,

    /// Color scheme: "vibrant", "warm", "cool", "mono", "earth", "neon"
    #[arg(long, default_value = "vibrant")]
    colors: String,

    /// Background color as hex (e.g. "ffffff" for white, "000000" for black)
    #[arg(long, default_value = "1a1a2e")]
    background: String,

    /// Minimum word length to include
    #[arg(long, default_value_t = 3)]
    min_word_len: usize,
}
```

Every field becomes a command-line flag. The `input` field is a positional argument (`Option<String>`) — when absent, the program reads from stdin. This is a nice Unix-friendly design: you can pipe text in or pass a filename.

The defaults tell the story of what a typical run looks like: a 1200x800 image, up to 200 words, font sizes ranging from 12px to 90px, the "vibrant" color palette on a dark navy (`#1a1a2e`) background, filtering out words shorter than 3 characters.

The `/// doc comments` above each field become the `--help` text — clap's derive macro reads them automatically.

## Hex Color Parsing (lines 55–64)

The user can set the background color via a hex string like `"ffffff"` or `"#000000"`. This small utility function handles the conversion:

```bash
sed -n '55,64p' rust-wordcloud/src/main.rs
```

```output
fn parse_hex_color(hex: &str) -> Rgb<u8> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return Rgb([0, 0, 0]);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Rgb([r, g, b])
}
```

Straightforward: strip an optional leading `#`, then parse three two-character hex slices into R, G, B bytes. If parsing fails at any point, it falls back to black (`0, 0, 0`). The function returns the `image` crate's `Rgb<u8>` type, which is what the canvas expects for pixel colors.

## Color Palette System (lines 66–129)

Six named color palettes are hardcoded as arrays of `Rgb` values. During word placement, each word cycles through its palette by index (`palette[idx % palette.len()]`). Let's look at the function signature and two representative palettes:

```bash
sed -n '66,77p' rust-wordcloud/src/main.rs
```

```output
fn get_color_palette(name: &str) -> Vec<Rgb<u8>> {
    match name {
        "warm" => vec![
            Rgb([255, 89, 94]),
            Rgb([255, 146, 43]),
            Rgb([255, 202, 58]),
            Rgb([255, 120, 79]),
            Rgb([230, 57, 70]),
            Rgb([244, 162, 97]),
            Rgb([255, 183, 77]),
            Rgb([239, 83, 80]),
        ],
```

Each palette has 5–10 colors. The `match` dispatches on the string name from the CLI's `--colors` flag. The available schemes are:

- **warm** — Reds, oranges, and yellows
- **cool** — Blues, teals, and cyans
- **mono** — White and gray shades (for dark backgrounds)
- **earth** — Browns, olives, and tans
- **neon** — Bright greens, pinks, cyans, and purples
- **vibrant** (default, the `_` catch-all) — A mix of coral, teal, yellow, purple, and mint

Let's see the palette names the match covers:

```bash
grep -n '".*" =>' rust-wordcloud/src/main.rs | head -6
```

```output
68:        "warm" => vec![
78:        "cool" => vec![
88:        "mono" => vec![
95:        "earth" => vec![
105:        "neon" => vec![
```

The default (`_`) arm at line 115 handles `"vibrant"` and any unrecognized name — a forgiving approach that avoids erroring on a typo.

## Stop Word Filtering (lines 131–170)

Before counting word frequencies, the program needs to throw out noise — articles, pronouns, prepositions, contractions, and web-related junk that would otherwise dominate any word cloud. This is handled by a simple lookup function:

```bash
sed -n '131,170p' rust-wordcloud/src/main.rs
```

```output
fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        // Common English stop words
        "the", "be", "to", "of", "and", "in", "that", "have", "it", "for",
        "not", "on", "with", "he", "as", "you", "do", "at", "this", "but", "his",
        "by", "from", "they", "we", "say", "her", "she", "or", "an", "will", "my",
        "one", "all", "would", "there", "their", "what", "so", "up", "out", "if",
        "about", "who", "get", "which", "go", "me", "when", "make", "can", "like",
        "time", "no", "just", "him", "know", "take", "people", "into", "year", "your",
        "good", "some", "could", "them", "see", "other", "than", "then", "now", "look",
        "only", "come", "its", "over", "think", "also", "back", "after", "use", "two",
        "how", "our", "work", "first", "well", "way", "even", "new", "want", "because",
        "any", "these", "give", "day", "most", "us", "is", "are", "was", "were", "been",
        "has", "had", "did", "does", "doing", "being", "am", "more", "very", "much",
        "too", "here", "where", "why", "each", "such", "should", "own", "same", "may",
        "still", "must", "got", "made", "don", "didn", "doesn", "isn", "aren", "wasn",
        "weren", "won", "can", "let", "thing", "things", "really",
        "going", "those", "need", "right", "put", "many", "used", "using", "through",
        "since", "long", "while", "before", "between", "under", "along", "both",
        "another", "around", "however", "without", "again", "off", "down", "away",
        "every", "found", "keep", "might", "left", "part", "point", "last",
        "different", "end", "set", "three", "few", "help", "start", "show",
        "world", "next", "began", "head", "turn", "real", "leave", "might",
        "great", "old", "big", "small", "large", "high", "little",
        // Contractions and fragments
        "it's", "i'm", "i've", "i'll", "don't", "didn't", "doesn't", "isn't",
        "that's", "there's", "they're", "you're", "we're", "he's", "she's",
        "it'd", "i'd", "we'd", "you'd", "they'd", "won't", "wouldn't",
        "couldn't", "shouldn't", "wasn't", "weren't", "aren't", "hasn't",
        "hadn't", "can't", "what's", "who's", "let's",
        "ll", "ve", "re",
        // Web/URL fragments
        "http", "https", "www", "com", "org", "net", "html", "htm",
        "png", "jpg", "jpeg", "gif", "svg", "css", "pdf",
        "class", "style", "width", "height", "div", "span", "href", "src",
        // Common markdown/url noise
        "nbsp", "amp", "quot", "rel", "nofollow",
    ];
    STOP_WORDS.contains(&word)
}
```

The list is declared as a `const` slice of string literals — compiled into the binary, zero allocation at runtime. It's organized into four categories:

1. **Common English stop words** (lines 134–154) — ~180 words covering articles, pronouns, verbs, adjectives, and adverbs
2. **Contractions and fragments** (lines 156–161) — `"don't"`, `"it's"`, etc., plus fragments like `"ll"`, `"ve"`, `"re"` that survive when apostrophes are used as split points
3. **Web/URL fragments** (lines 163–165) — Protocol prefixes, TLDs, file extensions, and HTML attribute names
4. **Markdown noise** (line 167) — HTML entities and common attribute values

The lookup is a linear scan (`.contains()`). With ~200 entries and short strings, this is fast enough — a `HashSet` would be overkill for a list this size.

## Word Counting and Ranking (lines 172–197)

This is where raw text becomes a ranked list of (word, count) pairs. The function performs tokenization, filtering, and frequency counting in one pass:

```bash
sed -n '172,197p' rust-wordcloud/src/main.rs
```

```output
fn count_words(text: &str, min_word_len: usize) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    // Split on non-alphanumeric characters (keeping apostrophes for contractions)
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
        let word = word.trim_matches('\'').to_lowercase();
        if word.len() < min_word_len {
            continue;
        }
        if word.chars().all(|c| c.is_numeric()) {
            continue;
        }
        // Skip hex-like strings (e.g. color codes)
        if word.len() == 6 && word.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if is_stop_word(&word) {
            continue;
        }
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut words: Vec<(String, usize)> = counts.into_iter().collect();
    words.sort_by(|a, b| b.1.cmp(&a.1));
    words
}
```

The tokenization strategy (line 176) is clever: split on any character that isn't alphanumeric *or* an apostrophe. This preserves contractions like `"don't"` as single tokens, while splitting on spaces, punctuation, hyphens, and everything else. The apostrophes at the edges of tokens are then trimmed.

Five filters run in sequence on each token:
1. **Minimum length** — Skip anything shorter than `min_word_len` (default: 3)
2. **All-numeric** — Skip pure numbers like `"42"` or `"2024"`
3. **Hex-like strings** — Skip 6-character strings that look like color codes (e.g. `"ff00aa"`)
4. **Stop words** — Skip common English words via `is_stop_word()`
5. **Counting** — Survivors get tallied in a `HashMap<String, usize>`

The final step (lines 194–196) converts the map to a vector and sorts it by count in descending order. The caller (`main`) will then `truncate()` to the top N words.

## Text Measurement (lines 199–215)

To place a word on the canvas without overlapping others, we need to know its exact pixel dimensions at a given font size. This function computes the bounding box using `ab_glyph`'s glyph metrics:

```bash
sed -n '199,215p' rust-wordcloud/src/main.rs
```

```output
fn measure_text(font: &FontRef, scale: PxScale, text: &str) -> (i32, i32) {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0f32;
    let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(prev) = prev_glyph {
            width += scaled.kern(prev, glyph_id);
        }
        width += scaled.h_advance(glyph_id);
        prev_glyph = Some(glyph_id);
    }

    let height = scaled.height();
    (width.ceil() as i32, height.ceil() as i32)
}
```

This walks through each character, accumulating the width via two metrics:

- **`h_advance`** — The horizontal advance width of each glyph (how far the cursor moves to the right after placing this character).
- **`kern`** — The kerning adjustment between adjacent glyph pairs. Some letter pairs like `"AV"` or `"To"` have negative kerning, pulling them closer together for visual balance.

The height is simpler — `scaled.height()` returns the line height for the font at this scale, which is the same for all characters.

Both values are ceiling-rounded to integers since pixel coordinates on the canvas are discrete. The result is an `(i32, i32)` bounding box: the exact pixel rectangle a word will occupy.

## SpatialGrid: Collision Detection (lines 217–279)

This is the most architecturally interesting piece of the program. A naive word cloud would check each new word against every previously placed word — O(n²) comparisons. The `SpatialGrid` brings this down to near-constant time by dividing the canvas into a grid of fixed-size cells.

### The Data Structure

```bash
sed -n '217,238p' rust-wordcloud/src/main.rs
```

```output
/// Grid-based spatial index for fast overlap checking
struct SpatialGrid {
    cell_size: i32,
    cols: i32,
    rows: i32,
    // Each cell stores indices into the placed rects Vec
    cells: Vec<Vec<usize>>,
    rects: Vec<(i32, i32, i32, i32)>,
}

impl SpatialGrid {
    fn new(width: i32, height: i32, cell_size: i32) -> Self {
        let cols = (width + cell_size - 1) / cell_size;
        let rows = (height + cell_size - 1) / cell_size;
        SpatialGrid {
            cell_size,
            cols,
            rows,
            cells: vec![Vec::new(); (cols * rows) as usize],
            rects: Vec::new(),
        }
    }
```

The grid divides the canvas into cells of `cell_size` pixels (set to 40px in `main`). For a 1200x800 image, that's 30 columns x 20 rows = 600 cells.

Two parallel data structures work together:
- **`rects`** — A flat list of all placed rectangles as `(x, y, width, height)` tuples, indexed by insertion order.
- **`cells`** — A 2D grid (flattened to 1D), where each cell holds a `Vec<usize>` of indices into `rects`. A rectangle that spans multiple cells is registered in all of them.

The constructor uses ceiling division (`(width + cell_size - 1) / cell_size`) to ensure the grid covers the full canvas even when dimensions aren't evenly divisible.

### Insertion

```bash
sed -n '240,254p' rust-wordcloud/src/main.rs
```

```output
    fn insert(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let idx = self.rects.len();
        self.rects.push((x, y, w, h));

        let c0 = (x / self.cell_size).max(0);
        let c1 = ((x + w) / self.cell_size).min(self.cols - 1);
        let r0 = (y / self.cell_size).max(0);
        let r1 = ((y + h) / self.cell_size).min(self.rows - 1);

        for r in r0..=r1 {
            for c in c0..=c1 {
                self.cells[(r * self.cols + c) as usize].push(idx);
            }
        }
    }
```

When a word is placed, `insert` records its bounding rectangle and registers its index in every grid cell the rectangle touches. The cell range is computed by dividing the rectangle's corners by the cell size, clamped to grid bounds. A small word might land in one cell; a large word at 90px font size might span several.

### Overlap Detection

```bash
sed -n '256,279p' rust-wordcloud/src/main.rs
```

```output
    fn overlaps(&self, x: i32, y: i32, w: i32, h: i32, padding: i32) -> bool {
        let px = x - padding;
        let py = y - padding;
        let pw = w + 2 * padding;
        let ph = h + 2 * padding;

        let c0 = (px / self.cell_size).max(0);
        let c1 = ((px + pw) / self.cell_size).min(self.cols - 1);
        let r0 = (py / self.cell_size).max(0);
        let r1 = ((py + ph) / self.cell_size).min(self.rows - 1);

        for r in r0..=r1 {
            for c in c0..=c1 {
                for &idx in &self.cells[(r * self.cols + c) as usize] {
                    let (rx, ry, rw, rh) = self.rects[idx];
                    if px < rx + rw && px + pw > rx && py < ry + rh && py + ph > ry {
                        return true;
                    }
                }
            }
        }
        false
    }
}
```

The `overlaps` method is the hot path — called potentially thousands of times per word as the spiral searches for an open spot. The `padding` parameter (set to 3px in `main`) expands the test rectangle on all sides, ensuring words don't touch each other.

The algorithm:
1. Expand the candidate rectangle by the padding amount
2. Find which grid cells this expanded rectangle overlaps
3. For each cell, check only the rectangles registered in that cell (not all placed rects)
4. Use the standard AABB (axis-aligned bounding box) overlap test: two rectangles overlap if and only if they overlap on *both* axes

Because most cells contain only a few rectangles, the inner loop typically checks 0–5 rectangles instead of all 200. This is what makes the placement loop practical — without it, placing 200 words would require up to 200 x 199 / 2 = ~20,000 pairwise checks per spiral step.

## Output Filename Generation (lines 281–298)

A small convenience feature: if no output filename is specified and `wordcloud.png` already exists, the program auto-generates `wordcloud-2.png`, `wordcloud-3.png`, etc.:

```bash
sed -n '281,298p' rust-wordcloud/src/main.rs
```

```output
fn find_output_filename(base: &str) -> String {
    let p = Path::new(base);
    if !p.exists() {
        return base.to_string();
    }

    let stem = p.file_stem().unwrap().to_str().unwrap();
    let ext = p.extension().map(|e| e.to_str().unwrap()).unwrap_or("png");

    let mut n = 2;
    loop {
        let name = format!("{}-{}.{}", stem, n, ext);
        if !Path::new(&name).exists() {
            return name;
        }
        n += 1;
    }
}
```

The logic is straightforward: if the base filename doesn't exist, use it. Otherwise, split it into stem and extension (defaulting to `"png"`), then increment a counter until we find an unused name. This lets you run the tool repeatedly without overwriting previous outputs — a thoughtful touch for an exploratory tool.

## The Main Function (lines 300–444)

This is where everything comes together. The `main` function orchestrates the full pipeline: parse arguments, read input, count words, load a font, place words on the canvas, and save the image. Let's walk through it phase by phase.

### Phase 1: Reading Input (lines 300–324)

```bash
sed -n '300,324p' rust-wordcloud/src/main.rs
```

```output
fn main() {
    let cli = Cli::parse();

    // Read input text
    let text = match &cli.input {
        Some(path) => {
            fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading file '{}': {}", path, e);
                std::process::exit(1);
            })
        }
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
                eprintln!("Error reading stdin: {}", e);
                std::process::exit(1);
            });
            buf
        }
    };

    if text.trim().is_empty() {
        eprintln!("Error: no input text provided");
        std::process::exit(1);
    }
```

`Cli::parse()` invokes clap to process command-line arguments (or print help/errors). The input text comes from either a file path or stdin, with clean error messages on failure. An empty-input guard prevents the program from silently producing a blank image.

### Phase 2: Word Counting and Font Setup (lines 326–356)

```bash
sed -n '326,356p' rust-wordcloud/src/main.rs
```

```output
    // Count and rank words
    let mut word_counts = count_words(&text, cli.min_word_len);
    word_counts.truncate(cli.max_words);

    if word_counts.is_empty() {
        eprintln!("Error: no words found after filtering");
        std::process::exit(1);
    }

    eprintln!("Top 15 words: {:?}", &word_counts[..word_counts.len().min(15)]);

    let max_count = word_counts[0].1 as f32;
    let min_count = word_counts.last().unwrap().1 as f32;

    // Load font
    let font_data = fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf")
        .unwrap_or_else(|_| {
            fs::read("/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf")
                .expect("Could not find a suitable font. Install dejavu or liberation fonts.")
        });
    let font = FontRef::try_from_slice(&font_data).expect("Failed to parse font");

    let palette = get_color_palette(&cli.colors);
    let bg_color = parse_hex_color(&cli.background);

    let width = cli.width;
    let height = cli.height;
    let mut img = RgbImage::from_pixel(width, height, bg_color);

    let mut rng = rand::rng();
    let mut grid = SpatialGrid::new(width as i32, height as i32, 40);
```

After counting words and truncating to the top N, the code extracts `max_count` and `min_count` — these drive the logarithmic font scaling later.

Font loading tries DejaVuSans-Bold first, falling back to LiberationSans-Bold. These are the two most common system fonts on Linux. The font is loaded as raw bytes and parsed into a `FontRef` — a borrowed, zero-copy font handle.

The canvas is created as an `RgbImage` filled with the background color. The `SpatialGrid` is initialized with a 40-pixel cell size — a reasonable balance between grid granularity and lookup overhead. A random number generator is created for the spiral angle offsets.

### Phase 3: The Placement Loop (lines 358–423)

This is the heart of the program — the loop that places each word on the canvas using an Archimedean spiral search:

```bash
sed -n '358,423p' rust-wordcloud/src/main.rs
```

```output
    let center_x = width as i32 / 2;
    let center_y = height as i32 / 2;

    let mut placed_count = 0;
    let mut skip_count = 0;

    for (idx, (word, count)) in word_counts.iter().enumerate() {
        // Scale font size: use log scale for better distribution
        let ratio = if max_count > min_count {
            let log_count = (*count as f32).ln();
            let log_min = min_count.ln();
            let log_max = max_count.ln();
            (log_count - log_min) / (log_max - log_min)
        } else {
            1.0
        };
        let font_size = cli.min_font_size + ratio * (cli.max_font_size - cli.min_font_size);
        let scale = PxScale::from(font_size);

        let (tw, th) = measure_text(&font, scale, word);
        if tw == 0 || th == 0 {
            continue;
        }

        // Spiral placement from center outward
        // Use Archimedean spiral: r = a * theta
        // The spiral needs to reach the corners of the canvas
        let mut found = false;
        let max_radius = ((width * width + height * height) as f32).sqrt() / 2.0;
        let angle_offset: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        // Step size in radians - smaller means tighter search
        let d_theta = 0.15_f32;
        // How fast the spiral expands (pixels per radian)
        let a = 1.2_f32;
        let max_steps = ((max_radius / a) / d_theta) as usize + 5000;

        for step in 0..max_steps {
            let theta = step as f32 * d_theta;
            let r = a * theta;
            let angle = theta + angle_offset;

            let x = center_x + (r * angle.cos()) as i32 - tw / 2;
            let y = center_y + (r * angle.sin()) as i32 - th / 2;

            // Check bounds
            if x < 4 || y < 4 || x + tw > width as i32 - 4 || y + th > height as i32 - 4 {
                if r > max_radius {
                    break;
                }
                continue;
            }

            if !grid.overlaps(x, y, tw, th, 3) {
                let color = palette[idx % palette.len()];
                draw_text_mut(&mut img, color, x, y, scale, &font, word);
                grid.insert(x, y, tw, th);
                placed_count += 1;
                found = true;
                break;
            }
        }

        if !found {
            skip_count += 1;
        }
    }
```

There's a lot going on here. Let's break it down:

**Logarithmic font scaling (lines 366–375):** Raw word frequencies follow a power law — a few words appear hundreds of times, most appear once or twice. If you scaled font size linearly with frequency, the top word would be huge and everything else would be tiny. The logarithmic scaling compresses this range:

`ratio = (ln(count) - ln(min)) / (ln(max) - ln(min))`

This maps each word's frequency to a 0.0–1.0 range in log-space, which is then linearly interpolated between `min_font_size` and `max_font_size`. The result: the most common word is still the largest, but the size difference between "50 occurrences" and "25 occurrences" is much smaller than between "5 occurrences" and "1 occurrence" — visually more balanced.

**The Archimedean spiral (lines 383–397):** The placement algorithm for each word works like this:

1. Start at the canvas center
2. Walk outward along a spiral defined by `r = a * theta`, where `a = 1.2` pixels per radian
3. At each step, increment `theta` by `d_theta = 0.15` radians (~8.6 degrees)
4. Add a random `angle_offset` (0 to 2pi) so each word starts the spiral at a different angle — this prevents the layout from looking like a neat spiral

At each point on the spiral, the word's center is placed at `(center_x + r*cos(angle), center_y + r*sin(angle))`, offset by half the word's width and height to center it.

**Bounds and collision checks (lines 403–417):**
- First, a bounds check with a 4-pixel margin ensures the word fits on the canvas
- If the radius exceeds the diagonal of the image, the search gives up (the word won't fit anywhere)
- If the position is within bounds, `grid.overlaps()` checks for collision with existing words (with 3px padding)
- On a clean hit, the word is drawn with `draw_text_mut`, registered in the grid, and the loop moves to the next word

### Phase 4: Output (lines 425–444)

```bash
sed -n '425,444p' rust-wordcloud/src/main.rs
```

```output
    eprintln!(
        "Placed {} words, skipped {} (out of {} candidates)",
        placed_count,
        skip_count,
        word_counts.len()
    );

    // Determine output filename
    let output = match &cli.output {
        Some(name) => name.clone(),
        None => find_output_filename("wordcloud.png"),
    };

    img.save(&output).unwrap_or_else(|e| {
        eprintln!("Error saving image '{}': {}", output, e);
        std::process::exit(1);
    });

    println!("Saved word cloud to {}", output);
}
```

The final phase reports placement statistics to stderr (how many words were placed vs. skipped), resolves the output filename (using the auto-increment logic if no `-o` flag was given), saves the PNG via `image`'s built-in encoder, and prints the output path to stdout.

The separation of stderr for diagnostics and stdout for the result follows Unix convention — you could pipe the output filename to another program while still seeing the placement stats.

## Summary

Let's verify the file is the 444 lines we expected:

```bash
wc -l rust-wordcloud/src/main.rs
```

```output
444 rust-wordcloud/src/main.rs
```

```bash
grep -c 'fn ' rust-wordcloud/src/main.rs
```

```output
10
```

444 lines, 10 functions (including struct methods), 5 external crates — all in a single file. The architecture is clean and linear:

1. **`parse_hex_color`** — Converts hex strings to RGB pixels
2. **`get_color_palette`** — Returns one of six named color schemes
3. **`is_stop_word`** — Filters out common English words and web noise
4. **`count_words`** — Tokenizes text and builds a frequency-sorted word list
5. **`measure_text`** — Computes pixel bounding boxes for words at a given font size
6. **`SpatialGrid::new`** — Creates the grid-based spatial index
7. **`SpatialGrid::insert`** — Registers a placed word's rectangle in the grid
8. **`SpatialGrid::overlaps`** — Checks whether a candidate position collides with existing words
9. **`find_output_filename`** — Auto-increments output filenames to avoid overwrites
10. **`main`** — Orchestrates the pipeline: input -> word counting -> spiral placement -> PNG output

The key design decisions that make this work well:
- **Logarithmic font scaling** prevents the top word from visually dominating everything else
- **Archimedean spiral with random angle offsets** produces natural-looking layouts without obvious patterns
- **Grid-based spatial indexing** makes collision detection fast enough for hundreds of words
- **3px padding between words** ensures readability without excessive whitespace
