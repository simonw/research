#!/usr/bin/env python3
"""Explore the blog database structure and tag distribution."""

import sqlite3
import json

# Connect to database
conn = sqlite3.connect('simonwillisonblog.db')
cursor = conn.cursor()

# Get all tables
print("=== TABLES ===")
cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
tables = cursor.fetchall()
for table in tables:
    print(f"  {table[0]}")

# Get blog_entry schema
print("\n=== BLOG_ENTRY SCHEMA ===")
cursor.execute("PRAGMA table_info(blog_entry)")
for col in cursor.fetchall():
    print(f"  {col}")

# Check for tag-related tables
print("\n=== TAG-RELATED TABLES ===")
for table in tables:
    if 'tag' in table[0].lower():
        print(f"\n{table[0]}:")
        cursor.execute(f"PRAGMA table_info({table[0]})")
        for col in cursor.fetchall():
            print(f"  {col}")

# Count entries
cursor.execute("SELECT COUNT(*) FROM blog_entry")
total = cursor.fetchone()[0]
print(f"\n=== STATISTICS ===")
print(f"Total blog entries: {total}")

# Count entries with tags
cursor.execute("""
    SELECT COUNT(DISTINCT be.id)
    FROM blog_entry be
    JOIN blog_entry_tags bet ON be.id = bet.entry_id
""")
with_tags = cursor.fetchone()[0]
print(f"Entries with tags: {with_tags}")
print(f"Entries without tags: {total - with_tags}")

# Get tag distribution
print("\n=== TOP 20 TAGS ===")
cursor.execute("""
    SELECT t.tag, COUNT(*) as count
    FROM blog_tag t
    JOIN blog_entry_tags bet ON t.id = bet.tag_id
    GROUP BY t.id
    ORDER BY count DESC
    LIMIT 20
""")
for tag, count in cursor.fetchall():
    print(f"  {tag}: {count}")

# Sample entry with tags
print("\n=== SAMPLE ENTRY WITH TAGS ===")
cursor.execute("""
    SELECT be.id, be.created, be.title, GROUP_CONCAT(bt.tag, ', ') as tags
    FROM blog_entry be
    JOIN blog_entry_tags bet ON be.id = bet.entry_id
    JOIN blog_tag bt ON bet.tag_id = bt.id
    GROUP BY be.id
    ORDER BY be.created DESC
    LIMIT 1
""")
sample = cursor.fetchone()
print(f"ID: {sample[0]}")
print(f"Date: {sample[1]}")
print(f"Title: {sample[2]}")
print(f"Tags: {sample[3]}")

# Sample entry without tags
print("\n=== SAMPLE ENTRY WITHOUT TAGS ===")
cursor.execute("""
    SELECT be.id, be.created, be.title
    FROM blog_entry be
    LEFT JOIN blog_entry_tags bet ON be.id = bet.entry_id
    WHERE bet.tag_id IS NULL
    ORDER BY be.created ASC
    LIMIT 1
""")
sample = cursor.fetchone()
print(f"ID: {sample[0]}")
print(f"Date: {sample[1]}")
print(f"Title: {sample[2]}")

# Check what text fields are available
print("\n=== SAMPLE ENTRY FIELDS ===")
cursor.execute("SELECT * FROM blog_entry LIMIT 1")
row = cursor.fetchone()
cursor.execute("PRAGMA table_info(blog_entry)")
cols = cursor.fetchall()
for i, col in enumerate(cols):
    print(f"{col[1]}: {str(row[i])[:100]}")

conn.close()
