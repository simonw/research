---
name: starlette
description: >
  Build async web applications and APIs with Starlette 1.0, the lightweight ASGI framework for Python.
  Use this skill whenever a user wants to create an async Python web app, REST API, WebSocket server,
  or ASGI application using Starlette. Triggers include mentions of 'Starlette', 'ASGI', async Python
  web frameworks, or requests to build lightweight async APIs, WebSocket services, streaming responses,
  or middleware pipelines. Also use when the user is working with FastAPI internals (which is built on
  Starlette), needs ASGI middleware patterns, or wants a minimal async web server without a full
  framework. Covers routing, requests/responses, WebSockets, middleware, templates, static files,
  authentication, lifespan, background tasks, config, testing, schemas, and more.
---

# Starlette 1.0

Starlette is a lightweight ASGI framework/toolkit for building high-performance async web services in Python.
Version 1.0 was released on March 22, 2026 — the first stable release after nearly eight years of development.
It requires **Python 3.10+** and depends only on `anyio`. It is the foundation for FastAPI and the Python MCP SDK.

## Installation

```bash
pip install starlette
pip install uvicorn          # ASGI server
pip install starlette[full]  # all optional deps: httpx, jinja2, python-multipart, itsdangerous, pyyaml
```

---

## 1 · Application

The `Starlette` class ties together routing, middleware, exception handling, and lifespan.

```python
from contextlib import asynccontextmanager
from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.sessions import SessionMiddleware
from starlette.responses import JSONResponse
from starlette.routing import Route

@asynccontextmanager
async def lifespan(app):
    print("Starting up")
    yield
    print("Shutting down")

async def homepage(request):
    return JSONResponse({"hello": "world"})

app = Starlette(
    debug=True,
    routes=[Route("/", homepage)],
    middleware=[Middleware(SessionMiddleware, secret_key="secret")],
    exception_handlers={},
    lifespan=lifespan,
)
```

Run with: `uvicorn main:app --reload`

### Starlette constructor parameters (1.0)

| Parameter | Description |
|---|---|
| `debug` | Enable debug tracebacks on errors |
| `routes` | List of `Route`, `WebSocketRoute`, `Mount`, `Host` |
| `middleware` | List of `Middleware(cls, **kwargs)` |
| `exception_handlers` | Dict mapping status codes or exception types to handlers |
| `lifespan` | Async context manager for startup/shutdown |

> **1.0 Breaking Change:** `on_startup`, `on_shutdown`, `@app.route()`, `@app.websocket_route()`,
> `@app.middleware()`, `@app.exception_handler()`, `on_event()`, and `add_event_handler()` have all
> been **removed**. Use `lifespan`, `routes`, `middleware`, and `exception_handlers` parameters instead.

### Storing and accessing state

```python
# On the app
app.state.ADMIN_EMAIL = "admin@example.org"

# In an endpoint, access via request.app
async def dashboard(request):
    email = request.app.state.ADMIN_EMAIL
    return JSONResponse({"admin": email})
```

### Using Starlette as a pure ASGI toolkit

You can use any Starlette component standalone, without the `Starlette` class:

```python
from starlette.responses import PlainTextResponse

async def app(scope, receive, send):
    assert scope["type"] == "http"
    response = PlainTextResponse("Hello, world!")
    await response(scope, receive, send)
```

---

## 2 · Routing

### Basic routes

```python
from starlette.routing import Route, Mount, WebSocketRoute
from starlette.responses import PlainTextResponse

async def homepage(request):
    return PlainTextResponse("Home")

async def user_detail(request):
    username = request.path_params["username"]
    return PlainTextResponse(f"Hello, {username}")

routes = [
    Route("/", homepage),
    Route("/users/{username}", user_detail, methods=["GET"]),
]
```

### Path parameter convertors

Built-in convertors: `str` (default), `int`, `float`, `uuid`, `path`.

```python
Route("/items/{item_id:int}", get_item)
Route("/files/{file_path:path}", get_file)
Route("/values/{val:float}", get_value)
Route("/objects/{obj_id:uuid}", get_object)
```

### Custom convertors

