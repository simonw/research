// wfjs is a prototype of `shot-scraper javascript` built on webframe: load a
// page in a hidden WebKit view, run JavaScript against it and print the
// result as JSON.
//
//	wfjs https://example.com/ "document.title"
//	wfjs -r https://example.com/ "document.title"
//	wfjs https://example.com/ "async () => { ... }"
//	wfjs -i script.js https://example.com/
//	echo "document.title" | wfjs https://example.com/
package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"runtime"
	"strings"
	"sync"
	"time"

	"tractor.dev/webframe"
)

func init() { runtime.LockOSThread() }

func main() {
	input := flag.String("i", "", "read JavaScript from this file (- for stdin)")
	raw := flag.Bool("r", false, "output JSON strings as raw text")
	output := flag.String("o", "", "save output to this file")
	width := flag.Int("width", 1280, "viewport width")
	height := flag.Int("height", 720, "viewport height")
	timeout := flag.Duration("timeout", 30*time.Second, "fail if it takes longer than this")
	wait := flag.Duration("wait", 0, "wait this long after the load event")
	waitFor := flag.String("wait-for", "", "wait until this JavaScript expression is truthy")
	logConsole := flag.Bool("log-console", false, "write console output and page errors to stderr")
	fail := flag.Bool("fail", false, "fail if the page returns an HTTP error status")
	persist := flag.Bool("persist", false, "keep cookies/storage between runs (default: fresh profile)")
	sandbox := flag.Bool("sandbox", false, "run the page in WebKit's bubblewrap sandbox (Linux)")
	flag.Usage = func() {
		fmt.Fprintln(os.Stderr, "usage: wfjs [flags] URL [JAVASCRIPT]")
		flag.PrintDefaults()
	}
	flag.Parse()
	args := flag.Args()
	if len(args) < 1 || len(args) > 2 {
		flag.Usage()
		os.Exit(2)
	}
	url := normalizeURL(args[0])
	js, err := readJS(args[1:], *input)
	if err != nil {
		die(err.Error())
	}

	errOut = quietNative()
	// A hidden GTK window is never given a size, so the page's viewport would
	// be 0x0. On our own private Xvfb nobody can see a window anyway.
	ownDisplay := os.Getenv("DISPLAY") == "" && os.Getenv("WAYLAND_DISPLAY") == ""
	if err := ensureDisplay(); err != nil {
		die(err.Error())
	}

	var (
		frame   *webframe.Frame
		mu      sync.Mutex
		running bool
		retries int
	)
	stderr := func(s string) {
		if *logConsole {
			fmt.Fprintln(errOut, s)
		}
	}
	finish := func(result string) {
		writeResult(result, *raw, *output)
		os.Exit(0)
	}
	run := func() {
		defer func() { mu.Lock(); running = false; mu.Unlock() }()
		steps := []string{
			// The load event, so the same point Playwright's goto() waits for.
			`new Promise(r => document.readyState === "complete" ? r() : addEventListener("load", () => r(), {once: true}))`,
		}
		if *fail {
			// Neither WebKit (no PerformanceNavigationTiming.responseStatus) nor
			// glaze reports the page's HTTP status, so ask again, preferring
			// the cache. A workaround: it can mean a second request.
			steps = append(steps, `fetch(location.href, {cache: "force-cache"}).then(r => { if (r.status >= 400) throw new Error("HTTP " + r.status + " for " + location.href) }, () => {})`)
		}
		if *wait > 0 {
			steps = append(steps, fmt.Sprintf(`new Promise(r => setTimeout(r, %d))`, wait.Milliseconds()))
		}
		if *waitFor != "" {
			steps = append(steps, fmt.Sprintf(`new Promise(r => { const t = () => { let ok = false; try { ok = (%s) } catch (e) {} ok ? r() : setTimeout(t, 100) }; t() })`, *waitFor))
		}
		for _, s := range steps {
			if _, err := evalSync(frame, s); err != nil {
				if errors.Is(err, webframe.ErrReloaded) {
					return // the page navigated; run again on its next Ready
				}
				die(cleanErr(err))
			}
		}
		res, err := evalSync(frame, wrapEval(js))
		if err != nil && strings.Contains(err.Error(), "unsafe-eval") {
			// The page's CSP forbids eval: inline the code as an expression,
			// which webframe injects without eval.
			res, err = evalSync(frame, wrapExpr(js))
		}
		if errors.Is(err, webframe.ErrReloaded) {
			return
		}
		if err != nil {
			die(cleanErr(err))
		}
		// Eval quotes string results; ours is always a JSON string.
		var out string
		if err := json.Unmarshal([]byte(res), &out); err != nil {
			die("unexpected result: " + res)
		}
		finish(out)
	}

	frame, err = webframe.New(webframe.Options{
		Window:    ownDisplay,
		Width:     *width,
		Height:    *height,
		Ephemeral: !*persist,
		Sandbox:   *sandbox,
		Console: func(level, text string) {
			stderr("console." + level + ": " + text)
		},
		Exception: func(text string) {
			if strings.HasPrefix(text, "Failed to load ") {
				die(text)
			}
			stderr(text)
		},
		Closed: func() { die("page closed itself before returning a result") },
		Ready: func() {
			mu.Lock()
			defer mu.Unlock()
			if running {
				return
			}
			if retries++; retries > 5 {
				die("page kept navigating")
			}
			running = true
			go run()
		},
	})
	if err != nil {
		die(err.Error())
	}
	time.AfterFunc(*timeout, func() { die(fmt.Sprintf("timed out after %s", *timeout)) })
	frame.Navigate(url)
	frame.Run()
}

