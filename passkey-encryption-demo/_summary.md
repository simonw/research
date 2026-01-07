Leveraged via a minimal Next.js application, this project explores how the WebAuthn PRF extension can generate stable, hardware-backed keys for client-side data protection. By integrating HKDF to derive AES-GCM encryption keys from passkey outputs, the demo enables secure local encryption of text and files without ever exposing secrets to a server. The implementation serves as a functional proof-of-concept for zero-knowledge architectures, highlighting the seamless transition from biometric authentication to cryptographic operations within the browser.

* Registers resident passkeys specifically utilizing the WebAuthn PRF extension.
* Derives stable AES-GCM keys for local file and text encryption via HKDF.
* Operates entirely client-side to ensure no sensitive key material reaches the backend.

Key resources: [WebAuthn PRF Extension](https://w3c.github.io/webauthn/#prf-extension) and [Next.js Framework](https://nextjs.org/)
