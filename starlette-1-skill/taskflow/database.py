"""Database layer using aiosqlite for the task management app."""

import aiosqlite
import os
from datetime import datetime, timezone

DB_PATH = os.path.join(os.path.dirname(__file__), "taskflow.db")


async def get_db() -> aiosqlite.Connection:
    db = await aiosqlite.connect(DB_PATH)
    db.row_factory = aiosqlite.Row
    await db.execute("PRAGMA journal_mode=WAL")
    await db.execute("PRAGMA foreign_keys=ON")
    return db


async def init_db():
    db = await get_db()
    try:
        await db.executescript("""
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                color TEXT DEFAULT '#6366f1',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS labels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT DEFAULT '#8b5cf6',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                description TEXT DEFAULT '',
                status TEXT DEFAULT 'todo' CHECK(status IN ('todo','in_progress','done')),
                priority TEXT DEFAULT 'medium' CHECK(priority IN ('low','medium','high','urgent')),
                due_date TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS task_labels (
                task_id INTEGER NOT NULL,
                label_id INTEGER NOT NULL,
                PRIMARY KEY (task_id, label_id),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (label_id) REFERENCES labels(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                content TEXT NOT NULL,
                author TEXT DEFAULT 'You',
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
        """)
        await db.commit()

        # Seed some data if empty
        cursor = await db.execute("SELECT COUNT(*) as cnt FROM projects")
        row = await cursor.fetchone()
        if row["cnt"] == 0:
            await _seed_data(db)
    finally:
        await db.close()


async def _seed_data(db: aiosqlite.Connection):
    now = _now()

    # Labels
    labels = [
        ("Bug", "#ef4444"), ("Feature", "#3b82f6"), ("Enhancement", "#10b981"),
        ("Documentation", "#f59e0b"), ("Design", "#ec4899"), ("Backend", "#6366f1"),
        ("Frontend", "#8b5cf6"), ("DevOps", "#14b8a6"),
    ]
    for name, color in labels:
        await db.execute(
            "INSERT INTO labels (name, color, created_at) VALUES (?, ?, ?)",
            (name, color, now),
        )

    # Projects
    projects = [
        ("Website Redesign", "Complete overhaul of the marketing website", "#6366f1"),
        ("Mobile App v2", "Next major version of the mobile application", "#ec4899"),
        ("API Platform", "Public API and developer documentation", "#10b981"),
    ]
    for name, desc, color in projects:
        await db.execute(
            "INSERT INTO projects (name, description, color, created_at, updated_at) VALUES (?,?,?,?,?)",
            (name, desc, color, now, now),
        )

    # Tasks
    tasks = [
        (1, "Design new homepage layout", "Create wireframes and high-fidelity mockups", "in_progress", "high", None),
        (1, "Implement responsive navigation", "Mobile-first nav with hamburger menu", "todo", "medium", None),
        (1, "Set up CI/CD pipeline", "GitHub Actions for staging deploys", "done", "high", None),
        (2, "Migrate to React Native 0.76", "Update all deps and fix breaking changes", "in_progress", "urgent", None),
        (2, "Add biometric authentication", "Face ID and fingerprint support", "todo", "high", None),
        (3, "Write OpenAPI spec", "Full spec for all public endpoints", "in_progress", "high", None),
        (3, "Rate limiting middleware", "Token bucket algorithm with Redis", "todo", "medium", None),
    ]
    for pid, title, desc, status, priority, due in tasks:
        await db.execute(
            "INSERT INTO tasks (project_id, title, description, status, priority, due_date, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?)",
            (pid, title, desc, status, priority, due, now, now),
        )

    # Task-labels
    task_labels = [(1, 5), (1, 7), (2, 7), (3, 8), (4, 7), (4, 1), (5, 2), (6, 4), (7, 6)]
    for tid, lid in task_labels:
        await db.execute("INSERT INTO task_labels (task_id, label_id) VALUES (?, ?)", (tid, lid))

    # Comments
    comments = [
        (1, "Started with low-fi wireframes in Figma. Sharing link tomorrow.", "Alice"),
        (1, "Looking great! Let's make sure we include the testimonials section.", "Bob"),
        (4, "Found 3 breaking changes in the navigation module. Documenting fixes.", "Charlie"),
        (6, "Using Redocly for the docs site. Draft ready for review.", "Alice"),
    ]
    for tid, content, author in comments:
        await db.execute(
            "INSERT INTO comments (task_id, content, author, created_at) VALUES (?,?,?,?)",
            (tid, content, author, now),
        )

    await db.commit()


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


# ── Project CRUD ──

async def get_projects(db):
    cursor = await db.execute("""
        SELECT p.*, COUNT(t.id) as task_count,
               SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END) as done_count
        FROM projects p LEFT JOIN tasks t ON t.project_id = p.id
        GROUP BY p.id ORDER BY p.created_at DESC
    """)
    return [dict(r) for r in await cursor.fetchall()]


