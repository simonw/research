# How LLM Training Works: A Deep Dive Through nanochat

Training a large language model is, at its heart, a process of teaching a computer to predict the next word. That single idea — next-token prediction — is the engine behind every chatbot, code assistant, and AI writing tool you have ever used. But the machinery required to make it work is vast and intricate, spanning tokenization, neural network architecture, optimization theory, and reinforcement learning.

This essay explains the entire training pipeline by walking through [nanochat](https://github.com/karpathy/nanochat), Andrej Karpathy's open-source project that trains a GPT-2-class language model from scratch in about three hours on eight GPUs for roughly $73. nanochat is deliberately minimal — around 3,000 lines of Python — yet it covers every major stage of modern LLM development. That makes it an ideal lens through which to understand the real mechanics of how these models learn.

We will proceed through the pipeline in the order that nanochat itself executes it, from raw text all the way to a chatbot you can talk to.

---

## Table of Contents

1. [Tokenization — Turning Text into Numbers](#1-tokenization--turning-text-into-numbers)
2. [The Dataset Pipeline — Feeding Text to the Model](#2-the-dataset-pipeline--feeding-text-to-the-model)
3. [The Transformer Architecture — The Brain of the Model](#3-the-transformer-architecture--the-brain-of-the-model)
4. [The Forward Pass — Making a Prediction](#4-the-forward-pass--making-a-prediction)
5. [The Loss Function — Measuring How Wrong the Prediction Was](#5-the-loss-function--measuring-how-wrong-the-prediction-was)
6. [Backpropagation — Learning from Mistakes](#6-backpropagation--learning-from-mistakes)
7. [The Optimizer — Deciding How to Update the Weights](#7-the-optimizer--deciding-how-to-update-the-weights)
8. [The Training Loop — Putting It All Together](#8-the-training-loop--putting-it-all-together)
9. [Supervised Fine-Tuning — Teaching the Model to Chat](#9-supervised-fine-tuning--teaching-the-model-to-chat)
10. [Reinforcement Learning — Learning from Trial and Error](#10-reinforcement-learning--learning-from-trial-and-error)
11. [Evaluation — Measuring How Good the Model Is](#11-evaluation--measuring-how-good-the-model-is)
12. [Inference — Using the Trained Model](#12-inference--using-the-trained-model)
13. [Scaling Laws — How Size Affects Intelligence](#13-scaling-laws--how-size-affects-intelligence)
14. [Conclusion](#14-conclusion)

---

## 1. Tokenization — Turning Text into Numbers

Neural networks cannot read text. They operate on numbers — specifically, on large arrays of floating-point values. Before training can begin, every piece of text must be converted into a sequence of integers. This conversion is called **tokenization**, and the system that performs it is called a **tokenizer**.

### Why Not Just Use Individual Characters?

The simplest approach would be to assign each character a number: 'a' = 0, 'b' = 1, and so on. This works, but it is extremely wasteful. The word "the" would become three tokens. The phrase "the cat sat on the mat" would become 22 tokens (including spaces). The model would have to learn, over and over again, that 't' followed by 'h' followed by 'e' is a common English word. That is a lot of wasted learning capacity.

The opposite extreme — giving every English word its own number — has the problem that there are too many words. You would need hundreds of thousands of token IDs just for common English, and you still would not handle typos, URLs, code, or other languages.

### Byte-Pair Encoding: The Middle Ground

nanochat uses **Byte-Pair Encoding (BPE)**, the same algorithm used by GPT-2, GPT-3, GPT-4, and most modern LLMs. BPE finds a middle ground between character-level and word-level tokenization by building a vocabulary of common subword chunks.

The BPE training algorithm works as follows:

1. Start with a base vocabulary of 256 individual bytes (every possible byte value).
2. Scan the training text and find the most frequently occurring pair of adjacent tokens.
3. Merge that pair into a single new token. Add it to the vocabulary.
4. Repeat steps 2-3 until you reach a target vocabulary size.

For example, if "th" appears very frequently, it becomes a single token. Then if "the" appears frequently, the token "th" followed by "e" gets merged into "the". Common words like "the", "and", "ing" end up as single tokens. Rare words get split into smaller pieces. A word the model has never seen, like "nanochatting", might be tokenized as ["nano", "chat", "ting"].

nanochat trains a BPE tokenizer with a vocabulary of 32,768 tokens (that is two to the power of fifteen). The training code is concise:

```python
# Train a BPE tokenizer on ~2 billion characters of text
tokenizer = RustBPETokenizer.train_from_iterator(text_iter, vocab_size=32768)
```

The training scans approximately two billion characters of text from the FineWeb-Edu dataset. It uses a Rust-based BPE implementation for speed, then wraps the result in a `tiktoken` encoding for fast inference.

### The Pre-Tokenization Regex

Before BPE merging begins, the raw text is split into chunks using a regex pattern. This prevents the BPE algorithm from merging tokens across word boundaries or mixing letters with numbers in unwanted ways. nanochat uses a GPT-4-style regex:

```python
SPLIT_PATTERN = r"""'(?i:[sdmt]|ll|ve|re)|[^\r\n\p{L}\p{N}]?+\p{L}+|\p{N}{1,2}| ?[^\s\p{L}\p{N}]++[\r\n]*|\s*[\r\n]|\s+(?!\S)|\s+"""
```

This looks intimidating, but it is doing sensible things: it keeps English contractions together ("don't" stays as one chunk), groups letters into words, limits number chunks to one or two digits at a time, and groups whitespace separately. The one-or-two digit limitation for numbers (rather than GPT-4's one-to-three) is a deliberate choice for nanochat's smaller vocabulary size — it wastes fewer tokens on large multi-digit combinations.

### Special Tokens

Beyond the learned BPE vocabulary, nanochat adds nine **special tokens** that the model uses as structural markers:

```python
SPECIAL_TOKENS = [
    "<|bos|>",              # Beginning of sequence — marks the start of every document
    "<|user_start|>",       # Marks the beginning of a user message
    "<|user_end|>",         # Marks the end of a user message
    "<|assistant_start|>",  # Marks the beginning of an assistant response
    "<|assistant_end|>",    # Marks the end of an assistant response
    "<|python_start|>",     # The assistant is invoking a Python calculator
    "<|python_end|>",       # End of Python expression
    "<|output_start|>",     # Calculator result begins
    "<|output_end|>",       # Calculator result ends
]
```

These tokens are never produced by BPE merging — they are reserved IDs inserted explicitly during data preparation. During pretraining, only `<|bos|>` is used (to separate documents). The conversation tokens come into play later during fine-tuning.

### Compression Ratio and Bits Per Byte

A good tokenizer compresses text efficiently. If the average token represents many bytes of text, then a fixed-length context window can "see" more text. nanochat's 32K-token BPE tokenizer achieves a compression ratio where the average non-special token represents roughly 4-5 bytes of English text. The tokenizer training script also precomputes a lookup table mapping each token ID to its byte length, which is essential for the "bits per byte" evaluation metric used later.

---

## 2. The Dataset Pipeline — Feeding Text to the Model

With a tokenizer in hand, the next challenge is feeding data to the model efficiently. Language model pretraining is extraordinarily data-hungry — nanochat trains on roughly ten billion tokens, drawn from hundreds of gigabytes of text. Getting that data to the GPU fast enough that it never has to wait is a real engineering problem.

### The Source Data: FineWeb-Edu

nanochat trains on **FineWeb-Edu**, a large dataset of educational web text curated by HuggingFace. The dataset is stored as Parquet files — a columnar storage format common in data engineering. Each Parquet file contains many "row groups", and each row group contains a batch of text documents in a column called `text`.

The download is incremental. The `speedrun.sh` script first downloads eight shards (about 800MB compressed) — just enough to start training the tokenizer. While the tokenizer trains, it kicks off a background download of the remaining 362 shards needed for the full ten billion tokens of pretraining data:

```python
# Download first 8 shards for tokenizer training
python -m nanochat.dataset -n 8
# Kick off background download of all shards needed for pretraining
python -m nanochat.dataset -n 370 &
```

### The BOS-Aligned Best-Fit Dataloader

The core data-loading challenge is this: documents vary wildly in length — some are a sentence, others are pages. But the model expects fixed-size batches of shape `(B, T)`, where `B` is the batch size and `T` is the sequence length (2,048 tokens by default). You need to pack variable-length documents into these fixed-size rows somehow.

nanochat uses what it calls "BOS-aligned best-fit packing." The key principles are:

1. **Every row starts with a BOS token.** This means the model always has a clean beginning-of-sequence marker and never starts in the middle of a document.
2. **Documents are packed using a best-fit algorithm.** Multiple short documents can share a single row.
3. **When nothing fits, crop a document to fill the remaining space exactly.** This gives 100% utilization — no padding tokens are wasted.

The algorithm for filling each row works like this in pseudocode:

```
for each row in the batch:
    row = []
    while row has space remaining:
        look through a buffer of tokenized documents
        find the LARGEST document that fits entirely in the remaining space
        if one fits:
            pop it from the buffer, append it to the row
        else:
            pop the SHORTEST document from the buffer
            crop it to fill the remaining space exactly
            append the cropped portion to the row
```

The buffer holds about 1,000 tokenized documents. By searching for the largest document that fits, the algorithm minimizes wasted space from cropping. When forced to crop (because no document fits), it crops the shortest available document, which minimizes the fraction of tokens lost.

This approach discards roughly 35% of tokens to cropping — a meaningful loss. But the tradeoff is that every token in every row can attend all the way back to a clean BOS marker, which means the model never sees confusing partial context.

### Inputs and Targets: The Shifted Pair

Once a row of `T+1` tokens is assembled, the dataloader splits it into an **input** sequence and a **target** sequence:

```python
# row_data is [B, T+1] tokens
cpu_inputs.copy_(row_data[:, :-1])   # First T tokens: what the model sees
cpu_targets.copy_(row_data[:, 1:])   # Last T tokens: what the model should predict
```

This is the fundamental structure of next-token prediction. If the row contains the tokens `[BOS, The, cat, sat, on, the, mat]`, then:
- Input:  `[BOS, The, cat, sat, on, the]`
- Target: `[The, cat, sat, on, the, mat]`

At every position, the model's job is to predict the next token. Given `BOS`, predict `The`. Given `The`, predict `cat`. And so on. This single, simple objective is what the entire training process optimizes.

### Efficient CPU-to-GPU Transfer

The dataloader is carefully optimized for throughput. It pre-allocates pinned CPU memory and persistent GPU buffers, then performs a single host-to-device copy per batch:

```python
# Pre-allocate once: inputs and targets side by side in one contiguous buffer
cpu_buffer = torch.empty(2 * B * T, dtype=torch.long, pin_memory=True)
gpu_buffer = torch.empty(2 * B * T, dtype=torch.long, device="cuda")

# Each batch: single copy from pinned CPU to GPU
gpu_buffer.copy_(cpu_buffer, non_blocking=True)
```

The `pin_memory=True` flag tells the operating system to lock this memory in physical RAM (preventing it from being swapped to disk), which enables faster DMA transfers to the GPU. The `non_blocking=True` flag allows the copy to proceed asynchronously — the CPU can start preparing the next batch while the GPU is still receiving the current one.

### Distributed Data Sharding

When training on multiple GPUs, each GPU needs different data. nanochat handles this simply: each GPU (called a "rank" in distributed training terminology) reads every Nth row group from the Parquet files, where N is the number of GPUs:

```python
# Each rank starts at its own offset and strides by world_size
rg_idx = ddp_rank  # GPU 0 reads row groups 0, 8, 16, ... GPU 1 reads 1, 9, 17, ...
while rg_idx < pf.num_row_groups:
    rg = pf.read_row_group(rg_idx)
    # ... process this row group ...
    rg_idx += ddp_world_size
```

This ensures all GPUs see different data without any communication overhead for data distribution. The data iterator also tracks its position (which Parquet file and which row group) so that training can be resumed from a checkpoint without re-reading data the model has already seen.
