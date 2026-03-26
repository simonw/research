#!/usr/bin/env python3
"""Create test SQLite databases of various sizes for benchmarking."""
import sqlite3
import os
import json
import struct
import sys

DB_DIR = os.path.join(os.path.dirname(__file__), "static", "dbs")


def create_db(name, num_rows, page_size=4096):
    """Create a test database with the given number of rows."""
    os.makedirs(DB_DIR, exist_ok=True)
    path = os.path.join(DB_DIR, f"{name}.db")
    if os.path.exists(path):
        os.remove(path)

    conn = sqlite3.connect(path)
    conn.execute(f"PRAGMA page_size = {page_size}")
    conn.execute("PRAGMA journal_mode = DELETE")

    # Users table
    conn.execute("""
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER,
            city TEXT,
            score REAL
        )
    """)
    conn.execute("CREATE INDEX idx_users_email ON users(email)")
    conn.execute("CREATE INDEX idx_users_city ON users(city)")
    conn.execute("CREATE INDEX idx_users_age ON users(age)")

    # Products table
    conn.execute("""
        CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            category TEXT NOT NULL,
            price REAL NOT NULL,
            stock INTEGER DEFAULT 0
        )
    """)
    conn.execute("CREATE INDEX idx_products_category ON products(category)")
    conn.execute("CREATE INDEX idx_products_price ON products(price)")

    # Orders table (join target)
    conn.execute("""
        CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            user_id INTEGER REFERENCES users(id),
            product_id INTEGER REFERENCES products(id),
            quantity INTEGER,
            total REAL,
            order_date TEXT
        )
    """)
    conn.execute("CREATE INDEX idx_orders_user ON orders(user_id)")
    conn.execute("CREATE INDEX idx_orders_product ON orders(product_id)")
    conn.execute("CREATE INDEX idx_orders_date ON orders(order_date)")

    cities = ["New York", "London", "Tokyo", "Paris", "Berlin", "Sydney",
              "Toronto", "Mumbai", "São Paulo", "Lagos"]
    categories = ["Electronics", "Books", "Clothing", "Food", "Sports",
                  "Home", "Garden", "Toys", "Music", "Art"]

    # Insert users
    batch = []
    for i in range(num_rows):
        batch.append((
            f"User_{i}",
            f"user_{i}@example.com",
            20 + (i % 60),
            cities[i % len(cities)],
            round((i * 17 % 1000) / 10.0, 1),
        ))
        if len(batch) >= 5000:
            conn.executemany(
                "INSERT INTO users (name, email, age, city, score) VALUES (?, ?, ?, ?, ?)",
                batch
            )
            batch = []
    if batch:
        conn.executemany(
            "INSERT INTO users (name, email, age, city, score) VALUES (?, ?, ?, ?, ?)",
            batch
        )

    # Insert products (1/10th of users)
    num_products = max(10, num_rows // 10)
    batch = []
    for i in range(num_products):
        batch.append((
            f"Product_{i}",
            categories[i % len(categories)],
            round(1.0 + (i * 31 % 10000) / 100.0, 2),
            i * 7 % 500,
        ))
        if len(batch) >= 5000:
            conn.executemany(
                "INSERT INTO products (name, category, price, stock) VALUES (?, ?, ?, ?)",
                batch
            )
            batch = []
    if batch:
        conn.executemany(
            "INSERT INTO products (name, category, price, stock) VALUES (?, ?, ?, ?)",
            batch
        )

    # Insert orders (2x users)
    num_orders = num_rows * 2
    batch = []
    for i in range(num_orders):
        uid = (i * 13 % num_rows) + 1
        pid = (i * 7 % num_products) + 1
        qty = 1 + (i % 10)
        batch.append((
            uid, pid, qty,
            round(qty * (1.0 + (pid * 31 % 10000) / 100.0), 2),
            f"2024-{1 + i % 12:02d}-{1 + i % 28:02d}",
        ))
        if len(batch) >= 5000:
            conn.executemany(
                "INSERT INTO orders (user_id, product_id, quantity, total, order_date) VALUES (?, ?, ?, ?, ?)",
                batch
            )
            batch = []
    if batch:
        conn.executemany(
            "INSERT INTO orders (user_id, product_id, quantity, total, order_date) VALUES (?, ?, ?, ?, ?)",
            batch
        )

    conn.execute("ANALYZE")
    conn.commit()

    # Get stats
    total_users = conn.execute("SELECT COUNT(*) FROM users").fetchone()[0]
    total_products = conn.execute("SELECT COUNT(*) FROM products").fetchone()[0]
    total_orders = conn.execute("SELECT COUNT(*) FROM orders").fetchone()[0]
    page_count = conn.execute("PRAGMA page_count").fetchone()[0]
    conn.close()

    size = os.path.getsize(path)
    print(f"  {name}: {total_users} users, {total_products} products, {total_orders} orders")
    print(f"    {page_count} pages, {size:,} bytes ({size / 1024 / 1024:.1f} MB)")
    return path


def build_page_index(db_path):
    """Build a JSON index of page offsets for HTTP range request VFS.
    For plain SQLite, pages are at deterministic offsets."""
    conn = sqlite3.connect(db_path)
    page_size = conn.execute("PRAGMA page_size").fetchone()[0]
    page_count = conn.execute("PRAGMA page_count").fetchone()[0]
    conn.close()

    file_size = os.path.getsize(db_path)
    index = {
        "format": "sqlite3",
        "pageSize": page_size,
        "pageCount": page_count,
        "fileSize": file_size,
    }

    index_path = db_path + ".index.json"
    with open(index_path, "w") as f:
        json.dump(index, f)
    print(f"    Index written: {index_path}")
    return index_path


if __name__ == "__main__":
    print("Creating test databases...")
    sizes = {
        "tiny": 100,
        "small": 1_000,
        "medium": 10_000,
        "large": 100_000,
        "xlarge": 500_000,
    }
    for name, rows in sizes.items():
        path = create_db(name, rows)
        build_page_index(path)
    print("Done!")
