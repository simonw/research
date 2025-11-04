"""Explore the APIs of all three h3 libraries"""
import h3
import h3o_python as h3o
import sqlite3
import sys

print("=" * 60)
print("EXPLORING H3 LIBRARY APIs")
print("=" * 60)

# Test coordinates
lat, lng = 37.775938728915946, -122.41795063018799
resolution = 9

print("\n1. h3-py (uber/h3-py)")
print("-" * 60)
print(f"Version: {h3.__version__ if hasattr(h3, '__version__') else 'unknown'}")
print(f"Available functions: {len([x for x in dir(h3) if not x.startswith('_')])}")

# Test h3-py functions
cell_h3py = h3.latlng_to_cell(lat, lng, resolution)
print(f"latlng_to_cell({lat}, {lng}, {resolution}) = {cell_h3py}")
print(f"  Type: {type(cell_h3py)}")
print(f"  Note: h3-py v4 returns string by default")

lat_out, lng_out = h3.cell_to_latlng(cell_h3py)
print(f"cell_to_latlng(cell) = ({lat_out}, {lng_out})")

neighbors = h3.grid_disk(cell_h3py, 1)
print(f"grid_disk(cell, 1) = {len(neighbors)} cells")

area = h3.cell_area(cell_h3py, unit='km^2')
print(f"cell_area(cell, 'km^2') = {area:.6f} km²")

avg_area = h3.average_hexagon_area(resolution, unit='km^2')
print(f"average_hexagon_area({resolution}, 'km^2') = {avg_area:.6f} km²")

paris = (48.864716, 2.349014)
shanghai = (31.224361, 121.46917)
distance = h3.great_circle_distance(paris, shanghai, unit='km')
print(f"great_circle_distance(Paris, Shanghai, 'km') = {distance:.2f} km")

print("\n2. h3o-python (Rust-based)")
print("-" * 60)
print(f"Available functions: {len([x for x in dir(h3o) if not x.startswith('_')])}")

# Test h3o functions
cell_h3o = h3o.latlng_to_cell(lat, lng, resolution)
print(f"latlng_to_cell({lat}, {lng}, {resolution}) = {cell_h3o}")
print(f"  Type: {type(cell_h3o)}")

cell_str_h3o = h3o.cell_to_string(cell_h3o)
print(f"cell_to_string(cell) = {cell_str_h3o}")

lat_out_h3o, lng_out_h3o = h3o.cell_to_latlng(cell_h3o)
print(f"cell_to_latlng(cell) = ({lat_out_h3o}, {lng_out_h3o})")

neighbors_h3o = h3o.grid_disk(cell_h3o, 1)
print(f"grid_disk(cell, 1) = {len(neighbors_h3o)} cells")

area_h3o = h3o.cell_area_km2(cell_h3o)
print(f"cell_area_km2(cell) = {area_h3o:.6f} km²")

avg_area_h3o = h3o.average_hexagon_area_km2(resolution)
print(f"average_hexagon_area_km2({resolution}) = {avg_area_h3o:.6f} km²")

distance_h3o = h3o.great_circle_distance_km(*paris, *shanghai)
print(f"great_circle_distance_km(Paris, Shanghai) = {distance_h3o:.2f} km")

print("\n3. h3-sqlite3 (SQLite extension)")
print("-" * 60)
# Load h3-sqlite3 extension
conn = sqlite3.connect(':memory:')
cursor = conn.cursor()

# Try to load the extension
extension_path = '/home/user/research/h3-library-benchmark/h3-sqlite3/build/libh3ext'
try:
    conn.enable_load_extension(True)
    cursor.execute(f"SELECT load_extension('{extension_path}')")
    print(f"Extension loaded from: {extension_path}")

    # Test h3-sqlite3 functions
    cursor.execute(f"SELECT printf('%x', latLngToCell({lat}, {lng}, {resolution}))")
    cell_sqlite = cursor.fetchone()[0]
    print(f"latLngToCell({lat}, {lng}, {resolution}) = {cell_sqlite}")
    print(f"  Type: hex string")

    # Convert to int for comparison
    cell_sqlite_int = int(cell_sqlite, 16)
    print(f"  As integer: {cell_sqlite_int}")

    cursor.execute(f"SELECT cellToLat({cell_sqlite_int}), cellToLng({cell_sqlite_int})")
    lat_s, lng_s = cursor.fetchone()
    print(f"cellToLat/cellToLng(cell) = ({lat_s}, {lng_s})")

    print(f"\nAvailable h3-sqlite3 functions (from README):")
    print("  - latLngToCell, cellToLat, cellToLng")
    print("  - cellToParent, getResolution, getBaseCellNumber")
    print("  - stringToH3, h3ToString, isValidCell")
    print("  - isResClassIII, isPentagon")

except Exception as e:
    print(f"Failed to load extension: {e}")
    print("Note: h3-sqlite3 has limited functionality via SQLite extension")

conn.close()

print("\n" + "=" * 60)
print("API COMPATIBILITY SUMMARY")
print("=" * 60)
print("\nCommon operations:")
print("✓ latlng_to_cell - All three support")
print("✓ cell_to_latlng - All three support")
print("✓ cell to string conversion - h3-py and h3o-python support")
print("✓ grid_disk (neighbors) - h3-py and h3o-python support")
print("✓ cell_area - h3-py and h3o-python support")
print("✓ average_hexagon_area - h3-py and h3o-python support")
print("✓ great_circle_distance - h3-py and h3o-python support")
print("\nNote: h3-sqlite3 has more limited functionality")
print("      - Focuses on core lat/lng ↔ cell conversions")
print("      - Accessed via SQL queries (different paradigm)")
