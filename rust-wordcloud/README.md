# Rust Word Cloud CLI

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A command-line tool that generates word cloud PNG images from text input. Built in Rust with a custom layout algorithm — only external crates are used for PNG generation (`image`/`imageproc`) and font rendering (`ab_glyph`).

Inspired by Max Woolf's description of building a Rust word cloud generator: https://minimaxir.com/2026/02/ai-agent-coding/

## Features

- **Reads from file or stdin** — pass a filename or pipe text in
- **Auto-incrementing output names** — generates `wordcloud.png`, `wordcloud-2.png`, etc. to avoid overwriting
- **Configurable dimensions** — `--width` and `--height` flags
- **Multiple color schemes** — vibrant (default), warm, cool, mono, earth, neon
- **Custom background color** — any hex color via `--background`
- **Tunable parameters** — min/max font size, max words, minimum word length

## How It Works

1. **Text processing**: Tokenizes input, filters stop words (English common words, contractions, web/URL fragments), removes short words and numbers
2. **Frequency counting**: Ranks words by occurrence count
3. **Font scaling**: Maps word frequency to font size using logarithmic scaling for better visual distribution
4. **Spiral layout**: Places words along an Archimedean spiral from center outward, with random angular offsets per word
5. **Collision detection**: Uses a grid-based spatial index for fast overlap checking (much faster than brute-force for 200+ words)

## Usage

```bash
# From a file
wordcloud input.txt

# From stdin
cat mytext.txt | wordcloud

# With options
wordcloud input.txt -o output.png --width 1600 --height 1000 --colors warm --background 000000

# Customize word selection
wordcloud input.txt --max-words 100 --min-font-size 16 --max-font-size 120 --min-word-len 4
```

## Options

```
Arguments:
  [INPUT]  Input text file (reads from stdin if not provided)

Options:
  -o, --output <OUTPUT>                Output PNG filename (auto-generates wordcloud.png, wordcloud-2.png, etc.)
      --width <WIDTH>                  Image width in pixels [default: 1200]
      --height <HEIGHT>                Image height in pixels [default: 800]
      --max-words <MAX_WORDS>          Maximum number of words to display [default: 200]
      --min-font-size <MIN_FONT_SIZE>  Minimum font size in pixels [default: 12]
      --max-font-size <MAX_FONT_SIZE>  Maximum font size in pixels [default: 90]
      --colors <COLORS>                Color scheme: vibrant, warm, cool, mono, earth, neon [default: vibrant]
      --background <BACKGROUND>        Background color as hex [default: 1a1a2e]
      --min-word-len <MIN_WORD_LEN>    Minimum word length to include [default: 3]
```

## Color Schemes

| Scheme | Description |
|--------|-------------|
| `vibrant` | Bright, high-contrast multi-color (default) |
| `warm` | Reds, oranges, yellows |
| `cool` | Blues, cyans, teals |
| `mono` | White/gray tones on dark background |
| `earth` | Browns, olives, greens |
| `neon` | Electric bright colors |

## Example Output

Generated from Simon Willison's blog content for February 2026 (`simonwillison.net/dashboard/all-content-in-a-month.json?month=2026-02`):

![Word Cloud](wordcloud.png)

Top words from the dataset: "code" (158), "claude" (134), "showboat" (123), "github" (123), "simonwillison" (78), "rodney" (76), "coding" (66), "model" (64), "agent" (63), "sqlite" (62).

## Building

```bash
cargo build --release
```

Requires a system font — looks for DejaVu Sans Bold or Liberation Sans Bold.

## Implementation Details

- **No word cloud crate used** — the layout algorithm, word counting, stop word filtering, and spatial indexing are all implemented from scratch
- **Archimedean spiral placement** with per-word random angular offset for natural-looking layouts
- **Grid-based spatial index** (40px cells) for O(1) average-case collision checking
- **Logarithmic font scaling** to prevent top words from dominating the visual
- Places 190+ words in a 1200x800 canvas with the default settings
