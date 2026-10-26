#!/usr/bin/env python3
"""
Comprehensive benchmark comparing zvec (Alibaba's Proxima-based vector DB)
vs hnswlib for approximate nearest neighbor search.

Benchmark dimensions:
1. Index build time (varying dataset sizes)
2. Query latency & recall@k at varying ef_search (Pareto frontier)
3. High-dimensional stress test
4. Incremental insertion performance
5. Search with varying k
6. Effect of M parameter
"""

import json
import os
import sys
import time
import traceback
import shutil
import numpy as np
import resource
import gc
from dataclasses import dataclass, field, asdict
from typing import List, Dict, Any, Optional

# ──────────────────────────────────────────────────────────────────────
# Configuration
# ──────────────────────────────────────────────────────────────────────

RESULTS_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
os.makedirs(RESULTS_DIR, exist_ok=True)

SEED = 42
np.random.seed(SEED)

ZVEC_INITIALIZED = False

# ──────────────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────────────

@dataclass
class BenchmarkResult:
    test_name: str
    engine: str
    params: Dict[str, Any] = field(default_factory=dict)
    metrics: Dict[str, Any] = field(default_factory=dict)

def get_rss_mb():
    """Get resident set size in MB."""
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024

def compute_recall(ground_truth, predictions, k):
    """Compute recall@k."""
    assert ground_truth.shape[0] == predictions.shape[0]
    n = ground_truth.shape[0]
    recall_sum = 0.0
    for i in range(n):
        gt_set = set(ground_truth[i, :k].tolist())
        pred_set = set(predictions[i, :k].tolist())
        recall_sum += len(gt_set & pred_set) / k
    return recall_sum / n

def brute_force_knn(data, queries, k):
    """Compute exact k-NN using brute force for ground truth (L2 distance)."""
    n_queries = queries.shape[0]
    results = np.zeros((n_queries, k), dtype=np.int64)
    for i in range(n_queries):
        dists = np.sum((data - queries[i]) ** 2, axis=1)
        results[i] = np.argpartition(dists, k)[:k]
        top_k_dists = dists[results[i]]
        sorted_idx = np.argsort(top_k_dists)
        results[i] = results[i][sorted_idx]
    return results

def generate_clustered_data(n, dim, n_clusters=20):
    """Generate clustered data that's more realistic than uniform random."""
    points_per_cluster = n // n_clusters
    remainder = n % n_clusters
    data = []
    centroids = np.random.randn(n_clusters, dim).astype(np.float32) * 10
    for i in range(n_clusters):
        count = points_per_cluster + (1 if i < remainder else 0)
        cluster_data = centroids[i] + np.random.randn(count, dim).astype(np.float32) * 0.5
        data.append(cluster_data)
    data = np.vstack(data)
    np.random.shuffle(data)
    return data

# ──────────────────────────────────────────────────────────────────────
# Zvec Wrapper
# ──────────────────────────────────────────────────────────────────────

