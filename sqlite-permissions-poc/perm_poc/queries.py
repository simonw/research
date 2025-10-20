
import sqlite3
import json

SCHEMA_SQL = """
CREATE TABLE resources (
  db   TEXT NOT NULL,
  tbl  TEXT NOT NULL,
  PRIMARY KEY (db, tbl)
);
"""

SAMPLE_DATA_SQL = """
INSERT INTO resources(db, tbl) VALUES
  ('analytics',  'users'),
  ('analytics',  'events'),
  ('analytics',  'sensitive'),
  ('production', 'customers'),
  ('production', 'orders');
"""

MAIN_QUERY_SQL = """
WITH
actor AS (
  SELECT
    json_extract(:actor_json, '$.id')   AS actor_id,
    json_extract(:actor_json, '$.role') AS actor_role
),
base AS ( SELECT db, tbl FROM resources ),

hook1_config AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_id FROM actor) = 'alice' THEN 1 ELSE NULL END allow,
         'config: alice has global allow' AS reason
),
hook2_role_based AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE NULL END allow,
         'role-based: admins have global allow' AS reason
  UNION ALL
  SELECT 'analytics', NULL,
         CASE WHEN (SELECT actor_role FROM actor) = 'analyst' THEN 1 ELSE NULL END allow,
         'role-based: analysts can access analytics DB' AS reason
),
hook3_security AS (
  SELECT 'analytics' parent, 'sensitive' child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE 0 END allow,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin'
              THEN 'security: admin exception for analytics.sensitive'
              ELSE 'security: sensitive tables denied except for admins'
         END AS reason
),
hook4_business AS (
  SELECT 'production' parent, NULL child, 0 allow,
  'business: production DB denied during maintenance window' AS reason
),
hook5_business_exception AS (
  SELECT 'production' parent, 'orders' child,
         CASE WHEN (SELECT actor_id FROM actor) IN ('carol') THEN 1 ELSE NULL END allow,
         'business-exception: allow production.orders for carol' AS reason
),
all_rules AS (
  SELECT * FROM hook1_config
  UNION ALL SELECT * FROM hook2_role_based
  UNION ALL SELECT * FROM hook3_security
  UNION ALL SELECT * FROM hook4_business
  UNION ALL SELECT * FROM hook5_business_exception
),
child_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child = b.tbl
  GROUP BY b.db, b.tbl
),
parent_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
global_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent IS NULL AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
decisions AS (
  SELECT
    b.db, b.tbl,
    CASE
      WHEN cl.any_deny = 1 THEN 0
      WHEN cl.any_allow = 1 THEN 1
      WHEN pl.any_deny = 1 THEN 0
      WHEN pl.any_allow = 1 THEN 1
      WHEN gl.any_deny = 1 THEN 0
      WHEN gl.any_allow = 1 THEN 1
      ELSE 0
    END AS is_allowed
  FROM base b
  JOIN child_lvl  cl USING (db, tbl)
  JOIN parent_lvl pl USING (db, tbl)
  JOIN global_lvl gl USING (db, tbl)
)
SELECT db, tbl
FROM decisions
WHERE is_allowed = 1
ORDER BY db, tbl;
"""

TOKEN_QUERY_SQL = """
WITH
actor AS (
  SELECT
    json_extract(:actor_json, '$.id')   AS actor_id,
    json_extract(:actor_json, '$.role') AS actor_role
),
base AS ( SELECT db, tbl FROM resources ),

hook1_config AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_id FROM actor) = 'alice' THEN 1 ELSE NULL END allow,
         'config: alice has global allow' AS reason
),
hook2_role_based AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE NULL END allow,
         'role-based: admins have global allow' AS reason
  UNION ALL
  SELECT 'analytics', NULL,
         CASE WHEN (SELECT actor_role FROM actor) = 'analyst' THEN 1 ELSE NULL END allow,
         'role-based: analysts can access analytics DB' AS reason
),
hook3_security AS (
  SELECT 'analytics' parent, 'sensitive' child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE 0 END allow,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin'
              THEN 'security: admin exception for analytics.sensitive'
              ELSE 'security: sensitive tables denied except for admins'
         END AS reason
),
hook4_business AS (
  SELECT 'production' parent, NULL child, 0 allow,
  'business: production DB denied during maintenance window' AS reason
),
hook5_business_exception AS (
  SELECT 'production' parent, 'orders' child,
         CASE WHEN (SELECT actor_id FROM actor) IN ('carol') THEN 1 ELSE NULL END allow,
         'business-exception: allow production.orders for carol' AS reason
),
all_rules AS (
  SELECT * FROM hook1_config
  UNION ALL SELECT * FROM hook2_role_based
  UNION ALL SELECT * FROM hook3_security
  UNION ALL SELECT * FROM hook4_business
  UNION ALL SELECT * FROM hook5_business_exception
),
child_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child = b.tbl
  GROUP BY b.db, b.tbl
),
parent_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
global_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent IS NULL AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
decisions AS (
  SELECT
    b.db, b.tbl,
    CASE
      WHEN cl.any_deny = 1 THEN 0
      WHEN cl.any_allow = 1 THEN 1
      WHEN pl.any_deny = 1 THEN 0
      WHEN pl.any_allow = 1 THEN 1
      WHEN gl.any_deny = 1 THEN 0
      WHEN gl.any_allow = 1 THEN 1
      ELSE 0
    END AS is_allowed
  FROM base b
  JOIN child_lvl  cl USING (db, tbl)
  JOIN parent_lvl pl USING (db, tbl)
  JOIN global_lvl gl USING (db, tbl)
),
actor_allowed AS ( SELECT db, tbl FROM decisions WHERE is_allowed = 1 ),
token_input AS (
  SELECT json_extract(j.value, '$.db')  AS db,
         json_extract(j.value, '$.tbl') AS tbl
  FROM json_each(COALESCE(:token_json, json('{}')), '$.resources') AS j
),
token_input_has_rows AS ( SELECT COUNT(*) AS n FROM token_input ),
token_allowed AS (
  SELECT r.db, r.tbl
  FROM resources r
  WHERE (SELECT n FROM token_input_has_rows) = 0
  UNION
  SELECT r.db, r.tbl
  FROM token_input ti
  JOIN resources r ON r.db = ti.db AND (ti.tbl IS NULL OR r.tbl = ti.tbl)
  WHERE (SELECT n FROM token_input_has_rows) > 0
)
SELECT db, tbl
FROM actor_allowed
INTERSECT
SELECT db, tbl
FROM token_allowed
ORDER BY db, tbl;
"""

