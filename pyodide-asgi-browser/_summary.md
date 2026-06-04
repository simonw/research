By running Python ASGI web applications entirely in the browser using [Pyodide](https://pyodide.org/) and a dedicated service worker, this project intercepts all same-origin requests under `/app/` and executes them against the Python app via the ASGI protocol—removing the need for a backend server except for static files. The mechanism is demonstrated with both a FastAPI demo and the full [Datasette](https://datasette.io/) app, confirming its generality across ASGI apps. The design leverages a shell page that manages a persistent Pyodide Web Worker, with requests brokered from the service worker to Python. Thorough testing includes unit and browser tests, all passing, and offline operation is ensured by vendoring Pyodide and wheels locally. 

Key findings:
- Intercepted navigations, forms, and fetches are answered in-browser with Python ASGI, supporting redirects, JSON APIs, and full SQL interaction.
- The bridge works with both FastAPI and Datasette—proving the approach is not framework-specific.
- Fulfills complex requirements like authenticated root sessions, mitigating frame-busting headers and handling app-specific URL quirks.
- All 27 unit and browser tests pass; the system is fully offline-capable and restores navigable, bookmarkable URLs.
- See source and demos for implementation details and test harnesses.
