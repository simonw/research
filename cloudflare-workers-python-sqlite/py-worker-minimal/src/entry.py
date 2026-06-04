from workers import WorkerEntrypoint, Response
from urllib.parse import urlparse, parse_qs

# Try to import sqlite3 - this might fail if the package needs to be downloaded
try:
    import sqlite3
    SQLITE_AVAILABLE = True
except ImportError:
    SQLITE_AVAILABLE = False

class Default(WorkerEntrypoint):
    async def on_fetch(self, request):
        url = urlparse(request.url)
        path = url.path

        # Hello world route
        if path == "/" or path == "/hello":
            return Response("Hello World from Python Worker!")

        # Form route
        if path == "/form":
            if request.method == "GET":
                html = """<!DOCTYPE html>
<html>
<head><title>Python Worker Form</title></head>
<body>
<h1>Python Worker Form</h1>
<form method="POST" action="/form">
  <label>Name: <input type="text" name="name"></label><br><br>
  <label>Message: <textarea name="message"></textarea></label><br><br>
  <button type="submit">Submit</button>
</form>
</body>
</html>"""
                return Response(html, headers={"Content-Type": "text/html"})
            elif request.method == "POST":
                body = await request.text()
                params = parse_qs(body)
                name = params.get("name", [""])[0]
                message = params.get("message", [""])[0]
                response_html = f"""<!DOCTYPE html>
<html>
<head><title>Form Response</title></head>
<body>
<h1>Form Submitted!</h1>
<p><strong>Name:</strong> {name}</p>
<p><strong>Message:</strong> {message}</p>
<a href="/form">Submit another</a>
</body>
</html>"""
                return Response(response_html, headers={"Content-Type": "text/html"})

        # Counter route
        if path == "/counter":
            if SQLITE_AVAILABLE:
                # Use SQLite for the counter
                conn = sqlite3.connect(":memory:")
                cursor = conn.cursor()
                cursor.execute("CREATE TABLE IF NOT EXISTS counts (key TEXT PRIMARY KEY, value INTEGER)")
                cursor.execute("INSERT OR REPLACE INTO counts VALUES ('counter', COALESCE((SELECT value FROM counts WHERE key='counter'), 0) + 1)")
                cursor.execute("SELECT value FROM counts WHERE key='counter'")
                count = cursor.fetchone()[0]
                conn.commit()
                conn.close()
                return Response(f"SQLite Counter: {count}")
            else:
                return Response("SQLite not available - package download required")

        # Status route
        if path == "/status":
            return Response(f"Python Worker Status:\n- SQLite available: {SQLITE_AVAILABLE}")

        # 404 for other routes
        return Response("Not Found", status=404)
