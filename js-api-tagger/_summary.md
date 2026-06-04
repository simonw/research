Efficiently categorizing the 155 HTML tools in [simonw/tools](https://github.com/simonw/tools) by their JavaScript API usage, this project developed an automated pipeline combining [Cheerio](https://cheerio.js.org/) for HTML parsing and [Acorn](https://github.com/acornjs/acorn) for JavaScript AST analysis. The solution robustly filters out false positives from comments, strings, and non-code regions, accurately tagging over 60 Web APIs and handling modern ES modules and edge script types. Beyond API detection, the system analyzes external libraries, HTML structure, accessibility, interaction patterns, and data handling, providing multidimensional insight into each tool’s capabilities and design. Results show frequent use of APIs like Fetch, Clipboard, and localStorage, common libraries such as Pyodide and Marked, and a dominant pattern of utilities and file processors among the tools.

**Key findings:**
- Fetch API, Clipboard API, and localStorage are among the most widely used JS APIs.
- CDN sources like jsdelivr and cloudflare are common for library loading.
- Pyodide for Python, Marked for Markdown, and React for UI are notable detected libraries.
- Most tools fall into categories such as utility, clipboard handler, or file processor.