class ZvecBenchmark:
    def __init__(self, dim, M=16, ef_construction=200, db_path=None):
        import zvec
        global ZVEC_INITIALIZED
        self.dim = dim
        self.M = M
        self.ef_construction = ef_construction
        self.db_path = db_path or f"/tmp/zvec_bench_{int(time.time() * 1000)}"
        if os.path.exists(self.db_path):
            shutil.rmtree(self.db_path)

        if not ZVEC_INITIALIZED:
            zvec.init()
            ZVEC_INITIALIZED = True

        id_field = zvec.FieldSchema('id', zvec.DataType.INT64)
        vec_field = zvec.VectorSchema(
            'vec', zvec.DataType.VECTOR_FP32, dimension=dim,
            index_param=zvec.HnswIndexParam(
                m=M, ef_construction=ef_construction,
                metric_type=zvec.MetricType.L2
            )
        )
        schema = zvec.CollectionSchema('bench', fields=id_field, vectors=vec_field)
        self.collection = zvec.create_and_open(self.db_path, schema)
        self.zvec = zvec

    def add_items(self, data, ids=None):
        """Add vectors to the collection."""
        n = data.shape[0]
        if ids is None:
            ids = np.arange(n)

        docs = []
        for i in range(n):
            doc = self.zvec.Doc(
                id=str(int(ids[i])),
                vectors={'vec': data[i].tolist()},
                fields={'id': int(ids[i])}
            )
            docs.append(doc)

        # Insert in batches (zvec max is 1000 per insert call)
        batch_size = 1000
        for start in range(0, n, batch_size):
            end = min(start + batch_size, n)
            self.collection.insert(docs[start:end])

    def optimize(self):
        """Optimize the index."""
        self.collection.optimize()

    def search(self, queries, k=10, ef_search=None):
        """Search for nearest neighbors."""
        n_queries = queries.shape[0]
        all_labels = np.zeros((n_queries, k), dtype=np.int64)
        all_distances = np.zeros((n_queries, k), dtype=np.float32)

        for i in range(n_queries):
            param = None
            if ef_search is not None:
                param = self.zvec.HnswQueryParam(ef=ef_search)
            vq = self.zvec.VectorQuery('vec', vector=queries[i].tolist(), param=param)
            results = self.collection.query(vectors=vq, topk=k)
            for j, doc in enumerate(results):
                if j < k:
                    all_labels[i, j] = int(doc.id)
                    all_distances[i, j] = doc.score

        return all_labels, all_distances

    def cleanup(self):
        """Clean up resources."""
        try:
            del self.collection
        except:
            pass
        if os.path.exists(self.db_path):
            shutil.rmtree(self.db_path, ignore_errors=True)

# ──────────────────────────────────────────────────────────────────────
# Hnswlib Wrapper
# ──────────────────────────────────────────────────────────────────────

class HnswlibBenchmark:
    def __init__(self, dim, M=16, ef_construction=200, max_elements=100000):
        import hnswlib
        self.dim = dim
        self.M = M
        self.ef_construction = ef_construction
        self.index = hnswlib.Index(space='l2', dim=dim)
        self.index.init_index(
            max_elements=max_elements,
            M=M,
            ef_construction=ef_construction,
            random_seed=SEED
        )

    def add_items(self, data, ids=None):
        """Add vectors to the index."""
        if ids is None:
            ids = np.arange(data.shape[0])
        self.index.add_items(data, ids, num_threads=1)

    def search(self, queries, k=10, ef_search=None):
        """Search for nearest neighbors."""
        if ef_search is not None:
            self.index.set_ef(ef_search)
        labels, distances = self.index.knn_query(queries, k=k, num_threads=1)
        return labels, distances

    def cleanup(self):
        try:
            del self.index
        except:
            pass

# ──────────────────────────────────────────────────────────────────────
# Benchmark Tests
# ──────────────────────────────────────────────────────────────────────

