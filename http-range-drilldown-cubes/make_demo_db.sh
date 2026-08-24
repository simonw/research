#!/usr/bin/env bash
# Regenerate cube_demo.db: a synthetic 200,000-event 311-ish dataset rolled up
# into 8 grouping sets in a `cube` table, clustered by a WITHOUT ROWID primary
# key ordered (gset, filter columns..., period). Input for pack_cube.py.
# Output is ~38MB, which is why the .db itself is not committed.
set -euo pipefail
rm -f cube_demo.db
sqlite3 cube_demo.db <<'SQL'
.bail on
CREATE TABLE requests (created_date TEXT, agency TEXT, complaint_type TEXT,
                       submission_type TEXT, borough TEXT);
WITH RECURSIVE seq(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM seq WHERE i < 200000)
INSERT INTO requests
SELECT
  datetime('2010-01-01', '+' || (abs(random()) % 5844) || ' days',
                         '+' || (abs(random()) % 86400) || ' seconds'),
  CASE abs(random())%5 WHEN 0 THEN 'NYPD' WHEN 1 THEN 'HPD' WHEN 2 THEN 'DOT'
                       WHEN 3 THEN 'DSNY' ELSE 'DEP' END,
  CASE abs(random())%6 WHEN 0 THEN 'Noise - Residential' WHEN 1 THEN 'Illegal Parking'
                       WHEN 2 THEN 'HEAT/HOT WATER' WHEN 3 THEN 'Blocked Driveway'
                       WHEN 4 THEN 'Street Condition' ELSE 'Water Leak' END,
  CASE abs(random())%3 WHEN 0 THEN 'PHONE' WHEN 1 THEN 'ONLINE' ELSE 'MOBILE' END,
  CASE abs(random())%5 WHEN 0 THEN 'BROOKLYN' WHEN 1 THEN 'QUEENS' WHEN 2 THEN 'MANHATTAN'
                       WHEN 3 THEN 'BRONX' ELSE 'STATEN ISLAND' END
FROM seq;

CREATE TABLE cube (
  gset TEXT NOT NULL, agency TEXT NOT NULL, complaint_type TEXT NOT NULL,
  submission_type TEXT NOT NULL, borough TEXT NOT NULL, period TEXT NOT NULL,
  n INTEGER NOT NULL,
  PRIMARY KEY (gset, agency, complaint_type, submission_type, borough, period)
) WITHOUT ROWID;

WITH src AS (
  SELECT date(created_date) AS day,
         date(created_date,'weekday 0','-6 days') AS week,   -- Monday of week
         strftime('%Y',created_date) AS year,
         agency, complaint_type, submission_type, borough
  FROM requests
)
INSERT INTO cube
SELECT 'total:agency', agency, '*','*','*','*', count(*) FROM src GROUP BY agency
UNION ALL SELECT 'total:complaint_type','*',complaint_type,'*','*','*',count(*) FROM src GROUP BY complaint_type
UNION ALL SELECT 'total:submission_type','*','*',submission_type,'*','*',count(*) FROM src GROUP BY submission_type
UNION ALL SELECT 'total:borough','*','*','*',borough,'*',count(*) FROM src GROUP BY borough
UNION ALL SELECT 'day:overall','*','*','*','*',day,count(*) FROM src GROUP BY day
UNION ALL SELECT 'day:full',agency,complaint_type,submission_type,borough,day,count(*)
  FROM src GROUP BY agency,complaint_type,submission_type,borough,day
UNION ALL SELECT 'week:full',agency,complaint_type,submission_type,borough,week,count(*)
  FROM src GROUP BY agency,complaint_type,submission_type,borough,week
UNION ALL SELECT 'year:full',agency,complaint_type,submission_type,borough,year,count(*)
  FROM src GROUP BY agency,complaint_type,submission_type,borough,year;

SELECT gset, count(*) AS n_rows, sum(n) AS events FROM cube GROUP BY gset ORDER BY gset;
SQL
echo "wrote cube_demo.db"
