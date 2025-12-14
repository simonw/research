# Streaming File Upload Prototype

A prototype demonstrating the [streaming-form-data](https://github.com/siddhantgoel/streaming-form-data) library for handling large file uploads in an asyncio ASGI application by streaming them directly to disk.

## Overview

Traditional file upload handling loads the entire file into memory before processing. This approach fails for large files (memory exhaustion) and is inefficient for moderate files. The `streaming-form-data` library solves this by parsing `multipart/form-data` incrementally, allowing each chunk to be written to disk as it arrives.

## Key Features

- **True streaming**: Files are written to disk as data arrives, never fully loaded into memory
- **Checksum calculation**: SHA256 is computed during streaming (no second pass needed)
- **Multiple file support**: Handle multiple files in a single request
- **ASGI compatible**: Works with Starlette, FastAPI, and other ASGI frameworks
- **High performance**: Cython-based parser for efficient parsing

## Architecture

```
┌─────────────────┐      ┌──────────────────────┐      ┌─────────────────┐
│  HTTP Request   │──────│  ASGI Application    │──────│  Temp File      │
│  (multipart)    │      │  (Starlette)         │      │  on Disk        │
└─────────────────┘      └──────────────────────┘      └─────────────────┘
                                   │
                         ┌─────────▼─────────┐
                         │  StreamingForm    │
                         │  DataParser       │
                         │  (Cython)         │
                         └───────────────────┘
```

## Files

| File | Description |
|------|-------------|
| `app.py` | ASGI application with upload endpoints |
| `test_app.py` | Comprehensive test suite (12 tests) |
| `notes.md` | Development notes and learnings |
| `README.md` | This documentation |

## Usage

### Installation

```bash
pip install streaming-form-data starlette uvicorn
```

### Running the Server

```bash
python app.py
# or
uvicorn app:app --host 0.0.0.0 --port 8000
```

Visit http://localhost:8000 for the upload form.

### API Endpoints

#### `POST /upload`
Upload a single file with optional description.

```bash
curl -X POST http://localhost:8000/upload \
  -F "file=@myfile.pdf" \
  -F "description=My document"
```

Response:
```json
{
  "success": true,
  "file": {
    "original_filename": "myfile.pdf",
    "temp_path": "/tmp/streaming_uploads/upload_abc123.tmp",
    "size": 1048576,
    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  },
  "description": "My document"
}
```

#### `POST /upload-multiple`
Upload multiple files.

```bash
curl -X POST http://localhost:8000/upload-multiple \
  -F "file0=@file1.txt" \
  -F "file1=@file2.pdf"
```

### Running Tests

```bash
pip install pytest pytest-asyncio httpx
pytest test_app.py -v
```

## How It Works

### 1. Request Streaming

The ASGI application uses Starlette's `request.stream()` to get the request body as an async iterator of chunks:

```python
async for chunk in request.stream():
    parser.data_received(chunk)
```

### 2. Incremental Parsing

The `StreamingFormDataParser` processes each chunk incrementally:

```python
from streaming_form_data import StreamingFormDataParser
from streaming_form_data.targets import FileTarget

parser = StreamingFormDataParser(headers={"Content-Type": content_type})
parser.register("file", FileTarget("/path/to/output"))

for chunk in chunks:
    parser.data_received(chunk)  # Data is written immediately
```

### 3. Custom Target for Checksum

A custom target extends `BaseTarget` to add checksum calculation:

```python
class SHA256FileTarget(BaseTarget):
    def on_start(self):
        self._file = open(self.filepath, 'wb')
        self._hasher = hashlib.sha256()

    def on_data_received(self, chunk: bytes):
        self._file.write(chunk)
        self._hasher.update(chunk)

    def on_finish(self):
        self._file.close()
```

## Test Results

```
test_app.py::test_health_endpoint PASSED
test_app.py::test_home_page PASSED
test_app.py::test_upload_small_file PASSED
test_app.py::test_upload_medium_file PASSED
test_app.py::test_upload_large_file PASSED
test_app.py::test_upload_with_description PASSED
test_app.py::test_upload_no_file PASSED
test_app.py::test_upload_wrong_content_type PASSED
test_app.py::test_upload_multiple_files PASSED
test_app.py::test_upload_preserves_binary_content PASSED
test_app.py::test_upload_unicode_filename PASSED
test_app.py::test_streaming_chunked_upload PASSED

12 passed in 0.49s
```

## Production Considerations

For production use, consider adding:

1. **File size limits**: Prevent denial-of-service via huge uploads
2. **Rate limiting**: Throttle upload requests per client
3. **Temp file cleanup**: Schedule cleanup of orphaned temp files
4. **Virus scanning**: Scan uploaded files before processing
5. **Storage backends**: Use S3Target or GCSTarget for cloud storage
6. **Progress tracking**: Report upload progress to clients

## Dependencies

- `streaming-form-data>=1.19.0` - Streaming multipart parser
- `starlette>=0.27.0` - ASGI framework
- `uvicorn>=0.23.0` - ASGI server (for running)
- `httpx>=0.24.0` - Async HTTP client (for testing)
- `pytest>=7.0.0` - Testing framework
- `pytest-asyncio>=0.21.0` - Async test support

## References

- [streaming-form-data GitHub](https://github.com/siddhantgoel/streaming-form-data)
- [streaming-form-data Documentation](https://streaming-form-data.readthedocs.io/)
- [Starlette](https://www.starlette.io/)
- [ASGI Specification](https://asgi.readthedocs.io/)