```python
from datetime import datetime
from starlette.convertors import Convertor, register_url_convertor

class DateTimeConvertor(Convertor):
    regex = r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"

    def convert(self, value: str) -> datetime:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%S")

    def to_string(self, value: datetime) -> str:
        return value.strftime("%Y-%m-%dT%H:%M:%S")

register_url_convertor("datetime", DateTimeConvertor())
# Usage: Route("/history/{date:datetime}", history)
```

### Submounting routes

```python
from starlette.routing import Mount
from starlette.staticfiles import StaticFiles

routes = [
    Route("/", homepage),
    Mount("/users", routes=[
        Route("/", users_list, methods=["GET", "POST"]),
        Route("/{username}", user_detail),
    ]),
    Mount("/static", app=StaticFiles(directory="static"), name="static"),
]
```

### Reverse URL lookups

```python
# In an endpoint — produces a full URL
url = request.url_for("user_detail", username="alice")

# With named mounts
routes = [
    Mount("/users", name="users", routes=[
        Route("/{username}", user_detail, name="user_detail"),
    ])
]
url = request.url_for("users:user_detail", username="alice")

# Without a request (returns path only)
path = app.url_path_for("user_detail", username="alice")
```

### Host-based routing

```python
from starlette.routing import Host, Router

api = Router(routes=[...])
routes = [
    Host("api.example.org", app=api, name="api"),
    Host("{subdomain}.example.org", name="sub", app=Router(routes=[...])),
]
```

### WebSocket routing

```python
async def ws_endpoint(websocket):
    await websocket.accept()
    await websocket.send_text("Hello, WebSocket!")
    await websocket.close()

routes = [
    WebSocketRoute("/ws", ws_endpoint),
    WebSocketRoute("/ws/{name}", ws_user),
]
```

### Route priority

More specific routes must come before general ones:

```python
routes = [
    Route("/users/me", current_user),       # Specific first
    Route("/users/{username}", user_detail), # General second
]
```

### Using Router directly

```python
from starlette.routing import Router

app = Router(routes=[
    Route("/", homepage),
    Mount("/api", routes=[...]),
])
```

---

## 3 · Requests

```python
from starlette.requests import Request

async def endpoint(request: Request):
    # Method
    method = request.method  # "GET", "POST", etc.

    # URL components
    url = request.url           # string-like, e.g. "http://localhost/path?q=1"
    path = request.url.path     # "/path"
    scheme = request.url.scheme # "http"
    port = request.url.port     # 8000

    # Headers (immutable, case-insensitive multi-dict)
    ct = request.headers["content-type"]

    # Query parameters (immutable multi-dict)
    search = request.query_params["search"]
    # or request.query_params.getlist("tags")

    # Path parameters
    user_id = request.path_params["user_id"]

    # Client address
    if request.client:
        host = request.client.host
        port = request.client.port

    # Cookies
    token = request.cookies.get("session_token")

    # Body (various interfaces)
    body_bytes = await request.body()
    body_json = await request.json()

    # Form data (requires python-multipart)
    async with request.form() as form:
        username = form["username"]

    # Streaming body
    body = b""
    async for chunk in request.stream():
        body += chunk

    # Check client disconnection
    if await request.is_disconnected():
        return

    # Access app instance
    app = request.app

    # Arbitrary state
    request.state.start_time = 1234567890
```

### File uploads

```python
async def upload(request):
    async with request.form(max_files=1000, max_fields=1000, max_part_size=1024*1024) as form:
        upload_file = form["file"]  # UploadFile instance
        filename = upload_file.filename       # "photo.jpg"
        content_type = upload_file.content_type  # "image/jpeg"
        size = upload_file.size               # bytes
        contents = await upload_file.read()
        await upload_file.seek(0)
        await upload_file.close()
```

---

## 4 · Responses

### Basic response types

