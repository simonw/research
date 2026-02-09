# Research Notes: Go Rod Library for Chrome Automation

## Sources Consulted

1. **GitHub README**: https://github.com/go-rod/rod (fetched raw from main branch)
2. **Documentation site**: https://go-rod.github.io/#/ (SPA, did not render via fetch; fetched raw markdown from https://github.com/go-rod/go-rod.github.io instead)
3. **pkg.go.dev API docs**: https://pkg.go.dev/github.com/go-rod/rod (comprehensive type/method listing)
4. **examples_test.go**: Fetched directly from GitHub raw content

## Key Findings

### Documentation Site is a Docsify SPA
- The go-rod.github.io site uses Docsify (client-side JS rendering), so fetching the HTML directly returns an empty shell
- The actual content lives as markdown files in the go-rod/go-rod.github.io GitHub repo
- Successfully fetched: get-started/README.md, custom-launch.md, javascript-runtime.md, selectors/README.md, input.md, context-and-timeout.md, error-handling.md, browsers-pages.md, network/README.md, events/README.md

### API Design Philosophy
- Every method has two variants: `Method()` returns `(result, error)` and `MustMethod()` panics on error
- The `Must` prefix pattern is used throughout for quick scripting
- Chained context/timeout design borrowed from Go's context package
- Auto-waiting: element queries retry until element appears or timeout

### Browser Lifecycle
- `rod.New()` creates a Browser instance (does NOT launch yet)
- `MustConnect()` / `Connect()` actually launches or connects to a browser
- Default behavior: downloads a specific Chromium version for reproducibility
- `launcher.New()` provides fine-grained control over launch args
- Can connect to existing browser via `ControlURL(wsURL)`
- User mode: `launcher.NewUserMode()` connects to regular user browser session
- `launcher.Manager` enables remote browser management for production

### Thread Safety
- All operations are thread-safe
- PagePool and BrowserPool for concurrent resource management

### Error Handling
- `rod.Try(func())` catches panics from Must methods, returns error
- Standard Go error checking with `errors.Is()` and `errors.As()`
- Specific error types: EvalError, ElementNotFoundError, CoveredError, etc.

## Research Process
1. Fetched GitHub README - got high-level feature list but not detailed examples
2. Tried fetching documentation SPA pages - all returned empty shells
3. Discovered docs are markdown in go-rod/go-rod.github.io repo
4. Fetched raw markdown for all key documentation pages
5. Fetched pkg.go.dev for complete API reference
6. Fetched examples_test.go for concrete code examples
