
# SQLite Hierarchical Permission System — Proof of Concept

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This project demonstrates a **fully SQLite-based hierarchical permission system** with **cascading rules**, **DENY precedence**, and **token-based access restriction**.  

It is designed to illustrate how a complex RBAC-like model can be implemented purely in SQL, without custom functions or extensions.

---

## 🧠 Overview

This system answers the question:

> “Given an actor and (optionally) a restricted API token, what resources can they access?”

Resources are organized hierarchically (global → database → table).  
Permissions are evaluated in **three levels**:
- **Global** (affects everything)
- **Parent** (database-level)
- **Child** (table-level)

The result is a set of database/table pairs the actor is authorized to access, after applying both **actor rules** and **token restrictions**.

---

## ✨ Key Features

- ✅ Pure SQLite — no UDFs or extensions required.
- 🪜 Hierarchical permission cascade (child > parent > global).
- 🛑 DENY takes precedence over ALLOW at the same level.
- 🆗 ALLOW can override DENY from less specific levels.
- 🧩 Token scoping — allows restricting access to a **subset** of resources.
- 🧪 Comprehensive pytest test suite (11 scenarios).
- 📝 Simple schema and extensible logic.

---

## 🏗️ Architecture

```
actor (JSON)
   │
   ▼
+-----------+
| SQL Rules |
+-----------+
   │
   ▼
[child rules] ──► [parent rules] ──► [global rules]
   │                    │                │
   ▼                    ▼                ▼
 DENY beats ALLOW at same level
 Child beats Parent beats Global
   │
   ▼
 Actor's allowed resources
   │
   ▼
∩ (INTERSECT)
   │
   ▼
 Token restricted resources (optional)
   │
   ▼
 Final allowed resource list
```

---

## 🗂️ Data Model

### Resources

```sql
CREATE TABLE resources (
  db  TEXT NOT NULL,
  tbl TEXT NOT NULL,
  PRIMARY KEY (db, tbl)
);
```
Example data:

| db          | tbl         |
|-------------|------------|
| analytics   | users      |
| analytics   | events     |
| analytics   | sensitive  |
| production  | customers  |
| production  | orders     |

---

### Permission Rules

Rules are defined via multiple **"hooks"**, simulating different subsystems:

| Hook | Example                           | Effect                                  |
|------|------------------------------------|------------------------------------------|
| 1    | Config                             | `alice` gets global allow               |
| 2    | Role-based                         | analysts get analytics DB               |
| 3    | Security                           | deny sensitive except admins            |
| 4    | Business logic                     | deny production DB                      |
| 5    | Exception                          | allow carol → production.orders         |

---

### Actors

Actors are represented as JSON:

```json
{"id":"alice","role":"analyst"}
```

The query extracts `$.id` and `$.role` via SQLite's JSON functions.

---

### Tokens

A token can **restrict** what resources can be accessed.  
It **cannot grant** access beyond what the actor already has.

Example token JSON:

```json
{
  "resources": [
    {"db": "analytics", "tbl": "users"},
    {"db": "production", "tbl": null}
  ]
}
```

- `tbl: null` → expands to all tables in that database.
- If the token has no resources or is `NULL` → unrestricted (no intersection applied).

---

## ⚖️ Cascading Logic

For each resource:

1. **Child level** (db + table)
   - If any DENY → block immediately
   - Else if any ALLOW → grant
   - Else → check parent
2. **Parent level** (db only)
   - If any DENY → block
   - Else if any ALLOW → grant
   - Else → check global
3. **Global level**
   - If any DENY → block
   - Else if any ALLOW → grant
   - Else → system default (DENY)

At the same specificity level:
- **DENY beats ALLOW**

Across specificity levels:
- **Child beats Parent beats Global**

---

## 🔐 Token Restrictions

Token rules are applied **after actor permissions** via `INTERSECT`:

```
final_permissions = actor_allowed INTERSECT token_allowed
```

This guarantees a token can only **further restrict**, never grant.

---

## 🧮 How the SQL Works

- **CTEs** are used to structure the query clearly.
- Each level (child, parent, global) computes whether any DENY or ALLOW applies.
- A `CASE` expression collapses the results according to the cascade rules.
- The token query parses JSON using `json_each`.
- If token is empty → bypass intersection.

---

## 🧪 Running the Project

Requirements:

- Python 3.8+
- `pytest`

### 1. Create a virtual environment

```bash
python -m venv venv
source venv/bin/activate        # Windows: venv\Scriptsctivate
```

### 2. Install dependencies (pytest)

```bash
pip install -r requirements.txt
```

---

## 🧪 Running the Tests

```bash
pytest
```

All 11 tests should pass. They cover:

- ✅ Global allow (alice)
- ✅ Parent allow / child deny conflict (analyst role)
- ✅ Child allow overrides parent deny (carol)
- ✅ Default deny when no rules
- ✅ Token restricting access correctly
- ✅ Token cannot grant denied resource
- ✅ Empty token → unrestricted
- ✅ Parent conflict deny beats allow (eve)