```python
from starlette.responses import (
    Response,
    PlainTextResponse,
    HTMLResponse,
    JSONResponse,
    RedirectResponse,
    StreamingResponse,
    FileResponse,
)

# Plain response
Response("Hello", media_type="text/plain", status_code=200, headers={"X-Custom": "value"})

# Text
PlainTextResponse("Hello, world!")

# HTML
HTMLResponse("<h1>Hello</h1>")

# JSON
JSONResponse({"key": "value"})

# Redirect (default 307)
RedirectResponse(url="/new-location", status_code=301)
```

### Custom JSON serialization (e.g. orjson)

```python
import orjson
from starlette.responses import JSONResponse
from typing import Any

class OrjsonResponse(JSONResponse):
    def render(self, content: Any) -> bytes:
        return orjson.dumps(content)
```

### Streaming response

```python
import asyncio
from starlette.responses import StreamingResponse

async def generate():
    for i in range(10):
        yield f"data: {i}\n\n"
        await asyncio.sleep(0.5)

async def stream(request):
    return StreamingResponse(generate(), media_type="text/event-stream")
```

### File response (with HTTP range request support)

```python
from starlette.responses import FileResponse

async def download(request):
    return FileResponse(
        path="reports/annual.pdf",
        filename="annual-report.pdf",
        media_type="application/pdf",
        content_disposition_type="attachment",  # or "inline"
    )
```

`FileResponse` automatically includes `Content-Length`, `Last-Modified`, `ETag`, and `Accept-Ranges: bytes` headers.

### Setting and deleting cookies

```python
from starlette.responses import JSONResponse

async def login(request):
    response = JSONResponse({"ok": True})
    response.set_cookie(
        key="session",
        value="abc123",
        max_age=3600,
        httponly=True,
        secure=True,
        samesite="lax",
    )
    return response

async def logout(request):
    response = JSONResponse({"ok": True})
    response.delete_cookie(key="session")
    return response
```

---

## 5 · WebSockets

```python
from starlette.websockets import WebSocket

async def ws_app(scope, receive, send):
    websocket = WebSocket(scope=scope, receive=receive, send=send)
    await websocket.accept()

    # Send data
    await websocket.send_text("hello")
    await websocket.send_bytes(b"\x00\x01")
    await websocket.send_json({"key": "value"})

    # Receive data
    text = await websocket.receive_text()
    data = await websocket.receive_bytes()
    obj = await websocket.receive_json()

    # Iterate over messages (exits on disconnect)
    async for message in websocket.iter_text():
        await websocket.send_text(f"Echo: {message}")

    # Close
    await websocket.close(code=1000, reason="Done")
```

### WebSocket denial response

If you reject before accepting, a 403 is sent automatically. For custom denial:

```python
from starlette.responses import JSONResponse

async def ws_endpoint(websocket):
    if not authorized:
        response = JSONResponse({"error": "Unauthorized"}, status_code=401)
        await websocket.send_denial_response(response)
        return
    await websocket.accept()
```

Or use `HTTPException` within a Starlette app (before `accept()`):

```python
from starlette.exceptions import HTTPException

async def ws_endpoint(websocket):
    if not is_valid(websocket):
        raise HTTPException(status_code=401, detail="Unauthorized")
    await websocket.accept()
    ...
```

---

## 6 · Class-Based Endpoints

### HTTPEndpoint

```python
from starlette.endpoints import HTTPEndpoint
from starlette.responses import JSONResponse
from starlette.routing import Route

class UserEndpoint(HTTPEndpoint):
    async def get(self, request):
        return JSONResponse({"users": []})

    async def post(self, request):
        data = await request.json()
        return JSONResponse({"created": data}, status_code=201)

routes = [Route("/users", UserEndpoint)]
```

Responds with 405 for unimplemented methods automatically.

### WebSocketEndpoint

```python
from starlette.endpoints import WebSocketEndpoint
from starlette.routing import WebSocketRoute

class EchoEndpoint(WebSocketEndpoint):
    encoding = "text"  # "json", "bytes", or "text"

    async def on_connect(self, websocket):
        await websocket.accept()

    async def on_receive(self, websocket, data):
        await websocket.send_text(f"Echo: {data}")

    async def on_disconnect(self, websocket, close_code):
        pass

routes = [WebSocketRoute("/ws", EchoEndpoint)]
```

---

## 7 · Middleware

