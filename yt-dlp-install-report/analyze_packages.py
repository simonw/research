#!/usr/bin/env python3
"""Analyze installed yt-dlp packages and their contents."""

import os
import csv
from pathlib import Path
from collections import defaultdict

DIST_PACKAGES = Path("/usr/local/lib/python3.11/dist-packages")
PACKAGES = [
    "brotli-1.2.0",
    "mutagen-1.47.0",
    "pycryptodomex-3.23.0",
    "websockets-15.0.1",
    "yt_dlp-2025.11.12",
    "yt_dlp_ejs-0.3.1"
]

def analyze_package(pkg_name):
    """Analyze a single package and return statistics."""
    dist_info = DIST_PACKAGES / f"{pkg_name}.dist-info"
    record_file = dist_info / "RECORD"

    if not record_file.exists():
        return None

    files = []
    total_size = 0
    file_types = defaultdict(int)

    with open(record_file, 'r') as f:
        reader = csv.reader(f)
        for row in reader:
            if not row:
                continue

            file_path = row[0]
            file_hash = row[1] if len(row) > 1 else ""
            file_size = row[2] if len(row) > 2 else "0"

            # Handle missing or empty size
            try:
                size = int(file_size) if file_size else 0
            except ValueError:
                size = 0

            # Calculate actual size for files without recorded size
            if size == 0:
                full_path = DIST_PACKAGES / file_path
                if full_path.exists() and full_path.is_file():
                    size = full_path.stat().st_size

            total_size += size

            # Determine file type
            ext = Path(file_path).suffix.lower()
            if ext:
                file_types[ext] += 1
            else:
                file_types['no_extension'] += 1

            files.append({
                'path': file_path,
                'size': size,
                'hash': file_hash
            })

    return {
        'name': pkg_name,
        'files': files,
        'total_size': total_size,
        'file_count': len(files),
        'file_types': dict(file_types)
    }

def format_size(bytes_size):
    """Format bytes to human-readable size."""
    for unit in ['B', 'KB', 'MB', 'GB']:
        if bytes_size < 1024.0:
            return f"{bytes_size:.2f} {unit}"
        bytes_size /= 1024.0
    return f"{bytes_size:.2f} TB"

def main():
    print("Analyzing yt-dlp[default] installation...\n")

    results = []
    for pkg in PACKAGES:
        result = analyze_package(pkg)
        if result:
            results.append(result)

    # Print summary
    print("=" * 80)
    print("PACKAGE SUMMARY")
    print("=" * 80)

    grand_total = 0
    for result in results:
        print(f"\n{result['name']}")
        print(f"  Files: {result['file_count']}")
        print(f"  Total Size: {format_size(result['total_size'])} ({result['total_size']:,} bytes)")
        print(f"  File Types: {result['file_types']}")
        grand_total += result['total_size']

    print(f"\n{'=' * 80}")
    print(f"GRAND TOTAL: {format_size(grand_total)} ({grand_total:,} bytes)")
    print(f"Total Packages: {len(results)}")
    print("=" * 80)

    # Save detailed results
    output_file = "/home/user/research/yt-dlp-install-report/package_analysis.txt"
    with open(output_file, 'w') as f:
        f.write("DETAILED PACKAGE ANALYSIS\n")
        f.write("=" * 80 + "\n\n")

        for result in results:
            f.write(f"\n{result['name']}\n")
            f.write("-" * 80 + "\n")
            f.write(f"Total Files: {result['file_count']}\n")
            f.write(f"Total Size: {format_size(result['total_size'])} ({result['total_size']:,} bytes)\n")
            f.write(f"File Types: {result['file_types']}\n\n")

            # List files sorted by size (largest first)
            sorted_files = sorted(result['files'], key=lambda x: x['size'], reverse=True)
            f.write("Files (sorted by size):\n")
            for file_info in sorted_files:
                f.write(f"  {format_size(file_info['size']):>12} - {file_info['path']}\n")
            f.write("\n")

        f.write("\n" + "=" * 80 + "\n")
        f.write(f"GRAND TOTAL: {format_size(grand_total)} ({grand_total:,} bytes)\n")
        f.write(f"Total Packages: {len(results)}\n")

    print(f"\nDetailed analysis saved to: {output_file}")

if __name__ == "__main__":
    main()
