# Google and OpenAI Tagged Content Days in 2024

## Task

> Use this API to find every day within 2024 when there with at least one item of content tagged google and at least one other item of content tagged openai (or both tags on same item)
>
> Start with this to see the schema - be sure to consider content across entries and blogmarks and quotes and notes
>
> https://datasette.simonwillison.net/simonwillisonblog.json?sql=select+sql+from+sqlite_master&_shape=array

## Summary

Analysis of Simon Willison's blog database to identify days in 2024 when content was tagged with both "google" and "openai" tags.

**Key Finding:** There were **18 days in 2024** with content tagged with both Google and OpenAI.

## Methodology

### Data Source
- **Database:** https://datasette.simonwillison.net/simonwillisonblog
- **Content Types Analyzed:**
  - Blog entries (`blog_entry`)
  - Blogmarks (`blog_blogmark`)
  - Quotations (`blog_quotation`)
  - Notes (`blog_note`)

### Query Approach

1. **UNION Query:** Combined all four content types with their associated tags
2. **Filtering:** Limited to 2024 content tagged with either 'google' or 'openai'
3. **Date Normalization:** Used SQLite's `date()` function to normalize TEXT dates
4. **Aggregation:** Grouped results by date to identify days with both tags present

### Implementation

The analysis used a SQL query with a CTE (Common Table Expression) to union all tagged content:

```sql
WITH all_tagged_content AS (
  SELECT date(e.created) as content_date, t.tag, 'entry' as content_type, ...
  FROM blog_entry e
  JOIN blog_entry_tags et ON e.id = et.entry_id
  JOIN blog_tag t ON et.tag_id = t.id
  WHERE t.tag IN ('google', 'openai') AND e.created LIKE '2024%'

  UNION ALL
  -- Similar queries for blogmarks, quotations, and notes
)
```

Post-processing in Python grouped results by date and identified days with both tags.

## Interactive Queries

Explore the data yourself with these Datasette queries:

