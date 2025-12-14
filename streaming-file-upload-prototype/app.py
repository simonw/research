"""
Streaming File Upload ASGI Application

This application demonstrates using the streaming-form-data library to handle
large file uploads by streaming them directly to temporary files on disk,
avoiding memory issues with large uploads.
"""

import hashlib
import os
import tempfile
from pathlib import Path
from typing import Optional

from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import JSONResponse, HTMLResponse
from starlette.routing import Route

from streaming_form_data import StreamingFormDataParser
from streaming_form_data.targets import FileTarget, ValueTarget, BaseTarget


class SHA256FileTarget(BaseTarget):
    """
    Custom target that writes to a file while calculating SHA256 checksum.
    """

    def __init__(self, filepath: str, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.filepath = filepath
        self._file = None
        self._hasher = None
        self._size = 0
        self._original_filename: Optional[str] = None

    def on_start(self):
        self._file = open(self.filepath, 'wb')
        self._hasher = hashlib.sha256()
        self._size = 0

    def on_data_received(self, chunk: bytes):
        self._file.write(chunk)
        self._hasher.update(chunk)
        self._size += len(chunk)

    def on_finish(self):
        if self._file:
            self._file.close()

    def set_multipart_filename(self, filename: str):
        """Called by parser when filename is extracted from headers."""
        self._original_filename = filename

    @property
    def sha256(self) -> str:
        return self._hasher.hexdigest() if self._hasher else ""

    @property
    def size(self) -> int:
        return self._size

    @property
    def original_filename(self) -> Optional[str]:
        return self._original_filename


# Directory for temporary uploads
UPLOAD_DIR = Path(tempfile.gettempdir()) / "streaming_uploads"
UPLOAD_DIR.mkdir(exist_ok=True)


async def upload_file(request: Request) -> JSONResponse:
    """
    Handle streaming file upload.

    Streams the incoming multipart/form-data directly to a temporary file
    without loading the entire file into memory.
    """
    content_type = request.headers.get("content-type", "")

    if "multipart/form-data" not in content_type:
        return JSONResponse(
            {"error": "Content-Type must be multipart/form-data"},
            status_code=400
        )

    # Create parser with request headers
    parser = StreamingFormDataParser(headers={"Content-Type": content_type})

    # Create temporary file path for the upload
    temp_path = UPLOAD_DIR / f"upload_{os.urandom(8).hex()}.tmp"

    # Create targets for the file and optional description field
    file_target = SHA256FileTarget(str(temp_path))
    description_target = ValueTarget()

    # Register targets with their field names
    parser.register("file", file_target)
    parser.register("description", description_target)

    # Stream the request body and feed to parser
    async for chunk in request.stream():
        parser.data_received(chunk)

    # Check if we received a file
    if file_target.size == 0:
        # Clean up empty file
        if temp_path.exists():
            temp_path.unlink()
        return JSONResponse(
            {"error": "No file was uploaded"},
            status_code=400
        )

    # Get description if provided
    description = ""
    if description_target.value:
        description = description_target.value.decode("utf-8", errors="replace")

    return JSONResponse({
        "success": True,
        "file": {
            "original_filename": file_target.original_filename,
            "temp_path": str(temp_path),
            "size": file_target.size,
            "sha256": file_target.sha256,
        },
        "description": description,
    })


async def upload_multiple_files(request: Request) -> JSONResponse:
    """
    Handle streaming upload of multiple files.
    """
    content_type = request.headers.get("content-type", "")

    if "multipart/form-data" not in content_type:
        return JSONResponse(
            {"error": "Content-Type must be multipart/form-data"},
            status_code=400
        )

    parser = StreamingFormDataParser(headers={"Content-Type": content_type})

    # Support up to 10 files
    file_targets = []
    for i in range(10):
        temp_path = UPLOAD_DIR / f"upload_{os.urandom(8).hex()}.tmp"
        target = SHA256FileTarget(str(temp_path))
        file_targets.append((temp_path, target))
        parser.register(f"file{i}", target)
        # Also register without number for flexible naming
        if i == 0:
            parser.register("file", target)

    # Stream the request body
    async for chunk in request.stream():
        parser.data_received(chunk)

    # Collect results for files that were actually uploaded
    results = []
    for temp_path, target in file_targets:
        if target.size > 0:
            results.append({
                "original_filename": target.original_filename,
                "temp_path": str(temp_path),
                "size": target.size,
                "sha256": target.sha256,
            })
        elif temp_path.exists():
            # Clean up empty files
            temp_path.unlink()

    if not results:
        return JSONResponse(
            {"error": "No files were uploaded"},
            status_code=400
        )

    return JSONResponse({
        "success": True,
        "files": results,
        "count": len(results),
    })


async def home(request: Request) -> HTMLResponse:
    """Simple HTML form for testing uploads."""
    html = """
    <!DOCTYPE html>
    <html>
    <head>
        <title>Streaming File Upload Demo</title>
        <style>
            body { font-family: sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }
            .form-group { margin: 20px 0; }
            input, textarea { padding: 10px; margin: 5px 0; }
            button { padding: 10px 20px; background: #007bff; color: white; border: none; cursor: pointer; }
            button:hover { background: #0056b3; }
            pre { background: #f4f4f4; padding: 15px; overflow-x: auto; }
        </style>
    </head>
    <body>
        <h1>Streaming File Upload Demo</h1>
        <p>This demo uses the <code>streaming-form-data</code> library to stream uploads directly to disk.</p>

        <h2>Single File Upload</h2>
        <form action="/upload" method="post" enctype="multipart/form-data">
            <div class="form-group">
                <label>File:</label><br>
                <input type="file" name="file" required>
            </div>
            <div class="form-group">
                <label>Description (optional):</label><br>
                <textarea name="description" rows="3" cols="50"></textarea>
            </div>
            <button type="submit">Upload</button>
        </form>

        <h2>Multiple File Upload</h2>
        <form action="/upload-multiple" method="post" enctype="multipart/form-data">
            <div class="form-group">
                <label>Files:</label><br>
                <input type="file" name="file0" required><br>
                <input type="file" name="file1"><br>
                <input type="file" name="file2">
            </div>
            <button type="submit">Upload Multiple</button>
        </form>
    </body>
    </html>
    """
    return HTMLResponse(html)


async def health(request: Request) -> JSONResponse:
    """Health check endpoint."""
    return JSONResponse({"status": "ok"})


# Create the Starlette application
routes = [
    Route("/", home),
    Route("/health", health),
    Route("/upload", upload_file, methods=["POST"]),
    Route("/upload-multiple", upload_multiple_files, methods=["POST"]),
]

app = Starlette(routes=routes)


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
