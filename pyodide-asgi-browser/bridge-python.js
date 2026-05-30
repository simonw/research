// bridge-python.js — the ASGI bridge harness, shared by every Pyodide worker
// (worker.js for FastAPI, worker-datasette.js for Datasette).
//
// It is embedded as a String.raw block so the pure-Python unit tests can extract
// and exercise the exact source that runs in the browser. Constraint: the Python
// must contain no backticks and no "${" so it survives String.raw.
//
// PYTHON-BEGIN asgi_bridge
const ASGI_BRIDGE_PY = String.raw`
import asyncio


class ASGIBridge:
    """Drives an ASGI app: runs the lifespan protocol once and handles HTTP
    requests one message-cycle at a time, returning a plain dict response."""

    def __init__(self, app, root_path=""):
        self.app = app
        self.root_path = root_path
        self._lifespan_task = None
        self._startup_complete = None
        self._shutdown_complete = None
        self._recv_queue = None

    async def startup(self):
        loop = asyncio.get_event_loop()
        self._recv_queue = asyncio.Queue()
        self._startup_complete = loop.create_future()
        self._shutdown_complete = loop.create_future()
        scope = {"type": "lifespan", "asgi": {"version": "3.0", "spec_version": "2.0"}}
        await self._recv_queue.put({"type": "lifespan.startup"})

        async def receive():
            return await self._recv_queue.get()

        async def send(message):
            t = message["type"]
            if t == "lifespan.startup.complete":
                if not self._startup_complete.done():
                    self._startup_complete.set_result(True)
            elif t == "lifespan.startup.failed":
                if not self._startup_complete.done():
                    self._startup_complete.set_exception(
                        RuntimeError(message.get("message", "lifespan startup failed"))
                    )
            elif t == "lifespan.shutdown.complete":
                if not self._shutdown_complete.done():
                    self._shutdown_complete.set_result(True)

        async def run():
            try:
                await self.app(scope, receive, send)
            except Exception as exc:
                if not self._startup_complete.done():
                    self._startup_complete.set_exception(exc)
                if not self._shutdown_complete.done():
                    self._shutdown_complete.set_exception(exc)

        self._lifespan_task = asyncio.ensure_future(run())
        await self._startup_complete

    async def shutdown(self):
        if self._recv_queue is None:
            return
        await self._recv_queue.put({"type": "lifespan.shutdown"})
        await self._shutdown_complete

    async def handle(self, method, path, query_string, headers, body=b"",
                     scheme="http", host="localhost", port=80):
        if isinstance(query_string, str):
            query_string = query_string.encode("latin-1")
        if isinstance(body, str):
            body = body.encode("utf-8")
        if body is None:
            body = b""

        raw_headers = []
        have_host = False
        for name, value in headers:
            name_bytes = name.lower().encode("latin-1")
            if name_bytes == b"host":
                have_host = True
            raw_headers.append((name_bytes, value.encode("latin-1")))
        if not have_host:
            # A service worker cannot forward the browser-managed Host header, so
            # synthesize one. The non-default port MUST be included, otherwise
            # url_for() / redirects emit port-less URLs that fail in the browser.
            host_header = host
            if int(port) not in (80, 443):
                host_header = host + ":" + str(int(port))
            raw_headers.append((b"host", host_header.encode("latin-1")))

        scope = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method.upper(),
            "scheme": scheme,
            "path": path,
            "raw_path": path.encode("utf-8"),
            "query_string": query_string,
            "root_path": self.root_path,
            "headers": raw_headers,
            "server": (host, int(port)),
            "client": ("127.0.0.1", 0),
        }

        request_messages = [{"type": "http.request", "body": body, "more_body": False}]

        async def receive():
            if request_messages:
                return request_messages.pop(0)
            return {"type": "http.disconnect"}

        response = {"status": 500, "headers": [], "body": bytearray()}

        async def send(message):
            t = message["type"]
            if t == "http.response.start":
                response["status"] = message["status"]
                response["headers"] = [
                    [k.decode("latin-1"), v.decode("latin-1")]
                    for k, v in message.get("headers", [])
                ]
            elif t == "http.response.body":
                chunk = message.get("body", b"") or b""
                response["body"].extend(chunk)

        await self.app(scope, receive, send)
        return {
            "status": response["status"],
            "headers": response["headers"],
            "body": bytes(response["body"]),
        }
`;
// PYTHON-END asgi_bridge
