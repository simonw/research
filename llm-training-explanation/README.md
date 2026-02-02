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

---

## 3. The Transformer Architecture — The Brain of the Model

The model at the center of nanochat is a **GPT** — a Generative Pre-trained Transformer. It is a neural network whose job is: given a sequence of tokens, predict a probability distribution over what the next token should be. The architecture is a modernized version of the original GPT-2 design, incorporating several improvements discovered between 2019 and 2025.

Let us build up the architecture piece by piece.

### The High-Level Structure

At the highest level, a GPT model is a stack of repeating blocks sandwiched between an embedding layer at the input and an "unembedding" layer at the output:

```
Input token IDs:   [12, 445, 7823, 91, ...]
        |
        v
  Token Embedding        (look up a vector for each token)
        |
        v
  RMS Norm               (normalize the vectors)
        |
        v
  Transformer Block 1    (attention + MLP)
  Transformer Block 2
  ...
  Transformer Block N
        |
        v
  RMS Norm               (normalize again)
        |
        v
  Linear "lm_head"       (project to vocabulary-sized logits)
        |
        v
  Logit Softcapping      (squash extreme values)
        |
        v
Output: probability distribution over next token
```

nanochat's model complexity is controlled by a single parameter called **depth**. For the speedrun model, the depth is 24, meaning 24 Transformer blocks. The model dimension (the width of the internal vectors) is `depth * 64`, so a depth-24 model has a model dimension of 1,536. This "constant aspect ratio" design is a deliberate simplification that makes scaling experiments clean.

### Token Embeddings

The very first operation is **embedding lookup**. The model has a large table of vectors — one vector per token in the vocabulary. For nanochat's 32,768-token vocabulary with a model dimension of 1,536, this table has 32,768 rows and 1,536 columns, totaling about 50 million parameters just for this one table.

When a token ID comes in, the model simply looks up the corresponding row:

```python
x = self.transformer.wte(idx)  # idx is [B, T] of token IDs -> x is [B, T, model_dim]
```

This is not a learned transformation — it is a table lookup. But the values in the table are learned during training. Over time, the model learns to place semantically similar tokens near each other in this high-dimensional space.

After embedding, nanochat applies **RMS normalization** — a simple operation that scales each vector to have a consistent magnitude. Unlike many other models, nanochat's RMS norm has no learnable parameters; it is purely functional:

```python
def norm(x):
    return F.rms_norm(x, (x.size(-1),))
```

RMS norm divides each vector by the square root of the mean of its squared elements. In plain language: it ensures every vector has roughly the same "length," preventing any single dimension from dominating.

### The Transformer Block: Attention + MLP

Each of the 24 Transformer blocks has the same structure: an **attention** layer followed by an **MLP** (multi-layer perceptron) layer, each wrapped with a residual connection:

```python
class Block(nn.Module):
    def forward(self, x, ve, cos_sin, window_size, kv_cache):
        x = x + self.attn(norm(x), ve, cos_sin, window_size, kv_cache)
        x = x + self.mlp(norm(x))
        return x
```

The residual connections (the `x = x + ...` pattern) are crucial. They mean that information can flow directly from the input of a block to its output without being forced through the attention and MLP layers. This makes the network much easier to train — without residuals, deep networks tend to have vanishing gradients that prevent learning.

### Attention: The Core Innovation

The **self-attention mechanism** is the defining feature of the Transformer architecture and the reason LLMs work as well as they do. At its core, attention answers the question: "For each token in the sequence, which other tokens should it pay attention to, and how much?"

Here is how it works conceptually. For each token position, the model computes three vectors:

- **Query (Q):** "What am I looking for?"
- **Key (K):** "What do I contain?"
- **Value (V):** "What information should I pass along?"

The attention score between two positions is computed by comparing the query of one position against the key of the other. If a query and a key are similar (pointing in the same direction in high-dimensional space), the score is high, and the corresponding value gets a large weight.

In nanochat's code, the projections are straightforward linear layers:

```python
q = self.c_q(x).view(B, T, self.n_head, self.head_dim)    # queries
k = self.c_k(x).view(B, T, self.n_kv_head, self.head_dim) # keys
v = self.c_v(x).view(B, T, self.n_kv_head, self.head_dim) # values
```

The input `x` of shape `[B, T, model_dim]` is projected into multiple "heads." nanochat uses multiple attention heads (for example, 12 heads for a depth-24 model), each with a head dimension of 128. Each head independently learns different attention patterns — one might specialize in syntactic relationships, another in semantic ones, and so on.

