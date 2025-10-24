#!/usr/bin/env python3
"""Prepare training and prediction data from the blog database."""

import sqlite3
import json
import pandas as pd
from sklearn.preprocessing import MultiLabelBinarizer

def prepare_data():
    """Extract and prepare data for training and prediction."""
    conn = sqlite3.connect('simonwillisonblog.db')

    # Get entries with tags (for training)
    print("Fetching entries with tags...")
    query_with_tags = """
        SELECT
            be.id,
            be.title,
            be.body,
            be.created,
            GROUP_CONCAT(bt.tag, '|') as tags
        FROM blog_entry be
        JOIN blog_entry_tags bet ON be.id = bet.entry_id
        JOIN blog_tag bt ON bet.tag_id = bt.id
        WHERE be.is_draft = 0
        GROUP BY be.id
        ORDER BY be.created
    """

    df_with_tags = pd.read_sql_query(query_with_tags, conn)
    print(f"Found {len(df_with_tags)} entries with tags")

    # Get entries without tags (for prediction)
    print("Fetching entries without tags...")
    query_without_tags = """
        SELECT
            be.id,
            be.title,
            be.body,
            be.created
        FROM blog_entry be
        LEFT JOIN blog_entry_tags bet ON be.id = bet.entry_id
        WHERE bet.tag_id IS NULL AND be.is_draft = 0
        ORDER BY be.created
    """

    df_without_tags = pd.read_sql_query(query_without_tags, conn)
    print(f"Found {len(df_without_tags)} entries without tags")

    # Process tags (split by |)
    df_with_tags['tags'] = df_with_tags['tags'].apply(lambda x: x.split('|'))

    # Create combined text field (title + body)
    df_with_tags['text'] = df_with_tags['title'].fillna('') + ' ' + df_with_tags['body'].fillna('')
    df_without_tags['text'] = df_without_tags['title'].fillna('') + ' ' + df_without_tags['body'].fillna('')

    # Get unique tags
    all_tags = set()
    for tags in df_with_tags['tags']:
        all_tags.update(tags)
    print(f"Found {len(all_tags)} unique tags")

    # Filter to tags with at least 10 occurrences (for better training)
    tag_counts = {}
    for tags in df_with_tags['tags']:
        for tag in tags:
            tag_counts[tag] = tag_counts.get(tag, 0) + 1

    min_tag_count = 10
    frequent_tags = {tag for tag, count in tag_counts.items() if count >= min_tag_count}
    print(f"Using {len(frequent_tags)} tags with at least {min_tag_count} occurrences")

    # Filter entries to only include those with at least one frequent tag
    df_with_tags['filtered_tags'] = df_with_tags['tags'].apply(
        lambda tags: [t for t in tags if t in frequent_tags]
    )
    df_with_tags = df_with_tags[df_with_tags['filtered_tags'].apply(len) > 0].copy()
    print(f"After filtering: {len(df_with_tags)} training entries")

    # Save prepared data
    data = {
        'training': df_with_tags[['id', 'title', 'text', 'created', 'filtered_tags']].to_dict('records'),
        'prediction': df_without_tags[['id', 'title', 'text', 'created']].to_dict('records'),
        'tag_counts': tag_counts,
        'frequent_tags': sorted(frequent_tags),
        'stats': {
            'total_training_entries': len(df_with_tags),
            'total_prediction_entries': len(df_without_tags),
            'total_unique_tags': len(all_tags),
            'frequent_tags_count': len(frequent_tags),
            'min_tag_count': min_tag_count
        }
    }

    with open('prepared_data.json', 'w') as f:
        json.dump(data, f, indent=2)

    print(f"\nData saved to prepared_data.json")
    print(f"Stats: {data['stats']}")

    conn.close()
    return data

if __name__ == '__main__':
    prepare_data()
