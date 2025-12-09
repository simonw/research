# Hacker News Search API Documentation

This document provides comprehensive documentation for the Hacker News Search API, powered by Algolia. The API enables developers to programmatically search and access Hacker News data.

**Base URL:** `https://hn.algolia.com/api/v1`

## Table of Contents

1. [API Endpoints](#api-endpoints)
   - [Search](#search)
   - [Search by Date](#search-by-date)
   - [Get Item](#get-item)
   - [Get User](#get-user)
2. [Query Parameters](#query-parameters)
3. [Search Syntax](#search-syntax)
   - [Basic Search](#basic-search)
   - [Advanced Search Syntax](#advanced-search-syntax)
4. [Filtering](#filtering)
   - [Tag Filters](#tag-filters)
   - [Numeric Filters](#numeric-filters)
5. [Response Format](#response-format)
6. [Rate Limits](#rate-limits)
7. [Examples](#examples)

---

## API Endpoints

### Search

Search items sorted by relevance, then points, then number of comments.

```
GET https://hn.algolia.com/api/v1/search?query=...
```

### Search by Date

Search items sorted by date, with most recent first.

```
GET https://hn.algolia.com/api/v1/search_by_date?query=...
```

### Get Item

Retrieve a specific item (story, comment, poll, etc.) by its ID. Returns the item with all its nested children (comments).

```
GET https://hn.algolia.com/api/v1/items/:id
```

**Example Response:**
```json
{
  "id": 1,
  "created_at": "2006-10-09T18:21:51.000Z",
  "created_at_i": 1160418111,
  "type": "story",
  "author": "pg",
  "title": "Y Combinator",
  "url": "https://ycombinator.com",
  "text": null,
  "points": 57,
  "parent_id": null,
  "story_id": null,
  "children": [
    {
      "id": 15,
      "created_at": "2006-10-09T19:51:01.000Z",
      "created_at_i": 1160423461,
      "type": "comment",
      "author": "sama",
      "text": "Comment text here...",
      "points": 5,
      "parent_id": 1,
      "children": []
    }
  ]
}
```

### Get User

Retrieve a user profile by username.

```
GET https://hn.algolia.com/api/v1/users/:username
```

**Example Response:**
```json
{
  "username": "pg",
  "about": "User bio text...",
  "karma": 99999,
  "delay": null,
  "submitted": 12345,
  "submission_count": 500,
  "comment_count": 2000
}
```

---

## Query Parameters

### Common Parameters

| Parameter | Description | Type | Default |
|-----------|-------------|------|---------|
| `query` | Full-text search query | String | - |
| `tags` | Filter by tags (see Tag Filters) | String | - |
| `numericFilters` | Filter by numeric fields (see Numeric Filters) | String | - |
| `page` | Page number (0-indexed) | Integer | 0 |
| `hitsPerPage` | Number of results per page | Integer | 20 |

### Advanced Algolia Parameters

The API supports additional Algolia search parameters:

| Parameter | Description | Type |
|-----------|-------------|------|
| `attributesToRetrieve` | Attributes to include in response | String (comma-separated) |
| `attributesToHighlight` | Attributes to highlight in results | String (comma-separated) |
| `restrictSearchableAttributes` | Limit search to specific attributes | String (comma-separated) |
| `typoTolerance` | Enable/disable typo tolerance | Boolean/String |
| `minWordSizefor1Typo` | Minimum word size for 1 typo | Integer |
| `minWordSizefor2Typos` | Minimum word size for 2 typos | Integer |

---

## Search Syntax

### Basic Search

Simply enter keywords to search across all indexed fields (title, story text, comment text, URL, author).

```
https://hn.algolia.com/api/v1/search?query=javascript
```

### Advanced Search Syntax

The API supports advanced syntax when `advancedSyntax=true` (enabled by default):

#### Phrase Search

Use double quotes to match an exact phrase:

```
"machine learning"
```

#### Negative Search

Use a minus sign to exclude terms:

```
javascript -react
```

Returns results containing "javascript" but not "react".

#### Author Filter

Filter by author username:

```
author:pg
by:pg
```

Both formats are equivalent.

#### Story Filter

Filter comments by parent story ID:

```
story:12345
```

#### Points Filter

Filter by number of points:

```
points>100
points<50
points=42
points>=100
points<=50
points!=0
```

#### Comments Filter

Filter by number of comments:

```
comments>10
comments<5
comments=0
comments>=100
comments<=50
```

#### Date Filter

Filter by creation date (Unix timestamp):

```
date>1609459200
date<1640995200
```

---

## Filtering

### Tag Filters

Tags are used to filter results by type and other attributes. Tags can be combined using AND/OR logic.

#### Available Tags

| Tag | Description |
|-----|-------------|
| `story` | Stories (articles) |
| `comment` | Comments |
| `poll` | Polls |
| `pollopt` | Poll options |
| `job` | Job postings |
| `show_hn` | "Show HN" posts |
| `ask_hn` | "Ask HN" posts |
| `front_page` | Items currently on the front page |
| `author_{username}` | Items by specific author |
| `story_{id}` | Comments belonging to a specific story |

#### Tag Logic

- **AND logic (default):** Separate tags with commas
  ```
  tags=story,author_pg
  ```
  Matches stories by author "pg"

- **OR logic:** Wrap tags in parentheses
  ```
  tags=(story,poll)
  ```
  Matches stories OR polls

- **Combined:**
  ```
  tags=author_pg,(story,poll)
  ```
  Matches items by "pg" that are stories OR polls

### Numeric Filters

Filter on numeric fields using comparison operators.

#### Available Numeric Fields

| Field | Description |
|-------|-------------|
| `created_at_i` | Creation timestamp (Unix seconds) |
| `points` | Number of points/votes |
| `num_comments` | Number of comments |

#### Operators

| Operator | Description |
|----------|-------------|
| `<` | Less than |
| `<=` | Less than or equal |
| `=` | Equal |
| `!=` | Not equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

#### Examples

```
numericFilters=points>100
numericFilters=created_at_i>1609459200
numericFilters=points>50,num_comments>10
numericFilters=created_at_i>1609459200,created_at_i<1640995200
```

---

## Response Format

### Search Response

```json
{
  "hits": [
    {
      "objectID": "12345",
      "title": "Article Title",
      "url": "https://example.com/article",
      "author": "username",
      "points": 150,
      "story_text": null,
      "comment_text": null,
      "story_id": null,
      "story_title": null,
      "story_url": null,
      "parent_id": null,
      "created_at": "2023-01-15T10:30:00.000Z",
      "created_at_i": 1673778600,
      "num_comments": 45,
      "_tags": ["story", "author_username", "story_12345"],
      "_highlightResult": {
        "title": {
          "value": "<em>Article</em> Title",
          "matchLevel": "full",
          "matchedWords": ["article"]
        },
        "url": {
          "value": "https://example.com/article",
          "matchLevel": "none",
          "matchedWords": []
        },
        "author": {
          "value": "username",
          "matchLevel": "none",
          "matchedWords": []
        }
      }
    }
  ],
  "page": 0,
  "nbHits": 1234,
  "nbPages": 62,
  "hitsPerPage": 20,
  "processingTimeMS": 3,
  "query": "search query",
  "params": "query=search+query&tags=story"
}
```

### Hit Object Fields

| Field | Description | Present In |
|-------|-------------|------------|
| `objectID` | Unique identifier (same as HN item ID) | All |
| `title` | Story/poll title | Stories, Polls |
| `url` | External URL | Stories |
| `author` | Author username | All |
| `points` | Number of points | All |
| `story_text` | Text content of story | Stories |
| `comment_text` | Text content of comment | Comments |
| `story_id` | Parent story ID | Comments |
| `story_title` | Parent story title | Comments |
| `story_url` | Parent story URL | Comments |
| `parent_id` | Direct parent ID | Comments |
| `created_at` | ISO 8601 timestamp | All |
| `created_at_i` | Unix timestamp | All |
| `num_comments` | Number of comments | Stories, Polls |
| `_tags` | Array of associated tags | All |
| `_highlightResult` | Search match highlighting | All |

### Pagination Fields

| Field | Description |
|-------|-------------|
| `page` | Current page number (0-indexed) |
| `nbHits` | Total number of results |
| `nbPages` | Total number of pages |
| `hitsPerPage` | Results per page |
| `processingTimeMS` | Query processing time in milliseconds |

---

## Rate Limits

- **10,000 requests per hour** per IP address
- Exceeding this limit will result in temporary blacklisting
- Contact support@algolia.com if you believe you've been blocked in error

---

## Examples

### Get All Stories Matching a Query

```
https://hn.algolia.com/api/v1/search?query=javascript&tags=story
```

### Get All Comments Matching a Query

```
https://hn.algolia.com/api/v1/search?query=react&tags=comment
```

### Search Only URLs

```
https://hn.algolia.com/api/v1/search?query=github.com&restrictSearchableAttributes=url
```

### Get Front Page Stories

```
https://hn.algolia.com/api/v1/search?tags=front_page
```

### Get Latest Stories

```
https://hn.algolia.com/api/v1/search_by_date?tags=story
```

### Get Latest Stories or Polls

```
https://hn.algolia.com/api/v1/search_by_date?tags=(story,poll)
```

### Get Comments Since a Timestamp

```
https://hn.algolia.com/api/v1/search_by_date?tags=comment&numericFilters=created_at_i>1609459200
```

### Get Stories Between Two Dates

```
https://hn.algolia.com/api/v1/search_by_date?tags=story&numericFilters=created_at_i>1609459200,created_at_i<1640995200
```

### Get Stories by a Specific Author

```
https://hn.algolia.com/api/v1/search?tags=story,author_pg
```

### Get Comments on a Specific Story

```
https://hn.algolia.com/api/v1/search?tags=comment,story_12345
```

### Get High-Scoring Stories

```
https://hn.algolia.com/api/v1/search?tags=story&numericFilters=points>500
```

### Get Popular Show HN Posts

```
https://hn.algolia.com/api/v1/search?tags=show_hn&numericFilters=points>100
```

### Get Ask HN Posts with Many Comments

```
https://hn.algolia.com/api/v1/search?tags=ask_hn&numericFilters=num_comments>50
```

### Combine Query and Filters

```
https://hn.algolia.com/api/v1/search?query=machine%20learning&tags=story&numericFilters=points>100,num_comments>20
```

### Pagination Example

```
https://hn.algolia.com/api/v1/search?query=python&tags=story&page=2&hitsPerPage=50
```

### Get Full Item Tree

```
https://hn.algolia.com/api/v1/items/8863
```

Returns the item with all nested comment children.

---

## Item Types Reference

| Type Code | Type | Description |
|-----------|------|-------------|
| 0 | story | Regular story submission |
| 1 | comment | Comment on a story or another comment |
| 2 | poll | Poll post |
| 3 | pollopt | Poll option |
| 4 | unknown | Unknown type |
| 5 | job | Job posting |

---

## Searchable Attributes

The following attributes are indexed and searchable:

| Attribute | Ranking | Description |
|-----------|---------|-------------|
| `title` | Highest | Story/poll title |
| `story_text` | High | Story text content |
| `comment_text` | High | Comment text content |
| `url` | Medium | External URL |
| `author` | Lower | Author username |
| `created_at_i` | Lowest | Timestamp (for date sorting) |

The ranking follows the order above, meaning matches in `title` are weighted more heavily than matches in `url`.

---

## Custom Ranking

Results are ranked using the following criteria (in order):

1. **Typo tolerance** - Fewer typos rank higher
2. **Proximity** - Words closer together rank higher
3. **Attribute** - Matches in higher-ranked attributes rank higher
4. **Custom ranking:**
   - Higher points rank higher
   - More comments rank higher

For date-sorted results (`search_by_date`), results are ranked purely by `created_at_i` (most recent first).

---

## Notes

- All timestamps are in Unix seconds (not milliseconds)
- The `objectID` in search results corresponds to the HN item ID
- Deleted and dead items are not returned in search results
- The API is built on Algolia's search infrastructure
- Full Algolia search parameter documentation: https://www.algolia.com/doc/rest_api#QueryIndex
