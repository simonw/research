#!/usr/bin/env python3
"""Analyze binary files (.so) installed with yt-dlp packages."""

import os
import subprocess
from pathlib import Path
from collections import defaultdict

DIST_PACKAGES = Path("/usr/local/lib/python3.11/dist-packages")

def get_file_info(file_path):
    """Get information about a binary file."""
    stat = file_path.stat()

    # Get file type
    try:
        file_output = subprocess.check_output(
            ['file', str(file_path)],
            stderr=subprocess.DEVNULL,
            text=True
        ).strip()
    except:
        file_output = "Unknown"

    # Get ldd info
    ldd_output = []
    try:
        ldd_result = subprocess.check_output(
            ['ldd', str(file_path)],
            stderr=subprocess.STDOUT,
            text=True
        )
        ldd_output = [line.strip() for line in ldd_result.split('\n') if line.strip()]
    except:
        ldd_output = ["Could not determine dependencies"]

    return {
        'size': stat.st_size,
        'file_type': file_output,
        'dependencies': ldd_output
    }

def format_size(bytes_size):
    """Format bytes to human-readable size."""
    for unit in ['B', 'KB', 'MB', 'GB']:
        if bytes_size < 1024.0:
            return f"{bytes_size:.2f} {unit}"
        bytes_size /= 1024.0
    return f"{bytes_size:.2f} TB"

def main():
    print("Analyzing binary files in yt-dlp installation...\n")

    # Find all .so files
    so_files = list(DIST_PACKAGES.glob("**/*.so"))

    # Group by package
    packages = defaultdict(list)
    for so_file in so_files:
        # Determine which package this belongs to
        rel_path = so_file.relative_to(DIST_PACKAGES)
        pkg_name = str(rel_path.parts[0]) if rel_path.parts else "unknown"
        packages[pkg_name].append(so_file)

    # Analyze each package's binaries
    output_file = "/home/user/research/yt-dlp-install-report/binary_details.txt"
    with open(output_file, 'w') as f:
        f.write("BINARY FILES ANALYSIS\n")
        f.write("=" * 80 + "\n\n")

        total_binaries = 0
        total_size = 0

        for pkg_name in sorted(packages.keys()):
            binaries = packages[pkg_name]
            pkg_size = sum(b.stat().st_size for b in binaries)

            f.write(f"\n{pkg_name}\n")
            f.write("-" * 80 + "\n")
            f.write(f"Binary files: {len(binaries)}\n")
            f.write(f"Total binary size: {format_size(pkg_size)} ({pkg_size:,} bytes)\n\n")

            print(f"{pkg_name}: {len(binaries)} binaries, {format_size(pkg_size)}")

            for binary in sorted(binaries):
                rel_path = binary.relative_to(DIST_PACKAGES)
                info = get_file_info(binary)

                f.write(f"\nFile: {rel_path}\n")
                f.write(f"  Size: {format_size(info['size'])} ({info['size']:,} bytes)\n")
                f.write(f"  Type: {info['file_type']}\n")
                f.write(f"  Dependencies:\n")
                for dep in info['dependencies']:
                    f.write(f"    {dep}\n")

            total_binaries += len(binaries)
            total_size += pkg_size

        f.write("\n" + "=" * 80 + "\n")
        f.write(f"TOTAL BINARIES: {total_binaries}\n")
        f.write(f"TOTAL BINARY SIZE: {format_size(total_size)} ({total_size:,} bytes)\n")

        print(f"\nTotal: {total_binaries} binaries, {format_size(total_size)}")

    print(f"\nDetailed binary analysis saved to: {output_file}")

if __name__ == "__main__":
    main()