CONFLICT_QUERY_SQL = """
WITH
actor AS (
  SELECT
    json_extract(:actor_json, '$.id')   AS actor_id,
    json_extract(:actor_json, '$.role') AS actor_role
),
base AS ( SELECT db, tbl FROM resources ),

hook6_conflict AS (
  SELECT 'production' parent, NULL child,
         CASE WHEN (SELECT actor_id FROM actor) = 'eve' THEN 1 ELSE NULL END allow,
         'conflict: allow production DB for eve' AS reason
),
hook1_config AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_id FROM actor) = 'alice' THEN 1 ELSE NULL END allow,
         'config: alice has global allow' AS reason
),
hook2_role_based AS (
  SELECT NULL parent, NULL child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE NULL END allow,
         'role-based: admins have global allow' AS reason
  UNION ALL
  SELECT 'analytics', NULL,
         CASE WHEN (SELECT actor_role FROM actor) = 'analyst' THEN 1 ELSE NULL END allow,
         'role-based: analysts can access analytics DB' AS reason
),
hook3_security AS (
  SELECT 'analytics' parent, 'sensitive' child,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin' THEN 1 ELSE 0 END allow,
         CASE WHEN (SELECT actor_role FROM actor) = 'admin'
              THEN 'security: admin exception for analytics.sensitive'
              ELSE 'security: sensitive tables denied except for admins'
         END AS reason
),
hook4_business AS (
  SELECT 'production' parent, NULL child, 0 allow,
  'business: production DB denied during maintenance window' AS reason
),
hook5_business_exception AS (
  SELECT 'production' parent, 'orders' child,
         CASE WHEN (SELECT actor_id FROM actor) IN ('carol') THEN 1 ELSE NULL END allow,
         'business-exception: allow production.orders for carol' AS reason
),
all_rules AS (
  SELECT * FROM hook6_conflict
  UNION ALL SELECT * FROM hook1_config
  UNION ALL SELECT * FROM hook2_role_based
  UNION ALL SELECT * FROM hook3_security
  UNION ALL SELECT * FROM hook4_business
  UNION ALL SELECT * FROM hook5_business_exception
),
child_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child = b.tbl
  GROUP BY b.db, b.tbl
),
parent_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent = b.db AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
global_lvl AS (
  SELECT b.db, b.tbl,
         MAX(CASE WHEN ar.allow = 0 THEN 1 ELSE 0 END) AS any_deny,
         MAX(CASE WHEN ar.allow = 1 THEN 1 ELSE 0 END) AS any_allow
  FROM base b
  LEFT JOIN all_rules ar ON ar.parent IS NULL AND ar.child IS NULL
  GROUP BY b.db, b.tbl
),
decisions AS (
  SELECT
    b.db, b.tbl,
    CASE
      WHEN cl.any_deny = 1 THEN 0
      WHEN cl.any_allow = 1 THEN 1
      WHEN pl.any_deny = 1 THEN 0
      WHEN pl.any_allow = 1 THEN 1
      WHEN gl.any_deny = 1 THEN 0
      WHEN gl.any_allow = 1 THEN 1
      ELSE 0
    END AS is_allowed
  FROM base b
  JOIN child_lvl  cl USING (db, tbl)
  JOIN parent_lvl pl USING (db, tbl)
  JOIN global_lvl gl USING (db, tbl)
)
SELECT db, tbl
FROM decisions
WHERE is_allowed = 1
ORDER BY db, tbl;
"""

def connect_and_seed():
    conn = sqlite3.connect(":memory:")
    conn.executescript(SCHEMA_SQL)
    conn.executescript(SAMPLE_DATA_SQL)
    return conn

def _dump(obj):
    return json.dumps(obj)

def run_main(conn, actor_json):
    params = {"actor_json": _dump(actor_json)}
    cur = conn.execute(MAIN_QUERY_SQL, params)
    return cur.fetchall()

def run_token(conn, actor_json, token_json):
    params = {"actor_json": _dump(actor_json),
              "token_json": (None if token_json is None else _dump(token_json))}
    cur = conn.execute(TOKEN_QUERY_SQL, params)
    return cur.fetchall()

def run_with_conflict(conn, actor_json):
    params = {"actor_json": _dump(actor_json)}
    cur = conn.execute(CONFLICT_QUERY_SQL, params)
    return cur.fetchall()
