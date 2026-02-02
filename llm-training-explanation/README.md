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

---

## 4. The Forward Pass — Making a Prediction

Now that we understand the architecture, let us trace a single forward pass — the process of feeding tokens into the model and getting a prediction out. This is the heart of both training and inference.

### Step by Step Through the Forward Pass

Here is the full forward pass in nanochat, annotated:

```python
def forward(self, idx, targets=None, kv_cache=None, loss_reduction='mean'):
    B, T = idx.size()  # B=batch size, T=sequence length

    # 1. Get rotary embeddings for this sequence length
    cos_sin = self.cos[:, T0:T0+T], self.sin[:, T0:T0+T]

    # 2. Embed the tokens and normalize
    x = self.transformer.wte(idx)   # [B, T] -> [B, T, model_dim]
    x = norm(x)                     # RMS normalize
    x0 = x                          # save for x0 residual connections

    # 3. Pass through each Transformer block
    for i, block in enumerate(self.transformer.h):
        x = self.resid_lambdas[i] * x + self.x0_lambdas[i] * x0
        ve = self.value_embeds[str(i)](idx) if str(i) in self.value_embeds else None
        x = block(x, ve, cos_sin, self.window_sizes[i], kv_cache)

    # 4. Final normalization and projection to logits
    x = norm(x)
    logits = self.lm_head(x)
    logits = logits[..., :self.config.vocab_size]
    logits = logits.float()
    logits = 15 * torch.tanh(logits / 15)  # softcap

    # 5. If targets provided, compute loss; otherwise return logits
    if targets is not None:
        loss = F.cross_entropy(logits.view(-1, logits.size(-1)),
                               targets.view(-1), ignore_index=-1)
        return loss
    else:
        return logits
```

Let us trace what happens to a concrete example. Suppose our batch has one sequence: `[BOS, The, cat, sat]` (four token IDs).

**Step 1:** The token IDs `[0, 412, 5765, 2143]` enter the model.

**Step 2:** Each ID is looked up in the embedding table, producing four vectors of dimension 1,536. These are normalized by RMS norm.

**Step 3:** The four vectors pass through 24 Transformer blocks. In each block:
- The attention mechanism lets each token attend to all previous tokens (subject to the sliding window). The "sat" token can attend to "BOS," "The," and "cat." Through this mechanism, information flows between positions — "sat" can learn that the previous word was "cat," which is useful for predicting what comes next.
- The MLP then transforms each position's representation independently, applying the model's learned knowledge.

After 24 blocks of this, the four vectors have been thoroughly mixed and transformed. The vector at the "sat" position now encodes not just "the word 'sat'" but "the word 'sat' in the context of 'The cat sat.'"

**Step 4:** The final vector at each position is projected to a 32,768-dimensional vector of logits. The logit for token "on" at the "sat" position should be high (since "The cat sat on" is a natural continuation), while the logit for token "quantum" should be low.

**Step 5:** If we are training, these logits are compared to the actual target tokens using cross-entropy loss. If we are doing inference, the logits are returned directly so we can sample the next token.

### Mixed Precision: bfloat16 and float32

nanochat runs most of the forward pass in **bfloat16** (brain floating point 16-bit) precision. This format uses 16 bits instead of the standard 32 bits, which halves memory usage and roughly doubles computation speed on modern GPUs that have specialized hardware for it.

However, certain operations are done in float32 for numerical stability:
- The logit computation and softcapping are in float32
- The cross-entropy loss is in float32
- The embeddings are stored in bfloat16 (which is unusual — most models keep embeddings in float32, but nanochat found the optimizer can tolerate it)

The autocast context manager handles this transparently:

```python
autocast_ctx = torch.amp.autocast(device_type="cuda", dtype=torch.bfloat16)
with autocast_ctx:
    loss = model(x, y)
```

### torch.compile: Making it Fast

Before training begins, the model is compiled using PyTorch's JIT compiler:

```python
model = torch.compile(model, dynamic=False)
```

This analyzes the model's computation graph and fuses operations together, eliminates redundant memory accesses, and generates optimized GPU kernels. The `dynamic=False` flag tells the compiler that the input shapes will never change (which is true during training — every batch has the same shape), allowing more aggressive optimization.

---

## 5. The Loss Function — Measuring How Wrong the Prediction Was

The forward pass produces logits — a score for every token in the vocabulary, at every position in the sequence. But how do we quantify how good or bad these predictions are? That is the job of the **loss function**.