### Applying middleware

```python
from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.cors import CORSMiddleware
from starlette.middleware.trustedhost import TrustedHostMiddleware
from starlette.middleware.httpsredirect import HTTPSRedirectMiddleware
from starlette.middleware.gzip import GZipMiddleware
from starlette.middleware.sessions import SessionMiddleware

middleware = [
    Middleware(TrustedHostMiddleware, allowed_hosts=["example.com", "*.example.com"]),
    Middleware(HTTPSRedirectMiddleware),
    Middleware(CORSMiddleware,
        allow_origins=["https://example.com"],
        allow_methods=["*"],
        allow_headers=["*"],
        allow_credentials=True,
        allow_private_network=False,
        expose_headers=[],
        max_age=600,
    ),
    Middleware(GZipMiddleware, minimum_size=500, compresslevel=9),
    Middleware(SessionMiddleware,
        secret_key="super-secret",
        session_cookie="session",
        max_age=14 * 24 * 3600,
        same_site="lax",
        https_only=True,
    ),
]

app = Starlette(routes=routes, middleware=middleware)
```

Execution order: `ServerErrorMiddleware` → user middleware (top-to-bottom) → `ExceptionMiddleware` → Router → Endpoint.

### CORSMiddleware

```python
Middleware(CORSMiddleware,
    allow_origins=["https://example.com"],
    allow_origin_regex=r"https://[a-zA-Z0-9-]+\.example\.com",
    allow_methods=["GET", "POST"],
    allow_headers=["Authorization"],
    allow_credentials=True,
    allow_private_network=True,
    expose_headers=["X-Request-Id"],
    max_age=600,
)
```

For CORS on error responses too, wrap the whole app:

```python
app = Starlette(routes=routes)
app = CORSMiddleware(app=app, allow_origins=["*"])
```

### SessionMiddleware

```python
async def set_session(request):
    request.session["username"] = "alice"
    return JSONResponse({"ok": True})

async def get_session(request):
    username = request.session.get("username", "anonymous")
    return JSONResponse({"user": username})
```

### BaseHTTPMiddleware (custom middleware)

```python
from starlette.middleware.base import BaseHTTPMiddleware

class TimingMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request, call_next):
        import time
        start = time.time()
        response = await call_next(request)
        duration = time.time() - start
        response.headers["X-Process-Time"] = str(duration)
        return response

middleware = [Middleware(TimingMiddleware)]
```

> **Note:** `BaseHTTPMiddleware` has a limitation — `contextvars.ContextVar` changes from endpoints
> don't propagate back through the middleware. Use pure ASGI middleware for full control.

### Pure ASGI middleware

```python
from starlette.types import ASGIApp, Scope, Receive, Send, Message
from starlette.datastructures import MutableHeaders
from starlette.requests import Request

class SecurityHeadersMiddleware:
    def __init__(self, app: ASGIApp) -> None:
        self.app = app

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        async def send_with_headers(message: Message) -> None:
            if message["type"] == "http.response.start":
                headers = MutableHeaders(scope=message)
                headers.append("X-Content-Type-Options", "nosniff")
                headers.append("X-Frame-Options", "DENY")
            await send(message)

        await self.app(scope, receive, send_with_headers)
```

### Middleware on specific routes

```python
from starlette.routing import Mount, Route
from starlette.middleware import Middleware
from starlette.middleware.gzip import GZipMiddleware

routes = [
    Mount("/api", routes=[
        Route("/data", data_endpoint),
    ], middleware=[Middleware(GZipMiddleware)]),
    Route("/health", health_endpoint, middleware=[Middleware(GZipMiddleware)]),
]
```

---

## 8 · Lifespan (Startup & Shutdown)

