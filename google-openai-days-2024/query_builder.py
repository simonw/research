#!/usr/bin/env python3
"""
Query builder to find days in 2024 with both google and openai tagged content
"""

import requests
import json
from datetime import datetime
from collections import defaultdict
from urllib.parse import quote

BASE_URL = "https://datasette.simonwillison.net/simonwillisonblog.json"

def fetch_query(sql):
    """Execute a SQL query against the Datasette API"""
    url = f"{BASE_URL}?sql={quote(sql)}&_shape=array"
    print(f"Fetching: {url[:100]}...")
    response = requests.get(url)
    response.raise_for_status()
    return response.json()

# Query to get all content with google or openai tags in 2024
query = """
WITH all_tagged_content AS (
  -- Blog entries
  SELECT
    date(e.created) as content_date,
    t.tag,
    'entry' as content_type,
    e.id as content_id,
    e.title as title
  FROM blog_entry e
  JOIN blog_entry_tags et ON e.id = et.entry_id
  JOIN blog_tag t ON et.tag_id = t.id
  WHERE t.tag IN ('google', 'openai')
    AND e.created LIKE '2024%'

  UNION ALL

  -- Blogmarks
  SELECT
    date(b.created) as content_date,
    t.tag,
    'blogmark' as content_type,
    b.id as content_id,
    b.link_title as title
  FROM blog_blogmark b
  JOIN blog_blogmark_tags bt ON b.id = bt.blogmark_id
  JOIN blog_tag t ON bt.tag_id = t.id
  WHERE t.tag IN ('google', 'openai')
    AND b.created LIKE '2024%'

  UNION ALL

  -- Quotations
  SELECT
    date(q.created) as content_date,
    t.tag,
    'quotation' as content_type,
    q.id as content_id,
    substr(q.quotation, 1, 50) as title
  FROM blog_quotation q
  JOIN blog_quotation_tags qt ON q.id = qt.quotation_id
  JOIN blog_tag t ON qt.tag_id = t.id
  WHERE t.tag IN ('google', 'openai')
    AND q.created LIKE '2024%'

  UNION ALL

  -- Notes
  SELECT
    date(n.created) as content_date,
    t.tag,
    'note' as content_type,
    n.id as content_id,
    n.title as title
  FROM blog_note n
  JOIN blog_note_tags nt ON n.id = nt.note_id
  JOIN blog_tag t ON nt.tag_id = t.id
  WHERE t.tag IN ('google', 'openai')
    AND n.created LIKE '2024%'
)
SELECT
  content_date,
  content_type,
  content_id,
  title,
  tag
FROM all_tagged_content
ORDER BY content_date DESC, content_type, content_id
"""

def main():
    print("Fetching all content tagged with 'google' or 'openai' in 2024...")
    results = fetch_query(query)

    print(f"\nFound {len(results)} tagged content items")

    # Group by date to find days with both tags
    days_data = defaultdict(lambda: {'google': [], 'openai': []})

    for item in results:
        date = item['content_date']
        tag = item['tag']
        days_data[date][tag].append({
            'type': item['content_type'],
            'id': item['content_id'],
            'title': item['title']
        })

    # Find days with both tags
    days_with_both = []
    for date in sorted(days_data.keys(), reverse=True):
        has_google = len(days_data[date]['google']) > 0
        has_openai = len(days_data[date]['openai']) > 0

        if has_google and has_openai:
            days_with_both.append({
                'date': date,
                'google_items': days_data[date]['google'],
                'openai_items': days_data[date]['openai']
            })

    print(f"\nDays in 2024 with both 'google' and 'openai' tagged content: {len(days_with_both)}")
    print("\n" + "="*80)

    for day in days_with_both:
        print(f"\n📅 {day['date']}")
        print(f"   Google-tagged items: {len(day['google_items'])}")
        for item in day['google_items']:
            print(f"     - {item['type']}: {item['title']}")
        print(f"   OpenAI-tagged items: {len(day['openai_items'])}")
        for item in day['openai_items']:
            print(f"     - {item['type']}: {item['title']}")

    # Save results to JSON
    with open('results.json', 'w') as f:
        json.dump({
            'total_days': len(days_with_both),
            'days': days_with_both
        }, f, indent=2)

    print(f"\n✅ Results saved to results.json")

    return days_with_both

if __name__ == '__main__':
    main()
