WebMCP is a proposed browser API that enables web applications to expose structured, callable tools for AI agents, reducing the need for unreliable UI automation. This project demonstrates how to register and interact with WebMCP tools using a Python client over the Chrome DevTools Protocol (CDP), providing a bridge to discover and call these tools programmatically. While WebMCP’s native API allows only for tool registration (not querying or invocation), the demo introduces a custom registry (`window.__webmcp_tools`) to enable CDP-based automation. The approach is complementary to official efforts like [@mcp-b/global](https://www.npmjs.com/package/@mcp-b/global) and illustrates how AI agents can reliably manipulate page state through exposed APIs, with all code runnable on Chrome Canary 146+.

Key findings:
- Native WebMCP exposes only registration, unregistration, and context APIs; there is no built-in way to list or invoke tools from external code.
- CDP is leveraged through page script injection to discover and call registered tools programmatically.
- The demo registers seven common tools (e.g., get/set counter, add/delete notes), and workflows are validated end-to-end via headless Chrome and screenshots.
- [WebMCP Spec](https://webmachinelearning.github.io/webmcp/) provides further technical details.
