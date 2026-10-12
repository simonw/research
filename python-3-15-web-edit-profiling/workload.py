"""Synthetic editing session over real HTTP; never imported by the server."""

import hashlib
import json
import time
from collections import Counter, defaultdict
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import ProxyHandler, Request, build_opener


def seed(directory: Path, lines: int, files: int):
    directory.mkdir()
    (directory / "documents").mkdir()
    (directory / "uploads").mkdir()
    for index in range(files):
        (directory / "documents" / f"note-{index:04d}.txt").write_text(
            f"Demo document {index}.\nLocal generated fixture for web-edit.\n"
        )
    # Unique lines give difflib realistic matching work, with mixed line endings.
    with (directory / "article.txt").open("wb") as stream:
        for index in range(lines):
            ending = "\r\n" if index % 3 else "\n"
            stream.write(
                f"Section {index:05d}: An editable paragraph about Python profiling.{ending}".encode()
            )


class Session:
    def __init__(self, port: int):
        self.base = f"http://127.0.0.1:{port}"
        # Ignore system HTTP proxy settings for this loopback-only workload.
        self.opener = build_opener(ProxyHandler({}))
        self.counts = Counter()
        self.elapsed = defaultdict(float)
        self.received_bytes = 0
        self.cycle_number = 0

    def request(
        self,
        label,
        method,
        path,
        *,
        query=None,
        body=None,
        content_type=None,
        status=200,
    ):
        if query:
            path += "?" + urlencode(query)
        headers = {"X-Web-Edit": "1"}
        if content_type:
            headers["Content-Type"] = content_type
        started = time.perf_counter()
        with self.opener.open(
            Request(self.base + path, data=body, headers=headers, method=method),
            timeout=30,
        ) as response:
            data = response.read()
            if response.status != status:
                raise RuntimeError(
                    f"{label}: expected HTTP {status}, got {response.status}"
                )
        self.counts[label] += 1
        self.elapsed[label] += time.perf_counter() - started
        self.received_bytes += len(data)
        return data

    def cycle(self):
        self.cycle_number += 1
        number = self.cycle_number
        self.request("browse root", "GET", "/api/browse")
        listing = json.loads(
            self.request(
                "browse documents", "GET", "/api/browse", query={"path": "documents"}
            )
        )
        if not listing["entries"]:
            raise RuntimeError("Directory fixture is empty")
        document = json.loads(
            self.request("read text", "GET", "/api/text", query={"path": "article.txt"})
        )
        lines = document["content"].splitlines()
        for index in range(0, len(lines), 500):
            lines[index] = f"Section {index:05d}: Changed in editing pass {number:06d}."
        content = "\n".join(lines) + "\n"
        saved = json.loads(
            self.request(
                "save + preserve endings",
                "PUT",
                "/api/text",
                body=json.dumps(
                    {
                        "path": "article.txt",
                        "content": content,
                        "version": document["version"],
                    }
                ).encode(),
                content_type="application/json",
            )
        )
        downloaded = self.request(
            "download + verify", "GET", "/api/download", query={"path": "article.txt"}
        )
        if hashlib.sha256(downloaded).hexdigest() != saved["version"]:
            raise RuntimeError("Downloaded content does not match the saved version")
        if downloaded.decode().replace("\r\n", "\n") != content:
            raise RuntimeError("Save/read round trip changed the edited content")
        if b"\r\n" not in downloaded:
            raise RuntimeError("Expected preserved CRLF line endings")
        name = f"uploads/pass-{number:06d}.txt"
        boundary = "web-edit-demo-boundary-67951"
        payload = (
            (
                f'--{boundary}\r\nContent-Disposition: form-data; name="path"\r\n\r\n{name}\r\n'
                f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="upload.txt"\r\n'
                "Content-Type: text/plain\r\n\r\n"
            ).encode()
            + (f"Upload for editing pass {number}.\n".encode() * 2000)
            + f"\r\n--{boundary}--\r\n".encode()
        )
        self.request(
            "multipart upload",
            "POST",
            "/api/upload",
            body=payload,
            content_type=f"multipart/form-data; boundary={boundary}",
            status=201,
        )
        listing = json.loads(
            self.request(
                "browse uploads", "GET", "/api/browse", query={"path": "uploads"}
            )
        )
        entry = next(item for item in listing["entries"] if item["path"] == name)
        deleted = json.loads(
            self.request(
                "delete upload",
                "DELETE",
                "/api/file",
                query={
                    "path": name,
                    "kind": "file",
                    "version": entry["delete_version"],
                },
            )
        )
        if deleted.get("deleted") is not True:
            raise RuntimeError("Upload deletion failed")

    def summary(self):
        return {
            "cycles": self.cycle_number,
            "requests": sum(self.counts.values()),
            "received_bytes": self.received_bytes,
            "operations": [
                {
                    "operation": name,
                    "requests": count,
                    "mean_http_ms": round(self.elapsed[name] * 1000 / count, 3),
                }
                for name, count in self.counts.items()
            ],
        }
