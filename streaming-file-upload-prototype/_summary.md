Demonstrating efficient large file uploads, this prototype integrates the [streaming-form-data](https://github.com/siddhantgoel/streaming-form-data) library with a Starlette-based ASGI server to enable true streaming of multipart file data directly to disk, bypassing memory bottlenecks. It incrementally parses incoming form data and supports checksum calculation on-the-fly, handling multiple simultaneous file uploads via async workflows. The included test suite validates robust performance across scenarios including large files, chunked uploads, and multiple files. This architecture makes file handling scalable for production environments, with extensibility for further enhancements such as file size limits and external storage targets.

**Key Findings:**
- Streaming upload prevents memory exhaustion and improves performance for large files.
- Checksum is computed during upload—eliminating the need for a post-upload scan.
- Multiple files can be uploaded in one request; all endpoints are fully ASGI compatible.
- See [streaming-form-data documentation](https://streaming-form-data.readthedocs.io/) for further integration options.
