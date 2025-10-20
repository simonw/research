#!/usr/bin/env python3
"""
DeepSeek-OCR - Optimized for best text output
Uses "Free OCR" prompt for clean, readable text extraction
"""

import torch
from transformers import AutoModel, AutoTokenizer
import os
import sys
from pathlib import Path
import time

def main():
    print("=" * 60)
    print("DeepSeek-OCR - Best Text Output")
    print("=" * 60)
    print()

    # Configuration
    model_path = './DeepSeek-OCR-model'
    image_file = sys.argv[1] if len(sys.argv) > 1 else './test_image.jpeg'
    output_path = './ocr_output'

    if not os.path.exists(image_file):
        print(f"ERROR: Image not found: {image_file}")
        print(f"Usage: {sys.argv[0]} [image_file]")
        sys.exit(1)

    # Create output directory
    Path(output_path).mkdir(parents=True, exist_ok=True)

    print(f"Image: {image_file}")
    print(f"Output: {output_path}")
    print()

    # Load model
    print("Loading model...")
    start_time = time.time()

    tokenizer = AutoTokenizer.from_pretrained(model_path, trust_remote_code=True)
    model = AutoModel.from_pretrained(
        model_path,
        _attn_implementation='eager',
        trust_remote_code=True,
        use_safetensors=True,
        torch_dtype=torch.bfloat16,
        device_map='auto'
    ).eval()

    print(f"✓ Model loaded in {time.time() - start_time:.2f} seconds\n")

    # Best prompt for clean text output
    prompt = "<image>\nFree OCR."
    
    print("Running OCR...")
    inference_start = time.time()

    result = model.infer(
        tokenizer,
        prompt=prompt,
        image_file=image_file,
        output_path=output_path,
        base_size=1024,
        image_size=640,
        crop_mode=True,
        save_results=True,
        test_compress=False
    )

    print(f"\n✓ Completed in {time.time() - inference_start:.2f} seconds\n")

    # Display and save results
    if result:
        print("=" * 60)
        print("OCR TEXT OUTPUT")
        print("=" * 60)
        print(result)
        print("=" * 60)
        print()

        # Save to text file
        output_file = os.path.join(output_path, 'ocr_result.txt')
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(result)
        print(f"✓ Saved to: {output_file}")

    # Check for other output files
    mmd_file = os.path.join(output_path, 'result.mmd')
    if os.path.exists(mmd_file):
        print(f"✓ Markdown saved to: {mmd_file}")

    print()
    print(f"All outputs in: {output_path}/")

if __name__ == "__main__":
    main()
