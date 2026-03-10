This system implements an end-to-end encrypted web scraping architecture that terminates TLS directly within the user's browser using WebAssembly. By leveraging [Epoxy-TLS](https://github.com/mercuryworkshop/epoxy-transport) and the Wisp protocol, the backend acts as a blind TCP relay, ensuring that plaintext traffic and session cookies remain invisible to the server. Users can intercept live API data and perform DOM automation through an iframe-based proxy powered by the [Ultraviolet](https://github.com/titaniumnetwork-dev/Ultraviolet) service worker.

*   TLS termination occurs in-browser via Rustls, preventing the relay server from inspecting or tampering with traffic.
*   The framework supports dynamic GraphQL interception and DOM manipulation via a built-in puppet agent and custom connector scripts.
*   Current implementation is susceptible to anti-bot detection because Rustls produces a distinct JA3 TLS fingerprint that differs from standard browsers.
*   The client-side requirements include specific COEP/COOP headers to enable SharedArrayBuffer support for the WASM modules.
