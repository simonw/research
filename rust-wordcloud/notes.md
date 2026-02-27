# Development Notes: Rust Word Cloud CLI

## Approach

Built a word cloud generator in Rust from scratch. Only used external crates for:
- `image` + `imageproc` - PNG generation and text drawing
- `ab_glyph` - Font loading and text measurement
- `clap` - CLI argument parsing
- `rand` - Random angle offsets for spiral placement

Everything else (word counting, stop word filtering, layout algorithm, spatial indexing) was implemented from scratch.

## Key Design Decisions

### Word Frequency Scaling
- Used **logarithmic scaling** (`ln()`) instead of linear for font sizes. This prevents the top 1-2 words from dominating the canvas while still showing clear size differences.
- Without log scaling, the most frequent word ("code" at 158 occurrences) would be huge while words at 20-30 occurrences would be barely visible.

### Spiral Placement Algorithm
- Uses an **Archimedean spiral** (r = a * theta) emanating from the center of the canvas.
- Each word gets a random angular offset so the layout doesn't look too uniform.
- Key parameters: `d_theta = 0.15` (angular step), `a = 1.2` (radial growth per radian).
- The spiral expands until it reaches the diagonal of the canvas.

### Spatial Grid for Collision Detection
- First implementation used brute-force O(n) overlap checking against all placed words.
- Replaced with a **grid-based spatial index** (40px cells) that only checks nearby rectangles.
- This was essential for placing 200 words efficiently - the brute force approach was too slow with 15000+ spiral steps per word.

### Stop Word Filtering
- Implemented a comprehensive stop word list including standard English stop words, contractions, web/URL fragments (png, jpg, href, etc.), and HTML noise.
- Also filters out pure numbers and 6-character hex strings (color codes).

## Iterations

1. **v1**: Basic implementation with sqrt scaling and slow spiral. Only placed ~20 words.
2. **v2**: Switched to log scaling, added spatial grid, extended spiral to cover full canvas. Placed 190+ words.

## Things Learned

- `ab_glyph` requires explicitly importing the `Font` trait to call methods like `as_scaled()` on `FontRef`.
- The `imageproc` crate's `draw_text_mut` takes x,y as i32 coordinates.
- Archimedean spiral parameters need careful tuning: too tight and it doesn't cover the canvas, too loose and it misses gaps between words.
- The biggest factor in word cloud density is the angular step size of the spiral - 0.15 radians gave a good balance between thoroughness and speed.

## Test Data

Fetched from `https://simonwillison.net/dashboard/all-content-in-a-month.json?month=2026-02`.
- JSON with 87 content items (blog entries, blogmarks, quotations)
- Extracted title and body text, stripped HTML tags
- ~195K characters of text content
- Top words: "code" (158), "claude" (134), "showboat" (123), "github" (123)
