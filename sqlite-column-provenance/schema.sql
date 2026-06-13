CREATE TABLE t1 (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);
CREATE TABLE t2 (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);
INSERT INTO t1 (id, name, extra) VALUES (1, 'alice', 'x'), (2, 'bob', 'y');
INSERT INTO t2 (id, name, age) VALUES (10, 'alice', 30), (11, 'carol', 40);