- **[All content tagged 'google' or 'openai' in 2024](https://datasette.simonwillison.net/simonwillisonblog?sql=WITH+all_tagged_content+AS+%28%0D%0A++SELECT%0D%0A++++date%28e.created%29+as+content_date%2C%0D%0A++++t.tag%2C%0D%0A++++%27entry%27+as+content_type%2C%0D%0A++++e.id+as+content_id%2C%0D%0A++++e.title+as+title%0D%0A++FROM+blog_entry+e%0D%0A++JOIN+blog_entry_tags+et+ON+e.id+%3D+et.entry_id%0D%0A++JOIN+blog_tag+t+ON+et.tag_id+%3D+t.id%0D%0A++WHERE+t.tag+IN+%28%27google%27%2C+%27openai%27%29%0D%0A++++AND+e.created+LIKE+%272024%25%27%0D%0A%0D%0A++UNION+ALL%0D%0A%0D%0A++SELECT%0D%0A++++date%28b.created%29+as+content_date%2C%0D%0A++++t.tag%2C%0D%0A++++%27blogmark%27+as+content_type%2C%0D%0A++++b.id+as+content_id%2C%0D%0A++++b.link_title+as+title%0D%0A++FROM+blog_blogmark+b%0D%0A++JOIN+blog_blogmark_tags+bt+ON+b.id+%3D+bt.blogmark_id%0D%0A++JOIN+blog_tag+t+ON+bt.tag_id+%3D+t.id%0D%0A++WHERE+t.tag+IN+%28%27google%27%2C+%27openai%27%29%0D%0A++++AND+b.created+LIKE+%272024%25%27%0D%0A%0D%0A++UNION+ALL%0D%0A%0D%0A++SELECT%0D%0A++++date%28q.created%29+as+content_date%2C%0D%0A++++t.tag%2C%0D%0A++++%27quotation%27+as+content_type%2C%0D%0A++++q.id+as+content_id%2C%0D%0A++++substr%28q.quotation%2C+1%2C+50%29+as+title%0D%0A++FROM+blog_quotation+q%0D%0A++JOIN+blog_quotation_tags+qt+ON+q.id+%3D+qt.quotation_id%0D%0A++JOIN+blog_tag+t+ON+qt.tag_id+%3D+t.id%0D%0A++WHERE+t.tag+IN+%28%27google%27%2C+%27openai%27%29%0D%0A++++AND+q.created+LIKE+%272024%25%27%0D%0A%0D%0A++UNION+ALL%0D%0A%0D%0A++SELECT%0D%0A++++date%28n.created%29+as+content_date%2C%0D%0A++++t.tag%2C%0D%0A++++%27note%27+as+content_type%2C%0D%0A++++n.id+as+content_id%2C%0D%0A++++n.title+as+title%0D%0A++FROM+blog_note+n%0D%0A++JOIN+blog_note_tags+nt+ON+n.id+%3D+nt.note_id%0D%0A++JOIN+blog_tag+t+ON+nt.tag_id+%3D+t.id%0D%0A++WHERE+t.tag+IN+%28%27google%27%2C+%27openai%27%29%0D%0A++++AND+n.created+LIKE+%272024%25%27%0D%0A%29%0D%0ASELECT%0D%0A++content_date%2C%0D%0A++content_type%2C%0D%0A++content_id%2C%0D%0A++title%2C%0D%0A++tag%0D%0AFROM+all_tagged_content%0D%0AORDER+BY+content_date+DESC%2C+content_type%2C+content_id)**

- **[Just entries tagged 'google' or 'openai' in 2024](https://datasette.simonwillison.net/simonwillisonblog?sql=SELECT+date%28e.created%29+as+content_date%2C+t.tag%2C+e.title%0AFROM+blog_entry+e%0AJOIN+blog_entry_tags+et+ON+e.id+%3D+et.entry_id%0AJOIN+blog_tag+t+ON+et.tag_id+%3D+t.id%0AWHERE+t.tag+IN+%28%27google%27%2C+%27openai%27%29+AND+e.created+LIKE+%272024%25%27%0AORDER+BY+e.created+DESC)**

- **[Database schema](https://datasette.simonwillison.net/simonwillisonblog?sql=select+sql+from+sqlite_master+where+name+like+%27blog_%25%27+order+by+name)**

## Results

### Statistics
- **Total tagged items:** 178 content items with either 'google' or 'openai' tags in 2024
- **Days with both tags:** 18 days
- **Date range:** April 10, 2024 to December 31, 2024

### Complete List of Days

1. **2024-12-31** - Things we learned about LLMs in 2024 (tagged with both)
2. **2024-12-20** - Multiple items including OpenAI o3 coverage and December LLM roundup
3. **2024-12-16** - Veo 2 (Google) and WebDev Arena (OpenAI)
4. **2024-12-13** - Google model-viewer component and OpenAI postmortem/FAQ
5. **2024-12-04** - Google Genie 2 and Amazon Nova LLMs coverage
6. **2024-11-21** - NotebookLM quotation and tiktoken/chess LLM articles
7. **2024-11-19** - Gemini API ToS and Bing Chat manipulative AI notes
8. **2024-10-23** - Google Gemini tooling and OpenAI revenue quotation
9. **2024-10-03** - Gemini 1.5 Flash-8B (tagged with both)
10. **2024-08-24** - SQL pipe syntax and OAuth/LLM musings
11. **2024-08-08** - Gemini price drop and GPT-4o System Card
12. **2024-07-16** - Quotation about OpenAI and Anthropic (tagged with both)
13. **2024-07-09** - Google Hangouts code and OpenAI capabilities quotation
14. **2024-05-24** - AI Overviews issues and ChatGPT hallucination reports
15. **2024-05-17** - Gemini error understanding and OpenAI off-boarding quotation
16. **2024-05-15** - PaliGemma model and OpenAI Projects/ChatGPT 4o coverage
17. **2024-05-14** - Gemini context window/caching and voice assistant article
18. **2024-04-10** - Gemini 1.5 Pro preview and LLM releases coverage

### Content Type Distribution

The qualifying content includes:
- **Blog entries:** Full articles covering LLM developments, analysis, and commentary
- **Blogmarks:** Curated links to external resources with commentary
- **Quotations:** Notable quotes from articles, papers, or sources
- **Notes:** Shorter-form content and observations

### Patterns Observed

1. **Dual Tagging:** Some items are tagged with both 'google' and 'openai' (e.g., comparative content, industry roundups)
2. **Same-Day Coverage:** Many days have separate items covering Google and OpenAI news independently
3. **LLM News Cycles:** Clustering around major announcements (May: GPT-4o launch, December: OpenAI o3)
4. **Diverse Content:** Mix of technical coverage, business news, and critical commentary

## Files

- **`query_builder.py`** - Python script to query the Datasette API and analyze results
- **`results.json`** - Complete JSON output with all 18 days and their associated content
- **`notes.md`** - Working notes from the investigation process
- **`README.md`** - This report

## Usage

To reproduce these results:

```bash
python3 query_builder.py
```

This will:
1. Query the Datasette API for all 2024 content tagged with 'google' or 'openai'
2. Group results by date
3. Identify days with both tags present
4. Output results to console and save to `results.json`

## Technical Notes

- **Date handling:** The database stores dates as TEXT, requiring the `date()` function for normalization
- **Tag matching:** Case-sensitive matching on exact tag names 'google' and 'openai'
- **Year filtering:** Uses `LIKE '2024%'` pattern matching for efficiency
- **API format:** Results requested with `_shape=array` for cleaner JSON parsing

## Conclusion

The analysis successfully identified 18 days in 2024 where Simon Willison's blog contained content tagged with both Google and OpenAI. The content spans multiple formats (entries, blogmarks, quotations, notes) and covers a wide range of topics from technical announcements to industry commentary, reflecting the competitive and interconnected nature of the LLM landscape throughout 2024.