```python
from contextlib import asynccontextmanager
from collections.abc import AsyncIterator
from typing import TypedDict
import httpx

from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import PlainTextResponse
from starlette.routing import Route

class State(TypedDict):
    http_client: httpx.AsyncClient

@asynccontextmanager
async def lifespan(app: Starlette) -> AsyncIterator[State]:
    async with httpx.AsyncClient() as client:
        yield {"http_client": client}
    # Cleanup happens after yield

async def homepage(request: Request[State]) -> PlainTextResponse:
    # Attribute-style access
    client = request.state.http_client
    # Dictionary-style access (type-safe, new in 0.52+)
    client = request.state["http_client"]
    resp = await client.get("https://httpbin.org/get")
    return PlainTextResponse(resp.text)

app = Starlette(lifespan=lifespan, routes=[Route("/", homepage)])
```

Dictionary-style access also works on `WebSocket[State]`.

---

## 9 · Background Tasks

```python
from starlette.background import BackgroundTask, BackgroundTasks
from starlette.responses import JSONResponse

async def send_email(to: str, subject: str):
    ...  # send the email

async def log_signup(username: str):
    ...  # log to analytics

# Single task
async def signup(request):
    data = await request.json()
    task = BackgroundTask(send_email, to=data["email"], subject="Welcome!")
    return JSONResponse({"status": "ok"}, background=task)

# Multiple tasks (run sequentially)
async def signup_v2(request):
    data = await request.json()
    tasks = BackgroundTasks()
    tasks.add_task(send_email, to=data["email"], subject="Welcome!")
    tasks.add_task(log_signup, username=data["username"])
    return JSONResponse({"status": "ok"}, background=tasks)
```

Tasks run **after** the response is sent. They execute in order; if one raises, subsequent tasks are skipped.

---

## 10 · Templates (Jinja2)

Requires: `pip install jinja2`

```python
from starlette.templating import Jinja2Templates
from starlette.staticfiles import StaticFiles
from starlette.routing import Route, Mount

templates = Jinja2Templates(directory="templates")

async def homepage(request):
    return templates.TemplateResponse(request, "index.html", context={"title": "Home"})

routes = [
    Route("/", homepage),
    Mount("/static", StaticFiles(directory="static"), name="static"),
]
```

In templates, `url_for` is available automatically:

```html
<link href="{{ url_for('static', path='/css/style.css') }}" rel="stylesheet" />
```

### Custom Jinja2 environment

```python
import jinja2
from starlette.templating import Jinja2Templates

env = jinja2.Environment(loader=jinja2.FileSystemLoader("templates"), autoescape=True)
templates = Jinja2Templates(env=env)
```

> **1.0 Breaking Change:** The `**env_options` parameter and `TemplateResponse(name, context)` signature
> have been removed. Use `env=` parameter and `TemplateResponse(request, name, ...)` respectively.

### Custom filters

```python
templates = Jinja2Templates(directory="templates")
templates.env.filters["markdown"] = my_markdown_filter
```

### Context processors

```python
def app_context(request):
    return {"app_name": "MyApp", "user": request.state.user}

templates = Jinja2Templates(directory="templates", context_processors=[app_context])
```

### Testing template responses

```python
def test_homepage():
    client = TestClient(app)
    response = client.get("/")
    assert response.template.name == "index.html"
    assert "title" in response.context
```

---

## 11 · Static Files

```python
from starlette.staticfiles import StaticFiles
from starlette.routing import Mount

routes = [
    Mount("/static", app=StaticFiles(directory="static"), name="static"),
]
```

Parameters: `directory`, `packages`, `html` (serves `index.html` for dirs), `check_dir`, `follow_symlink`.

### HTML mode (SPA-like)

```python
Mount("/", app=StaticFiles(directory="build", html=True), name="spa")
```

Serves `index.html` for directories and `404.html` for missing files.

### Package static files

```python
Mount("/static", app=StaticFiles(packages=["bootstrap4"]), name="static")
# Or with custom subdirectory:
Mount("/static", app=StaticFiles(packages=[("mypackage", "assets")]), name="static")
```

---

## 12 · Exception Handling

