# Vector DB Benchmark: Investigation Notes

## Objective
Compare zvec (Alibaba's Proxima-based in-process vector DB) against hnswlib (the canonical HNSW implementation) across build performance, query speed, recall accuracy, and scalability.

## Approach

### Step 1: Cloning and Setup
- Cloned zvec from https://github.com/alibaba/zvec to /tmp/zvec
- Cloned hnswlib from https://github.com/nmslib/hnswlib to /tmp/hnswlib
- Building zvec from source failed due to Arrow dependency compile errors (CMake configuration issues with apache-arrow-21.0.0)
- Successfully installed zvec 0.2.0 from PyPI (pre-built manylinux wheel, 20.4MB)
- Successfully installed hnswlib 0.8.0 via pip (compiled from source with native SIMD)

### Step 2: API Exploration
- zvec has a document-oriented API (Doc, Collection, VectorQuery) -- it's a full database, not just an index
- hnswlib has a direct numpy array API (add_items, knn_query) -- it's a pure ANN library
- Key zvec quirk: max batch size for inserts is ~1000 docs per call
- zvec defaults: M=50, ef_construction=500, metric=IP (different from hnswlib defaults of M=16, ef_construction=200)
- For fair comparison, set identical parameters: M=16, ef_construction=200, metric=L2

### Step 3: Benchmark Design
Designed 6 tests:
1. **Build Time** - Index construction at 10K, 50K, 100K, 200K vectors (dim=128)
2. **Recall vs Speed** - Pareto frontier at ef_search={10,20,40,80,160,320,500} (100K vectors, dim=128)
3. **High Dimension** - Stress test at dim={64,256,512,768} (50K vectors)
4. **Incremental Insert** - 50K vectors in 50 batches of 1000
5. **Varying k** - Search with k={1,5,10,50,100} (100K vectors)
6. **Varying M** - M={8,16,32,48,64} (50K vectors)

Important caveat: zvec search is called per-query through Python (each query crosses Python->C++ boundary), while hnswlib processes all queries in a single batch call at the C++ level. This makes QPS comparison unfair for zvec -- the latency difference is dominated by Python overhead, not algorithmic differences.

### Step 4: Key Findings

#### Build Performance (zvec wins convincingly)
- At 200K vectors: zvec is 3.6x faster (17.8s vs 65.0s)
- zvec maintains ~11K vectors/s regardless of dataset size
- hnswlib degrades from 10K/s to 3K/s as dataset grows
- This is the most clear-cut result

#### Recall (comparable, zvec slightly better at high ef)
- At ef=80: zvec=0.648, hnswlib=0.622
- At ef=320: zvec=0.782, hnswlib=0.727
- At ef=500: zvec=0.785, hnswlib=0.771
- zvec achieves slightly better recall at the same ef_search values
- Both recall numbers are lower than expected because the data is clustered (harder for HNSW)

#### Query Speed (hnswlib wins, but unfair comparison)
- hnswlib: 2K-88K QPS depending on ef
- zvec: 1K-2.3K QPS
- BUT: hnswlib processes batch queries in C++ while zvec is called per-query from Python
- The real algorithmic latency difference is smaller than these numbers suggest

#### High-Dimensional Performance
- zvec build time advantage grows with dimension: 3.2x at dim=64, 7.1x at dim=768
- Recall consistently slightly better for zvec across all dimensions
- QPS advantage for hnswlib consistent but narrows at higher dimensions

#### Incremental Insert (zvec wins)
- zvec: no degradation (ratio 1.02x from first to last batch)
- hnswlib: significant degradation (ratio 2.77x)
- This reflects zvec's segment-based architecture vs hnswlib's in-place graph modification

#### Effect of M
- zvec consistently faster to build (1.5-2x advantage)
- Recall comparable, zvec slightly better at M=8 and M=64

### Step 5: Source Code Analysis

Read the core HNSW implementations of both libraries to understand WHY zvec builds faster but has higher per-query latency.

Key architectural differences documented in README.md.

## Lessons Learned
1. Benchmark fairness is hard -- the per-query Python overhead in zvec makes QPS comparison misleading
2. zvec is a full database with segments, WAL, ID maps -- extra layers add per-query latency but provide durability and incremental compaction
3. hnswlib is a pure in-memory index -- minimal abstraction, maximum per-query throughput
4. Build time advantage likely comes from zvec's multi-threaded builder with batch distance computation
5. zvec's segment architecture explains the flat insertion degradation curve
