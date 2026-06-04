JavaScript running inside a `sandbox="allow-scripts"` iframe cannot escape or disable a `<meta http-equiv="Content-Security-Policy">` tag, even through removal, modification, or document replacement. Extensive testing across Chromium and Firefox confirmed that CSP policies defined via meta tags are enforced at parse time, and persist even when the iframe is navigated to a data: URI. While the sandbox attribute restricts capabilities, it does not block network requests on its own—only the CSP meta tag reliably prevents resource fetching and data exfiltration across browsers. Notably, Firefox ignores the `csp` iframe attribute, so the meta tag must always be used for security.

Key findings:
- JavaScript cannot remove or modify an enforced CSP meta tag; the policy remains active.
- `document.write()` and navigation to data: URI do not reset or bypass the original CSP.
- The `sandbox` attribute alone is insufficient for blocking network requests from untrusted code.
- The `csp` iframe attribute only works in Chromium, not Firefox ([Rodney](https://github.com/simonw/rodney) and [Playwright](https://playwright.dev/) used for automation and validation).
- The CSP meta tag must be the first element in `srcdoc` for guaranteed enforcement.