```python
from starlette.applications import Starlette
from starlette.exceptions import HTTPException, WebSocketException
from starlette.requests import Request
from starlette.responses import JSONResponse
from starlette.websockets import WebSocket

async def not_found(request: Request, exc: HTTPException):
    return JSONResponse({"error": exc.detail}, status_code=404)

async def server_error(request: Request, exc: Exception):
    return JSONResponse({"error": "Internal server error"}, status_code=500)

async def http_exception_handler(request: Request, exc: HTTPException):
    return JSONResponse(
        {"detail": exc.detail},
        status_code=exc.status_code,
        headers=exc.headers,
    )

exception_handlers = {
    404: not_found,
    500: server_error,
    HTTPException: http_exception_handler,
}

app = Starlette(routes=routes, exception_handlers=exception_handlers)
```

### Raising exceptions in endpoints

```python
from starlette.exceptions import HTTPException

async def get_item(request):
    item_id = request.path_params["item_id"]
    item = db.get(item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    return JSONResponse(item)
```

### WebSocket exceptions

```python
from starlette.exceptions import WebSocketException

async def ws_endpoint(websocket):
    await websocket.accept()
    raise WebSocketException(code=1008, reason="Policy violation")
```

---

## 13 · Authentication

```python
import base64, binascii
from starlette.authentication import (
    AuthCredentials, AuthenticationBackend, AuthenticationError,
    SimpleUser, requires,
)
from starlette.middleware import Middleware
from starlette.middleware.authentication import AuthenticationMiddleware
from starlette.responses import PlainTextResponse, JSONResponse
from starlette.routing import Route

class BasicAuthBackend(AuthenticationBackend):
    async def authenticate(self, conn):
        if "Authorization" not in conn.headers:
            return None
        auth = conn.headers["Authorization"]
        try:
            scheme, credentials = auth.split()
            if scheme.lower() != "basic":
                return None
            decoded = base64.b64decode(credentials).decode("ascii")
        except (ValueError, UnicodeDecodeError, binascii.Error):
            raise AuthenticationError("Invalid credentials")
        username, _, password = decoded.partition(":")
        # Verify credentials here...
        return AuthCredentials(["authenticated"]), SimpleUser(username)

async def homepage(request):
    if request.user.is_authenticated:
        return PlainTextResponse(f"Hello, {request.user.display_name}")
    return PlainTextResponse("Hello, anonymous")

@requires("authenticated")
async def dashboard(request):
    return PlainTextResponse("Dashboard")

@requires(["authenticated", "admin"], status_code=404)
async def admin_panel(request):
    return PlainTextResponse("Admin")

@requires("authenticated", redirect="homepage")
async def settings(request):
    return PlainTextResponse("Settings")

def on_auth_error(request, exc):
    return JSONResponse({"error": str(exc)}, status_code=401)

routes = [
    Route("/", homepage, name="homepage"),
    Route("/dashboard", dashboard),
    Route("/admin", admin_panel),
    Route("/settings", settings),
]

middleware = [
    Middleware(AuthenticationMiddleware, backend=BasicAuthBackend(), on_error=on_auth_error),
]

app = Starlette(routes=routes, middleware=middleware)
```

---

## 14 · Configuration

```python
from starlette.config import Config
from starlette.datastructures import CommaSeparatedStrings, Secret

config = Config(".env")

DEBUG = config("DEBUG", cast=bool, default=False)
SECRET_KEY = config("SECRET_KEY", cast=Secret)
DATABASE_URL = config("DATABASE_URL")
ALLOWED_HOSTS = config("ALLOWED_HOSTS", cast=CommaSeparatedStrings)

# Prefixed env vars
config = Config(env_prefix="APP_")
DEBUG = config("DEBUG")  # reads APP_DEBUG

# Custom encoding
config = Config(".env", encoding="latin-1")
```

`.env` file:

```
DEBUG=True
SECRET_KEY=my-secret-key
DATABASE_URL=postgresql://user:pass@localhost/db
ALLOWED_HOSTS=127.0.0.1,localhost
```

### Secret values

```python
>>> SECRET_KEY
Secret('**********')
>>> str(SECRET_KEY)
'my-secret-key'
```

### Safe testing with environ

```python
# tests/conftest.py
from starlette.config import environ
environ["DEBUG"] = "TRUE"
environ["DATABASE_URL"] = "sqlite:///test.db"
```

---

## 15 · Test Client

Requires: `pip install httpx`

