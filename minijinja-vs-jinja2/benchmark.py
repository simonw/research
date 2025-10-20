#!/usr/bin/env python3
"""
Benchmark script comparing minijinja and jinja2 template rendering performance.
"""
import argparse
import json
import time
import sys
import statistics
from typing import Dict, List, Any
import random
import string


def generate_product_data(num_products: int = 50) -> List[Dict[str, Any]]:
    """Generate realistic product data for template rendering."""
    categories = ["Electronics", "Clothing", "Home & Garden", "Sports", "Books"]
    products = []

    for i in range(num_products):
        product = {
            "name": f"Product {i+1} - {''.join(random.choices(string.ascii_uppercase, k=3))}",
            "description": " ".join([
                "This is a high-quality product that offers excellent value.",
                "Features include advanced technology and modern design.",
                "Perfect for everyday use and special occasions."
            ]),
            "price": round(random.uniform(9.99, 999.99), 2),
            "original_price": round(random.uniform(10.00, 1000.00), 2),
            "on_sale": random.choice([True, False]),
            "is_new": random.choice([True, False]),
            "featured": random.choice([True, False]),
            "sku": f"SKU-{i+1:05d}-{random.randint(1000, 9999)}",
            "stock": random.randint(0, 100),
            "rating": random.randint(1, 5),
            "reviews": random.randint(0, 500),
            "tags": random.sample(
                ["popular", "bestseller", "eco-friendly", "premium", "limited-edition", "trending"],
                k=random.randint(1, 4)
            ),
            "category_path": random.choice([
                ["Electronics", "Computers", "Laptops"],
                ["Clothing", "Men", "Shirts"],
                ["Home & Garden", "Kitchen", "Appliances"],
                ["Sports", "Outdoor", "Camping"],
                ["Books", "Fiction", "Mystery"],
            ])
        }
        products.append(product)

    return products


def get_template_context(num_products: int = 50) -> Dict[str, Any]:
    """Generate the context data for template rendering."""
    return {
        "site_name": "BenchMark Store",
        "font_family": "Arial, sans-serif",
        "bg_color": "#f5f5f5",
        "year": 2025,
        "category": "Featured",
        "nav_items": [
            {"name": "Home", "url": "/"},
            {"name": "Products", "url": "/products"},
            {"name": "About", "url": "/about"},
            {"name": "Contact", "url": "/contact"},
        ],
        "filters": {
            "price_range": "$10 - $100",
            "rating": "4+ stars",
        },
        "products": generate_product_data(num_products),
    }


def benchmark_jinja2(template_dir: str, iterations: int, num_products: int) -> Dict[str, Any]:
    """Benchmark jinja2 template rendering."""
    try:
        from jinja2 import Environment, FileSystemLoader
    except ImportError:
        return {"error": "jinja2 not installed"}

    env = Environment(loader=FileSystemLoader(template_dir))
    template = env.get_template("products.html")

    # Warmup
    context = get_template_context(num_products)
    for _ in range(5):
        template.render(context)

    # Actual benchmark
    times = []
    for i in range(iterations):
        context = get_template_context(num_products)
        start = time.perf_counter()
        result = template.render(context)
        end = time.perf_counter()
        times.append((end - start) * 1000)  # Convert to milliseconds

    return {
        "engine": "jinja2",
        "iterations": iterations,
        "num_products": num_products,
        "times_ms": times,
        "mean_ms": statistics.mean(times),
        "median_ms": statistics.median(times),
        "stdev_ms": statistics.stdev(times) if len(times) > 1 else 0,
        "min_ms": min(times),
        "max_ms": max(times),
        "output_length": len(result),
    }


def benchmark_minijinja(template_dir: str, iterations: int, num_products: int) -> Dict[str, Any]:
    """Benchmark minijinja template rendering."""
    try:
        from minijinja import Environment
    except ImportError:
        return {"error": "minijinja not installed"}

    env = Environment()

    # Load templates
    with open(f"{template_dir}/base.html", "r") as f:
        env.add_template("base.html", f.read())

    with open(f"{template_dir}/products.html", "r") as f:
        env.add_template("products.html", f.read())

    # Warmup
    context = get_template_context(num_products)
    for _ in range(5):
        env.render_template("products.html", **context)

    # Actual benchmark
    times = []
    for i in range(iterations):
        context = get_template_context(num_products)
        start = time.perf_counter()
        result = env.render_template("products.html", **context)
        end = time.perf_counter()
        times.append((end - start) * 1000)  # Convert to milliseconds

    return {
        "engine": "minijinja",
        "iterations": iterations,
        "num_products": num_products,
        "times_ms": times,
        "mean_ms": statistics.mean(times),
        "median_ms": statistics.median(times),
        "stdev_ms": statistics.stdev(times) if len(times) > 1 else 0,
        "min_ms": min(times),
        "max_ms": max(times),
        "output_length": len(result),
    }


def main():
    parser = argparse.ArgumentParser(description="Benchmark template engines")
    parser.add_argument(
        "--engine",
        choices=["jinja2", "minijinja"],
        required=True,
        help="Template engine to benchmark"
    )
    parser.add_argument(
        "--template-dir",
        default="./templates",
        help="Directory containing templates"
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=100,
        help="Number of iterations to run"
    )
    parser.add_argument(
        "--num-products",
        type=int,
        default=50,
        help="Number of products to generate"
    )
    parser.add_argument(
        "--output",
        help="Output file for results (JSON)"
    )

    args = parser.parse_args()

    print(f"Running {args.engine} benchmark...")
    print(f"Iterations: {args.iterations}")
    print(f"Products: {args.num_products}")
    print(f"Python: {sys.version}")
    print()

    if args.engine == "jinja2":
        results = benchmark_jinja2(args.template_dir, args.iterations, args.num_products)
    else:
        results = benchmark_minijinja(args.template_dir, args.iterations, args.num_products)

    # Add system info
    results["python_version"] = sys.version
    results["python_implementation"] = sys.implementation.name

    # Check for free-threaded build
    try:
        results["free_threaded"] = sys._is_gil_enabled() == False
    except AttributeError:
        results["free_threaded"] = False

    if "error" in results:
        print(f"Error: {results['error']}", file=sys.stderr)
        sys.exit(1)

    print(f"Results for {results['engine']}:")
    print(f"  Mean: {results['mean_ms']:.3f} ms")
    print(f"  Median: {results['median_ms']:.3f} ms")
    print(f"  Std Dev: {results['stdev_ms']:.3f} ms")
    print(f"  Min: {results['min_ms']:.3f} ms")
    print(f"  Max: {results['max_ms']:.3f} ms")
    print(f"  Output size: {results['output_length']} characters")

    if args.output:
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults written to {args.output}")

    return results


if __name__ == "__main__":
    main()
