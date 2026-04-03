"""
Cross-browser CSP meta tag iframe escape tests.
Tests Chromium, Firefox, and WebKit using Playwright.
"""
import asyncio
import json
import http.server
import threading
from playwright.async_api import async_playwright

# Track server requests
server_requests = []

class TrackingHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        server_requests.append(("GET", self.path))
        super().do_GET()
    def do_POST(self):
        server_requests.append(("POST", self.path))
        self.send_response(200)
        self.end_headers()
    def log_message(self, format, *args):
        pass  # Suppress output

# The key tests as HTML pages served from the test directory
TEST_PAGES = {
    "round1": "index.html",
    "round5": "round5-data-uri-images.html",
    "round7": "round7-exfiltration.html",
    "round8": "round8-block-data-nav.html",
}

async def run_test_page(page, url, timeout_ms=15000):
    """Navigate to a test page and wait for all results."""
    await page.goto(url)
    try:
        await page.wait_for_function("window.__allDone === true", timeout=timeout_ms)
    except Exception:
        pass  # Timeout - collect whatever results we have

    results = await page.evaluate("""
        () => {
            const pre = document.querySelector('#results pre');
            return pre ? pre.innerText : 'NO RESULTS ELEMENT';
        }
    """)
    return results


async def run_all_browsers():
    # Start tracking HTTP server
    global server_requests

    server = http.server.HTTPServer(("127.0.0.1", 8001), TrackingHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    all_results = {}

    async with async_playwright() as p:
        browsers = {
            "chromium": (p.chromium, "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"),
            "firefox": (p.firefox, "/opt/pw-browsers/firefox-1495/firefox/firefox"),
            "webkit": (p.webkit, "/opt/pw-browsers/webkit-2215/pw_run.sh"),
        }

        for browser_name, (browser_type, exe_path) in browsers.items():
            print(f"\n{'='*60}")
            print(f"Testing: {browser_name}")
            print(f"{'='*60}")

            try:
                browser = await browser_type.launch(headless=True, executable_path=exe_path)
            except Exception as e:
                print(f"  SKIP - could not launch {browser_name}: {e}")
                all_results[browser_name] = {"error": str(e)}
                continue

            context = await browser.new_context()
            page = await context.new_page()
            browser_results = {}

            for test_name, test_file in TEST_PAGES.items():
                # Clear request log before each test
                server_requests.clear()

                # Use port 8001 for our tracking server
                # But the HTML files reference localhost:8000, so we need to
                # modify the pages. Instead, let's just run the key tests inline.
                pass

            # Run inline tests instead for precise control
            print(f"\n  --- Core CSP Tests ---")

            # Test A: Baseline CSP blocks fetch
            server_requests.clear()
            await page.set_content("""
            <iframe id="f" sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                fetch('http://127.0.0.1:8001/test-a').then(r=>r.text()).then(t=>
                  parent.postMessage({result:'FAIL-fetched:'+t},'*')
                ).catch(e=>
                  parent.postMessage({result:'PASS-blocked:'+e.message},'*')
                );
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 5000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=6000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-a" in r[1] for r in server_requests)
            print(f"  Test A (CSP blocks fetch): {result} | Server hit: {server_hit}")
            browser_results["A_csp_blocks_fetch"] = {"result": result, "server_hit": server_hit}

            # Test B: Remove meta tag then fetch
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                document.querySelector('meta[http-equiv=Content-Security-Policy]').remove();
                fetch('http://127.0.0.1:8001/test-b').then(r=>r.text()).then(t=>
                  parent.postMessage({result:'FAIL-fetched:'+t},'*')
                ).catch(e=>
                  parent.postMessage({result:'PASS-blocked:'+e.message},'*')
                );
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 5000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=6000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-b" in r[1] for r in server_requests)
            print(f"  Test B (remove meta, fetch): {result} | Server hit: {server_hit}")
            browser_results["B_remove_meta_fetch"] = {"result": result, "server_hit": server_hit}

            # Test C: document.write without CSP then fetch
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                document.open();
                document.write('<scr'+'ipt>fetch(&quot;http://127.0.0.1:8001/test-c&quot;).then(r=>r.text()).then(t=>parent.postMessage({result:&quot;FAIL-docwrite:&quot;+t},&quot;*&quot;)).catch(e=>parent.postMessage({result:&quot;PASS-docwrite-blocked:&quot;+e.message},&quot;*&quot;))</scr'+'ipt>');
                document.close();
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 5000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=6000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-c" in r[1] for r in server_requests)
            print(f"  Test C (doc.write, fetch): {result} | Server hit: {server_hit}")
            browser_results["C_docwrite_fetch"] = {"result": result, "server_hit": server_hit}

            # Test D: Navigate to data: URI, then fetch (no-cors)
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                location.href='data:text/html,<script>fetch(&quot;http://127.0.0.1:8001/test-d&quot;,{mode:&quot;no-cors&quot;}).then(function(r){parent.postMessage({result:&quot;FAIL-data-nocors:&quot;+r.type},&quot;*&quot;)}).catch(function(e){parent.postMessage({result:&quot;PASS-data-nocors-blocked:&quot;+e.message},&quot;*&quot;)})<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-d" in r[1] for r in server_requests)
            print(f"  Test D (data: URI + no-cors): {result} | Server hit: {server_hit}")
            browser_results["D_data_uri_nocors"] = {"result": result, "server_hit": server_hit}

            # Test E: Navigate to data: URI, then regular fetch
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                location.href='data:text/html,<script>fetch(&quot;http://127.0.0.1:8001/test-e&quot;).then(function(r){return r.text()}).then(function(t){parent.postMessage({result:&quot;FAIL-data-fetch:&quot;+t},&quot;*&quot;)}).catch(function(e){parent.postMessage({result:&quot;PASS-data-fetch-blocked:&quot;+e.message},&quot;*&quot;)})<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-e" in r[1] for r in server_requests)
            print(f"  Test E (data: URI + fetch): {result} | Server hit: {server_hit}")
            browser_results["E_data_uri_fetch"] = {"result": result, "server_hit": server_hit}

            # Test F: No CSP, sandbox only -> data: URI -> no-cors fetch
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <script>
                location.href='data:text/html,<script>fetch(&quot;http://127.0.0.1:8001/test-f&quot;,{mode:&quot;no-cors&quot;}).then(function(r){parent.postMessage({result:&quot;NOCSP-data-nocors:&quot;+r.type},&quot;*&quot;)}).catch(function(e){parent.postMessage({result:&quot;NOCSP-data-blocked:&quot;+e.message},&quot;*&quot;)})<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-f" in r[1] for r in server_requests)
            print(f"  Test F (no CSP, data: + no-cors): {result} | Server hit: {server_hit}")
            browser_results["F_no_csp_data_nocors"] = {"result": result, "server_hit": server_hit}

            # Test G: Image load from data: URI (with prior CSP)
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <meta http-equiv='Content-Security-Policy'
                    content=&quot;default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';&quot;>
              <script>
                location.href='data:text/html,<script>var img=new Image();img.src=&quot;http://127.0.0.1:8001/test-g-img&quot;;img.onload=function(){parent.postMessage({result:&quot;FAIL-img-loaded&quot;},&quot;*&quot;)};img.onerror=function(){parent.postMessage({result:&quot;img-onerror&quot;},&quot;*&quot;)};setTimeout(function(){parent.postMessage({result:&quot;img-timeout&quot;},&quot;*&quot;)},3000)<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-g" in r[1] for r in server_requests)
            print(f"  Test G (data: URI + image): {result} | Server hit: {server_hit}")
            browser_results["G_data_uri_image"] = {"result": result, "server_hit": server_hit}

            # Test H: No CSP, sandbox only -> data: URI -> image
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts" srcdoc="
              <script>
                location.href='data:text/html,<script>var img=new Image();img.src=&quot;http://127.0.0.1:8001/test-h-img&quot;;img.onload=function(){parent.postMessage({result:&quot;NOCSP-img-loaded&quot;},&quot;*&quot;)};img.onerror=function(){parent.postMessage({result:&quot;NOCSP-img-onerror&quot;},&quot;*&quot;)};setTimeout(function(){parent.postMessage({result:&quot;NOCSP-img-timeout&quot;},&quot;*&quot;)},3000)<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-h" in r[1] for r in server_requests)
            print(f"  Test H (no CSP, data: + image): {result} | Server hit: {server_hit}")
            browser_results["H_no_csp_data_image"] = {"result": result, "server_hit": server_hit}

            # Test I: csp attribute (Chromium feature) -> data: URI -> fetch
            server_requests.clear()
            await page.set_content("""
            <iframe sandbox="allow-scripts"
              csp="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';"
              srcdoc="
              <script>
                location.href='data:text/html,<script>fetch(&quot;http://127.0.0.1:8001/test-i&quot;,{mode:&quot;no-cors&quot;}).then(function(r){parent.postMessage({result:&quot;CSPATTR-nocors:&quot;+r.type},&quot;*&quot;)}).catch(function(e){parent.postMessage({result:&quot;CSPATTR-blocked:&quot;+e.message},&quot;*&quot;)})<\/script>';
              </script>
            "></iframe>
            <script>
              window.__result = null;
              window.addEventListener('message', e => { if(e.data&&e.data.result) window.__result = e.data.result; });
              setTimeout(()=>{ if(!window.__result) window.__result = 'TIMEOUT'; }, 8000);
            </script>
            """)
            try:
                await page.wait_for_function("window.__result !== null", timeout=9000)
            except:
                pass
            result = await page.evaluate("window.__result")
            server_hit = any("test-i" in r[1] for r in server_requests)
            print(f"  Test I (csp attr + data: + no-cors): {result} | Server hit: {server_hit}")
            browser_results["I_csp_attr_data_nocors"] = {"result": result, "server_hit": server_hit}

            all_results[browser_name] = browser_results
            await browser.close()

    server.shutdown()

    # Print comparison table
    print(f"\n\n{'='*80}")
    print("CROSS-BROWSER COMPARISON")
    print(f"{'='*80}")

    tests = ["A_csp_blocks_fetch", "B_remove_meta_fetch", "C_docwrite_fetch",
             "D_data_uri_nocors", "E_data_uri_fetch", "F_no_csp_data_nocors",
             "G_data_uri_image", "H_no_csp_data_image", "I_csp_attr_data_nocors"]

    header = f"{'Test':<30} | {'Chromium':<30} | {'Firefox':<30} | {'WebKit':<30}"
    print(header)
    print("-" * len(header))

    for test in tests:
        row = f"{test:<30}"
        for browser in ["chromium", "firefox", "webkit"]:
            if browser in all_results and not isinstance(all_results[browser], dict) or "error" in all_results.get(browser, {}):
                row += f" | {'SKIP':<30}"
            elif test in all_results.get(browser, {}):
                r = all_results[browser][test]
                short = r["result"][:20] if r["result"] else "None"
                hit = "HIT" if r["server_hit"] else "no-hit"
                row += f" | {short} [{hit}]".ljust(33)
            else:
                row += f" | {'N/A':<30}"
        print(row)

    # Save full results
    with open("/home/user/research/test-csp-iframe-escape/cross_browser_results.json", "w") as f:
        json.dump(all_results, f, indent=2)

    print(f"\nFull results saved to cross_browser_results.json")
    return all_results

asyncio.run(run_all_browsers())
