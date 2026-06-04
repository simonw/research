"""TaskFlow — Task Management App built with Starlette 1.0."""

import json
from contextlib import asynccontextmanager

from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.cors import CORSMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse, HTMLResponse
from starlette.routing import Route, Mount
from starlette.staticfiles import StaticFiles

import database as db


# ── Lifespan ──

@asynccontextmanager
async def lifespan(app):
    await db.init_db()
    yield


# ── Helpers ──

async def _json_body(request: Request) -> dict:
    return await request.json()


def _ok(data=None, status=200):
    return JSONResponse(data or {"ok": True}, status_code=status)


def _err(msg, status=400):
    return JSONResponse({"error": msg}, status_code=status)


# ── HTML Page ──

async def index(request: Request):
    with open("templates/index.html", "r") as f:
        return HTMLResponse(f.read())


# ── Project Endpoints ──

async def projects_list(request: Request):
    conn = await db.get_db()
    try:
        projects = await db.get_projects(conn)
        return JSONResponse(projects)
    finally:
        await conn.close()


async def projects_create(request: Request):
    data = await _json_body(request)
    if not data.get("name"):
        return _err("Name is required")
    conn = await db.get_db()
    try:
        pid = await db.create_project(conn, data["name"], data.get("description", ""), data.get("color", "#6366f1"))
        project = await db.get_project(conn, pid)
        return JSONResponse(project, status_code=201)
    finally:
        await conn.close()


async def projects_detail(request: Request):
    pid = int(request.path_params["project_id"])
    conn = await db.get_db()
    try:
        project = await db.get_project(conn, pid)
        if not project:
            return _err("Project not found", 404)
        return JSONResponse(project)
    finally:
        await conn.close()


async def projects_update(request: Request):
    pid = int(request.path_params["project_id"])
    data = await _json_body(request)
    conn = await db.get_db()
    try:
        await db.update_project(conn, pid, **{k: v for k, v in data.items() if k in ("name", "description", "color")})
        project = await db.get_project(conn, pid)
        return JSONResponse(project)
    finally:
        await conn.close()


async def projects_delete(request: Request):
    pid = int(request.path_params["project_id"])
    conn = await db.get_db()
    try:
        await db.delete_project(conn, pid)
        return _ok()
    finally:
        await conn.close()


# ── Task Endpoints ──

async def tasks_list(request: Request):
    project_id = request.query_params.get("project_id")
    status = request.query_params.get("status")
    conn = await db.get_db()
    try:
        tasks = await db.get_tasks(conn, project_id=project_id, status=status)
        return JSONResponse(tasks)
    finally:
        await conn.close()


async def tasks_create(request: Request):
    data = await _json_body(request)
    if not data.get("title") or not data.get("project_id"):
        return _err("title and project_id are required")
    conn = await db.get_db()
    try:
        tid = await db.create_task(
            conn, data["project_id"], data["title"],
            data.get("description", ""), data.get("priority", "medium"),
            data.get("status", "todo"), data.get("due_date"),
            data.get("label_ids"),
        )
        task = await db.get_task(conn, tid)
        return JSONResponse(task, status_code=201)
    finally:
        await conn.close()


async def tasks_detail(request: Request):
    tid = int(request.path_params["task_id"])
    conn = await db.get_db()
    try:
        task = await db.get_task(conn, tid)
        if not task:
            return _err("Task not found", 404)
        return JSONResponse(task)
    finally:
        await conn.close()


async def tasks_update(request: Request):
    tid = int(request.path_params["task_id"])
    data = await _json_body(request)
    label_ids = data.pop("label_ids", None)
    allowed = {"title", "description", "status", "priority", "due_date", "project_id"}
    kwargs = {k: v for k, v in data.items() if k in allowed}
    conn = await db.get_db()
    try:
        await db.update_task(conn, tid, label_ids=label_ids, **kwargs)
        task = await db.get_task(conn, tid)
        return JSONResponse(task)
    finally:
        await conn.close()


