/**
 * Puppet Agent - Injected into proxied pages for DOM automation.
 * Communicates with parent frame via postMessage.
 */
(function () {
  // Signal readiness to parent
  try {
    window.top.postMessage({ type: "PUPPET_READY" }, "*");
  } catch {
    // Cross-origin restriction fallback
    window.parent.postMessage({ type: "PUPPET_READY" }, "*");
  }

  function sendResult(id, result, error) {
    const msg = { type: "PUPPET_RESULT", id, result, error: error || null };
    try {
      window.top.postMessage(msg, "*");
    } catch {
      window.parent.postMessage(msg, "*");
    }
  }

  function serializeElement(el) {
    if (!el) return null;
    return {
      tagName: el.tagName,
      id: el.id,
      className: el.className,
      textContent: (el.textContent || "").slice(0, 500),
      innerText: (el.innerText || "").slice(0, 500),
      value: el.value || undefined,
      href: el.href || undefined,
      src: el.src || undefined,
      attributes: Array.from(el.attributes || []).reduce((acc, attr) => {
        acc[attr.name] = attr.value;
        return acc;
      }, {}),
    };
  }

  const commands = {
    query(args) {
      const el = document.querySelector(args.selector);
      return serializeElement(el);
    },

    queryAll(args) {
      const els = document.querySelectorAll(args.selector);
      return Array.from(els)
        .slice(0, args.limit || 50)
        .map(serializeElement);
    },

    click(args) {
      const el = document.querySelector(args.selector);
      if (!el) throw new Error(`Element not found: ${args.selector}`);
      el.click();
      return { clicked: true, selector: args.selector };
    },

    type(args) {
      const el = document.querySelector(args.selector);
      if (!el) throw new Error(`Element not found: ${args.selector}`);
      el.focus();
      // Clear existing value
      el.value = "";
      // Simulate typing
      for (const char of args.text) {
        el.value += char;
        el.dispatchEvent(new Event("input", { bubbles: true }));
      }
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { typed: true, selector: args.selector, text: args.text };
    },

    scroll(args) {
      if (args.selector) {
        const el = document.querySelector(args.selector);
        if (el) el.scrollIntoView({ behavior: "smooth" });
      } else {
        window.scrollBy({
          top: args.y || 0,
          left: args.x || 0,
          behavior: "smooth",
        });
      }
      return {
        scrolled: true,
        scrollY: window.scrollY,
        scrollX: window.scrollX,
      };
    },

    getPageInfo() {
      return {
        title: document.title,
        url: window.location.href,
        readyState: document.readyState,
        bodyLength: (document.body?.innerHTML || "").length,
        links: document.querySelectorAll("a").length,
        images: document.querySelectorAll("img").length,
        forms: document.querySelectorAll("form").length,
      };
    },

    async evaluate(args) {
      // Execute arbitrary JS and return result — supports async code
      const result = await (0, eval)(args.code);
      return result;
    },
  };

  window.addEventListener("message", async (event) => {
    if (!event.data || event.data.type !== "PUPPET_CMD") return;

    const { id, command, args } = event.data;
    try {
      if (!commands[command]) {
        throw new Error(`Unknown command: ${command}`);
      }
      const result = await commands[command](args || {});
      sendResult(id, result);
    } catch (err) {
      sendResult(id, null, err.message || String(err));
    }
  });
})();
