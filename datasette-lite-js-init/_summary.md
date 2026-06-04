Datasette-lite faces a core limitation: HTML content injected via `innerHTML` does not execute embedded JavaScript, breaking interactive features and plugin functionality. The proposed solution introduces a standardized initialization event (`datasette_init`) triggered after each content update, allowing dependent scripts and plugins to reinitialize reliably. This approach uses a public API (`window.__DATASETTE_INIT__`) that can target specific DOM containers and signal reinitialization, ensuring clean-up between navigations and preserving backwards compatibility. By aligning with Datasette's event-driven JavaScript architecture, the solution enables smooth operation both in classic and single-page environments like Datasette-lite, with minimal code changes for plugin authors. Prototype files, example integration code, and migration guidelines are provided ([datasette-lite](https://github.com/simonw/datasette-lite), [Datasette core](https://github.com/simonw/datasette)).

**Key Findings:**
- Reinitialization event pattern solves SPA script execution.
- Plugins must scope DOM queries to injected content and handle clean-up.
- Solution does not require risky manual script eval or iframes.
- Maintains full compatibility with existing Datasette usage.
- Offers a clear migration strategy for plugin developers.
