# Streaming File Upload Prototype - Notes

## Objective
Build a prototype ASGI application using the `streaming-form-data` library to handle large file uploads by streaming them to temporary files on disk.

## Library Research

### streaming-form-data (v1.19.1)
- High-performance Cython-based parser for `multipart/form-data`
- Supports streaming processing of large files without loading into memory
- Key components:
  - `StreamingFormDataParser`: Main parser class, initialized with request headers
  - `FileTarget`: Streams data to an on-disk file
  - `ValueTarget`: Stores form field values in memory
  - `DirectoryTarget`: Streams to a directory with auto-generated filenames
  - Other targets: S3Target, GCSTarget, NullTarget, SHA256Target, etc.

### API Pattern
```python
from streaming_form_data import StreamingFormDataParser
from streaming_form_data.targets import FileTarget, ValueTarget

# Create parser with Content-Type header (contains boundary)
parser = StreamingFormDataParser(headers={"Content-Type": content_type})

# Register field handlers
parser.register("file", FileTarget("/path/to/output.bin"))
parser.register("description", ValueTarget())

# Stream data as it arrives
parser.data_received(chunk)
```

### Important Notes
- Parser needs the Content-Type header with boundary parameter
- `data_received()` is synchronous but fast (Cython)
- FileTarget can accept a callable for dynamic filename generation
- The library handles chunked data naturally

## Implementation Plan
1. Create Starlette ASGI app with upload endpoint
2. Use `request.stream()` to get async body chunks
3. Feed chunks to StreamingFormDataParser
4. Use FileTarget with tempfile for streaming to disk
5. Return upload metadata (filename, size, checksum)

## Dependencies
- streaming-form-data==1.19.1
- starlette (ASGI framework)
- uvicorn (ASGI server)
- httpx (async HTTP client for tests)
- pytest, pytest-asyncio (testing)

## Testing Strategy
- Test small file uploads
- Test large file uploads (verify streaming works)
- Test multiple files
- Verify file integrity with checksums
- Test error handling

## Implementation Results

### What Was Built
1. **SHA256FileTarget** - Custom target class that:
   - Writes incoming data to a file
   - Calculates SHA256 checksum as data streams through
   - Tracks file size
   - Captures original filename from multipart headers

2. **ASGI Application (Starlette)** with endpoints:
   - `GET /` - HTML upload form for testing
   - `GET /health` - Health check
   - `POST /upload` - Single file upload with optional description
   - `POST /upload-multiple` - Multiple file upload

3. **Comprehensive Test Suite** - 12 tests covering:
   - Small file uploads (bytes)
   - Medium file uploads (1 MB)
   - Large file uploads (10 MB)
   - Binary content preservation
   - File integrity via SHA256 verification
   - Multiple file uploads
   - Error handling (wrong content type, no file)
   - Unicode filenames

### Test Results
```
12 passed in 0.49s
```

All tests pass, confirming:
- Streaming works correctly for files of all sizes
- File integrity is maintained (checksums match)
- No memory issues with large files (verified up to 10 MB)
- Error cases are handled properly

### Key Learnings
1. The `streaming-form-data` parser is synchronous but very fast (Cython)
2. Integrating with async ASGI is seamless - just call `parser.data_received(chunk)` in the async loop
3. Custom targets are easy to create by extending `BaseTarget`
4. The `set_multipart_filename()` method is called automatically by the parser to provide the original filename
5. For production, you'd want to add file size limits, rate limiting, and proper cleanup of temp files
