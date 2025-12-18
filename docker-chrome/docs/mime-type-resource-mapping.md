# MIME Type to Resource Type Mapping

## Purpose
Established a standard mapping from HTTP `Content-Type` headers (MIME types) to user-friendly "Resource Types" (e.g., Script, Image, XHR) for the Docker Chrome network analysis tools.

## Key Findings

### Mapping Logic
*   **Standard:** Align with Chrome DevTools Protocol `ResourceType` values where possible.
*   **Normalization:** Always lowercase and strip parameters (e.g., `; charset=utf-8`) before matching.
*   **Hierarchy:**
    1.  **Exact Match:** (e.g., `text/html` -> `Document`)
    2.  **Prefix Match:** (e.g., `image/*` -> `Image`, `video/*` -> `Media`)
    3.  **Compound Match:** (e.g., `application/vnd.api+json` -> `Script` via `json` check)
    4.  **Fallback:** `Other`

### Critical Distinctions
*   **XHR vs. Fetch:** MIME types **cannot** distinguish these. This requires initiator context from the CDP `Network.requestWillBeSent` event.
*   **JSON:** Typically classified as `Script` or `XHR` depending on context, not a standalone type.
*   **Compound Types:** `image/svg+xml` should be treated as `Image`, not `Document`.

## Standard Mapping Table (Excerpt)
| Category | MIME Types |
|----------|------------|
| **Document** | `text/html`, `application/xhtml+xml` |
| **Script** | `application/javascript`, `application/json` |
| **Stylesheet** | `text/css` |
| **Font** | `font/*`, `application/vnd.ms-fontobject` |
| **Image** | `image/*` |
| **Media** | `video/*`, `audio/*` |

## Output
A TypeScript implementation `getResourceTypeFromMimeType` was designed based on these findings.