**Causal masking** is essential: the model is only allowed to look at previous tokens, never future ones. When predicting the token after "The cat," the model can attend to "The" and "cat" but not to any later tokens. This is what makes the model autoregressive — each prediction depends only on what came before.

### Rotary Positional Embeddings (RoPE)

One critical question: how does the model know the *order* of tokens? The basic attention mechanism treats the input as a set — it has no inherent notion of position. The original Transformer added learned positional embeddings, but nanochat uses a more modern approach: **Rotary Positional Embeddings (RoPE)**.

RoPE works by rotating the query and key vectors by an angle that depends on their position in the sequence. Two tokens that are close together get rotated by similar angles, so their dot product is high. Tokens far apart get rotated differently, reducing their dot product.

The implementation splits each head's dimensions into pairs and applies a rotation to each pair:

```python
def apply_rotary_emb(x, cos, sin):
    d = x.shape[3] // 2
    x1, x2 = x[..., :d], x[..., d:]  # split into pairs
    y1 = x1 * cos + x2 * sin          # rotate each pair
    y2 = x1 * (-sin) + x2 * cos
    return torch.cat([y1, y2], 3)
```

Think of it like this: each pair of dimensions forms a 2D plane, and the position of a token determines the rotation angle in that plane. Lower-frequency rotations (in the early dimension pairs) encode coarse position information (is this token near the beginning or end?), while higher-frequency rotations (in later pairs) encode fine-grained position differences (is this token one position or two positions away?).

The rotation frequencies are precomputed once:

```python
inv_freq = 1.0 / (base ** (channel_range / head_dim))  # base=10000
freqs = torch.outer(t, inv_freq)  # t is [0, 1, 2, ..., seq_len-1]
cos, sin = freqs.cos(), freqs.sin()
```

A key advantage of RoPE over learned positional embeddings is that the relative position between any two tokens is encoded naturally — the model does not need to learn separate representations for absolute positions.

### QK Normalization

After applying rotary embeddings, nanochat normalizes both queries and keys:

```python
q, k = norm(q), norm(k)  # RMS norm on queries and keys
```

This is called **QK normalization** and it stabilizes training by preventing the attention scores from growing too large or too small. Without this, the dot products between queries and keys can become enormous in magnitude, causing the softmax to saturate (producing near-zero gradients) and destabilizing training. QK norm ensures the vectors always have controlled magnitude before the dot product is computed.

### Sliding Window Attention

Standard attention allows every token to attend to every previous token. For a 2,048-token sequence, this means each token potentially looks at all 2,047 tokens before it. nanochat optimizes this with **sliding window attention**, where some layers only attend to a window of recent tokens.

The pattern is controlled by a string like `"SSSL"` (the default), which tiles across layers:
- **S (Short):** Attend to only the last 1,024 tokens (half the context)
- **L (Long):** Attend to the full context

So in a 24-layer model with pattern "SSSL", layers 0, 1, 2 use short windows, layer 3 uses the full context, layers 4, 5, 6 use short windows, layer 7 uses the full context, and so on. The final layer always uses the full context regardless of the pattern.

This is a compute-versus-quality tradeoff. Short-window attention is cheaper because the attention computation scales with the window size. But the interleaving of short and long layers means that information can still propagate across the full context — it just takes a few layers to do so.

### Flash Attention

The actual attention computation is performed by **Flash Attention 3** on Hopper GPUs (H100s), with a fallback to PyTorch's Scaled Dot-Product Attention (SDPA) on other hardware. Flash Attention is a heavily optimized GPU kernel that computes attention without materializing the full attention matrix in memory. For a sequence of length T, naive attention would require storing a T-by-T matrix per head — which at 2,048 tokens is manageable, but at longer sequences becomes a bottleneck. Flash Attention avoids this by computing attention in tiles, streaming through the data without ever storing the full matrix.

From the model's perspective, the interface is simple:

```python
y = flash_attn.flash_attn_func(q, k, v, causal=True, window_size=window_size)
```

### Value Embeddings: The ResFormer Trick

One of nanochat's more unusual architectural choices is **value embeddings**. At alternating layers, the model looks up a second embedding table (separate from the main token embedding) and mixes it into the values used by attention:

```python
if ve is not None:
    ve = ve.view(B, T, self.n_kv_head, self.head_dim)
    gate = 2 * torch.sigmoid(self.ve_gate(x[..., :32]))  # input-dependent gate
    v = v + gate.unsqueeze(-1) * ve
```

This is inspired by the "ResFormer" paper. The idea is that the original token identity is useful information that tends to get diluted as it passes through many layers. By re-injecting a direct embedding of the token at certain layers, the model gets a "shortcut" back to the raw token identity. The gate is input-dependent — the model learns when this shortcut is useful and when it is not.

