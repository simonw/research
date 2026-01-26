from starlette.applications import Starlette
from starlette.responses import PlainTextResponse
from starlette.routing import Route
from workers import WorkerEntrypoint
import asgi


async def hello(request):
    return PlainTextResponse("Hello World from Python + Starlette on Cloudflare Workers!")


app = Starlette(routes=[
    Route("/", hello),
    Route("/hello", hello),
])


class Default(WorkerEntrypoint):
    async def fetch(self, request):
        return await asgi.fetch(app, request.js_object, self.env)