def benchmark_build_time(results_list: List[BenchmarkResult]):
    """Test 1: Index build time at varying dataset sizes."""
    print("\n" + "=" * 70)
    print("TEST 1: Index Build Time")
    print("=" * 70)

    dim = 128
    M = 16
    ef_construction = 200
    sizes = [10000, 50000, 100000, 200000]

    for n in sizes:
        print(f"\n  Dataset size: {n}, dim: {dim}")
        data = generate_clustered_data(n, dim)

        # --- hnswlib ---
        gc.collect()
        t0 = time.perf_counter()
        hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n)
        hnsw.add_items(data)
        t_hnsw = time.perf_counter() - t0
        hnsw.cleanup()
        del hnsw
        gc.collect()

        print(f"    hnswlib build: {t_hnsw:.3f}s ({n/t_hnsw:.0f} vectors/s)")
        results_list.append(BenchmarkResult(
            test_name="build_time", engine="hnswlib",
            params={"n": n, "dim": dim, "M": M, "ef_construction": ef_construction},
            metrics={"build_time_s": round(t_hnsw, 4), "vectors_per_sec": round(n / t_hnsw, 1)}
        ))

        # --- zvec ---
        gc.collect()
        t0 = time.perf_counter()
        zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)
        zv.add_items(data)
        t_zvec = time.perf_counter() - t0
        zv.cleanup()
        del zv
        gc.collect()

        print(f"    zvec build:    {t_zvec:.3f}s ({n/t_zvec:.0f} vectors/s)")
        results_list.append(BenchmarkResult(
            test_name="build_time", engine="zvec",
            params={"n": n, "dim": dim, "M": M, "ef_construction": ef_construction},
            metrics={"build_time_s": round(t_zvec, 4), "vectors_per_sec": round(n / t_zvec, 1)}
        ))

        ratio = t_zvec / t_hnsw if t_hnsw > 0 else float('inf')
        print(f"    Ratio (zvec/hnswlib): {ratio:.2f}x")


def benchmark_recall_vs_speed(results_list: List[BenchmarkResult]):
    """Test 2: Recall@10 vs query speed trade-off (Pareto frontier)."""
    print("\n" + "=" * 70)
    print("TEST 2: Recall@10 vs Query Speed (Pareto Frontier)")
    print("=" * 70)

    dim = 128
    n = 100000
    n_queries = 1000
    k = 10
    M = 16
    ef_construction = 200
    ef_search_values = [10, 20, 40, 80, 160, 320, 500]

    data = generate_clustered_data(n, dim)
    queries = np.random.randn(n_queries, dim).astype(np.float32)

    print(f"  Computing ground truth (brute force, n={n})...")
    gt = brute_force_knn(data, queries, k)

    # --- hnswlib ---
    print(f"\n  Building hnswlib index...")
    hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n)
    hnsw.add_items(data)

    for ef in ef_search_values:
        hnsw.search(queries[:10], k=k, ef_search=ef)  # warm-up

        t0 = time.perf_counter()
        labels, _ = hnsw.search(queries, k=k, ef_search=ef)
        query_time = time.perf_counter() - t0

        recall = compute_recall(gt, labels, k)
        qps = n_queries / query_time
        avg_latency_us = (query_time / n_queries) * 1e6

        print(f"    hnswlib ef={ef:>4d}: recall@{k}={recall:.4f}, QPS={qps:.0f}, latency={avg_latency_us:.0f}µs")
        results_list.append(BenchmarkResult(
            test_name="recall_vs_speed", engine="hnswlib",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef, "M": M},
            metrics={"recall": round(recall, 5), "qps": round(qps, 1),
                      "avg_latency_us": round(avg_latency_us, 1)}
        ))

    hnsw.cleanup()
    del hnsw
    gc.collect()

    # --- zvec ---
    print(f"\n  Building zvec index...")
    zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)
    zv.add_items(data)
    zv.optimize()

    for ef in ef_search_values:
        zv.search(queries[:10], k=k, ef_search=ef)  # warm-up

        t0 = time.perf_counter()
        labels, _ = zv.search(queries, k=k, ef_search=ef)
        query_time = time.perf_counter() - t0

        recall = compute_recall(gt, labels, k)
        qps = n_queries / query_time
        avg_latency_us = (query_time / n_queries) * 1e6

        print(f"    zvec    ef={ef:>4d}: recall@{k}={recall:.4f}, QPS={qps:.0f}, latency={avg_latency_us:.0f}µs")
        results_list.append(BenchmarkResult(
            test_name="recall_vs_speed", engine="zvec",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef, "M": M},
            metrics={"recall": round(recall, 5), "qps": round(qps, 1),
                      "avg_latency_us": round(avg_latency_us, 1)}
        ))

    zv.cleanup()
    del zv
    gc.collect()


