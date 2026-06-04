OpenAI's Skills API enables models to execute reusable, self-contained scripts and tools by packaging instructions and code (plus optional assets) with a `SKILL.md` manifest. This project demonstrates crafting a custom skill (“csv-insights”), uploading it via the `/v1/skills` endpoint, and invoking it in natural language through the Responses API’s hosted shell environment, where the model installs dependencies, executes scripts, and returns outputs such as markdown reports and plots. Further, it explores skill management operations like listing, retrieving, version pinning, inline (base64) skills, bundling assets, combining multiple skills, and lifecycle actions like deletion—confirming that skills are easily routable, modular, and production-ready. For details, see [OpenAI Skills API docs](https://platform.openai.com/docs/guides/skills) and the [Cookbook examples](https://github.com/openai/openai-cookbook/tree/main/examples/skills).

Key findings:
- Skills are auto-discovered and routed by models based on their `name` and `description`.
- Skills can be attached via upload (with version pinning) or inline as base64-encoded zips for prototyping.
- Asset bundling within skills allows test data and scripts to travel together, supporting fully self-contained workflows.
- Version must be a string in the API; deletion operations may be subject to eventual consistency.