async def get_project(db, project_id):
    cursor = await db.execute("SELECT * FROM projects WHERE id = ?", (project_id,))
    row = await cursor.fetchone()
    return dict(row) if row else None


async def create_project(db, name, description="", color="#6366f1"):
    now = _now()
    cursor = await db.execute(
        "INSERT INTO projects (name, description, color, created_at, updated_at) VALUES (?,?,?,?,?)",
        (name, description, color, now, now),
    )
    await db.commit()
    return cursor.lastrowid


async def update_project(db, project_id, **kwargs):
    kwargs["updated_at"] = _now()
    sets = ", ".join(f"{k} = ?" for k in kwargs)
    vals = list(kwargs.values()) + [project_id]
    await db.execute(f"UPDATE projects SET {sets} WHERE id = ?", vals)
    await db.commit()


async def delete_project(db, project_id):
    await db.execute("DELETE FROM projects WHERE id = ?", (project_id,))
    await db.commit()


# ── Task CRUD ──

async def get_tasks(db, project_id=None, status=None):
    query = """
        SELECT t.*, p.name as project_name, p.color as project_color
        FROM tasks t JOIN projects p ON t.project_id = p.id
        WHERE 1=1
    """
    params = []
    if project_id:
        query += " AND t.project_id = ?"
        params.append(project_id)
    if status:
        query += " AND t.status = ?"
        params.append(status)
    query += " ORDER BY CASE t.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, t.created_at DESC"
    cursor = await db.execute(query, params)
    tasks = [dict(r) for r in await cursor.fetchall()]

    # Attach labels
    for task in tasks:
        lc = await db.execute("""
            SELECT l.* FROM labels l
            JOIN task_labels tl ON tl.label_id = l.id
            WHERE tl.task_id = ?
        """, (task["id"],))
        task["labels"] = [dict(r) for r in await lc.fetchall()]

    return tasks


async def get_task(db, task_id):
    cursor = await db.execute("""
        SELECT t.*, p.name as project_name, p.color as project_color
        FROM tasks t JOIN projects p ON t.project_id = p.id
        WHERE t.id = ?
    """, (task_id,))
    row = await cursor.fetchone()
    if not row:
        return None
    task = dict(row)

    lc = await db.execute("""
        SELECT l.* FROM labels l JOIN task_labels tl ON tl.label_id = l.id WHERE tl.task_id = ?
    """, (task_id,))
    task["labels"] = [dict(r) for r in await lc.fetchall()]

    cc = await db.execute("SELECT * FROM comments WHERE task_id = ? ORDER BY created_at ASC", (task_id,))
    task["comments"] = [dict(r) for r in await cc.fetchall()]

    return task


async def create_task(db, project_id, title, description="", priority="medium", status="todo", due_date=None, label_ids=None):
    now = _now()
    cursor = await db.execute(
        "INSERT INTO tasks (project_id, title, description, status, priority, due_date, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?)",
        (project_id, title, description, status, priority, due_date, now, now),
    )
    task_id = cursor.lastrowid
    if label_ids:
        for lid in label_ids:
            await db.execute("INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?,?)", (task_id, lid))
    await db.commit()
    return task_id


async def update_task(db, task_id, label_ids=None, **kwargs):
    kwargs["updated_at"] = _now()
    sets = ", ".join(f"{k} = ?" for k in kwargs)
    vals = list(kwargs.values()) + [task_id]
    await db.execute(f"UPDATE tasks SET {sets} WHERE id = ?", vals)
    if label_ids is not None:
        await db.execute("DELETE FROM task_labels WHERE task_id = ?", (task_id,))
        for lid in label_ids:
            await db.execute("INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?,?)", (task_id, lid))
    await db.commit()


async def delete_task(db, task_id):
    await db.execute("DELETE FROM tasks WHERE id = ?", (task_id,))
    await db.commit()


# ── Comments ──

async def create_comment(db, task_id, content, author="You"):
    now = _now()
    cursor = await db.execute(
        "INSERT INTO comments (task_id, content, author, created_at) VALUES (?,?,?,?)",
        (task_id, content, author, now),
    )
    await db.commit()
    return cursor.lastrowid


async def delete_comment(db, comment_id):
    await db.execute("DELETE FROM comments WHERE id = ?", (comment_id,))
    await db.commit()


# ── Labels ──

async def get_labels(db):
    cursor = await db.execute("SELECT * FROM labels ORDER BY name")
    return [dict(r) for r in await cursor.fetchall()]


async def create_label(db, name, color="#8b5cf6"):
    now = _now()
    cursor = await db.execute(
        "INSERT INTO labels (name, color, created_at) VALUES (?,?,?)",
        (name, color, now),
    )
    await db.commit()
    return cursor.lastrowid


async def delete_label(db, label_id):
    await db.execute("DELETE FROM labels WHERE id = ?", (label_id,))
    await db.commit()
