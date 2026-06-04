By leveraging HTTP Range requests and fixed-width binary records, Unicode Explorer demonstrates efficient binary search for Unicode data directly from a static file with zero backend or dependencies. The client fetches only one 256-byte record per step, using signposts from `meta.json` to optimize initial narrowing, then performs real-time network-driven binary search, visualized in an interactive log. Each search transfers minimal data and never loads the full 76MB file, showcasing how indexed, record-based search can work entirely over HTTP. The project is available as a live demo and its code can be explored [here](https://github.com/paulgb/unicode-explorer).

**Key Findings:**
- Efficient binary search over large static files is possible using Range requests and fixed-width records.
- Signpost sampling speeds up the initial narrowing of the search range, reducing network requests.
- No server, database, or external JS dependencies are required; the complete search happens in-browser over HTTP.