def benchmark_high_dimension(results_list: List[BenchmarkResult]):
    """Test 3: High-dimensional stress test."""
    print("\n" + "=" * 70)
    print("TEST 3: High-Dimensional Stress Test")
    print("=" * 70)

    dims = [64, 256, 512, 768]
    n = 50000
    n_queries = 200
    k = 10
    M = 32
    ef_construction = 200
    ef_search = 100

    for dim in dims:
        print(f"\n  Dimension: {dim}, n={n}")
        data = np.random.randn(n, dim).astype(np.float32)
        queries = np.random.randn(n_queries, dim).astype(np.float32)

        gt = brute_force_knn(data, queries, k)

        # --- hnswlib ---
        gc.collect()
        t0 = time.perf_counter()
        hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n)
        hnsw.add_items(data)
        build_hnsw = time.perf_counter() - t0

        t0 = time.perf_counter()
        labels_h, _ = hnsw.search(queries, k=k, ef_search=ef_search)
        search_hnsw = time.perf_counter() - t0
        recall_hnsw = compute_recall(gt, labels_h, k)

        print(f"    hnswlib: build={build_hnsw:.2f}s, search={search_hnsw:.3f}s, recall={recall_hnsw:.4f}")
        results_list.append(BenchmarkResult(
            test_name="high_dimension", engine="hnswlib",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef_search, "M": M},
            metrics={"build_time_s": round(build_hnsw, 3), "search_time_s": round(search_hnsw, 4),
                      "recall": round(recall_hnsw, 5), "qps": round(n_queries / search_hnsw, 1)}
        ))
        hnsw.cleanup()
        del hnsw
        gc.collect()

        # --- zvec ---
        gc.collect()
        t0 = time.perf_counter()
        zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)
        zv.add_items(data)
        zv.optimize()
        build_zvec = time.perf_counter() - t0

        t0 = time.perf_counter()
        labels_z, _ = zv.search(queries, k=k, ef_search=ef_search)
        search_zvec = time.perf_counter() - t0
        recall_zvec = compute_recall(gt, labels_z, k)

        print(f"    zvec:    build={build_zvec:.2f}s, search={search_zvec:.3f}s, recall={recall_zvec:.4f}")
        results_list.append(BenchmarkResult(
            test_name="high_dimension", engine="zvec",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef_search, "M": M},
            metrics={"build_time_s": round(build_zvec, 3), "search_time_s": round(search_zvec, 4),
                      "recall": round(recall_zvec, 5), "qps": round(n_queries / search_zvec, 1)}
        ))
        zv.cleanup()
        del zv
        gc.collect()


