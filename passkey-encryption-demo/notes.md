# Notes

- Created new research folder.
- Plan: build a minimal Next.js demo that derives encryption keys from passkeys using WebAuthn PRF if available, then encrypt/decrypt strings/files.

- Implemented a minimal Next.js app with WebAuthn PRF-based key derivation and AES-GCM encryption/decryption flows.
- Added text and file encryption UI plus basic status logging.
- Used @simplewebauthn/browser to check WebAuthn/platform authenticator support.

- Attempted npm install to run the app for a screenshot, but npm registry access returned 403 Forbidden.

- Attempted to fetch the article directly and via proxy helpers, but received 403 responses from confer.to (proxy blocked), so could not read the article content from this environment.
- Removed the @simplewebauthn/browser dependency to avoid blocked package install; replaced checks with native WebAuthn feature detection.
- Added UI to clear stored passkey metadata and surfaced PRF extension enablement status on registration.

- npm install still failed with 403 (registry blocked for @types/react), so I could not run the dev server or capture a screenshot.
