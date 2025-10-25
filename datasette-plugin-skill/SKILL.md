---
name: datasette-plugin-writer
description: Guide for writing Datasette plugins. This skill should be used when users want to create or develop plugins for Datasette, including information about plugin hooks, the cookiecutter template, database APIs, request/response handling, and plugin configuration.
license: Apache-2.0
---

# Writing Datasette Plugins

Use this skill to build plugins for Datasette, the open source multi-tool for exploring and publishing data.

## Quick Start with Cookiecutter Template

Start a new plugin using the datasette-plugin cookiecutter template with newline-delimited variables:

```bash
echo "plugin_name
description of plugin
plugin-hyphenated-name
plugin_underscored_name
github_username
Author Name
y
y" | uvx cookiecutter gh:simonw/datasette-plugin
```

Example for a plugin called "my-cool-plugin":

```bash
echo "my cool plugin
A plugin that does cool things
my-cool-plugin
my_cool_plugin
username
Your Name
y
y" | uvx cookiecutter gh:simonw/datasette-plugin
```

The last two `y` responses enable static/ and templates/ directories.

After creating the plugin:

```bash
cd datasette-my-cool-plugin
python -m venv venv
source venv/bin/activate
pip install -e '.[test]'
datasette plugins  # Verify plugin is visible
python -m pytest   # Run tests
```

## Plugin Structure

A typical plugin structure:

```
datasette-my-plugin/
├── datasette_my_plugin/
│   ├── __init__.py        # Plugin hooks go here
│   ├── static/            # Optional: CSS, JavaScript
│   └── templates/         # Optional: Custom templates
├── tests/
│   └── test_my_plugin.py
├── setup.py or pyproject.toml
└── README.md
```

## Essential Plugin Hooks

### prepare_connection(conn, database, datasette)

Register custom SQL functions. Called when SQLite connections are created:

```python
from datasette import hookimpl

@hookimpl
def prepare_connection(conn):
    conn.create_function("hello_world", 0, lambda: "Hello world!")
```

### register_routes(datasette)

Add custom URL routes. Return list of (regex, view_function) pairs:

```python
from datasette import hookimpl, Response

async def my_page(request):
    return Response.html("<h1>Hello!</h1>")

@hookimpl
def register_routes():
    return [
        (r"^/-/my-page$", my_page)
    ]
```

View functions can accept: datasette, request, scope, send, receive.

### render_cell(row, value, column, table, database, datasette, request)

Customize how table cell values are displayed:

```python
from datasette import hookimpl
import markupsafe

@hookimpl
def render_cell(value, column):
    if column == "stars":
        return markupsafe.Markup("⭐" * int(value))
```

### extra_template_vars(template, database, table, columns, view_name, request, datasette)

Add variables to template context:

```python
@hookimpl
def extra_template_vars(request, datasette):
    return {
        "user_agent": request.headers.get("user-agent"),
        "custom_data": "value"
    }
```

Can also return async functions for database queries.

### table_actions(datasette, actor, database, table, request)

Add menu items to table pages:

```python
@hookimpl
def table_actions(datasette, database, table):
    return [{
        "href": datasette.urls.path(f"/-/export/{database}/{table}"),
        "label": "Export this table",
        "description": "Download as CSV"
    }]
```

### actor_from_request(datasette, request)

Implement authentication. Return actor dict or None:

```python
@hookimpl
def actor_from_request(request):
    token = request.args.get("_token")
    if token == "secret":
        return {"id": "user123", "name": "Alice"}
```

Can return async function for database lookups.

### permission_allowed(datasette, actor, action, resource)

Control permissions. Return True (allow), False (deny), or None (no opinion):

```python
@hookimpl
def permission_allowed(actor, action, resource):
    if action == "execute-sql" and actor and actor.get("id") == "admin":
        return True
```

## Request and Response Objects

### Request Object

Available in many plugin hooks:

```python
request.method          # "GET" or "POST"
request.url             # Full URL
request.path            # Path without query string
request.full_path       # Path with query string
request.query_string    # Query string without ?
request.args            # MultiParams object for query params
request.args.get("key") # Get single param value
request.args.getlist("key")  # Get list of values
request.headers         # Dict of headers (lowercase keys)
request.cookies         # Dict of cookies
request.actor           # Current authenticated actor or None
request.url_vars        # Variables from URL regex

# Async methods:
body = await request.post_body()      # Raw POST body as bytes
form_vars = await request.post_vars() # Form data as dict
```

### Response Object

Create responses in view functions:

```python
from datasette.utils.asgi import Response

# HTML response
return Response.html("<h1>Hello</h1>")

# JSON response
return Response.json({"status": "ok"})

# Text response
return Response.text("Plain text")

# Redirect
return Response.redirect("/other-page")

# Custom response
return Response(
    body="Content",
    status=200,
    headers={"X-Custom": "value"},
    content_type="text/plain"
)

# Set cookies
response = Response.html("<h1>Hello</h1>")
response.set_cookie("session", datasette.sign({"id": "123"}, "cookie"))
```

## Database API

Access databases in plugins:

