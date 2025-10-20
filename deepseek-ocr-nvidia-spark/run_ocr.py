#!/usr/bin/env python3
"""
DeepSeek-OCR Inference Script
Performs OCR on an image using the DeepSeek-OCR model
Adapted for ARM64 + CUDA 13.0 environment
"""

import torch
from transformers import AutoModel, AutoTokenizer
import os
import sys
from pathlib import Path
import time

def main():
    print("=" * 60)
    print("DeepSeek-OCR Inference")
    print("=" * 60)
    print()

    # Configuration
    model_path = './DeepSeek-OCR-model'
    image_file = './test_image.jpeg'
    output_path = './output'

    # Check if model exists
    if not os.path.exists(model_path):
        print(f"ERROR: Model not found at {model_path}")
        print("Please ensure you've cloned the model repository.")
        sys.exit(1)

    # Check if image exists
    if not os.path.exists(image_file):
        print(f"ERROR: Test image not found at {image_file}")
        print("Please run: bash download_test_image.sh")
        sys.exit(1)

    # Create output directory
    Path(output_path).mkdir(parents=True, exist_ok=True)

    # Print environment info
    print(f"PyTorch version: {torch.__version__}")
    print(f"CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"CUDA device: {torch.cuda.get_device_name(0)}")
        print(f"CUDA version: {torch.version.cuda}")
    print()

    # Determine attention implementation
    try:
        import flash_attn
        attn_implementation = 'flash_attention_2'
        print("Using Flash Attention 2")
    except ImportError:
        attn_implementation = 'eager'
        print("Flash Attention not available, using eager attention")
        print("(This may be slower but will work fine)")
    print()

    # Load model and tokenizer
    print(f"Loading model from {model_path}...")
    print("This may take a minute...")
    start_time = time.time()

    try:
        tokenizer = AutoTokenizer.from_pretrained(
            model_path,
            trust_remote_code=True
        )
        print("✓ Tokenizer loaded")

        model = AutoModel.from_pretrained(
            model_path,
            _attn_implementation=attn_implementation,
            trust_remote_code=True,
            use_safetensors=True,
            torch_dtype=torch.bfloat16,
            device_map='auto'  # Automatically handle device placement
        )
        model = model.eval()
        print(f"✓ Model loaded in {time.time() - start_time:.2f} seconds")

    except Exception as e:
        print(f"ERROR loading model: {e}")
        print("\nTrying with alternative settings...")
        try:
            model = AutoModel.from_pretrained(
                model_path,
                _attn_implementation='eager',
                trust_remote_code=True,
                use_safetensors=True,
                torch_dtype=torch.float16,
                device_map='auto'
            )
            model = model.eval()
            print(f"✓ Model loaded with fallback settings in {time.time() - start_time:.2f} seconds")
        except Exception as e2:
            print(f"ERROR: Failed to load model even with fallback: {e2}")
            sys.exit(1)

    print()

    # Perform OCR inference
    print(f"Processing image: {image_file}")

    # Different prompts for different use cases
    prompts = {
        'document': "<image>\n<|grounding|>Convert the document to markdown.",
        'general': "<image>\n<|grounding|>OCR this image.",
        'free': "<image>\nFree OCR.",
        'detailed': "<image>\nDescribe this image in detail."
    }

    # Use document prompt as default (most comprehensive)
    prompt = prompts['general']
    print(f"Using prompt: {prompt}")
    print()

    # Run inference
    print("Running OCR inference...")
    inference_start = time.time()

    try:
        result = model.infer(
            tokenizer,
            prompt=prompt,
            image_file=image_file,
            output_path=output_path,
            base_size=1024,
            image_size=640,
            crop_mode=True,
            save_results=True,
            test_compress=True
        )

        inference_time = time.time() - inference_start
        print(f"✓ Inference completed in {inference_time:.2f} seconds")
        print()

        # Display results
        print("=" * 60)
        print("OCR RESULTS")
        print("=" * 60)
        print()
        if result:
            print(result)
        else:
            print("(No text output returned, check output directory for saved files)")
        print()

        # Check output directory
        output_files = list(Path(output_path).glob('*'))
        if output_files:
            print("Output files saved:")
            for f in output_files:
                print(f"  - {f}")
        print()

        print("=" * 60)
        print("SUCCESS!")
        print("=" * 60)

    except Exception as e:
        print(f"ERROR during inference: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
