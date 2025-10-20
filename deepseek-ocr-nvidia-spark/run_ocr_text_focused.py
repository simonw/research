#!/usr/bin/env python3
"""
DeepSeek-OCR Text Extraction (optimized for readable output)
"""

import torch
from transformers import AutoModel, AutoTokenizer
import os
import sys
from pathlib import Path
import time

def main():
    print("=" * 60)
    print("DeepSeek-OCR Text Extraction")
    print("=" * 60)
    print()

    # Configuration
    model_path = './DeepSeek-OCR-model'
    image_file = './test_image.jpeg'
    output_path = './output_text'

    # Create output directory
    Path(output_path).mkdir(parents=True, exist_ok=True)

    print(f"PyTorch version: {torch.__version__}")
    print(f"CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"CUDA device: {torch.cuda.get_device_name(0)}")
    print()

    # Load model
    print(f"Loading model from {model_path}...")
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

    print(f"✓ Model loaded in {time.time() - start_time:.2f} seconds")
    print()

    # Try different prompts for better text output
    prompts = {
        'markdown': "<image>\n<|grounding|>Convert the document to markdown.",
        'free_ocr': "<image>\nFree OCR.",
        'detailed': "<image>\nDescribe this image in detail.",
    }

    for prompt_name, prompt in prompts.items():
        print(f"\n{'=' * 60}")
        print(f"Testing prompt: {prompt_name}")
        print(f"Prompt: {prompt}")
        print(f"{'=' * 60}\n")

        inference_start = time.time()

        try:
            result = model.infer(
                tokenizer,
                prompt=prompt,
                image_file=image_file,
                output_path=f"{output_path}/{prompt_name}",
                base_size=1024,
                image_size=640,
                crop_mode=True,
                save_results=True,
                test_compress=False
            )

            inference_time = time.time() - inference_start
            print(f"\n✓ Completed in {inference_time:.2f} seconds")

            if result:
                print("\n--- RESULT TEXT ---")
                print(result)
                print("--- END RESULT ---\n")

                # Save result to a text file
                result_file = f"{output_path}/{prompt_name}_result.txt"
                with open(result_file, 'w', encoding='utf-8') as f:
                    f.write(result)
                print(f"Saved to: {result_file}")
            else:
                print("(No direct text output, check output directory)")

        except Exception as e:
            print(f"Error with {prompt_name}: {e}")
            import traceback
            traceback.print_exc()

    print("\n" + "=" * 60)
    print("All prompts tested!")
    print("=" * 60)
    print(f"\nCheck output directory: {output_path}/")

if __name__ == "__main__":
    main()
