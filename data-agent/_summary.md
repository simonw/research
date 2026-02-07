Data-agent leverages LLMs to automate the creation of deterministic data extraction scripts by observing browser interactions and analyzing network traffic. By correlating user actions with API responses captured in HAR recordings, the system generates standalone [Playwright](https://playwright.dev/) scripts that can be replayed without requiring further LLM inference. This architecture supports a self-correcting validation loop and provides a [Model Context Protocol](https://modelcontextprotocol.io/) server for seamless integration with AI coding assistants.

* Natural language intent parsing for automated site exploration and navigation.
* API-first extraction that prioritizes intercepting network responses over brittle HTML selectors.
* Automatic error classification and script refinement through a five-iteration validation loop.
* Global registry for saved extractors to enable high-speed, deterministic replays.
