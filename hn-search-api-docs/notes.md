# HN Search API Documentation - Investigation Notes

## Source Repository

Cloned from: https://github.com/algolia/hn-search into /tmp/hn-search

## Key Files Reviewed

### Backend (Ruby on Rails)

1. **Routes** (`config/routes.rb:1-35`)
   - API namespace under `/api/v1/`
   - Endpoints: `items/:id`, `users/:username`, `search`, `search_by_date`

2. **Controllers**
   - `app/controllers/api/v1/search_controller.rb` - Search endpoints
   - `app/controllers/api/v1/items_controller.rb` - Item retrieval
   - `app/controllers/api/v1/users_controller.rb` - User retrieval
   - `app/controllers/api/v1/base_controller.rb` - Base controller with Algolia client

3. **Models**
   - `app/models/item.rb` - Item model with Algolia search configuration
   - `app/models/user.rb` - User model with Algolia search configuration

4. **Serializers**
   - `app/serializers/item_serializer.rb` - Item response format
   - `app/serializers/user_serializer.rb` - User response format

5. **Database Schema** (`db/schema.rb`)
   - Items table with: item_type_cd, created_at, deleted, dead, author, title, url, text, points, parent_id, story_id, front_page
   - Users table with: username, about, karma, created_at, avg, delay, submitted

### Frontend (TypeScript/React)

1. **API Documentation View** (`app/javascript/views/Api.tsx`)
   - Contains the user-facing API documentation

2. **Search Settings** (`app/javascript/providers/SearchSettings.ts`)
   - Query parsing logic for advanced search syntax
   - Tag filters and numeric filters extraction

3. **Search Provider** (`app/javascript/providers/SearchProvider.tsx`)
   - Algolia indexes: Item_production, Item_production_sort_date, Item_production_ordered

4. **Help View** (`app/javascript/views/Help.tsx`)
   - Advanced search syntax documentation

## Key Findings

### Algolia Indexes
- `Item_production` - Main index sorted by relevance/popularity
- `Item_production_sort_date` - Index sorted by date
- `Item_production_ordered` - Index with ordered title matching
- `User_production` - User index

### Searchable Attributes (from Item model)
- title (unordered)
- story_text (unordered)
- comment_text (unordered)
- url (unordered)
- author
- created_at_i (Unix timestamp)

### Item Types
- story
- comment
- poll
- pollopt
- job
- unknown

### Tags System
- Item type tags: `story`, `comment`, `poll`, `pollopt`, `job`
- Author tags: `author_{username}`
- Story tags: `story_{id}`
- Special tags: `show_hn`, `ask_hn`, `front_page`

### Custom Ranking
- Primary: desc(points)
- Secondary: desc(num_comments)
- For date sort: desc(created_at_i)

### Advanced Search Syntax (Frontend)
Extracted from SearchSettings.ts:
- `author:USERNAME` or `by:USERNAME` - Filter by author
- `story:ID` - Filter by story ID
- `points<N`, `points>N`, `points=N` - Filter by points
- `comments<N`, `comments>N`, `comments=N` - Filter by comment count
- `date<TIMESTAMP`, `date>TIMESTAMP` - Filter by creation date

### Rate Limits
- 10,000 requests per IP per hour