### Cross-Entropy Loss

nanochat uses **cross-entropy loss**, the standard loss function for classification tasks. In the context of language modeling, at each position the model produces a probability distribution over the vocabulary (by applying softmax to the logits), and cross-entropy measures how far that distribution is from the "correct" distribution (which puts all probability on the single correct next token).

In pseudocode, cross-entropy at a single position works like this:

```
Given:
  logits = [2.1, -0.5, 0.3, ..., 1.7]  (one score per vocab token)
  target = 445  (the correct next token ID)

Step 1: Convert logits to probabilities via softmax
  probs[i] = exp(logits[i]) / sum(exp(logits[j]) for all j)

Step 2: The loss is the negative log of the probability assigned to the correct token
  loss = -log(probs[target])
```

If the model assigned high probability to the correct token, the loss is low. If the model assigned low probability, the loss is high. A perfect model would assign probability 1.0 to the correct token, giving a loss of zero. A model that assigns probability 1/32768 (uniform random) gives a loss of about 10.4.

In nanochat's code, this is a single PyTorch call:

```python
loss = F.cross_entropy(
    logits.view(-1, logits.size(-1)),   # reshape to [B*T, vocab_size]
    targets.view(-1),                    # reshape to [B*T]
    ignore_index=-1,                     # skip masked positions
    reduction='mean'                     # average over all positions
)
```

The `ignore_index=-1` parameter is important: positions with a target of -1 (used for padding during SFT) are excluded from the loss. They contribute zero gradient, so the model does not try to learn from padded positions.

### Why Cross-Entropy Works

Cross-entropy is deeply connected to information theory. Minimizing cross-entropy is equivalent to maximizing the probability that the model assigns to the training data. It is also equivalent to minimizing the KL divergence between the model's predictions and the true data distribution. In simpler terms: a model with lower cross-entropy loss is one that would have been less "surprised" by the training data — it assigns higher probability to the sequences it actually saw.

### Bits Per Byte: A Fairer Metric

Raw cross-entropy loss depends on the vocabulary size — a model with 100,000 tokens has a harder prediction task than one with 32,000 tokens, even if both models are equally good at understanding language. To get a fair comparison across different tokenizers, nanochat uses **bits per byte (BPB)**.

BPB normalizes the loss by the number of bytes the tokens represent:

```python
def evaluate_bpb(model, batches, steps, token_bytes):
    total_nats = 0.0   # sum of all losses (in nats)
    total_bytes = 0     # sum of all byte lengths
    for x, y in batches:
        loss2d = model(x, y, loss_reduction='none')  # per-position loss
        num_bytes2d = token_bytes[y]                  # bytes per target token
        total_nats += (loss2d * (num_bytes2d > 0)).sum()
        total_bytes += num_bytes2d.sum()
    bpb = total_nats / (log(2) * total_bytes)
    return bpb
```

The conversion from nats (the natural unit of cross-entropy) to bits involves dividing by log(2). Special tokens (with zero byte length) are excluded. The result is a number like 0.89 BPB, meaning the model needs on average 0.89 bits to encode each byte of text. For reference, raw English text has an entropy of roughly 1.0-1.5 bits per byte, and GPT-2 achieves about 0.93 BPB on typical validation sets. Lower is better.

---

## 6. Backpropagation — Learning from Mistakes

The forward pass produces a loss. Backpropagation is the algorithm that turns that loss into a signal telling every parameter in the model how it should change to make the loss smaller.

### The Chain Rule in Action

Backpropagation is, fundamentally, a systematic application of the chain rule of calculus. The loss depends on the logits, which depend on the final layer's weights, which depend on the previous layer's outputs, and so on all the way back to the embedding table. The chain rule lets us compute how much the loss would change if we wiggled each parameter by a tiny amount.

In PyTorch, this is a single line of code:

```python
loss.backward()
```

This call traverses the computation graph in reverse (from loss back to parameters), computing the **gradient** of the loss with respect to every parameter that has `requires_grad=True`. After this call, every parameter tensor `p` has a `.grad` attribute containing its gradient.

### What Gradients Mean

A gradient is a direction — for each parameter, it tells you: "if you increase this parameter by a tiny amount, the loss will increase by this much." To reduce the loss, you want to move each parameter in the opposite direction of its gradient. The magnitude of the gradient tells you how sensitive the loss is to that parameter.

