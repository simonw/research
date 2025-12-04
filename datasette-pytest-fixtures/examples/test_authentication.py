"""
Example tests demonstrating authentication and permission testing.

These examples show how to test authenticated requests and permissions.
"""

import pytest
from datasette_pytest.helpers import actor_cookies, assert_permission_denied


@pytest.mark.asyncio
async def test_authenticated_requests(ds, authenticated_client):
    """
    Make requests as an authenticated user.

    The `authenticated_client` factory creates clients with actor cookies.
    """
    # Create a client authenticated as a specific user
    client = authenticated_client(ds, actor_id="alice")

    # The request includes the actor cookie
    response = await client.get("/-/actor.json")
    assert response.status_code == 200
    assert response.json()["actor"]["id"] == "alice"


@pytest.mark.asyncio
async def test_root_user(ds, root_client):
    """
    Make requests as the root user.

    The `root_client` fixture is a shortcut for root authentication.
    """
    client = root_client(ds)

    response = await client.get("/-/actor.json")
    assert response.status_code == 200
    assert response.json()["actor"]["id"] == "root"


@pytest.mark.asyncio
async def test_post_with_csrf(ds, root_client, memory_db):
    """
    POST with automatic CSRF token handling.

    The `post_with_csrf` method handles CSRF tokens automatically.
    """
    client = root_client(ds)
    db = memory_db(ds, "data")
    await db.execute_write("CREATE TABLE dogs (id INTEGER PRIMARY KEY, name TEXT)")

    # POST with automatic CSRF handling
    response = await client.post_with_csrf(
        "/data/-/create-token",
        data={}  # Your form data here
    )
    # The CSRF token was automatically fetched and included
    # (This will fail with 404 since create-token isn't a real endpoint,
    # but demonstrates the pattern)


@pytest.mark.asyncio
async def test_permission_denied(ds):
    """
    Test that unauthenticated users are denied access.
    """
    # Without authentication, protected endpoints should return 403
    response = await ds.client.get("/-/create-token")
    assert_permission_denied(response)


@pytest.mark.asyncio
async def test_actor_with_roles(ds, authenticated_client):
    """
    Test with a complex actor including roles.
    """
    # Full actor dict with custom attributes
    client = authenticated_client(ds, actor={
        "id": "alice",
        "roles": ["editor", "viewer"],
        "email": "alice@example.com"
    })

    response = await client.get("/-/actor.json")
    actor = response.json()["actor"]
    assert actor["id"] == "alice"
    assert actor["roles"] == ["editor", "viewer"]
    assert actor["email"] == "alice@example.com"


@pytest.mark.asyncio
async def test_manual_actor_cookies(ds):
    """
    Create actor cookies manually for lower-level control.

    Use `actor_cookies()` when you need manual cookie management.
    """
    cookies = actor_cookies(ds, "bob")

    response = await ds.client.get("/-/actor.json", cookies=cookies)
    assert response.json()["actor"]["id"] == "bob"

    # Can combine with other cookies
    other_cookies = {"some_cookie": "value"}
    all_cookies = {**cookies, **other_cookies}
    response2 = await ds.client.get("/-/actor.json", cookies=all_cookies)
    assert response2.json()["actor"]["id"] == "bob"


@pytest.mark.asyncio
@pytest.mark.requires_sqlite_utils
async def test_permissions_with_config(ds_with_db):
    """
    Test permission configuration via config dict.
    """
    ds = ds_with_db(
        {"private_data": [{"id": 1, "secret": "shhh"}]},
        config={
            "databases": {
                "test": {
                    "allow": {"id": "admin"}
                }
            }
        }
    )

    # Anonymous user can't access
    response = await ds.client.get("/test/private_data.json")
    assert response.status_code == 403

    # Admin can access
    cookies = actor_cookies(ds, "admin")
    response2 = await ds.client.get("/test/private_data.json", cookies=cookies)
    assert response2.status_code == 200