async def tasks_delete(request: Request):
    tid = int(request.path_params["task_id"])
    conn = await db.get_db()
    try:
        await db.delete_task(conn, tid)
        return _ok()
    finally:
        await conn.close()


# ── Comment Endpoints ──

async def comments_create(request: Request):
    data = await _json_body(request)
    if not data.get("task_id") or not data.get("content"):
        return _err("task_id and content are required")
    conn = await db.get_db()
    try:
        cid = await db.create_comment(conn, data["task_id"], data["content"], data.get("author", "You"))
        return JSONResponse({"id": cid}, status_code=201)
    finally:
        await conn.close()


async def comments_delete(request: Request):
    cid = int(request.path_params["comment_id"])
    conn = await db.get_db()
    try:
        await db.delete_comment(conn, cid)
        return _ok()
    finally:
        await conn.close()


# ── Label Endpoints ──

async def labels_list(request: Request):
    conn = await db.get_db()
    try:
        labels = await db.get_labels(conn)
        return JSONResponse(labels)
    finally:
        await conn.close()


async def labels_create(request: Request):
    data = await _json_body(request)
    if not data.get("name"):
        return _err("Name is required")
    conn = await db.get_db()
    try:
        lid = await db.create_label(conn, data["name"], data.get("color", "#8b5cf6"))
        return JSONResponse({"id": lid}, status_code=201)
    finally:
        await conn.close()


async def labels_delete(request: Request):
    lid = int(request.path_params["label_id"])
    conn = await db.get_db()
    try:
        await db.delete_label(conn, lid)
        return _ok()
    finally:
        await conn.close()


# ── Dashboard Stats ──

async def stats(request: Request):
    conn = await db.get_db()
    try:
        cur = await conn.execute("SELECT COUNT(*) as c FROM projects")
        project_count = (await cur.fetchone())["c"]
        cur = await conn.execute("SELECT COUNT(*) as c FROM tasks")
        task_count = (await cur.fetchone())["c"]
        cur = await conn.execute("SELECT COUNT(*) as c FROM tasks WHERE status = 'done'")
        done_count = (await cur.fetchone())["c"]
        cur = await conn.execute("SELECT COUNT(*) as c FROM tasks WHERE priority = 'urgent'")
        urgent_count = (await cur.fetchone())["c"]
        cur = await conn.execute("SELECT COUNT(*) as c FROM comments")
        comment_count = (await cur.fetchone())["c"]
        return JSONResponse({
            "projects": project_count,
            "tasks": task_count,
            "done": done_count,
            "urgent": urgent_count,
            "comments": comment_count,
        })
    finally:
        await conn.close()


# ── Routes ──

routes = [
    Route("/", index),
    Mount("/api", routes=[
        Route("/stats", stats, methods=["GET"]),
        Route("/projects", projects_list, methods=["GET"]),
        Route("/projects", projects_create, methods=["POST"]),
        Route("/projects/{project_id:int}", projects_detail, methods=["GET"]),
        Route("/projects/{project_id:int}", projects_update, methods=["PUT"]),
        Route("/projects/{project_id:int}", projects_delete, methods=["DELETE"]),
        Route("/tasks", tasks_list, methods=["GET"]),
        Route("/tasks", tasks_create, methods=["POST"]),
        Route("/tasks/{task_id:int}", tasks_detail, methods=["GET"]),
        Route("/tasks/{task_id:int}", tasks_update, methods=["PUT"]),
        Route("/tasks/{task_id:int}", tasks_delete, methods=["DELETE"]),
        Route("/comments", comments_create, methods=["POST"]),
        Route("/comments/{comment_id:int}", comments_delete, methods=["DELETE"]),
        Route("/labels", labels_list, methods=["GET"]),
        Route("/labels", labels_create, methods=["POST"]),
        Route("/labels/{label_id:int}", labels_delete, methods=["DELETE"]),
    ]),
]

middleware = [
    Middleware(CORSMiddleware, allow_origins=["*"], allow_methods=["*"], allow_headers=["*"]),
]

app = Starlette(
    debug=True,
    routes=routes,
    middleware=middleware,
    lifespan=lifespan,
)
