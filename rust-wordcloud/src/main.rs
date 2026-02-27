use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use clap::Parser;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

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
        "cool" => vec![
            Rgb([72, 202, 228]),
            Rgb([0, 150, 199]),
            Rgb([144, 224, 239]),
            Rgb([2, 62, 138]),
            Rgb([86, 207, 225]),
            Rgb([100, 149, 237]),
            Rgb([0, 180, 216]),
            Rgb([72, 149, 239]),
        ],
        "mono" => vec![
            Rgb([255, 255, 255]),
            Rgb([220, 220, 220]),
            Rgb([190, 190, 190]),
            Rgb([160, 160, 160]),
            Rgb([240, 240, 240]),
        ],
        "earth" => vec![
            Rgb([188, 108, 37]),
            Rgb([85, 107, 47]),
            Rgb([160, 82, 45]),
            Rgb([107, 142, 35]),
            Rgb([210, 180, 140]),
            Rgb([139, 90, 43]),
            Rgb([154, 205, 50]),
            Rgb([218, 165, 32]),
        ],
        "neon" => vec![
            Rgb([57, 255, 20]),
            Rgb([255, 20, 147]),
            Rgb([0, 255, 255]),
            Rgb([255, 255, 0]),
            Rgb([255, 105, 180]),
            Rgb([0, 255, 127]),
            Rgb([138, 43, 226]),
            Rgb([255, 69, 0]),
        ],
        _ => vec![
            // "vibrant" (default)
            Rgb([255, 107, 107]),
            Rgb([78, 205, 196]),
            Rgb([255, 230, 109]),
            Rgb([170, 120, 250]),
            Rgb([69, 183, 209]),
            Rgb([255, 177, 66]),
            Rgb([99, 255, 170]),
            Rgb([255, 143, 177]),
            Rgb([129, 236, 236]),
            Rgb([250, 177, 160]),
        ],
    }
}

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
