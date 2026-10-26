# Investigation Notes: How LLM Training Works in nanochat

## What I'm investigating

Writing an extensive explanatory essay about how the LLM training process works, based on the nanochat codebase (https://github.com/karpathy/nanochat). The essay should avoid mathematical notation but can include C/pseudocode snippets. It should cover LLM theory in depth.

## Key files studied

- `scripts/base_train.py` - The main pretraining script (469 lines)
- `scripts/chat_sft.py` - Supervised fine-tuning script (388 lines)
- `scripts/chat_rl.py` - Reinforcement learning script (342 lines)
- `nanochat/gpt.py` - The GPT model architecture (455 lines)
- `nanochat/optim.py` - MuonAdamW optimizer (529 lines)
- `nanochat/dataloader.py` - BOS-aligned bestfit data loader (166 lines)
- `nanochat/tokenizer.py` - BPE tokenizer (407 lines)
- `nanochat/engine.py` - Inference engine with KV cache (357 lines)
- `nanochat/flash_attention.py` - Flash Attention 3 / SDPA fallback (180 lines)
- `nanochat/loss_eval.py` - Bits per byte evaluation (66 lines)
- `nanochat/core_eval.py` - CORE metric evaluation (263 lines)
- `nanochat/dataset.py` - Dataset download/iteration (129 lines)
- `nanochat/checkpoint_manager.py` - Checkpoint save/load (174 lines)
- `nanochat/common.py` - Common utilities (259 lines)
- `tasks/gsm8k.py` - GSM8K task for RL (118 lines)
- `tasks/common.py` - Task base class and mixtures (148 lines)
- `scripts/tok_train.py` - Tokenizer training (107 lines)
- `runs/speedrun.sh` - Full pipeline script (101 lines)
- `dev/LOG.md` - Development experiment log (712 lines)

## Architecture observations

### Model (GPT)
- Uses rotary positional embeddings (RoPE) instead of learned positional embeddings
- QK normalization (RMS norm on queries and keys)
- Untied embedding/unembedding weights
- ReLU-squared activation in MLP (not GELU like original GPT-2)
- No learnable parameters in RMS norm (purely functional)
- No bias in any linear layers
- Group-Query Attention (GQA) support
- Flash Attention 3 integration
- Sliding window attention with configurable pattern (default: SSSL - 3 short, 1 long)
- Value embeddings (ResFormer-style) at alternating layers with input-dependent gates
- Per-layer learnable scalars: resid_lambdas and x0_lambdas
- Logit softcapping at 15 (tanh squashing)
- Vocab padding for tensor core alignment

### Optimizer (MuonAdamW)
- Split optimizer: Muon for 2D matrix params, AdamW for everything else
- Muon uses Polar Express orthogonalization (replaces Newton-Schulz)
- Adafactor-style variance reduction
- Cautious weight decay (only decays when update and weight have same sign)
- Weight decay scheduled linearly to zero
- Nesterov momentum in Muon
- Fused kernels via torch.compile for both AdamW and Muon steps
- Distributed version (DistMuonAdamW) with ZeRO-2-style sharding

### Data Pipeline
- Pretraining data: FineWeb-Edu 100B (shuffled), from HuggingFace
- BOS-aligned dataloader with best-fit packing
- Every row starts with a BOS token
- Documents are packed into rows; when nothing fits, shortest doc is cropped
- ~34.6% tokens cropped, 100% utilization (no padding)
- Tokenizer: BPE (32K vocab), trained with rustbpe, inference with tiktoken
- Special tokens for conversation rendering: bos, user_start/end, assistant_start/end, python_start/end, output_start/end

### Training Pipeline (3 stages)
1. **Pretraining**: Next-token prediction on FineWeb-Edu
   - Cross-entropy loss
   - Gradient accumulation for large effective batch sizes
   - LR warmup + constant + warmdown schedule
   - Muon momentum warmup from 0.85 to 0.95
   - Weight decay linear decay to zero

2. **SFT**: Supervised fine-tuning on conversation data
   - SmolTalk (460K), MMLU (100K), GSM8K (16K), identity (2K), spelling (280K)
   - Only trains on assistant tokens (user tokens masked)
   - Best-fit padding packing (no cropping, pad instead)
   - Simpler LR schedule: flat 80%, then linear decay

3. **RL**: Reinforcement learning on GSM8K
   - GRPO-style but simplified to essentially REINFORCE
   - No trust region / KL regularization
   - On-policy (no PPO ratio+clip needed)
   - Token-level normalization (DAPO style)
   - Advantage = reward - mean(rewards) (no division by std)
   - Calculator tool use during rollouts
   - Binary reward: 1 if answer matches, 0 otherwise

### Scaling Laws
- Model complexity controlled by single "depth" parameter
- model_dim = depth * 64 (aspect ratio)
- Optimal data:params ratio found to be ~10.5 (Kaplan-style counting)
- Weight decay scales as 1/depth^2
- LR scales as sqrt(batch_size) for both Adam and Muon

### Evaluation
- CORE metric (from DCLM paper) - aggregate of multiple benchmarks
- Bits per byte (BPB) - tokenizer-independent loss metric
- GSM8K pass@k for RL evaluation

## Key learnings

1. The entire pipeline from tokenizer training through pretraining, SFT, and RL fits in ~3000 lines of Python
2. Modern LLM training can reproduce GPT-2 quality in ~3 hours on 8xH100 for ~$73
3. The optimizer choice matters enormously - Muon for matrices, AdamW for embeddings
4. Many architectural ideas from modded-nanogpt were tried; value embeddings and x0 lambdas were the winners
5. The model is parameter-heavy in embeddings (token, value, bigram) relative to the transformer matrices
6. Sliding window attention with SSSL pattern provides good compute/quality tradeoff
7. The training data quality (FineWeb-Edu vs alternatives) matters as much as architecture