For a model with 300 million parameters, backpropagation computes 300 million gradient values in a single pass. This is remarkably efficient — it costs roughly twice the compute of a forward pass (not 300 million separate forward passes, as a naive approach would require). This efficiency is why neural networks with billions of parameters are trainable at all.

### Gradient Accumulation

A critical practical consideration: the desired batch size may be too large to fit in GPU memory at once. nanochat's default total batch size is 524,288 tokens, but a single GPU might only handle 32 sequences of 2,048 tokens (65,536 tokens) at a time.

The solution is **gradient accumulation**: run multiple forward-backward passes on smaller "micro-batches," accumulating the gradients, before performing a single optimizer step:

```python
grad_accum_steps = total_batch_size // world_tokens_per_fwdbwd  # e.g., 8

for micro_step in range(grad_accum_steps):
    loss = model(x, y)           # forward pass
    loss = loss / grad_accum_steps  # normalize so gradients average correctly
    loss.backward()              # accumulate gradients (they sum by default)
    x, y = next(train_loader)   # prefetch next batch during GPU work
```

The key detail is `loss = loss / grad_accum_steps`. Because PyTorch's `.backward()` adds gradients to any existing gradients (it does not replace them), running 8 micro-steps accumulates 8 times the gradient. Dividing the loss by 8 before backpropagation ensures the final accumulated gradient is the correct average, identical to what you would get from a single forward pass with 8 times the batch size.

Notice also the clever overlapping: `next(train_loader)` is called *inside* the micro-step loop, right after `.backward()`. This prefetches the next batch of data from CPU to GPU while the GPU is busy computing gradients — overlapping data transfer with computation for better utilization.

---

## 7. The Optimizer — Deciding How to Update the Weights

After backpropagation, every parameter has a gradient — a direction in which the loss would decrease. But the gradient only tells you the direction, not how far to step. Choosing the step size and the exact update rule is the job of the **optimizer**, and nanochat uses a remarkable split optimizer called **MuonAdamW** that applies different algorithms to different kinds of parameters.

### Why Not Just Follow the Gradient?

The simplest optimizer is **Stochastic Gradient Descent (SGD)**: multiply each gradient by a small learning rate and subtract it from the parameter. This works, but it is slow to converge because:

1. **Noisy gradients:** Each mini-batch gives a slightly different gradient. SGD jumps around noisily.
2. **Ill-conditioned landscapes:** Some parameters are much more sensitive than others. A learning rate that works for sensitive parameters is too small for insensitive ones, and vice versa.
3. **Saddle points and ravines:** The loss landscape has many directions where the gradient is small but the curvature is unfavorable. SGD gets stuck oscillating in these regions.

Modern optimizers address these problems with various forms of momentum and adaptive learning rates.

### AdamW: The Workhorse for Embeddings

For embedding parameters, the unembedding head, value embeddings, and per-layer scalars, nanochat uses **AdamW** — the most popular optimizer in deep learning.

AdamW maintains two running averages for each parameter:
- **First moment (exp_avg):** An exponentially weighted moving average of past gradients. This is momentum — it smooths out noise and keeps the optimizer moving in directions that are consistently beneficial.
- **Second moment (exp_avg_sq):** An exponentially weighted moving average of the squared gradients. This tracks how variable each parameter's gradient has been, and is used to set per-parameter learning rates.

Here is nanochat's fused AdamW implementation, annotated:

```python
def adamw_step_fused(p, grad, exp_avg, exp_avg_sq, step, lr, beta1, beta2, eps, wd):
    # 1. Weight decay: shrink the parameter slightly
    #    This is "decoupled" weight decay (applied to the parameter directly,
    #    not mixed into the gradient like in L2 regularization)
    p.mul_(1 - lr * wd)

    # 2. Update momentum (first moment)
    #    New momentum = beta1 * old_momentum + (1 - beta1) * gradient
    exp_avg.lerp_(grad, 1 - beta1)

    # 3. Update squared momentum (second moment)
    #    New sq_momentum = beta2 * old_sq_momentum + (1 - beta2) * gradient^2
    exp_avg_sq.lerp_(grad.square(), 1 - beta2)

    # 4. Bias correction (the running averages start at zero, so early
    #    values are biased low; this corrects for that)
    bias1 = 1 - beta1 ** step
    bias2 = 1 - beta2 ** step

    # 5. Compute the update: momentum / sqrt(squared_momentum)
    #    Parameters with large historical gradients get smaller updates
    #    Parameters with small historical gradients get larger updates
    denom = (exp_avg_sq / bias2).sqrt() + eps
    step_size = lr / bias1
    p.add_(exp_avg / denom, alpha=-step_size)
```