def benchmark_incremental_insert(results_list: List[BenchmarkResult]):
    """Test 4: Incremental insertion performance (simulating real-world streaming)."""
    print("\n" + "=" * 70)
    print("TEST 4: Incremental Insertion Performance")
    print("=" * 70)

    dim = 128
    n_total = 50000
    batch_size = 1000
    n_batches = n_total // batch_size
    M = 16
    ef_construction = 200

    data = generate_clustered_data(n_total, dim)

    # --- hnswlib ---
    print(f"\n  hnswlib: inserting {n_total} vectors in {n_batches} batches of {batch_size}")
    gc.collect()
    hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n_total)

    batch_times_hnsw = []
    for i in range(n_batches):
        start = i * batch_size
        end = start + batch_size
        t0 = time.perf_counter()
        hnsw.add_items(data[start:end], ids=np.arange(start, end))
        batch_times_hnsw.append(time.perf_counter() - t0)

    first_10 = np.mean(batch_times_hnsw[:10])
    last_10 = np.mean(batch_times_hnsw[-10:])
    total_hnsw = sum(batch_times_hnsw)
    print(f"    Total: {total_hnsw:.2f}s, First 10 avg: {first_10:.4f}s, Last 10 avg: {last_10:.4f}s")
    print(f"    Degradation ratio (last/first): {last_10/first_10:.2f}x")

    results_list.append(BenchmarkResult(
        test_name="incremental_insert", engine="hnswlib",
        params={"n_total": n_total, "batch_size": batch_size, "dim": dim, "M": M},
        metrics={
            "total_time_s": round(total_hnsw, 3),
            "first_10_batch_avg_s": round(first_10, 5),
            "last_10_batch_avg_s": round(last_10, 5),
            "degradation_ratio": round(last_10 / first_10, 3),
            "batch_times": [round(t, 5) for t in batch_times_hnsw]
        }
    ))
    hnsw.cleanup()
    del hnsw
    gc.collect()

    # --- zvec ---
    print(f"\n  zvec: inserting {n_total} vectors in {n_batches} batches of {batch_size}")
    gc.collect()
    zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)

    batch_times_zvec = []
    for i in range(n_batches):
        start = i * batch_size
        end = start + batch_size
        batch_data = data[start:end]
        batch_ids = np.arange(start, end)
        t0 = time.perf_counter()
        zv.add_items(batch_data, ids=batch_ids)
        batch_times_zvec.append(time.perf_counter() - t0)

    first_10 = np.mean(batch_times_zvec[:10])
    last_10 = np.mean(batch_times_zvec[-10:])
    total_zvec = sum(batch_times_zvec)
    print(f"    Total: {total_zvec:.2f}s, First 10 avg: {first_10:.4f}s, Last 10 avg: {last_10:.4f}s")
    print(f"    Degradation ratio (last/first): {last_10/first_10:.2f}x")

    results_list.append(BenchmarkResult(
        test_name="incremental_insert", engine="zvec",
        params={"n_total": n_total, "batch_size": batch_size, "dim": dim, "M": M},
        metrics={
            "total_time_s": round(total_zvec, 3),
            "first_10_batch_avg_s": round(first_10, 5),
            "last_10_batch_avg_s": round(last_10, 5),
            "degradation_ratio": round(last_10 / first_10, 3),
            "batch_times": [round(t, 5) for t in batch_times_zvec]
        }
    ))
    zv.cleanup()
    del zv
    gc.collect()


def benchmark_varying_k(results_list: List[BenchmarkResult]):
    """Test 5: Search with varying k values."""
    print("\n" + "=" * 70)
    print("TEST 5: Search with Varying k")
    print("=" * 70)

    dim = 128
    n = 100000
    n_queries = 500
    M = 16
    ef_construction = 200
    ef_search = 200
    k_values = [1, 5, 10, 50, 100]

    data = generate_clustered_data(n, dim)
    queries = np.random.randn(n_queries, dim).astype(np.float32)

    max_k = max(k_values)
    print(f"  Computing ground truth for k={max_k}...")
    gt = brute_force_knn(data, queries, max_k)

    # --- hnswlib ---
    print(f"\n  Building hnswlib index (n={n})...")
    hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n)
    hnsw.add_items(data)

    for k in k_values:
        t0 = time.perf_counter()
        labels, _ = hnsw.search(queries, k=k, ef_search=ef_search)
        search_time = time.perf_counter() - t0
        recall = compute_recall(gt[:, :k], labels, k)
        qps = n_queries / search_time

        print(f"    hnswlib k={k:>3d}: recall={recall:.4f}, QPS={qps:.0f}")
        results_list.append(BenchmarkResult(
            test_name="varying_k", engine="hnswlib",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef_search, "M": M},
            metrics={"recall": round(recall, 5), "qps": round(qps, 1),
                      "search_time_s": round(search_time, 4)}
        ))

    hnsw.cleanup()
    del hnsw
    gc.collect()

    # --- zvec ---
    print(f"\n  Building zvec index (n={n})...")
    zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)
    zv.add_items(data)
    zv.optimize()

    for k in k_values:
        t0 = time.perf_counter()
        labels, _ = zv.search(queries, k=k, ef_search=ef_search)
        search_time = time.perf_counter() - t0
        recall = compute_recall(gt[:, :k], labels, k)
        qps = n_queries / search_time

        print(f"    zvec    k={k:>3d}: recall={recall:.4f}, QPS={qps:.0f}")
        results_list.append(BenchmarkResult(
            test_name="varying_k", engine="zvec",
            params={"n": n, "dim": dim, "k": k, "ef_search": ef_search, "M": M},
            metrics={"recall": round(recall, 5), "qps": round(qps, 1),
                      "search_time_s": round(search_time, 4)}
        ))

    zv.cleanup()
    del zv
    gc.collect()