```python
# Get database object
db = datasette.get_database("mydb")  # Named database
db = datasette.get_database()        # First database

# Execute read query
results = await db.execute("SELECT * FROM mytable WHERE id = ?", [123])
for row in results:
    print(row["column_name"])

# Query properties
results.rows         # List of Row objects
results.columns      # List of column names
results.truncated    # True if results were truncated
results.first()      # First row or None
results.single_value()  # Single value if query returned one

# Execute write query
await db.execute_write(
    "INSERT INTO mytable (name) VALUES (?)",
    ["value"]
)

# Execute multiple writes
await db.execute_write_many(
    "INSERT INTO mytable (id, name) VALUES (?, ?)",
    [(1, "Alice"), (2, "Bob")]
)

# Execute function with write connection
def insert_and_count(conn):
    conn.execute("INSERT INTO mytable (name) VALUES (?)", ["Alice"])
    return conn.execute("SELECT COUNT(*) FROM mytable").fetchone()[0]

count = await db.execute_write_fn(insert_and_count)

# Introspection
tables = await db.table_names()
views = await db.view_names()
columns = await db.table_columns("mytable")
exists = await db.table_exists("mytable")
```

## Plugin Configuration

Users configure plugins in datasette.yaml:

```yaml
plugins:
  datasette-my-plugin:
    api_key: secret123
    enabled: true
```

Or per-database:

```yaml
databases:
  mydb:
    plugins:
      datasette-my-plugin:
        setting: value
```

Access in plugin code:

```python
config = datasette.plugin_config("datasette-my-plugin")
api_key = config.get("api_key") if config else None

# With database/table context
config = datasette.plugin_config(
    "datasette-my-plugin",
    database="mydb",
    table="mytable"
)
```

Configuration lookup: table → database → instance level.

## Static Assets and Templates

### Static Files

Place in `static/` directory, reference with:

```python
# In Python
url = datasette.urls.static_plugins("datasette_my_plugin", "app.js")

# In templates
<script src="{{ urls.static_plugins('datasette_my_plugin', 'app.js') }}"></script>
```

### Templates

Place in `templates/` directory. Override Datasette templates:

- `database.html` - Database page
- `table.html` - Table page
- `row.html` - Row page
- `query.html` - Query page

Access template functions:

```jinja
{{ csrftoken() }}                    {# CSRF token for forms #}
{{ urls.instance() }}                {# Homepage URL #}
{{ urls.database("mydb") }}          {# Database URL #}
{{ urls.table("mydb", "mytable") }}  {# Table URL #}
```

## Common Patterns

### Add a custom SQL function

```python
@hookimpl
def prepare_connection(conn):
    import hashlib
    def md5(text):
        return hashlib.md5(text.encode()).hexdigest()
    conn.create_function("md5", 1, md5)
```

### Add a custom page

```python
@hookimpl
def register_routes(datasette):
    async def stats_page(request):
        db = datasette.get_database()
        tables = await db.table_names()
        return Response.html(
            await datasette.render_template(
                "stats.html",
                {"tables": tables},
                request=request
            )
        )

    return [(r"^/-/stats$", stats_page)]
```

### Render custom output format

```python
@hookimpl
def register_output_renderer(datasette):
    def render_csv(columns, rows):
        import csv, io
        output = io.StringIO()
        writer = csv.writer(output)
        writer.writerow(columns)
        writer.writerows(rows)
        return Response(
            output.getvalue(),
            content_type="text/csv"
        )

    return {
        "extension": "csv",
        "render": render_csv
    }
```

### Check permissions

```python
@hookimpl
def register_routes(datasette):
    async def admin_page(request):
        # Check if user has permission
        allowed = await datasette.permission_allowed(
            request.actor,
            "admin-page",
            default=False
        )
        if not allowed:
            from datasette import Forbidden
            raise Forbidden("Admin access required")

        return Response.html("<h1>Admin Page</h1>")

    return [(r"^/-/admin$", admin_page)]
```

## URL Design

Use `/-/` prefix to avoid conflicts with database names:

- `/-/my-plugin` - Instance-level page
- `/dbname/-/my-plugin` - Database-level page
- `/dbname/table/-/my-plugin` - Table-level page

Build URLs with base_url support:

```python
datasette.urls.path("/-/my-page")
datasette.urls.database("mydb")
datasette.urls.table("mydb", "mytable")
```

## Testing Plugins

Create tests in `tests/test_my_plugin.py`:

```python
from datasette.app import Datasette
import pytest

@pytest.mark.asyncio
async def test_my_plugin():
    datasette = Datasette()
    await datasette.invoke_startup()

    # Test with client
    response = await datasette.client.get("/-/my-page")
    assert response.status_code == 200

    # Test database operations
    db = datasette.get_database()
    result = await db.execute("SELECT hello_world()")
    assert result.first()[0] == "Hello world!"
```

Run tests: `pytest`

## Publishing

### To GitHub

```bash
git init
git add .
git commit -m "Initial commit"
git branch -m main
git remote add origin git@github.com:username/datasette-my-plugin.git
git push -u origin main
```

### To PyPI

Configure GitHub release environment with PyPI trusted publisher, then create a GitHub release matching your version number. The GitHub Action will automatically publish to PyPI.

## Key Resources

- Plugin hooks reference: https://docs.datasette.io/en/stable/plugin_hooks.html
- Internals documentation: https://docs.datasette.io/en/stable/internals.html
- Example plugins: https://datasette.io/plugins
- Cookiecutter template: https://github.com/simonw/datasette-plugin

## Important Notes

- Accept only the parameters you need in hook functions (dependency injection)
- Use `@hookimpl` decorator for all plugin hooks
- Async functions need `async def` and `await`
- CSRF tokens required for POST forms: `<input type="hidden" name="csrftoken" value="{{ csrftoken() }}">`
- Plugin package name uses underscores, pip name uses hyphens
- Use `-` prefix in URLs: `/-/plugin-path`
- Access internal database with `datasette.get_internal_database()`
