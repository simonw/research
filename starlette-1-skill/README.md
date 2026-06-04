<!-- not-ai-generated -->
# Starlette 1.0 skill

See [SKILL.md](SKILL.md) for a Skill that shows how to build apps using Starlette 1.0. Read [Experimenting with Starlette 1.0 with Claude skills](https://simonwillison.net/2026/Mar/22/starlette/) for background on this project.

The demo application was created with this skill and the prompt:

> Build a task management app with Starlette, it should have projects and tasks and comments and labels

To run the demo application:
```bash
cd taskflow
uv run --with starlette --with uvicorn --with aiosqlite --with jinja2 uvicorn main:app --reload --port 8006
```