// wrapEval runs js as a global script, like the webframe console: statements
// are allowed. A function result is called; a promise is awaited.
func wrapEval(js string) string {
	src, _ := json.Marshal(js)
	return fmt.Sprintf(`(async () => { let r = (0, eval)(%s); if (typeof r === "function") r = r(); r = await r; return JSON.stringify(r === undefined ? null : r); })()`, src)
}

// wrapExpr is wrapEval for pages whose CSP blocks eval: js must be a single
// expression, which is the common case for scraping.
func wrapExpr(js string) string {
	return fmt.Sprintf("(async () => { let r = (\n%s\n); if (typeof r === \"function\") r = r(); r = await r; return JSON.stringify(r === undefined ? null : r); })()", js)
}

// cleanErr drops webframe's "Uncaught " prefix and stack lines that only
// point into the eval machinery.
func cleanErr(err error) string {
	lines := strings.Split(strings.TrimPrefix(err.Error(), "Uncaught "), "\n")
	out := lines[:1]
	for _, l := range lines[1:] {
		if t := strings.TrimSpace(l); t != "" && t != "@" {
			out = append(out, l)
		}
	}
	return strings.Join(out, "\n")
}

func evalSync(f *webframe.Frame, src string) (string, error) {
	type result struct {
		s   string
		err error
	}
	ch := make(chan result, 1)
	f.Eval(src, func(s string, err error) { ch <- result{s, err} })
	r := <-ch
	return r.s, r.err
}

func readJS(args []string, input string) (string, error) {
	switch {
	case input == "-":
		b, err := io.ReadAll(os.Stdin)
		return string(b), err
	case input != "":
		b, err := os.ReadFile(input)
		return string(b), err
	case len(args) == 1:
		return args[0], nil
	}
	if fi, err := os.Stdin.Stat(); err == nil && fi.Mode()&os.ModeCharDevice == 0 {
		b, err := io.ReadAll(os.Stdin)
		return string(b), err
	}
	return "", errors.New("no JavaScript given")
}

func normalizeURL(u string) string {
	if strings.Contains(u, "://") || strings.HasPrefix(u, "about:") {
		return u
	}
	if _, err := os.Stat(u); err == nil {
		abs, _ := os.Getwd()
		if !strings.HasPrefix(u, "/") {
			u = abs + "/" + u
		}
		return "file://" + u
	}
	return "https://" + u
}

func writeResult(result string, raw bool, output string) {
	var out []byte
	var s string
	if raw && json.Unmarshal([]byte(result), &s) == nil {
		out = []byte(s + "\n")
	} else {
		var buf bytes.Buffer
		if err := json.Indent(&buf, []byte(result), "", "    "); err != nil {
			buf.Reset()
			buf.WriteString(result)
		}
		buf.WriteByte('\n')
		out = buf.Bytes()
	}
	if output != "" && output != "-" {
		if err := os.WriteFile(output, out, 0o644); err != nil {
			die(err.Error())
		}
		return
	}
	os.Stdout.Write(out)
}

var (
	dieOnce sync.Once
	errOut  = os.Stderr
)

func die(msg string) {
	dieOnce.Do(func() {
		fmt.Fprintln(errOut, "Error:", msg)
		os.Exit(1)
	})
	select {}
}
