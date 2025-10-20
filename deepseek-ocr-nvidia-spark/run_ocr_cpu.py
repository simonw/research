#!/usr/bin/env python3
"""
DeepSeek-OCR Inference Script (CPU-only version)
Performs OCR on an image using the DeepSeek-OCR model on CPU
For systems where CUDA compute capability is not supported
"""

import torch
from transformers import AutoModel, AutoTokenizer
import os
import sys
from pathlib import Path
import time

def main():
    print("=" * 60)
    print("DeepSeek-OCR Inference (CPU Mode)")
    print("=" * 60)
    print()
    print("NOTE: Running on CPU - this will be slower than GPU")
    print("      but will work on systems with unsupported CUDA")
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
    print(f"Device: CPU")
    print()

    # Load model and tokenizer
    print(f"Loading model from {model_path}...")
    print("This may take a minute...")
    print("(CPU loading is slower than GPU)")
    start_time = time.time()

    try:
        tokenizer = AutoTokenizer.from_pretrained(
            model_path,
            trust_remote_code=True
        )
        print("✓ Tokenizer loaded")

        # Force CPU and use eager attention
        model = AutoModel.from_pretrained(
            model_path,
            _attn_implementation='eager',
            trust_remote_code=True,
            use_safetensors=True,
            torch_dtype=torch.float32,  # CPU works better with float32
            device_map='cpu'  # Force CPU
        )
        model = model.eval()
        print(f"✓ Model loaded in {time.time() - start_time:.2f} seconds")

    except Exception as e:
        print(f"ERROR loading model: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    print()

    # Perform OCR inference
    print(f"Processing image: {image_file}")
    print()

    # Use a simple prompt
    prompt = "<image>\nFree OCR."
    print(f"Using prompt: {prompt}")
    print()

    # Run inference
    print("Running OCR inference on CPU...")
    print("This will take several minutes on CPU - please be patient!")
    inference_start = time.time()

    try:
        # Use smaller sizes for CPU to speed things up
        result = model.infer(
            tokenizer,
            prompt=prompt,
            image_file=image_file,
            output_path=output_path,
            base_size=512,  # Smaller for CPU
            image_size=512,  # Smaller for CPU
            crop_mode=False,  # Disable cropping for simplicity
            save_results=True,
            test_compress=False  # Disable compression for CPU
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
