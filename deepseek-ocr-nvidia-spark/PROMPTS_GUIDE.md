# DeepSeek-OCR Prompt Guide

## Overview

DeepSeek-OCR supports different prompts that optimize for different use cases. The prompt you choose affects both the output format and what information is extracted.

## Recommended Prompts

### 1. "Free OCR" - Best for Clean Text ⭐ RECOMMENDED

**Prompt:**
```python
prompt = "<image>\nFree OCR."
```

**Best for:**
- General text extraction
- Clean, readable output
- Articles and documents
- When you just want the text

**Output format:**
- Plain text
- Natural reading flow
- Minimal formatting
- Fast inference (~24 seconds)

**Example output:**
```
# The perils of vibe coding

Elaine Moore

new OpenAI model arrived this month with a glossy livestream,
group watch parties and a lingering sense of disappointment.

The YouTube comment section was underwhelmed...
```

**Use this script:**
```bash
python3 run_ocr_best.py your_image.jpg
```

---

### 2. "Markdown" - Best for Document Structure

**Prompt:**
```python
prompt = "<image>\n<|grounding|>Convert the document to markdown."
```

**Best for:**
- Documents with structure
- Articles with headings
- When you need formatting preserved
- Images with embedded figures

**Output format:**
- Markdown formatted
- Headings (##)
- Image references
- Bounding box coordinates
- Slower (~39 seconds)

**Example output:**
```markdown
## The perils of vibe coding

TECHNOLOGY
Elaine Moore

![](images/0.jpg)

new OpenAI model arrived this month...
```

---

### 3. "Grounding OCR" - Best for Bounding Boxes

**Prompt:**
```python
prompt = "<image>\n<|grounding|>OCR this image."
```

**Best for:**
- When you need text locations
- Building UI annotation tools
- Document analysis
- Precise text positioning

**Output format:**
- Text with coordinates
- `<|ref|>text<|/ref|><|det|>[[x1, y1, x2, y2]]<|/det|>` format
- Generates annotated image
- Most detailed (~58 seconds)

**Example output:**
```
<|ref|>The perils of vibe coding<|/ref|><|det|>[[352, 30, 624, 111]]<|/det|>
<|ref|>TECHNOLOGY<|/ref|><|det|>[[33, 199, 127, 230]]<|/det|>
```

**Outputs:**
- `result_with_boxes.jpg` - Image with bounding boxes
- Console shows coordinates

---

### 4. "Detailed Description" - Best for Image Analysis

**Prompt:**
```python
prompt = "<image>\nDescribe this image in detail."
```

**Best for:**
- Understanding image content
- Image analysis
- When you need context about the image
- Not primarily for OCR

**Output format:**
- Descriptive text about the image
- Layout analysis
- Visual description
- Fastest (~9 seconds)

**Example output:**
```
The image displays a printed page from a publication,
likely a magazine or a book, with a focus on technology
and AI. The page is divided into two main sections...
```

---

## Image Size Modes

Adjust these parameters for speed vs quality:

```python
# Tiny (fastest, 64 tokens)
base_size=512, image_size=512, crop_mode=False

# Small (fast, 100 tokens)
base_size=640, image_size=640, crop_mode=False

# Base (balanced, 256 tokens)
base_size=1024, image_size=1024, crop_mode=False

# Large (best quality, 400 tokens)
base_size=1280, image_size=1280, crop_mode=False

# Gundam (dynamic, 356+ tokens) - RECOMMENDED
base_size=1024, image_size=640, crop_mode=True
```

---

## Quick Reference Scripts

### Use "Free OCR" (Best Text Output)
```bash
python3 run_ocr_best.py test_image.jpeg
```

### Try All Prompts
```bash
python3 run_ocr_text_focused.py
```

### Original (with bounding boxes)
```bash
python3 run_ocr.py
```

---

## Comparison Table

| Prompt | Speed | Text Quality | Structure | Coordinates | Best Use Case |
|--------|-------|--------------|-----------|-------------|---------------|
| **Free OCR** | ⚡⚡⚡ Fast | ⭐⭐⭐ Excellent | ⭐ Basic | ❌ No | **General OCR** |
| Markdown | ⚡⚡ Medium | ⭐⭐⭐ Excellent | ⭐⭐⭐ Full | ⭐⭐ Partial | Documents |
| Grounding | ⚡ Slow | ⭐⭐ Good | ⭐ Basic | ⭐⭐⭐ Full | Annotations |
| Detailed | ⚡⚡⚡ Fastest | ⭐ N/A | ❌ N/A | ❌ No | Image analysis |

---

## Performance Benchmarks

Test image: 3503×1668 pixels (Financial Times article)

| Prompt | Time | Output Size | Tokens |
|--------|------|-------------|--------|
| Free OCR | 24s | Clean text | 2257 |
| Markdown | 39s | Formatted MD | 2257 + structure |
| Grounding | 58s | Text + coords | 2257 + boxes |
| Detailed | 9s | Description | ~300 |

---

## Tips for Best Results

### 1. Choose the Right Prompt
- **Just want text?** → Use "Free OCR"
- **Document with structure?** → Use "Markdown"
- **Need coordinates?** → Use "Grounding OCR"
- **Analyzing the image?** → Use "Detailed"

### 2. Adjust Image Size
- Large images: Use Gundam mode (`crop_mode=True`)
- Small images: Use appropriate base_size
- Speed matters: Use smaller sizes (512/640)
- Quality matters: Use larger sizes (1024/1280)

### 3. Post-Processing
- "Free OCR" may have minor flow issues
- "Markdown" preserves structure better
- Manual cleanup may be needed for complex layouts

---

## Example Usage

### Python Script
```python
from transformers import AutoModel, AutoTokenizer
import torch

model_path = './DeepSeek-OCR-model'
tokenizer = AutoTokenizer.from_pretrained(model_path, trust_remote_code=True)
model = AutoModel.from_pretrained(
    model_path,
    _attn_implementation='eager',
    trust_remote_code=True,
    use_safetensors=True,
    torch_dtype=torch.bfloat16,
    device_map='auto'
).eval()

# For clean text output
result = model.infer(
    tokenizer,
    prompt="<image>\nFree OCR.",
    image_file='document.jpg',
    output_path='./output',
    base_size=1024,
    image_size=640,
    crop_mode=True,
    save_results=True
)

print(result)
```

### Command Line
```bash
# Best text output
python3 run_ocr_best.py document.jpg

# Check output
cat ocr_output/ocr_result.txt
```

---

## Output Files

All prompts save to the specified `output_path`:

```
output/
├── result.mmd           # Main output (format varies by prompt)
├── result_with_boxes.jpg # (Only with grounding prompts)
└── images/              # Intermediate processing images
```

---

## Common Issues

### Empty or Whitespace Output
**Problem:** `result.mmd` contains only whitespace
**Solution:** Use "Free OCR" or "Markdown" prompt instead of "Grounding OCR"

### Text Flow Issues
**Problem:** Text doesn't flow naturally
**Solution:** Try different prompts or adjust `crop_mode`

### Missing Text
**Problem:** Some text not detected
**Solution:**
- Increase image_size or base_size
- Check image quality
- Try different crop_mode settings

---

## Conclusion

**For most use cases, use:**
```bash
python3 run_ocr_best.py your_image.jpg
```

This uses the "Free OCR" prompt which provides the cleanest, most readable text output.

For specialized needs (structure, coordinates, analysis), refer to the table above to choose the appropriate prompt.
