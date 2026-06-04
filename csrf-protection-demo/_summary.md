Modern browser security now enables robust Cross-Site Request Forgery (CSRF) prevention without requiring tokens. This demo project contrasts a vulnerable FastAPI bank app with a protected version, showcasing how browser-sent headers like `Sec-Fetch-Site` and `Origin` empower servers to automatically reject cross-origin POST requests. By combining server middleware checks (as seen in [Filippo Valsorda's CSRF approach](https://words.filippo.io/csrf/)) with `SameSite` cookies, state-changing attacks are reliably blocked, while legitimate API requests (e.g., curl) still function. This paradigm shift simplifies protection, requiring only a single middleware for all endpoints and leveraging headers that cannot be spoofed by client JavaScript.

**Key findings:**
- CSRF tokens are obsolete for browsers supporting `Sec-Fetch-Site`; [Go 1.25 http.CrossOriginProtection](https://pkg.go.dev/net/http#CrossOriginProtection) implements this method.
- Protected endpoints combined with `SameSite` cookies block attacks even if one layer fails (“defense in depth”).
- API clients or cURL are unaffected, since CSRF is a browser-specific threat.
- Headers like `Sec-Fetch-Site` and `Origin` cannot be manipulated by malicious scripts within the browser.