def benchmark_varying_M(results_list: List[BenchmarkResult]):
    """Test 6: Effect of M parameter on build time, memory, and recall."""
    print("\n" + "=" * 70)
    print("TEST 6: Effect of M Parameter")
    print("=" * 70)

    dim = 128
    n = 50000
    n_queries = 500
    k = 10
    ef_construction = 200
    ef_search = 100
    M_values = [8, 16, 32, 48, 64]

    data = generate_clustered_data(n, dim)
    queries = np.random.randn(n_queries, dim).astype(np.float32)

    gt = brute_force_knn(data, queries, k)

    for M in M_values:
        print(f"\n  M={M}")

        # --- hnswlib ---
        gc.collect()
        rss_before = get_rss_mb()
        t0 = time.perf_counter()
        hnsw = HnswlibBenchmark(dim, M=M, ef_construction=ef_construction, max_elements=n)
        hnsw.add_items(data)
        build_time = time.perf_counter() - t0
        rss_after = get_rss_mb()

        labels, _ = hnsw.search(queries, k=k, ef_search=ef_search)
        recall = compute_recall(gt, labels, k)

        print(f"    hnswlib: build={build_time:.2f}s, recall={recall:.4f}, rss_delta={rss_after-rss_before:.0f}MB")
        results_list.append(BenchmarkResult(
            test_name="varying_M", engine="hnswlib",
            params={"n": n, "dim": dim, "M": M, "ef_construction": ef_construction, "k": k},
            metrics={"build_time_s": round(build_time, 3), "recall": round(recall, 5),
                      "rss_delta_mb": round(rss_after - rss_before, 1)}
        ))
        hnsw.cleanup()
        del hnsw
        gc.collect()

        # --- zvec ---
        gc.collect()
        rss_before = get_rss_mb()
        t0 = time.perf_counter()
        zv = ZvecBenchmark(dim, M=M, ef_construction=ef_construction)
        zv.add_items(data)
        zv.optimize()
        build_time = time.perf_counter() - t0
        rss_after = get_rss_mb()

        labels, _ = zv.search(queries, k=k, ef_search=ef_search)
        recall = compute_recall(gt, labels, k)

        print(f"    zvec:    build={build_time:.2f}s, recall={recall:.4f}, rss_delta={rss_after-rss_before:.0f}MB")
        results_list.append(BenchmarkResult(
            test_name="varying_M", engine="zvec",
            params={"n": n, "dim": dim, "M": M, "ef_construction": ef_construction, "k": k},
            metrics={"build_time_s": round(build_time, 3), "recall": round(recall, 5),
                      "rss_delta_mb": round(rss_after - rss_before, 1)}
        ))
        zv.cleanup()
        del zv
        gc.collect()


# ──────────────────────────────────────────────────────────────────────
# Main
# ──────────────────────────────────────────────────────────────────────

