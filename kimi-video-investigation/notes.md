# Kimi CLI Video Support Investigation - Notes

## Goal
Investigate how video support works in kimi-cli (https://github.com/MoonshotAI/kimi-cli)

## Steps Taken

### 1. Cloned the repo
- Cloned to /tmp/kimi-cli
- Python 3.12+ project, uses UV build system, typer CLI framework

### 2. Explored codebase structure
- Main source: `src/kimi_cli/`
- Chat provider abstraction layer: `packages/kosong/`
- SDK: `sdks/kimi-sdk/`

### 3. Found video-related files
Key files for video support:

| File | Role |
|------|------|
| `src/kimi_cli/tools/file/read_media.py` | ReadMediaFile tool — reads image/video files and produces content parts |
| `src/kimi_cli/tools/file/utils.py` | File type detection: suffix mapping, MIME types, magic byte sniffing |
| `packages/kosong/src/kosong/message.py` | `VideoURLPart` content part type definition |
| `packages/kosong/src/kosong/chat_provider/kimi.py` | `KimiFiles.upload_video()` — uploads video to Kimi's `/files` API |
| `src/kimi_cli/utils/media_tags.py` | Wraps media parts with XML-like `<video>` tags |
| `src/kimi_cli/llm.py` | Model capabilities including `video_in`; `derive_model_capabilities()` |
| `src/kimi_cli/auth/platforms.py` | `ModelInfo.supports_video_in` from API; capability derivation |
| `src/kimi_cli/soul/message.py` | `check_message()` validates `VideoURLPart` requires `video_in` capability |
| `src/kimi_cli/ui/shell/debug.py` | Terminal display: `[Video] <url>` in blue |
| `src/kimi_cli/wire/types.py` | Re-exports `VideoURLPart` for wire protocol |
| `packages/kosong/src/kosong/tooling/mcp.py` | MCP tool integration handles `video/*` MIME types |

### 4. Video-related git history
```
ec15b61 fix(tools): fix some MP4 files not being recognized as videos (#694)
43147dc feat(tools): use file path as media ID for image and video parts (#693)
628afeb feat(tools): add ReadMediaFile tool for image/video files (#661)
69c48a5 feat(kimi-sdk): add video upload functionality and add example (#660)
dc858ef feat(kosong,kimi): add video upload support for Kimi provider (#656)
4f67871 fix(tool): prevent TypeScript files from being misidentified as video (#602)
8d8f5f2 feat(tool): support reading image and video files in ReadFile tool (#596)
6a293ef feat(video): add video content support (#552)
```

### 5. Key findings

#### File type detection (utils.py)
- Two-layer detection: suffix-based first (fast path), then magic byte sniffing as fallback
- Suffix mapping covers: mp4, mkv, avi, mov, wmv, webm, m4v, flv, 3gp, 3g2
- Magic byte sniffing handles:
  - FTYP box brands for MP4-family (isom, iso2, iso5, mp41, mp42, avc1, mp4v, m4v, qt, 3gp*, 3g2)
  - RIFF+AVI signature
  - EBML container: distinguishes WebM from Matroska by looking for "webm" or "matroska" in header
  - FLV signature
  - ASF header (for WMV)
- Special fix: TypeScript `.ts` files explicitly mapped to `text/typescript` to avoid misidentification as MPEG Transport Stream (`video/mp2t`)

#### Upload mechanism (kimi.py)
- For Kimi provider: video bytes uploaded via POST to `/files` endpoint with `purpose="video"` and `multipart/form-data`
- Returns `ms://{file_id}` URI scheme (Moonshot internal file reference)
- Result wrapped in `VideoURLPart(video_url=VideoURLPart.VideoURL(url="ms://..."))`
- For non-Kimi providers: video base64-encoded as `data:video/mp4;base64,...` data URI

#### Content model (message.py)
- `VideoURLPart` is a `ContentPart` subclass with `type = "video_url"`
- Contains nested `VideoURL` model with `url` (str) and optional `id` (str|None)
- Serializes as `{"type": "video_url", "video_url": {"url": "...", "id": null}}`

#### Capability gating
- `ModelCapability` type includes `"video_in"` literal
- `kimi-for-coding` and `kimi-code` models automatically get `video_in`
- `kimi-k2.5` models also get `video_in` via platform model info
- ReadMediaFile tool skipped entirely if model lacks both `image_in` and `video_in`
- Individual file reads gated: video file + no `video_in` = error to user
- `check_message()` validates messages before sending to LLM

#### Media wrapping (media_tags.py)
- Video parts wrapped in XML-like tags: `<video path="/path/to/file.mp4">` [VideoURLPart] `</video>`
- This provides the LLM with context about the file path

#### MCP integration (mcp.py)
- MCP tool results with `video/*` MIME types converted to `VideoURLPart`
- Supports both embedded blobs (base64) and URI references

### 6. Max file size
- 100MB limit per video file (configurable via `MAX_MEDIA_MEGABYTES` constant)

### 7. Test coverage
- `tests/tools/test_read_media_file.py`: Unit tests for ReadMediaFile tool (read video, capability checks)
- `tests/e2e/test_media_e2e.py`: E2E tests in both print and wire modes using scripted echo provider
- `tests/utils/test_file_utils.py`: File type detection tests for suffixes and magic bytes
