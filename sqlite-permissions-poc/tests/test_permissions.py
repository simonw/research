
import pytest
from perm_poc.queries import connect_and_seed, run_main, run_token, run_with_conflict

@pytest.fixture()
def conn():
    c = connect_and_seed()
    try:
        yield c
    finally:
        c.close()

def test_admin_global_allow_actor_only(conn):
    rows = run_main(conn, {"id":"alice","role":"admin"})
    assert rows == [
        ('analytics','events'),
        ('analytics','sensitive'),
        ('analytics','users'),
    ]

def test_analyst_parent_allow_child_deny(conn):
    rows = run_main(conn, {"id":"dave","role":"analyst"})
    assert rows == [
        ('analytics','events'),
        ('analytics','users'),
    ]

def test_child_allow_overrides_parent_deny(conn):
    rows = run_main(conn, {"id":"carol","role":"viewer"})
    assert rows == [
        ('production','orders'),
    ]

def test_default_deny_when_no_rules(conn):
    rows = run_main(conn, {"id":"zoe","role":"viewer"})
    assert rows == []

def test_token_unrestricted_behaves_like_actor(conn):
    actor_rows = run_main(conn, {"id":"dave","role":"analyst"})
    token_rows = run_token(conn, {"id":"dave","role":"analyst"}, None)
    assert actor_rows == token_rows

def test_token_limits_admin_to_specific_resources(conn):
    rows = run_token(conn,
                     {"id":"alice","role":"admin"},
                     {"resources":[{"db":"analytics","tbl":"users"},
                                   {"db":"production","tbl":None}]})
    assert rows == [('analytics','users')]

def test_token_requests_unavailable_db_drops_invalid(conn):
    rows = run_token(conn,
                     {"id":"bob","role":"analyst"},
                     {"resources":[{"db":"analytics","tbl":"foobar"},
                                   {"db":"analytics","tbl":"events"}]})
    assert rows == [('analytics','events')]

def test_token_empty_list_is_unrestricted(conn):
    actor_rows = run_main(conn, {"id":"carol","role":"viewer"})
    token_rows = run_token(conn, {"id":"carol","role":"viewer"}, {"resources":[]})
    assert actor_rows == token_rows == [('production','orders')]

def test_token_production_star_intersects_carol_child_allow(conn):
    rows = run_token(conn,
                     {"id":"carol","role":"viewer"},
                     {"resources":[{"db":"production","tbl":None}]})
    assert rows == [('production','orders')]

def test_token_cannot_override_child_deny(conn):
    rows = run_token(conn,
                     {"id":"dave","role":"analyst"},
                     {"resources":[{"db":"analytics","tbl":"sensitive"},
                                   {"db":"analytics","tbl":"users"}]})
    assert rows == [('analytics','users')]

def test_same_level_parent_deny_beats_allow(conn):
    rows = run_with_conflict(conn, {"id":"eve","role":"viewer"})
    assert rows == []
