# How Video Support Works in kimi-cli

Investigation of video support in [kimi-cli](https://github.com/MoonshotAI/kimi-cli), Moonshot AI's terminal-based AI coding agent.

## Overview

Kimi CLI supports sending video files to LLMs as part of a conversation. The video support is built across several architectural layers:

1. **File type detection** — identifying video files by extension and magic bytes
2. **ReadMediaFile tool** — the agent tool that reads and processes video files
3. **Upload/encoding** — provider-specific handling (Kimi API upload vs. base64 data URIs)
4. **Content model** — the `VideoURLPart` type that represents video in messages
5. **Capability gating** — ensuring only models that support video receive video content

## Architecture

```
User mentions video file path
        │
        ▼
┌─────────────────────┐
│   ReadMediaFile     │  (src/kimi_cli/tools/file/read_media.py)
│   Tool              │
│                     │
│  1. Resolve path    │
│  2. Validate size   │
│     (≤100 MB)       │
│  3. Detect type     │
│  4. Check caps      │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│  detect_file_type() │  (src/kimi_cli/tools/file/utils.py)
│                     │
│  Suffix → MIME map  │
│  Magic byte sniff   │
│  Returns FileType   │
│  (kind + mime_type) │
└────────┬────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│            Provider-specific path           │
│                                             │
│  ┌─ Kimi provider ──────────────────────┐   │
│  │  POST /files (multipart/form-data)   │   │
│  │  purpose="video"                     │   │
│  │  Returns ms://{file_id}              │   │
│  └──────────────────────────────────────┘   │
│                                             │
│  ┌─ Other providers ────────────────────┐   │
│  │  Base64 encode → data:video/mp4;...  │   │
│  └──────────────────────────────────────┘   │
└────────┬────────────────────────────────────┘
         │
         ▼
┌─────────────────────┐
│  VideoURLPart       │  (packages/kosong/src/kosong/message.py)
│  wrapped in XML:    │
│  <video path="..."> │  (src/kimi_cli/utils/media_tags.py)
│    [VideoURLPart]   │
│  </video>           │
└────────┬────────────┘
         │
         ▼
   Added to message
   content → sent to LLM
```

## Detailed Component Analysis

### 1. File Type Detection

**File:** `src/kimi_cli/tools/file/utils.py`

Detection uses a two-layer approach:

**Layer 1 — Extension-based (fast path):**

Hardcoded suffix-to-MIME mapping for video formats:

| Extension | MIME Type |
|-----------|-----------|
| `.mp4` | `video/mp4` |
| `.mkv` | `video/x-matroska` |
| `.avi` | `video/x-msvideo` |
| `.mov` | `video/quicktime` |
| `.wmv` | `video/x-ms-wmv` |
| `.webm` | `video/webm` |
| `.m4v` | `video/x-m4v` |
| `.flv` | `video/x-flv` |
| `.3gp` | `video/3gpp` |
| `.3g2` | `video/3gpp2` |

If the suffix matches an image or video mapping, that result is returned immediately without reading file contents.

**Layer 2 — Magic byte sniffing (fallback):**

When no suffix match is found, the first 512 bytes (`MEDIA_SNIFF_BYTES`) are examined:

- **FTYP box** (ISO base media format): Reads bytes 8-12 for the brand string. Recognizes brands: `isom`, `iso2`, `iso5`, `mp41`, `mp42`, `avc1`, `mp4v`, `m4v`, `qt`, `3gp4`-`3gp7`, `3g2`. Also distinguishes HEIC/AVIF image brands from video brands.
- **RIFF container**: Checks for `RIFF` magic + `AVI ` chunk type at offset 8.
- **EBML container**: Detects `\x1a\x45\xdf\xa3` magic, then searches the header for `"webm"` or `"matroska"` to distinguish WebM from MKV.
- **FLV**: Looks for `FLV` magic bytes at the start.
- **ASF/WMV**: Matches the 16-byte ASF header GUID.

**Notable edge case:** TypeScript `.ts` files are explicitly overridden to `text/typescript` in `_EXTRA_MIME_TYPES` to prevent Python's `mimetypes` module from classifying them as `video/mp2t` (MPEG Transport Stream).

### 2. ReadMediaFile Tool

**File:** `src/kimi_cli/tools/file/read_media.py`

This is a callable tool (`CallableTool2`) exposed to the LLM agent. Key behaviors:

- **Initialization gate:** The tool is not registered at all (`SkipThisTool` raised) if the model has neither `image_in` nor `video_in` capability.
- **Path validation:** Ensures paths outside the working directory must be absolute.
- **Size limit:** Rejects files > 100 MB.
- **Type routing:** After detecting the file type, routes to either image or video handling.
- **Capability enforcement:** Even if the tool is registered, reading a video file with a model that only has `image_in` returns an error.

**Video reading flow:**
1. Read entire file bytes into memory
2. If using the Kimi provider → upload via `KimiFiles.upload_video()`
3. Otherwise → base64-encode as a `data:` URI
4. Wrap the resulting `VideoURLPart` in XML-like tags via `wrap_media_part()`
5. Return as `ToolOk` with a descriptive message

### 3. Video Upload (Kimi Provider)

**File:** `packages/kosong/src/kosong/chat_provider/kimi.py`

The `KimiFiles` class handles server-side video processing:

```python
class KimiFiles:
    async def upload_video(self, *, data: bytes, mime_type: str) -> VideoURLPart:
        url = await self._upload_file(data=data, mime_type=mime_type, purpose="video")
        return VideoURLPart(video_url=VideoURLPart.VideoURL(url=url))

    async def _upload_file(self, *, data: bytes, mime_type: str, purpose: str) -> str:
        filename = _guess_filename(mime_type)  # e.g., "upload.mp4"
        files = {"file": (filename, data, mime_type)}
        response = await self._client.post(
            "/files",
            body={"purpose": purpose},
            files=files,
            options={"headers": {"Content-Type": "multipart/form-data"}},
        )
        return f"ms://{response.id}"
```

- Uses the OpenAI-compatible client (`AsyncOpenAI`) to POST to `/files`
- The `purpose` field is set to `"video"`
- Returns a `ms://` URI scheme (Moonshot internal file reference)
- The `KimiFileObject` response model only extracts the `id` field
- The filename is guessed from the MIME type (e.g., `video/mp4` → `upload.mp4`)

For non-Kimi providers, videos are instead base64-encoded as data URIs inline.

### 4. Content Model (VideoURLPart)

**File:** `packages/kosong/src/kosong/message.py`

```python
class VideoURLPart(ContentPart):
    class VideoURL(BaseModel):
        url: str       # "ms://file_id" or "data:video/mp4;base64,..."
        id: str | None = None

    type: str = "video_url"
    video_url: VideoURL
```

- Extends abstract `ContentPart` base class
- Serializes as `{"type": "video_url", "video_url": {"url": "...", "id": null}}`
- The `id` field allows distinguishing multiple videos in the same message
- Part of the unified content model alongside `TextPart`, `ThinkPart`, `ImageURLPart`, `AudioURLPart`

### 5. Media Tag Wrapping

**File:** `src/kimi_cli/utils/media_tags.py`

Videos are wrapped in XML-like tags to provide context to the LLM:

```python
def wrap_media_part(part, *, tag, attrs):
    return [
        TextPart(text='<video path="/path/to/video.mp4">'),
        part,  # VideoURLPart
        TextPart(text='</video>'),
    ]
```

This produces a 3-element list of `ContentPart` objects. The XML tags tell the LLM where the file came from.

### 6. Capability Gating

**Files:** `src/kimi_cli/llm.py`, `src/kimi_cli/auth/platforms.py`, `src/kimi_cli/soul/message.py`

Video support is gated at three levels:

1. **Model info from API** (`platforms.py:16-24`): The `/models` API endpoint returns `supports_video_in: bool` per model. For Kimi K2.5 models, `video_in` is force-enabled.

2. **Capability derivation** (`llm.py:239-247`): The `derive_model_capabilities()` function sets capabilities from model config. Models named `kimi-for-coding` or `kimi-code` automatically receive `image_in` and `video_in`.

3. **Runtime validation** (`soul/message.py:64-76`): Before sending a message, `check_message()` scans content parts. If a `VideoURLPart` is found and the model lacks `video_in`, it reports missing capabilities.

Capabilities can also be overridden via the `KIMI_MODEL_CAPABILITIES` environment variable (comma-separated).

### 7. MCP Integration

**File:** `packages/kosong/src/kosong/tooling/mcp.py`

When MCP (Model Context Protocol) tools return results with `video/*` MIME types, they are automatically converted to `VideoURLPart`:

- **Embedded blobs**: `data:{mimeType};base64,{blob}` data URIs
- **URI references**: Direct URL passthrough

### 8. UI Display

**File:** `src/kimi_cli/ui/shell/debug.py`

Videos are displayed in the terminal debug view as:
```
[Video] ms://abc123def456... (blue text)
```
URLs longer than 80 characters are truncated with `...`.

## Supported Models

Video input is supported on:
- `kimi-for-coding` / `kimi-code` (automatically enabled)
- `kimi-k2.5` family models (detected from model name and API response)
- Any model where the API returns `supports_video_in: true`
- Any model with `KIMI_MODEL_CAPABILITIES=video_in` env var set

## Supported Video Formats

| Format | Extensions | Detection Method |
|--------|-----------|-----------------|
| MP4 | `.mp4`, `.m4v` | Suffix + FTYP brands |
| WebM | `.webm` | Suffix + EBML header |
| Matroska | `.mkv` | Suffix + EBML header |
| AVI | `.avi` | Suffix + RIFF magic |
| QuickTime | `.mov` | Suffix + FTYP `qt` brand |
| WMV | `.wmv` | Suffix + ASF header |
| FLV | `.flv` | Suffix + FLV magic |
| 3GPP | `.3gp`, `.3g2` | Suffix + FTYP brands |

## Commit History

Video support was added incrementally over several PRs:

| Commit | Description |
|--------|-------------|
| `6a293ef` | Initial video content type support (`VideoURLPart`) |
| `8d8f5f2` | Support reading image/video files in ReadFile tool |
| `4f67871` | Fix TypeScript files misidentified as video |
| `dc858ef` | Add video upload support for Kimi provider (`/files` API) |
| `69c48a5` | Add video upload to kimi-sdk |
| `628afeb` | Introduce dedicated `ReadMediaFile` tool |
| `ec15b61` | Fix MP4 files not recognized (added `iso5` FTYP brand) |
| `43147dc` | Use file path as media ID in XML tags |