The genius of Adam is in step 5: dividing the momentum by the square root of the squared momentum gives each parameter its own effective learning rate. A parameter whose gradients have been consistently large gets a smaller effective step (because its denominator is large), and vice versa. This automatic per-parameter scaling is what makes Adam so effective.

nanochat uses `beta1=0.8` and `beta2=0.95` (compared to Adam's typical defaults of 0.9 and 0.999). The lower beta1 means less smoothing and faster response to gradient changes. The lower beta2 means the adaptive learning rate adjusts faster — useful for the relatively short training runs that nanochat performs.

### Muon: A Radically Different Optimizer for Matrix Parameters

For the 2D weight matrices in the Transformer blocks (attention projections, MLP layers), nanochat uses **Muon** — "Momentum Orthogonalized by Newton-Schulz." This is a fundamentally different approach to optimization that was developed in the modded-nanogpt project and has been shown to train faster than Adam for these specific kinds of parameters.

The core idea behind Muon is: instead of using the raw gradient as the update direction, transform it into the nearest **orthogonal matrix**. An orthogonal matrix is one where all the row vectors are perpendicular to each other and have unit length. This has the effect of making updates more "balanced" across all directions — no single direction dominates the update.

Here is how Muon works, step by step:

**Step 1: Nesterov Momentum.** Like Adam, Muon uses momentum to smooth gradients. But it uses *Nesterov* momentum, which is a slightly different formulation that "looks ahead" — it evaluates the gradient at the point where momentum would take you, rather than at the current point:

```python
# Nesterov momentum update
momentum_buffer.lerp_(stacked_grads, 1 - momentum)
g = stacked_grads.lerp_(momentum_buffer, momentum)
```

The momentum coefficient starts at 0.85 and warms up to 0.95 over the first 300 steps, giving the optimizer more smoothing as training progresses.

**Step 2: Polar Express Orthogonalization.** This is the distinctive step. The gradient matrix is orthogonalized using an iterative algorithm called the "Polar Express sign method" (a variant of Newton-Schulz iteration). The idea is to find the nearest orthogonal matrix to the gradient.

In simplified pseudocode:

```
# Start with the gradient matrix, normalized
X = gradient / norm(gradient)

# Run 5 iterations of the Polar Express algorithm
for each iteration:
    if X is a tall matrix (more rows than columns):
        A = X_transposed @ X
        B = b * A + c * (A @ A)
        X = a * X + X @ B
    else:  # wide matrix
        A = X @ X_transposed
        B = b * A + c * (A @ A)
        X = a * X + B @ X

# X is now approximately orthogonal
```

The coefficients `a`, `b`, `c` are precomputed constants chosen to maximize convergence speed. After five iterations, the matrix X is approximately orthogonal — its singular values are all close to 1 (they end up roughly uniformly distributed between 0.5 and 1.5, which turns out to be good enough).

Why does orthogonalization help? Think of a weight matrix as defining a transformation between two vector spaces. An orthogonal update ensures that the transformation changes "uniformly" in all directions — it does not collapse any dimension or amplify any dimension. This prevents the common problem where some weights grow much faster than others, leading to unbalanced representations.

**Step 3: Adafactor-style Variance Reduction.** After orthogonalization, Muon applies an additional normalization step inspired by the Adafactor optimizer. It maintains a factored second-moment estimate (tracking variance per-row or per-column, not per-element, to save memory) and uses it to scale the update:

```python
# Track variance along the reduction dimension
v_mean = g.float().square().mean(dim=red_dim, keepdim=True)
second_momentum_buffer.lerp_(v_mean, 1 - beta2)
step_size = second_momentum_buffer.clamp_min(1e-10).rsqrt()
```

This gives each row (or column) of the weight matrix its own learning rate, similar to how Adam gives each element its own rate — but with much less memory overhead.

**Step 4: Cautious Weight Decay.** Finally, Muon applies weight decay — but with a twist called "cautious" weight decay. It only decays weights when the update and the weight point in the same direction:

```python
mask = (g * stacked_params) >= 0
stacked_params.sub_(lr * g + lr * wd * stacked_params * mask)
```

The intuition: weight decay is supposed to prevent weights from growing too large. But if the optimizer is trying to push a weight in the opposite direction from its current value (trying to shrink it), applying weight decay on top of that would be counterproductive — you would be fighting yourself. Cautious decay only activates when the update is reinforcing the weight's current direction.

### Why Two Different Optimizers?

The split between Muon (for matrices) and AdamW (for everything else) is not arbitrary. It reflects a fundamental difference in the structure of these parameters:

- **Embedding tables** are lookup tables — each row is essentially an independent vector. There is no spatial structure to exploit. AdamW's per-element adaptive learning rate is ideal here.
- **Weight matrices** define linear transformations between vector spaces. They have rich structure — rows and columns interact, and the quality of the transformation depends on properties like the distribution of singular values. Muon's orthogonalization exploits this structure directly.
- **Scalars** (like resid_lambdas and x0_lambdas) are individual numbers with no structure at all. AdamW handles these fine.

nanochat also uses different learning rates for each group:
- Embedding parameters: lr=0.3 (high, because embeddings can tolerate large steps)
- Unembedding head: lr=0.004 (much lower, because the output projection is more sensitive)
- Matrix parameters (Muon): lr=0.02
- Scalar parameters: lr=0.5 (resid_lambdas) or lr=0.005 (x0_lambdas)

All learning rates are further scaled by a factor proportional to `1/sqrt(model_dim)` for the AdamW groups, so that larger models automatically get smaller learning rates for their embeddings.

### Fused Kernels and Zero-D Tensors

Both optimizer steps are decorated with `@torch.compile`, which fuses all the element-wise operations into a single GPU kernel. Without this, each operation (multiply, add, square, etc.) would launch a separate GPU kernel, with overhead for each launch. Fusing them into one kernel eliminates this overhead.

An additional trick: hyperparameters like learning rate and momentum are stored as zero-dimensional CPU tensors rather than Python floats:

```python
self._muon_lr_t = torch.tensor(0.0, dtype=torch.float32, device="cpu")
# ...
self._muon_lr_t.fill_(group["lr"])  # update value without creating new tensor
```

This is because `torch.compile` would have to recompile the kernel every time a Python float value changes. By using the same tensor object and just changing its value, the compiled kernel remains valid across steps even as the learning rate changes according to the schedule.

---

## 8. The Training Loop — Putting It All Together

Now we can see the complete training loop — the outer structure that ties together data loading, forward pass, loss computation, backpropagation, and optimization into a repeating cycle that gradually teaches the model to predict text.

### How Many Steps?

A fundamental question before training begins: how long should we train? nanochat uses **scaling laws** to determine the answer automatically. The key insight is the ratio between the number of training tokens and the number of model parameters:

```python
# Default: data-to-parameter ratio of 10.5
target_tokens = target_param_data_ratio * num_scaling_params
num_iterations = target_tokens // total_batch_size
```

For the speedrun's depth-24 model with about 350 million "scaling parameters" (transformer matrices plus the output head — embeddings are excluded from this count) and a ratio of 12, this gives roughly 4.2 billion tokens, or about 8,000 optimizer steps at a batch size of 524,288 tokens. This is slightly "overtrained" relative to the compute-optimal ratio of 10.5 (which itself is based on the Chinchilla scaling laws), but the extra training pushes the model to beat GPT-2's CORE score.

### The Learning Rate Schedule

The learning rate is not constant during training. nanochat uses a **warmup-constant-warmdown** schedule:

```python
def get_lr_multiplier(it):
    warmup_iters = round(warmup_ratio * num_iterations)
    warmdown_iters = round(warmdown_ratio * num_iterations)
    if it < warmup_iters:
        return (it + 1) / warmup_iters           # linear warmup
    elif it <= num_iterations - warmdown_iters:
        return 1.0                                 # constant
    else:
        progress = (num_iterations - it) / warmdown_iters
        return progress * 1.0 + (1 - progress) * final_lr_frac  # linear decay
```

- **Warmup (first 0% by default — disabled for pretraining):** The learning rate ramps up linearly from near zero. This prevents the model from making huge, destructive updates when the gradients are most noisy (at the very start).
- **Constant (first 50%):** The learning rate stays at its full value. This is where most of the learning happens.
- **Warmdown (last 50%):** The learning rate decays linearly back toward zero. This is like gradually slowing down as you approach your destination — the coarse structure of knowledge has been learned, and now the model is refining details. Smaller updates prevent it from overshooting.

In addition to the learning rate, Muon's momentum coefficient warms up from 0.85 to 0.95 over the first 300 steps, and weight decay decays linearly to zero over the entire run:

```python
def get_muon_momentum(it):
    frac = min(it / 300, 1)
    return (1 - frac) * 0.85 + frac * 0.95

def get_weight_decay(it):
    return weight_decay_scaled * (1 - it / num_iterations)
```

### The Complete Training Step

Here is one complete training step, showing how all the pieces fit together:

```python
# For each micro-batch in the gradient accumulation
for micro_step in range(grad_accum_steps):
    with autocast_ctx:                          # enable bfloat16
        loss = model(x, y)                      # forward pass -> loss
    train_loss = loss.detach()                  # save for logging (no grad needed)
    loss = loss / grad_accum_steps              # normalize for accumulation
    loss.backward()                             # backward pass -> gradients accumulate
    x, y, state = next(train_loader)            # prefetch next batch

# Update hyperparameters for this step
lrm = get_lr_multiplier(step)
muon_momentum = get_muon_momentum(step)
muon_weight_decay = get_weight_decay(step)
for group in optimizer.param_groups:
    group["lr"] = group["initial_lr"] * lrm
    if group['kind'] == 'muon':
        group["momentum"] = muon_momentum
        group["weight_decay"] = muon_weight_decay

# Apply the accumulated gradients
optimizer.step()

# Clear all gradients for the next step
model.zero_grad(set_to_none=True)
```

The `set_to_none=True` flag in `zero_grad` is a performance optimization: instead of filling gradient tensors with zeros (which requires writing to all that memory), it deallocates them entirely. They will be re-allocated on the next `.backward()` call.

### Distributed Training: Multiple GPUs

When training on 8 GPUs, nanochat uses **DistributedDataParallel (DDP)** via `torchrun`. Each GPU runs an identical copy of the training loop on different data. The gradients need to be averaged across all GPUs before the optimizer step, so that all copies of the model stay in sync.

nanochat's distributed optimizer (`DistMuonAdamW`) handles this with an overlapped three-phase communication pattern:

**Phase 1 — Launch all reduces:** For each parameter group, kick off an asynchronous gradient reduction operation. For AdamW's large parameters and Muon's stacked matrices, this uses `reduce_scatter` (each GPU gets one slice of the averaged gradient). For AdamW's small parameters (scalars, biases), it uses `all_reduce` (every GPU gets the full averaged gradient).

**Phase 2 — Compute updates as reduces finish:** Process each group in order: wait for its reduce to complete, compute the optimizer step on this GPU's slice, and immediately launch an `all_gather` to broadcast the updated parameters back to all GPUs.

**Phase 3 — Wait for gathers, copy back:** Wait for all gathers to finish, then copy the updated parameters from the gathered buffer back to the model's parameter tensors.

This overlapped design means that while one GPU is computing the Muon update for group A, it might simultaneously be receiving the gathered parameters for a group that finished earlier, and sending out the reduced gradients for a group that was launched later. The result is that communication and computation overlap, minimizing idle time.

For Muon specifically, the distributed version shards the parameter stack across GPUs. If there are 24 weight matrices of the same shape and 8 GPUs, each GPU "owns" 3 matrices. It only computes the Muon update for those 3 (including maintaining momentum buffers only for those 3), then gathers the results back to all GPUs. This gives near-linear memory savings for the optimizer state — a form of **ZeRO-2 optimization**.

### What Happens During a Three-Hour Run

To give a concrete sense of scale, here is what the speedrun looks like on 8xH100:

- **Model:** 24 layers, 1,536-dim, ~350M parameters
- **Batch size:** 524,288 tokens (32 sequences per GPU times 2,048 tokens, times 8 GPUs — one gradient accumulation step)
- **Steps:** ~8,300 optimizer steps
- **Tokens processed:** ~4.3 billion
- **Speed:** ~480,000 tokens/second, ~50% MFU (model FLOPS utilization)
- **Time:** ~3 hours wall clock

Every 250 steps the model is evaluated on a validation set, and every 2,000 steps it runs the full CORE benchmark. Sample generations are printed periodically so the operator can qualitatively see the model improving. At the end, a checkpoint is saved containing the model weights, optimizer state, and metadata for resumption.
