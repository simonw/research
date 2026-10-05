# Investigation Notes: Google and OpenAI Tagged Content Days in 2024

## Objective
Find every day within 2024 when there was at least one item of content tagged "google" and at least one other item of content tagged "openai" (or both tags on same item).

## Schema Discovery

### Content Types
- **blog_entry** - Main blog posts
- **blog_blogmark** - Bookmarked links
- **blog_quotation** - Quotations
- **blog_note** - Short notes

### Tag System
- **blog_tag** - Tags table
- **blog_entry_tags**, **blog_blogmark_tags**, **blog_quotation_tags**, **blog_note_tags** - Junction tables linking content to tags

Each content type has:
- A date field (need to identify exact field names)
- A tags junction table linking to blog_tag

## Key Findings
- All content tables have a `created` field (TEXT type)
- Tags 'google' and 'openai' both exist in the database
- Need to UNION across all 4 content types to get complete picture

## Approach
1. Create a CTE that UNIONs all content types with their tags and dates
2. Filter for 2024 content tagged with 'google' or 'openai'
3. Group results by date to identify days with both tags present
4. Built Python script (query_builder.py) to execute query and analyze results

## Query Execution Results

Successfully ran the query and found:
- **178 total tagged content items** (entries, blogmarks, quotations, notes) with either 'google' or 'openai' tags in 2024
- **18 days in 2024** had content tagged with BOTH 'google' and 'openai'

### Observations
1. Content is spread across all four types: entries, blogmarks, quotations, and notes
2. Some days have a single item tagged with both tags (e.g., 2024-12-31)
3. Other days have multiple items, each tagged with different tags (e.g., 2024-12-20 has 1 google item and 4 openai items)
4. The query correctly handles both scenarios:
   - Single item tagged with both google AND openai
   - Multiple items on same day, some tagged google, others tagged openai

### Date Range
- Latest: 2024-12-31 (Things we learned about LLMs in 2024)
- Earliest: 2024-04-10 (Gemini 1.5 Pro public preview and LLM releases)

## Technical Details
- Used SQLite date() function to normalize dates from TEXT format
- UNION ALL across all 4 content types
- Filtered by year using `created LIKE '2024%'`
- Post-processed results in Python to group by date and identify days with both tags
