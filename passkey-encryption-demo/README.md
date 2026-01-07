# Passkey Encryption Demo

This folder contains a minimal Next.js app that demonstrates the ideas in the
passkey encryption article (WebAuthn PRF-derived key material used for
client-side encryption/decryption).

## What it does

- Registers a resident passkey with the WebAuthn PRF extension.
- Uses PRF output to derive a stable AES-GCM key via HKDF.
- Encrypts/decrypts text and files locally in the browser.
- Includes native WebAuthn capability checks and a reset button to clear stored
  passkey metadata.

## Running the demo

```bash
npm install
npm run dev
```

Then open `http://localhost:3000` and:

1. Create/Register a passkey.
2. Derive the encryption key.
3. Encrypt/decrypt text or files.

## Notes

- PRF is not supported in all browsers or authenticators. Chrome on a recent
  platform authenticator is most likely to work.
- The encryption happens locally; no secrets are sent to a server in this demo.

## Files of interest

- `app/page.tsx` contains the passkey registration, key derivation, and
  encryption/decryption flows.
- `app/globals.css` provides the simple UI styling.
