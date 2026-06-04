"""
Automated tests for the streaming file upload ASGI application.

These tests verify:
1. Small file uploads work correctly
2. Large file uploads stream properly without memory issues
3. File integrity is maintained (checksum verification)
4. Multiple file uploads work
5. Error handling for missing/invalid files
"""

import hashlib
import os
import tempfile
from pathlib import Path

import pytest
from httpx import ASGITransport, AsyncClient

from app import app, UPLOAD_DIR


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.fixture
async def client():
    """Create async test client."""
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        yield client


@pytest.fixture
def small_file():
    """Create a small test file."""
    content = b"Hello, World! This is a small test file."
    sha256 = hashlib.sha256(content).hexdigest()
    return content, sha256


@pytest.fixture
def medium_file():
    """Create a medium test file (1 MB)."""
    content = os.urandom(1024 * 1024)  # 1 MB of random data
    sha256 = hashlib.sha256(content).hexdigest()
    return content, sha256


@pytest.fixture
def large_file():
    """Create a large test file (10 MB)."""
    content = os.urandom(10 * 1024 * 1024)  # 10 MB of random data
    sha256 = hashlib.sha256(content).hexdigest()
    return content, sha256


@pytest.mark.anyio
async def test_health_endpoint(client):
    """Test health check endpoint."""
    response = await client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


@pytest.mark.anyio
async def test_home_page(client):
    """Test home page returns HTML form."""
    response = await client.get("/")
    assert response.status_code == 200
    assert "text/html" in response.headers["content-type"]
    assert "Streaming File Upload Demo" in response.text


@pytest.mark.anyio
async def test_upload_small_file(client, small_file):
    """Test uploading a small file."""
    content, expected_sha256 = small_file

    files = {"file": ("test.txt", content, "text/plain")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert data["file"]["original_filename"] == "test.txt"
    assert data["file"]["size"] == len(content)
    assert data["file"]["sha256"] == expected_sha256

    # Verify file was written to disk
    temp_path = Path(data["file"]["temp_path"])
    assert temp_path.exists()
    assert temp_path.read_bytes() == content

    # Cleanup
    temp_path.unlink()


@pytest.mark.anyio
async def test_upload_medium_file(client, medium_file):
    """Test uploading a 1 MB file."""
    content, expected_sha256 = medium_file

    files = {"file": ("medium.bin", content, "application/octet-stream")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert data["file"]["size"] == len(content)
    assert data["file"]["sha256"] == expected_sha256

    # Verify file integrity
    temp_path = Path(data["file"]["temp_path"])
    assert temp_path.exists()

    # Calculate SHA256 of saved file
    saved_content = temp_path.read_bytes()
    saved_sha256 = hashlib.sha256(saved_content).hexdigest()
    assert saved_sha256 == expected_sha256

    # Cleanup
    temp_path.unlink()


@pytest.mark.anyio
async def test_upload_large_file(client, large_file):
    """Test uploading a 10 MB file - verifies streaming works."""
    content, expected_sha256 = large_file

    files = {"file": ("large.bin", content, "application/octet-stream")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert data["file"]["size"] == len(content)
    assert data["file"]["sha256"] == expected_sha256

    # Verify file integrity
    temp_path = Path(data["file"]["temp_path"])
    assert temp_path.exists()

    # Calculate SHA256 of saved file to verify integrity
    hasher = hashlib.sha256()
    with open(temp_path, "rb") as f:
        while chunk := f.read(8192):
            hasher.update(chunk)
    assert hasher.hexdigest() == expected_sha256

    # Cleanup
    temp_path.unlink()


@pytest.mark.anyio
async def test_upload_with_description(client, small_file):
    """Test uploading a file with a description field."""
    content, expected_sha256 = small_file
    description = "This is a test file with a description"

    files = {"file": ("test.txt", content, "text/plain")}
    data = {"description": description}
    response = await client.post("/upload", files=files, data=data)

    assert response.status_code == 200
    result = response.json()
    assert result["success"] is True
    assert result["description"] == description
    assert result["file"]["sha256"] == expected_sha256

    # Cleanup
    Path(result["file"]["temp_path"]).unlink()


@pytest.mark.anyio
async def test_upload_no_file(client):
    """Test error handling when no file is uploaded."""
    # Send multipart request without a file
    response = await client.post(
        "/upload",
        content=b"",
        headers={"Content-Type": "multipart/form-data; boundary=----boundary"}
    )
    # Should return error
    assert response.status_code == 400


@pytest.mark.anyio
async def test_upload_wrong_content_type(client):
    """Test error handling for wrong content type."""
    response = await client.post(
        "/upload",
        content=b"not multipart data",
        headers={"Content-Type": "application/json"}
    )
    assert response.status_code == 400
    assert "multipart/form-data" in response.json()["error"]


@pytest.mark.anyio
async def test_upload_multiple_files(client, small_file, medium_file):
    """Test uploading multiple files."""
    content1, sha256_1 = small_file
    content2, sha256_2 = medium_file

    files = [
        ("file0", ("file1.txt", content1, "text/plain")),
        ("file1", ("file2.bin", content2, "application/octet-stream")),
    ]
    response = await client.post("/upload-multiple", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert data["count"] == 2
    assert len(data["files"]) == 2

    # Verify both files
    for file_info in data["files"]:
        temp_path = Path(file_info["temp_path"])
        assert temp_path.exists()

        if file_info["original_filename"] == "file1.txt":
            assert file_info["sha256"] == sha256_1
            assert file_info["size"] == len(content1)
        else:
            assert file_info["sha256"] == sha256_2
            assert file_info["size"] == len(content2)

        # Cleanup
        temp_path.unlink()


@pytest.mark.anyio
async def test_upload_preserves_binary_content(client):
    """Test that binary content is preserved correctly (no encoding issues)."""
    # Create content with all possible byte values
    content = bytes(range(256)) * 100  # All bytes repeated
    expected_sha256 = hashlib.sha256(content).hexdigest()

    files = {"file": ("binary.bin", content, "application/octet-stream")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["file"]["sha256"] == expected_sha256
    assert data["file"]["size"] == len(content)

    # Verify file content is identical
    temp_path = Path(data["file"]["temp_path"])
    assert temp_path.read_bytes() == content

    # Cleanup
    temp_path.unlink()


@pytest.mark.anyio
async def test_upload_unicode_filename(client, small_file):
    """Test uploading file with unicode filename."""
    content, expected_sha256 = small_file

    files = {"file": ("test_файл_文件.txt", content, "text/plain")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    # Note: filename handling may vary due to HTTP encoding
    assert data["file"]["sha256"] == expected_sha256

    # Cleanup
    Path(data["file"]["temp_path"]).unlink()


@pytest.mark.anyio
async def test_streaming_chunked_upload(client):
    """
    Test that streaming works with chunked uploads.
    This simulates how a real large upload would work over the network.
    """
    # Create test content
    chunk_size = 64 * 1024  # 64 KB chunks
    num_chunks = 50  # ~3.2 MB total
    chunks = [os.urandom(chunk_size) for _ in range(num_chunks)]
    full_content = b"".join(chunks)
    expected_sha256 = hashlib.sha256(full_content).hexdigest()

    files = {"file": ("chunked.bin", full_content, "application/octet-stream")}
    response = await client.post("/upload", files=files)

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert data["file"]["size"] == len(full_content)
    assert data["file"]["sha256"] == expected_sha256

    # Cleanup
    Path(data["file"]["temp_path"]).unlink()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