---

## 🧪 Example Scenarios

| Actor                | Token                      | Result                                    |
|-----------------------|-----------------------------|--------------------------------------------|
| alice (admin)         | none                        | all resources except denied production     |
| dave (analyst)        | none                        | analytics.* minus sensitive                |
| carol                 | none                        | production.orders only                     |
| dave + token(users)   | restrict to users only      | analytics.users only                       |
| dave + token(sensitive)| try override deny          | analytics.users only (deny wins)           |
| carol + token(*prod)  | intersect allow             | production.orders                          |

---

## 🧰 Extending the System

- Add **actions** (e.g. `view-table`, `edit-table`) → extend schema with `action` column.
- Implement **group memberships** by enriching `actor` extraction.
- Add **time-based rules** (e.g. valid during business hours).
- Extend **token semantics** to include action scoping.

---

## 🏁 Conclusion

This proof of concept shows how to implement a **robust, cascading permission system** in **pure SQLite SQL**, with no external logic required.

It’s simple, portable, and ideal for:
- Embedded systems
- Local-first apps
- Lightweight SaaS products
- Teaching access control models

---

**Author**: POC generated via ChatGPT  

## Original prompt

Here's the prompt I [used with GPT-5](https://chatgpt.com/share/68f6532f-9920-8006-928a-364e15b6e9ef) to create this research POC (I had Claude generate most of this prompt based on an [extensive conversation](https://claude.ai/share/8fd432bc-a718-4883-9978-80ab82a75c87).)

----

Use your Python tool

Create a SQLite proof-of-concept that demonstrates a hierarchical permission system with the following requirements:

## System Overview

Build a permission checking system entirely in SQLite SQL (no custom functions) that determines which resources an actor can access based on cascading rules.

## Resource Hierarchy

Use database/table as the example hierarchy:

- **Global level**: permissions that apply to everything
- **Parent level**: permissions on a specific database (applies to all tables in that database)
- **Child level**: permissions on a specific table within a database

## Permission Rules

Rules come from multiple sources (simulating “hooks”) and each rule specifies:

- `parent`: database name or NULL (for global)
- `child`: table name or NULL (for parent-level grants)
- `allow`: 1 for ALLOW, 0 for DENY, NULL for ABSTAIN
- `reason`: human-readable explanation string

## Cascading Logic

When checking if an actor can perform action on a specific resource (e.g., “can alice view-table on analytics/users”):

1. Check child level (database=‘analytics’, table=‘users’): If any DENY, block immediately. If any ALLOW, grant. If all NULL, continue.
1. Check parent level (database=‘analytics’, table=NULL): If any DENY, block. If any ALLOW, grant. If all NULL, continue.
1. Check global level (database=NULL, table=NULL): If any DENY, block. If any ALLOW, grant. If all NULL, use system default (DENY).

**Critical rule**: At the same specificity level, DENY beats ALLOW. Across specificity levels, child beats parent beats global.

## Demo Requirements

Create a SQLite database with:

1. **Base resources table**: A list of all databases and tables that exist

```sql
-- Example data:
-- analytics, users
-- analytics, events  
-- analytics, sensitive
-- production, customers
-- production, orders
```

1. **Permission rules from multiple “hooks”**: Simulate 3-4 different rule sources

```sql
-- Example rules:
-- Hook 1 (config): alice has global allow
-- Hook 2 (role-based): anyone with analyst role can access analytics database
-- Hook 3 (security): sensitive tables denied except for admins
-- Hook 4 (business logic): production database denied during maintenance windows
```

1. **Actor data**: Pass actor as JSON string parameter like `'{"id":"alice","role":"analyst"}'`
1. **Main query**: A single SQL query using CTEs that:

- Starts with base resources
- UNIONs all permission rules
- Applies cascading logic (child → parent → global)
- Applies DENY-beats-ALLOW at each level
- Returns final list of database/table pairs the actor can access

1. **Test cases**: Show query results for several scenarios:

- Actor with global allow → sees everything
- Actor with parent-level allow → sees all tables in that database
- Actor with child-level allow → sees only that specific table
- Actor with child-level deny overriding parent-level allow

## Output Format

Provide:

1. Complete SQL schema (CREATE TABLE statements)
1. Sample data (INSERT statements)
1. The main permission-checking query with extensive comments explaining the cascading logic
1. Multiple test queries showing different scenarios with their results
1. Explanation of how the SQL implements the cascading rules

## Key Constraints

- Pure SQLite SQL only (no UDFs, no extensions)
- Must handle NULL (ABSTAIN) correctly
- Use CTEs for readability
- Must correctly implement “DENY beats ALLOW at same level” and “child beats parent across levels”
- Show that child-level ALLOW can override parent-level DENY
- Show that child-level DENY can override parent-level ALLOW

Make the demo realistic enough to prove the concept works but simple enough to understand the SQL logic