### The MLP: Where the Knowledge Lives

After attention, each block has a **multi-layer perceptron (MLP)**. If attention is about routing information between positions, the MLP is about transforming information within a single position. It is where much of the model's factual knowledge is believed to be stored.

nanochat's MLP is simple:

```python
class MLP(nn.Module):
    def forward(self, x):
        x = self.c_fc(x)       # project up: model_dim -> 4 * model_dim
        x = F.relu(x).square() # activation: ReLU squared
        x = self.c_proj(x)     # project down: 4 * model_dim -> model_dim
        return x
```

The structure is "expand, activate, compress." The input is projected to four times the model dimension (creating a wider internal representation), passed through a nonlinear activation function, and then projected back down.

The activation function **ReLU-squared** (also called "squared ReLU") is notable. Standard ReLU just zeroes out negative values: `relu(x) = max(0, x)`. ReLU-squared takes this a step further by squaring the result: `relu_sq(x) = max(0, x)^2`. This produces sparser activations — most values are zero, and the nonzero values grow quadratically, giving the network sharper selectivity. The original GPT-2 used GELU (a smoother cousin of ReLU), but ReLU-squared has been found to work better in recent architectures.

### Per-Layer Learnable Scalars

nanochat adds two per-layer learnable scalar values that modulate the residual stream:

```python
for i, block in enumerate(self.transformer.h):
    x = self.resid_lambdas[i] * x + self.x0_lambdas[i] * x0
    # ... then pass through block ...
```

- **resid_lambdas:** Scales the residual stream before each block (initialized to 1.0 — neutral).
- **x0_lambdas:** Blends in the original embedding `x0` from the very first layer (initialized to 0.1 — small but nonzero).

The `x0` skip connection is particularly interesting. It gives every layer a direct path back to the original token embedding, preventing the "representation drift" problem where the model's internal representations become so transformed that they lose contact with the original input. This was found experimentally to improve training stability and final quality.

### The Output Head and Logit Softcapping

After passing through all Transformer blocks, the final hidden states are projected to vocabulary-sized logits:

```python
logits = self.lm_head(x)                           # [B, T, vocab_size]
logits = logits[..., :self.config.vocab_size]       # remove padding
logits = logits.float()                             # switch to fp32
logits = softcap * torch.tanh(logits / softcap)     # squash to [-15, 15]
```

The `lm_head` is a simple linear layer that projects from `model_dim` to `vocab_size` — for each position, it produces one score per token in the vocabulary. These scores (called **logits**) indicate how likely each token is to come next.

Importantly, nanochat uses **untied embedding and unembedding weights**. Many earlier models shared the same weight matrix for the input embedding table and the output projection (since both connect model_dim to vocab_size). nanochat keeps them separate, which adds parameters but allows each to specialize for its own role.

The **logit softcapping** at 15 is a stability measure. It smoothly squashes logits into the range negative-15 to positive-15 using the hyperbolic tangent function. Without this, extremely confident predictions could produce very large logits that destabilize training. The tanh squashing makes the model's predictions "softer" — it can still be very confident, but it cannot produce infinitely confident predictions.

### Weight Initialization: Getting the Starting Point Right

How the model's weights are initialized before training begins matters enormously. Bad initialization can make training unstable or prevent the model from learning at all. nanochat uses a carefully designed initialization scheme:

```python
# Embedding: normal distribution with standard deviation 1.0
torch.nn.init.normal_(self.transformer.wte.weight, mean=0.0, std=1.0)
# Output head: very small values so initial predictions are near-uniform
torch.nn.init.normal_(self.lm_head.weight, mean=0.0, std=0.001)
# Attention and MLP input projections: uniform distribution
torch.nn.init.uniform_(block.attn.c_q.weight, -s, s)  # s = sqrt(3/model_dim)
# Attention and MLP output projections: zeros
torch.nn.init.zeros_(block.attn.c_proj.weight)
torch.nn.init.zeros_(block.mlp.c_proj.weight)
```

The key design choices here:
- **Output projections start at zero.** This means at initialization, each Transformer block is effectively a no-op — it contributes nothing to the residual stream. The model starts as a simple lookup table (embedding -> lm_head) and gradually learns to use the Transformer blocks.
- **The lm_head uses very small initial values** so that the model's initial predictions are close to uniform over the vocabulary. This prevents the model from being "overconfident" about random predictions before any learning has occurred.
- **Uniform initialization** for the input projections avoids the outlier values that normal distributions occasionally produce, which can destabilize early training.