```python
from starlette.testclient import TestClient

# Basic usage
client = TestClient(app)
response = client.get("/")
assert response.status_code == 200

# With lifespan support (must use context manager)
with TestClient(app) as client:
    response = client.get("/")

# Custom headers
client.headers = {"Authorization": "Bearer token123"}
response = client.get("/protected")

# POST with JSON
response = client.post("/users", json={"name": "Alice"})

# File upload
with open("photo.jpg", "rb") as f:
    response = client.post("/upload", files={"file": f})

# Disable server exception raising (for testing 500 pages)
client = TestClient(app, raise_server_exceptions=False)

# Custom client address
client = TestClient(app, client=("localhost", 8000))

# Using Trio backend
with TestClient(app, backend="trio") as client:
    response = client.get("/")
```

### Testing WebSockets

```python
def test_websocket():
    client = TestClient(app)
    with client.websocket_connect("/ws") as ws:
        ws.send_text("hello")
        data = ws.receive_text()
        assert data == "Echo: hello"
        ws.send_json({"key": "value"})
        result = ws.receive_json()
        ws.close()
```

### Async testing with httpx

```python
from httpx import AsyncClient, ASGITransport
import pytest

@pytest.mark.anyio
async def test_app():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.get("/")
        assert response.status_code == 200
```

---

## 16 · Schema Generation (OpenAPI)

Requires: `pip install pyyaml`

```python
from starlette.schemas import SchemaGenerator

schemas = SchemaGenerator(
    {"openapi": "3.0.0", "info": {"title": "My API", "version": "1.0"}}
)

def list_users(request):
    """
    responses:
      200:
        description: A list of users.
        examples:
          [{"username": "alice"}, {"username": "bob"}]
    """
    ...

def openapi_schema(request):
    return schemas.OpenAPIResponse(request=request)

routes = [
    Route("/users", list_users, methods=["GET"]),
    Route("/schema", openapi_schema, include_in_schema=False),
]
```

Generate schema programmatically:

```python
schema = schemas.get_schema(routes=app.routes)
```

---

## 17 · Data Structures

Starlette provides several useful data structures:

```python
from starlette.datastructures import (
    URL,
    Headers,
    MutableHeaders,
    QueryParams,
    State,
    UploadFile,
    FormData,
    Address,
    Secret,
    CommaSeparatedStrings,
)

# URL manipulation
url = URL("https://example.com/path?q=1")
url.scheme   # "https"
url.hostname # "example.com"
url.path     # "/path"
new_url = url.replace(port=8080, path="/new")

# Immutable headers
headers = Headers({"content-type": "text/html"})
headers["content-type"]  # "text/html"

# Mutable headers (for middleware)
mutable = MutableHeaders(headers=response.headers)
mutable["X-Custom"] = "value"

# Query params (immutable multi-dict)
params = QueryParams("page=1&tag=python&tag=async")
params["page"]          # "1"
params.getlist("tag")   # ["python", "async"]

# State
state = State()
state.counter = 0
state.counter  # 0
```

---

## 18 · Thread Pool

Starlette automatically runs synchronous code in a thread pool:

```python
# Sync endpoint — automatically runs in thread pool
def sync_homepage(request):
    import time
    time.sleep(1)  # won't block the event loop
    return PlainTextResponse("Done")

Route("/", sync_homepage)
```

Adjust the thread pool size:

```python
import anyio.to_thread

limiter = anyio.to_thread.current_default_thread_limiter()
limiter.total_tokens = 100  # default is 40
```

Applies to: sync endpoints, `FileResponse`, `UploadFile`, sync `BackgroundTask` functions.

---

## 19 · HTTP Status Codes

```python
from starlette.status import (
    HTTP_200_OK,
    HTTP_201_CREATED,
    HTTP_204_NO_CONTENT,
    HTTP_301_MOVED_PERMANENTLY,
    HTTP_400_BAD_REQUEST,
    HTTP_401_UNAUTHORIZED,
    HTTP_403_FORBIDDEN,
    HTTP_404_NOT_FOUND,
    HTTP_500_INTERNAL_SERVER_ERROR,
    WS_1000_NORMAL_CLOSURE,
    WS_1008_POLICY_VIOLATION,
)
```

