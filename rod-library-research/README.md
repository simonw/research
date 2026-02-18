# Rod: Go Library for Chrome Automation -- Comprehensive API Reference

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Rod is a high-level Go driver for the Chrome DevTools Protocol, designed for web automation and scraping. It provides both high-level convenience methods and low-level protocol access.

- **Repository**: https://github.com/go-rod/rod
- **Documentation**: https://go-rod.github.io/#/
- **API Reference**: https://pkg.go.dev/github.com/go-rod/rod
- **License**: MIT

## Table of Contents

1. [Installation](#installation)
2. [Core Concepts and Design Patterns](#core-concepts-and-design-patterns)
3. [Browser Initialization and Lifecycle](#browser-initialization-and-lifecycle)
4. [Navigation](#navigation)
5. [Selectors and Element Queries](#selectors-and-element-queries)
6. [Waiting Strategies](#waiting-strategies)
7. [JavaScript Execution](#javascript-execution)
8. [Clicking and Input](#clicking-and-input)
9. [Extracting Text and HTML](#extracting-text-and-html)
10. [Screenshots and PDF](#screenshots-and-pdf)
11. [Context, Timeouts, and Cancellation](#context-timeouts-and-cancellation)
12. [Error Handling](#error-handling)
13. [Network Interception](#network-interception)
14. [Event Handling](#event-handling)
15. [Concurrency and Pooling](#concurrency-and-pooling)
16. [Complete API Type Reference](#complete-api-type-reference)

---

## Installation

Rod requires Go 1.18+. No external dependencies like Selenium or browser drivers are needed -- Rod communicates directly over the Chrome DevTools Protocol.

```bash
go mod init myproject
go get github.com/go-rod/rod
```

On first run, Rod automatically downloads a known-good Chromium build for reproducibility. You can also point it at an existing Chrome/Chromium installation.

---

## Core Concepts and Design Patterns

### The Must Prefix Pattern

Every method in Rod has two variants:

| Non-Must (production)              | Must (scripting)                  |
|------------------------------------|-----------------------------------|
| `page.Element(sel)` -> `(*Element, error)` | `page.MustElement(sel)` -> `*Element` |
| `page.Navigate(url)` -> `error`    | `page.MustNavigate(url)` -> `*Page`   |
| `el.Text()` -> `(string, error)`   | `el.MustText()` -> `string`           |

`Must` methods panic on error. They are intended for quick scripts, examples, and tests. For production code that needs graceful error handling, use the non-Must variants.

The `Must` pattern is implemented as a thin wrapper:

```go
func (p *Page) MustElement(selector string) *Element {
    el, err := p.Element(selector)
    if err != nil {
        panic(err)
    }
    return el
}
```

### Method Chaining

Rod methods return the receiver to enable chaining:

```go
page.MustNavigate("https://example.com").MustWaitStable().MustScreenshot("page.png")
```

### Auto-Waiting

Single-element queries (`Element`, `MustElement`, etc.) automatically retry until the element appears or the timeout expires. You do not need to add explicit waits before querying for elements.

Multi-element queries (`Elements`, `MustElements`) return immediately with whatever matches exist (possibly an empty list).

### Three Core Types

- **`Browser`** -- represents a browser instance; manages pages, cookies, and lifecycle
- **`Page`** -- represents a tab/page; handles navigation, evaluation, screenshots
- **`Element`** -- represents a DOM element; handles clicks, input, text extraction

---

## Browser Initialization and Lifecycle

### Basic Launch (Auto-Managed)

```go
package main

import "github.com/go-rod/rod"

func main() {
    // Creates browser instance, downloads Chromium if needed, launches it
    browser := rod.New().MustConnect()
    defer browser.MustClose()

    page := browser.MustPage("https://example.com")
    fmt.Println(page.MustElement("h1").MustText())
}
```

`rod.New()` creates a Browser struct. `MustConnect()` actually launches the browser process.

### Headless vs Visible (Using Launcher)

```go
import (
    "github.com/go-rod/rod"
    "github.com/go-rod/rod/lib/launcher"
)

// Visible browser with DevTools open
url := launcher.New().
    Headless(false).
    Devtools(true).
    MustLaunch()

browser := rod.New().ControlURL(url).MustConnect()
defer browser.MustClose()
```

The `launcher` package provides fine-grained control over browser startup arguments.

### Command-Line Flag Shortcut

During development, use the `-rod` flag instead of modifying code:

```bash
go run . -rod=show              # show browser window
go run . -rod=show,devtools     # show browser + open DevTools
```

### Connect to an Existing Browser

Start Chrome manually with a debugging port:

```bash
google-chrome --headless --remote-debugging-port=9222
```

Then connect from Rod:

```go
browser := rod.New().ControlURL("ws://127.0.0.1:9222/devtools/browser/<id>").MustConnect()
```

### Custom Launcher Configuration

```go
u := launcher.New().
    Bin("/usr/bin/chromium").         // custom binary path
    Headless(true).                   // headless mode (default)
    UserDataDir("/tmp/my-profile").   // persist profile data
    Proxy("socks5://localhost:1080"). // proxy configuration
    Set("disable-gpu").               // arbitrary Chrome flags
    Delete("--headless").             // remove a default flag
    MustLaunch()

browser := rod.New().ControlURL(u).MustConnect()
```

### User Mode (Reuse Existing Browser Session)

```go
// Connects to the user's regular browser, preserving sessions/cookies
u := launcher.NewUserMode().MustLaunch()
browser := rod.New().ControlURL(u).MustConnect()
```

### Remote Browser Management

```go
// On the remote machine -- serve browser management
launcher.NewManager().MustLaunch()

// On the client -- connect to remote launcher
u := launcher.MustNewManaged("http://remote-host:7317").MustLaunch()
browser := rod.New().ControlURL(u).MustConnect()
```

### Browser Cleanup

```go
browser.MustClose()  // closes all pages and terminates the browser process
```

Rod ensures no zombie browser processes remain after crashes.

---

## Navigation

```go
// Navigate to a URL
page := browser.MustPage("https://example.com")

// Or navigate after page creation
page := browser.MustPage("")
page.MustNavigate("https://example.com")

// Wait for full page load after navigation
page.MustNavigate("https://example.com").MustWaitLoad()

// Wait for DOM stability (no layout changes)
page.MustNavigate("https://example.com").MustWaitStable()

// Back and forward
page.MustNavigateBack()
page.MustNavigateForward()

// Reload
page.MustReload()

// Stop loading
page.MustStopLoading()
```

### Wait for Navigation Events

```go
// Two-step pattern: subscribe BEFORE triggering the action
wait := page.MustWaitNavigation()
page.MustElement("a.next-page").MustClick()
wait()  // blocks until navigation completes
```

---

## Selectors and Element Queries

### CSS Selectors (Primary Method)

```go
// Single element (auto-waits until found or timeout)
el := page.MustElement("div.content > h1")
el := page.MustElement("#main-title")
el := page.MustElement("input[name='email']")

// Multiple elements (returns immediately, possibly empty)
items := page.MustElements("ul.results > li")
for _, item := range items {
    fmt.Println(item.MustText())
}
```

### Text/Regex Matching (ElementR)

Find elements whose visible text matches a JavaScript regex:

```go
// Find a <span> whose text contains "most widely used"
el := page.MustElementR("span", "most widely used")

// Case-insensitive matching
el := page.MustElementR("button", "/click me/i")
```

Note: `ElementR` matches against the **rendered visible text**, not the HTML source.

### XPath Selectors

```go
el := page.MustElementX("//h2")
els := page.MustElementsX("//div[@class='item']")
```

### JavaScript-Based Selectors

```go
el := page.MustElementByJS(`() => document.querySelector('.special')`)
```

### Check Element Existence

```go
// Returns bool (does not wait/retry)
exists := page.MustHas("div.optional")
existsX := page.MustHasX("//div[@class='optional']")
existsR := page.MustHasR("span", "some text")
```

### Element-Relative Queries (DOM Traversal)

```go
parent := el.MustParent()
ancestors := el.MustParents("div.wrapper")
next := el.MustNext()
prev := el.MustPrevious()
child := el.MustElement("span.label")    // query within element
children := el.MustElements("li")        // all matching children
```

### Iframes

```go
// Get the page context for an iframe
framePage := page.MustElement("iframe").MustFrame()
el := framePage.MustElement("button.submit")
```

### Shadow DOM

```go
shadowRoot := el.MustShadowRoot()
innerEl := shadowRoot.MustElement("div.inner")
```

### Search (Deep Nested Content)

```go
// Searches across iframes and shadow DOMs
result := page.MustSearch("search query text")
```

### Race Selectors (Handle Multiple Outcomes)

```go
// Wait for whichever element appears first
page.Race().
    Element(".success-message").MustHandle(func(e *rod.Element) {
        fmt.Println("Success:", e.MustText())
    }).
    Element(".error-message").MustHandle(func(e *rod.Element) {
        fmt.Println("Error:", e.MustText())
    }).
    MustDo()
```

### Element from Coordinates

```go
el := page.MustElementFromPoint(200, 300)
```

---

## Waiting Strategies

### Page-Level Waits

```go
page.MustWaitLoad()        // wait for window.onload event
page.MustWaitStable()      // wait until no DOM layout changes for a period
page.MustWaitDOMStable()   // wait for DOM content stability
page.MustWaitIdle()        // wait for network idle
```

### Wait for Network Idle (AJAX)

```go
// Subscribe first, then trigger the action, then wait
wait := page.MustWaitRequestIdle()
page.MustElement("#searchInput").MustInput("query")
wait()  // blocks until no pending network requests
```

You can exclude specific URL patterns:

```go
wait := page.MustWaitRequestIdle("/analytics", "/tracking")
```

### Wait for Custom JavaScript Condition

```go
page.MustWait(`() => document.title === 'Expected Title'`)
page.MustWait(`() => document.querySelectorAll('.item').length > 5`)
```

### Wait for Element Count

```go
page.MustWaitElementsMoreThan("ul > li", 10)
```

### Element-Level Waits

```go
el.MustWaitVisible()        // wait until element is visible
el.MustWaitInvisible()      // wait until element disappears
el.MustWaitStable()         // wait until element position stabilizes
el.MustWaitEnabled()        // wait until element is enabled
el.MustWaitWritable()       // wait until input is writable
el.MustWaitInteractable()   // wait until element can receive events
el.MustWaitLoad()           // wait for resource (e.g., image) to load
```

### Two-Step Event Wait Pattern

The key pattern for avoiding race conditions: subscribe to the event **before** triggering the action.

```go
wait := page.WaitEvent(&proto.PageLoadEventFired{})
page.MustNavigate("https://example.com")
wait()  // blocks until the event fires
```

---

## JavaScript Execution

### Basic Evaluation

```go
// Execute JS, ignore result
page.MustEval(`() => console.log("hello world")`)

// Execute JS, get return value
result := page.MustEval(`() => document.title`).Str()

// IMPORTANT: Eval requires a function expression, NOT bare code
// WRONG:  page.MustEval(`document.title`)
// RIGHT:  page.MustEval(`() => document.title`)
```

### Passing Parameters from Go to JavaScript

```go
// Parameters are passed as function arguments
result := page.MustEval(`(a, b) => a + b`, 1, 2).Int()  // returns 3

// Pass complex data
page.MustEval(`(key, value) => { window[key] = value }`, "myData", map[string]int{"x": 1})
```

### Return Value Access

The return type is `gson.JSON` with convenience getters:

```go
val := page.MustEval(`() => ({ name: "alice", age: 30 })`)
val.Get("name").Str()   // "alice"
val.Get("age").Int()     // 30
val.Str()                // JSON string representation
val.Num()                // float64
val.Bool()               // bool
val.Nil()                // check if null/undefined
```

### Eval on Elements (this = the element)

```go
el := page.MustElement("h1")

// 'this' refers to the element
text := el.MustEval(`() => this.innerText`).Str()

// Modify the element
el.MustEval(`() => this.style.color = "red"`)
el.MustEval(`() => this.innerText = "New Title"`)
```

### Reusing Remote Objects

```go
fn := page.MustEvaluate(rod.Eval(`() => Math.random`).ByObject())
num := page.MustEval(`f => f()`, fn).Num()
```

### Inject JavaScript on Every Page Load

```go
page.MustEvalOnNewDocument(`() => {
    Object.defineProperty(navigator, 'webdriver', { get: () => false })
}`)
```

### Expose Go Functions to JavaScript

```go
page.MustExpose("goAdd", func(args ...interface{}) interface{} {
    a := args[0].(float64)
    b := args[1].(float64)
    return a + b
})

// Now callable from page JS:
result := page.MustEval(`() => goAdd(3, 4)`).Int()  // 7
```

---

## Clicking and Input

### Mouse Clicks

```go
// Left click
page.MustElement("button.submit").MustClick()

// Double click
page.MustElement("div.editable").MustDoubleClick()

// Right click (requires proto import)
el.Click(proto.InputMouseButtonRight, 1)

// Hover
page.MustElement("div.tooltip-trigger").MustHover()

// Move mouse away
el.MustMoveMouseOut()

// Touch tap (for mobile emulation)
el.MustTap()
```

### Text Input

```go
// Type into an input field
page.MustElement("input#username").MustInput("alice")

// Clear an input and retype
page.MustElement("input#search").MustSelectAllText().MustInput("")
page.MustElement("input#search").MustInput("new query")

// Partial text selection and replacement
el.MustSelectText("old text")
el.MustInput("replacement")
```

### Keyboard Input

```go
import "github.com/go-rod/rod/lib/input"

// Type a key
page.MustElement("input").MustType(input.Enter)

// Key combinations (e.g., Shift+A for uppercase 'A')
page.MustElement("input").MustKeyActions().
    Press(input.ShiftLeft).
    Type('A').
    MustDo()

// Ctrl+Enter
page.MustElement("textarea").MustKeyActions().
    Press(input.ControlLeft).
    Type(input.Enter).
    MustDo()

// Global keyboard events
page.Keyboard.MustType(input.Slash)
```

### Form Elements

```go
// Select dropdown option by visible text
page.MustElement("select#country").MustSelect("United States")

// Checkbox (just click it)
page.MustElement("input[type=checkbox]").MustClick()

// File upload
page.MustElement("input[type=file]").MustSetFiles("/path/to/file.pdf")

// Date/time inputs
page.MustElement("input[type=date]").MustInputTime(time.Now())

// Color picker
page.MustElement("input[type=color]").MustInputColor("#ff0000")
```

### Low-Level Mouse/Keyboard/Touch

```go
// Direct mouse control
page.Mouse.MustMoveTo(100, 200)
page.Mouse.MustClick(proto.InputMouseButtonLeft)

// Direct keyboard
page.Keyboard.MustType(input.KeyA)

// Direct touch
page.Touch.MustTap(100, 200)
```

---

## Extracting Text and HTML

### Text Content

```go
// Get visible text content of an element
text := page.MustElement("h1").MustText()

// Get text of all matching elements
for _, el := range page.MustElements("ul > li") {
    fmt.Println(el.MustText())
}
```

### HTML Content

```go
// Get outer HTML of an element
html := page.MustElement("div.content").MustHTML()

// Get full page HTML
fullHTML := page.MustHTML()
```

### Attributes and Properties

```go
// Get attribute (returns *string, nil if not present)
href := page.MustElement("a").MustAttribute("href")
if href != nil {
    fmt.Println(*href)
}

// Get property (returns gson.JSON)
value := page.MustElement("input").MustProperty("value").Str()
checked := page.MustElement("input[type=checkbox]").MustProperty("checked").Bool()
```

### Element Visibility and State

```go
visible := el.MustVisible()        // bool
disabled := el.MustDisabled()       // bool
interactable := el.MustInteractable()  // bool
```

### Images and Resources

```go
// Download image/resource bytes
imgBytes := page.MustElement("img.logo").MustResource()
utils.OutputFile("logo.png", imgBytes)

// Get background image
bgBytes := el.MustBackgroundImage()

// Canvas to image
canvasBytes := page.MustElement("canvas").MustCanvasToImage()
```

---

## Screenshots and PDF

### Simple Page Screenshot

```go
// Screenshot to file
page.MustScreenshot("page.png")

// Screenshot to bytes
imgBytes := page.MustScreenshot()
```

### Customized Screenshot

```go
import "github.com/nicholasgasior/gcd/lib/proto"

img, _ := page.Screenshot(true, &proto.PageCaptureScreenshot{
    Format:  proto.PageCaptureScreenshotFormatJpeg,
    Quality: gson.Int(90),
    Clip: &proto.PageViewport{
        X:      0,
        Y:      0,
        Width:  300,
        Height: 200,
        Scale:  1,
    },
})
_ = utils.OutputFile("cropped.jpg", img)
```

### Full-Page Scroll Screenshot

```go
img := page.MustNavigate("https://example.com").
    MustWaitStable().
    MustScrollScreenshot()
utils.OutputFile("fullpage.png", img)
```

### Element Screenshot

```go
// Screenshot just a specific element
imgBytes := page.MustElement("div.chart").MustScreenshot()
utils.OutputFile("chart.png", imgBytes)
```

### PDF Generation

```go
// Simple PDF
page.MustPDF("output.pdf")

// Customized PDF
pdf, _ := page.PDF(&proto.PagePrintToPDF{
    PaperWidth:  gson.Num(8.5),
    PaperHeight: gson.Num(11),
    PageRanges:  "1-3",
})
_ = utils.OutputFile("output.pdf", pdf)
```

---

## Context, Timeouts, and Cancellation

### Timeout on Operations

```go
// Set timeout on a page (returns a shallow clone)
page.Timeout(5 * time.Second).MustNavigate("https://slow-site.com")

// Timeout applies to all chained operations
page.Timeout(5 * time.Second).
    MustElement("div.loaded").
    MustText()
```

### Cancel and Reset Timeout

```go
// Chain different timeouts for different operations
text := page.
    Timeout(2 * time.Second).MustElement("a").    // 2s to find element
    CancelTimeout().                                // reset
    Timeout(10 * time.Second).MustText()           // 10s to get text
```

### Using Go Context Directly

```go
ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
defer cancel()

page := browser.MustPage("").Context(ctx)
page.MustNavigate("https://example.com")
// All operations on this page clone respect the context
```

### Detect Timeout Errors

```go
err := rod.Try(func() {
    page.Timeout(2 * time.Second).MustElement("div.never-exists")
})
if errors.Is(err, context.DeadlineExceeded) {
    fmt.Println("Timed out waiting for element")
}
```

---

## Error Handling

### Using rod.Try

```go
err := rod.Try(func() {
    page.MustElement("a").MustHTML()
})
if err != nil {
    fmt.Println("Something went wrong:", err)
}
```

### Standard Go Error Handling (Non-Must)

```go
el, err := page.Element("div.content")
if err != nil {
    log.Fatal(err)
}
text, err := el.Text()
if err != nil {
    log.Fatal(err)
}
fmt.Println(text)
```

### Error Type Checking

```go
// Check for timeout
if errors.Is(err, context.DeadlineExceeded) {
    // handle timeout
}

// Check for JavaScript evaluation error
var evalErr *rod.EvalError
if errors.As(err, &evalErr) {
    fmt.Println("JS error at line:", evalErr.LineNumber)
}
```

### Key Error Types

| Error Type | Meaning |
|---|---|
| `context.DeadlineExceeded` | Operation timed out |
| `*rod.EvalError` | JavaScript evaluation failed |
| `*rod.ElementNotFoundError` | Element selector found nothing |
| `*rod.InvisibleShapeError` | Element is not visible |
| `*rod.CoveredError` | Element is covered by another element |
| `*rod.NotInteractableError` | Element cannot receive input |
| `*rod.NavigationError` | Page navigation failed |
| `*rod.NoPointerEventsError` | Element has `pointer-events: none` |

---

## Network Interception

### Hijack Requests

```go
router := browser.HijackRequests()

// Modify responses
router.MustAdd("*api.example.com*", func(ctx *rod.Hijack) {
    ctx.MustLoadResponse()
    body := ctx.Response.Body()
    // modify body...
    ctx.Response.SetBody(modifiedBody)
})

go router.Run()
defer router.MustStop()
```

### Block Resources (Images, CSS, etc.)

```go
router := page.HijackRequests()
router.MustAdd("*", func(ctx *rod.Hijack) {
    if ctx.Request.Type() == proto.NetworkResourceTypeImage {
        ctx.Response.Fail(proto.NetworkErrorReasonBlockedByClient)
        return
    }
    ctx.ContinueRequest(&proto.FetchContinueRequest{})
})
go router.Run()
```

### Network Throttling

```go
_ = proto.NetworkEmulateNetworkConditions{
    Offline:            false,
    Latency:            200,
    DownloadThroughput: 50000,
    UploadThroughput:   20000,
    ConnectionType:     proto.NetworkConnectionTypeCellular2g,
}.Call(page)
```

---

## Event Handling

### Two-Step Wait Pattern

```go
// 1. Subscribe BEFORE the action
wait := page.WaitEvent(&proto.PageLoadEventFired{})
// 2. Trigger the action
page.MustNavigate("https://example.com")
// 3. Block until event fires
wait()
```

### Capture Event Details

```go
var e proto.NetworkResponseReceived
wait := page.WaitEvent(&e)
page.MustNavigate("https://example.com")
wait()
fmt.Println("Status code:", e.Response.Status)
```

### Listen for Multiple Events

```go
wait := page.EachEvent(func(e *proto.PageLoadEventFired) bool {
    fmt.Println("Page loaded")
    return true  // return true to stop listening
}, func(e *proto.RuntimeConsoleAPICalled) bool {
    fmt.Println("Console:", e.Args[0].Value)
    return false  // keep listening
})
// ... trigger actions ...
wait()
```

### Using MustWait Helpers

```go
page.MustWaitNavigation()    // wait for navigation to complete
page.MustWaitLoad()          // wait for load event
page.MustWaitStable()        // wait for DOM stability
page.MustWaitRequestIdle()   // wait for network idle
```

---

## Concurrency and Pooling

### Page Pool

```go
pool := rod.NewPagePool(3)  // max 3 concurrent pages

create := func() *rod.Page {
    return browser.MustIncognito().MustPage()
}

// Get a page from pool (blocks if all in use)
page := pool.MustGet(create)
defer pool.Put(page)

page.MustNavigate("https://example.com")
```

### Browser Pool

```go
pool := rod.NewBrowserPool(2)

create := func() *rod.Browser {
    return rod.New().MustConnect()
}

browser := pool.MustGet(create)
defer pool.Put(browser)
```

### Incognito Contexts for Isolation

```go
// Each incognito context has its own cookies/cache
incognito := browser.MustIncognito()
page := incognito.MustPage("https://example.com")
```

### Thread Safety

All Rod operations are thread-safe. Multiple goroutines can safely use the same Browser, Page, or Element.

---

## Complete API Type Reference

### Browser Methods Summary

| Category | Key Methods |
|---|---|
| **Lifecycle** | `New()`, `Connect()`, `Close()`, `ControlURL()` |
| **Pages** | `Page()`, `Pages()`, `PageFromTarget()`, `Incognito()` |
| **Config** | `Context()`, `Timeout()`, `CancelTimeout()`, `Sleeper()`, `SlowMotion()`, `Trace()`, `Logger()`, `Monitor()`, `DefaultDevice()`, `NoDefaultDevice()` |
| **Cookies** | `GetCookies()`, `SetCookies()` |
| **Auth** | `HandleAuth()`, `IgnoreCertErrors()` |
| **Network** | `HijackRequests()`, `WaitDownload()` |
| **Events** | `Event()`, `EachEvent()`, `WaitEvent()` |
| **Advanced** | `Call()`, `ServeMonitor()`, `Version()` |

### Page Methods Summary

| Category | Key Methods |
|---|---|
| **Navigation** | `Navigate()`, `Reload()`, `NavigateBack()`, `NavigateForward()`, `StopLoading()` |
| **Selectors** | `Element()`, `Elements()`, `ElementX()`, `ElementsX()`, `ElementR()`, `ElementByJS()`, `ElementsByJS()`, `ElementFromPoint()`, `Has()`, `HasX()`, `HasR()` |
| **Content** | `HTML()`, `Text()`, `Info()` |
| **JavaScript** | `Eval()`, `Evaluate()`, `EvalOnNewDocument()`, `Expose()` |
| **Screenshots** | `Screenshot()`, `ScreenshotFullPage()`, `ScrollScreenshot()`, `PDF()` |
| **Waiting** | `WaitLoad()`, `WaitStable()`, `WaitDOMStable()`, `WaitIdle()`, `WaitRequestIdle()`, `WaitNavigation()`, `WaitOpen()`, `Wait()`, `WaitRepaint()`, `WaitElementsMoreThan()` |
| **Input** | `InsertText()`, `KeyActions()`, `Mouse`, `Keyboard`, `Touch` |
| **DOM** | `SetDocumentContent()`, `AddScriptTag()`, `AddStyleTag()`, `CaptureDOMSnapshot()` |
| **Viewport** | `SetViewport()`, `GetWindow()`, `SetWindow()`, `WindowMaximize()`, `WindowMinimize()`, `WindowFullscreen()` |
| **Dialogs** | `HandleDialog()`, `HandleFileDialog()` |
| **Cookies** | `Cookies()`, `SetCookies()`, `SetBlockedURLs()` |
| **Headers** | `SetUserAgent()`, `SetExtraHeaders()` |
| **Config** | `Context()`, `Timeout()`, `CancelTimeout()`, `Sleeper()`, `Emulate()`, `Race()`, `Search()` |

### Element Methods Summary

| Category | Key Methods |
|---|---|
| **Content** | `Text()`, `HTML()`, `Attribute()`, `Property()` |
| **State** | `Visible()`, `Disabled()`, `Interactable()`, `Matches()` |
| **Queries** | `Element()`, `Elements()`, `ElementX()`, `ElementR()`, `ElementByJS()`, `Has()`, `HasX()`, `HasR()` |
| **Traversal** | `Parent()`, `Parents()`, `Next()`, `Previous()` |
| **Mouse** | `Click()`, `DoubleClick()`, `Tap()`, `Hover()`, `MoveMouseOut()` |
| **Keyboard** | `Input()`, `Type()`, `SelectAllText()`, `SelectText()`, `Select()`, `KeyActions()` |
| **Forms** | `SetFiles()`, `InputColor()`, `InputTime()` |
| **Focus** | `Focus()`, `Blur()`, `ScrollIntoView()` |
| **Waiting** | `WaitVisible()`, `WaitInvisible()`, `WaitStable()`, `WaitEnabled()`, `WaitWritable()`, `WaitInteractable()`, `WaitLoad()`, `Wait()` |
| **JS** | `Eval()`, `Evaluate()` |
| **Visual** | `Screenshot()`, `CanvasToImage()`, `Resource()`, `BackgroundImage()` |
| **DOM** | `Describe()`, `Shape()`, `GetXPath()`, `Frame()`, `ShadowRoot()`, `Remove()`, `Equal()`, `ContainsElement()` |

---

## Common Patterns: Quick Reference

### Minimal Scraping Example

```go
browser := rod.New().MustConnect()
defer browser.MustClose()

page := browser.MustPage("https://news.ycombinator.com")
page.MustWaitStable()

titles := page.MustElements("a.titlelink")
for _, t := range titles {
    fmt.Println(t.MustText(), "->", *t.MustAttribute("href"))
}
```

### Form Submission

```go
page := browser.MustPage("https://example.com/login")
page.MustElement("#email").MustInput("user@example.com")
page.MustElement("#password").MustInput("secret")
page.MustElement("button[type=submit]").MustClick()
page.MustWaitNavigation()
```

### Wait for AJAX, Then Extract

```go
page := browser.MustPage("https://example.com/search")
wait := page.MustWaitRequestIdle()
page.MustElement("#query").MustInput("golang")
page.MustElement("#search-btn").MustClick()
wait()

results := page.MustElements(".result-item")
for _, r := range results {
    fmt.Println(r.MustElement("h3").MustText())
}
```

### Screenshot with Timeout

```go
err := rod.Try(func() {
    page.Timeout(10 * time.Second).
        MustNavigate("https://example.com").
        MustWaitStable().
        MustScreenshot("capture.png")
})
if err != nil {
    log.Println("Failed to capture screenshot:", err)
}
```

---

## Sources

- GitHub repository: https://github.com/go-rod/rod
- Documentation source: https://github.com/go-rod/go-rod.github.io
- API reference: https://pkg.go.dev/github.com/go-rod/rod
- examples_test.go in the main repository