def main():
    print("=" * 70)
    print("Vector DB Benchmark: zvec vs hnswlib")
    print("=" * 70)
    print(f"NumPy version: {np.__version__}")
    print(f"Seed: {SEED}")
    print()

    all_results: List[BenchmarkResult] = []

    tests = [
        ("Build Time", benchmark_build_time),
        ("Recall vs Speed", benchmark_recall_vs_speed),
        ("High Dimension", benchmark_high_dimension),
        ("Incremental Insert", benchmark_incremental_insert),
        ("Varying k", benchmark_varying_k),
        ("Varying M", benchmark_varying_M),
    ]

    for name, func in tests:
        try:
            func(all_results)
        except Exception as e:
            print(f"\n  ERROR in test '{name}': {e}")
            traceback.print_exc()

    # Save results
    output_path = os.path.join(RESULTS_DIR, "benchmark_results.json")
    with open(output_path, "w") as f:
        json.dump([asdict(r) for r in all_results], f, indent=2)
    print(f"\n\nResults saved to {output_path}")

    # Print summary
    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    print_summary(all_results)


def print_summary(results: List[BenchmarkResult]):
    """Print a formatted summary of results."""
    tests = {}
    for r in results:
        key = r.test_name
        if key not in tests:
            tests[key] = []
        tests[key].append(r)

    for test_name, test_results in tests.items():
        print(f"\n--- {test_name} ---")
        hnsw_results = [r for r in test_results if r.engine == "hnswlib"]
        zvec_results = [r for r in test_results if r.engine == "zvec"]

        if test_name == "build_time":
            for h, z in zip(hnsw_results, zvec_results):
                n = h.params["n"]
                ratio = z.metrics["build_time_s"] / h.metrics["build_time_s"] if h.metrics["build_time_s"] > 0 else float('inf')
                print(f"  n={n:>7d}: hnswlib={h.metrics['build_time_s']:.3f}s, zvec={z.metrics['build_time_s']:.3f}s (zvec/hnswlib={ratio:.2f}x)")

        elif test_name == "recall_vs_speed":
            print(f"  {'ef':>6s} | {'hnsw recall':>12s} {'hnsw QPS':>10s} | {'zvec recall':>12s} {'zvec QPS':>10s}")
            print(f"  {'-'*6}-+-{'-'*12}-{'-'*10}-+-{'-'*12}-{'-'*10}")
            for h, z in zip(hnsw_results, zvec_results):
                ef = h.params["ef_search"]
                print(f"  {ef:>6d} | {h.metrics['recall']:>12.4f} {h.metrics['qps']:>10.0f} | {z.metrics['recall']:>12.4f} {z.metrics['qps']:>10.0f}")

        elif test_name == "high_dimension":
            for h, z in zip(hnsw_results, zvec_results):
                dim = h.params["dim"]
                print(f"  dim={dim:>4d}: hnsw(build={h.metrics['build_time_s']:.2f}s, recall={h.metrics['recall']:.4f}, qps={h.metrics['qps']:.0f}) "
                      f"zvec(build={z.metrics['build_time_s']:.2f}s, recall={z.metrics['recall']:.4f}, qps={z.metrics['qps']:.0f})")

        elif test_name == "incremental_insert":
            for r in test_results:
                print(f"  {r.engine}: total={r.metrics['total_time_s']:.2f}s, "
                      f"degradation={r.metrics['degradation_ratio']:.2f}x")

        elif test_name == "varying_k":
            for h, z in zip(hnsw_results, zvec_results):
                k = h.params["k"]
                print(f"  k={k:>3d}: hnsw(recall={h.metrics['recall']:.4f}, qps={h.metrics['qps']:.0f}) "
                      f"zvec(recall={z.metrics['recall']:.4f}, qps={z.metrics['qps']:.0f})")

        elif test_name == "varying_M":
            for h, z in zip(hnsw_results, zvec_results):
                M = h.params["M"]
                print(f"  M={M:>3d}: hnsw(build={h.metrics['build_time_s']:.2f}s, recall={h.metrics['recall']:.4f}) "
                      f"zvec(build={z.metrics['build_time_s']:.2f}s, recall={z.metrics['recall']:.4f})")


if __name__ == "__main__":
    main()