---

## 20 · Complete Application Example

```python
from contextlib import asynccontextmanager
from collections.abc import AsyncIterator
from typing import TypedDict

import httpx
from starlette.applications import Starlette
from starlette.authentication import (
    AuthCredentials, AuthenticationBackend, SimpleUser, requires,
)
from starlette.background import BackgroundTask
from starlette.config import Config
from starlette.datastructures import Secret
from starlette.exceptions import HTTPException
from starlette.middleware import Middleware
from starlette.middleware.authentication import AuthenticationMiddleware
from starlette.middleware.cors import CORSMiddleware
from starlette.middleware.sessions import SessionMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse, PlainTextResponse
from starlette.routing import Route, Mount, WebSocketRoute
from starlette.staticfiles import StaticFiles
from starlette.templating import Jinja2Templates
from starlette.websockets import WebSocket

# --- Config ---
config = Config(".env")
DEBUG = config("DEBUG", cast=bool, default=False)
SECRET = config("SECRET_KEY", cast=Secret)

# --- Lifespan with typed state ---
class AppState(TypedDict):
    http_client: httpx.AsyncClient

@asynccontextmanager
async def lifespan(app: Starlette) -> AsyncIterator[AppState]:
    async with httpx.AsyncClient() as client:
        yield {"http_client": client}

# --- Auth ---
class TokenAuthBackend(AuthenticationBackend):
    async def authenticate(self, conn):
        token = conn.headers.get("Authorization", "")
        if token == "Bearer valid-token":
            return AuthCredentials(["authenticated"]), SimpleUser("admin")
        return None

# --- Templates ---
templates = Jinja2Templates(directory="templates")

# --- Endpoints ---
async def homepage(request: Request[AppState]):
    client = request.state["http_client"]
    return templates.TemplateResponse(request, "index.html")

@requires("authenticated")
async def api_data(request: Request[AppState]):
    client = request.state["http_client"]
    resp = await client.get("https://httpbin.org/json")
    return JSONResponse(resp.json())

async def create_item(request):
    data = await request.json()
    task = BackgroundTask(process_item, item=data)
    return JSONResponse({"status": "accepted"}, status_code=202, background=task)

async def process_item(item: dict):
    ...  # heavy processing

async def ws_echo(websocket: WebSocket):
    await websocket.accept()
    async for message in websocket.iter_text():
        await websocket.send_text(f"Echo: {message}")

async def not_found(request, exc):
    return JSONResponse({"error": "Not found"}, status_code=404)

# --- App assembly ---
routes = [
    Route("/", homepage, name="homepage"),
    Route("/api/data", api_data, methods=["GET"]),
    Route("/api/items", create_item, methods=["POST"]),
    WebSocketRoute("/ws", ws_echo),
    Mount("/static", StaticFiles(directory="static"), name="static"),
]

middleware = [
    Middleware(CORSMiddleware, allow_origins=["*"], allow_methods=["*"]),
    Middleware(SessionMiddleware, secret_key=str(SECRET)),
    Middleware(AuthenticationMiddleware, backend=TokenAuthBackend()),
]

app = Starlette(
    debug=DEBUG,
    routes=routes,
    middleware=middleware,
    exception_handlers={404: not_found},
    lifespan=lifespan,
)
```

---

## Quick Reference: What Was Removed in 1.0

| Removed API | Replacement |
|---|---|
| `on_startup` / `on_shutdown` params | `lifespan` async context manager |
| `@app.on_event("startup")` | `lifespan` |
| `@app.route()` / `@app.websocket_route()` | `Route` / `WebSocketRoute` in `routes=` |
| `@app.middleware()` | `Middleware()` in `middleware=` |
| `@app.exception_handler()` | `exception_handlers=` dict |
| `Jinja2Templates(**env_options)` | `Jinja2Templates(env=jinja2.Environment(...))` |
| `TemplateResponse(name, context)` | `TemplateResponse(request, name, ...)` |
| `FileResponse(method=...)` | Removed (was deprecated) |
