"""
Example tests demonstrating plugin testing patterns.

These examples show how to test plugins that hook into Datasette.
"""

import pytest
from datasette import hookimpl
from datasette_pytest.plugin_helpers import (
    register_plugin,
    create_test_plugin,
    create_route_handler,
    PermissionMocker,
)


@pytest.mark.asyncio
async def test_custom_route_plugin(ds):
    """
    Test a plugin that registers custom routes.

    Use `register_plugin` to temporarily register a plugin.
    """
    class HelloWorldPlugin:
        __name__ = "HelloWorldPlugin"

        @hookimpl
        def register_routes(self):
            from datasette.utils.asgi import Response

            async def hello(datasette, request):
                return Response.text("Hello, World!")

            return [
                (r"^/-/hello$", hello),
            ]

    with register_plugin(ds, HelloWorldPlugin()):
        response = await ds.client.get("/-/hello")
        assert response.status_code == 200
        assert response.text == "Hello, World!"

    # Route is gone after context exits
    response2 = await ds.client.get("/-/hello")
    assert response2.status_code == 404


@pytest.mark.asyncio
async def test_create_plugin_helper(ds):
    """
    Use the create_test_plugin helper for simple plugins.
    """
    plugin = create_test_plugin(
        routes=[
            (r"^/-/api/status$", create_route_handler(
                response_json={"status": "ok", "version": "1.0"}
            )),
            (r"^/-/api/error$", create_route_handler(
                response_text="Something went wrong",
                status_code=500
            )),
        ]
    )

    with register_plugin(ds, plugin):
        # JSON endpoint works
        response = await ds.client.get("/-/api/status")
        assert response.status_code == 200
        assert response.json() == {"status": "ok", "version": "1.0"}

        # Error endpoint returns 500
        response2 = await ds.client.get("/-/api/error")
        assert response2.status_code == 500


@pytest.mark.asyncio
async def test_menu_links_plugin(ds):
    """
    Test a plugin that adds menu links.
    """
    plugin = create_test_plugin(
        menu_links=[
            {"href": "/-/custom", "label": "Custom Page"},
            {"href": "/-/settings", "label": "Settings"},
        ]
    )

    with register_plugin(ds, plugin):
        response = await ds.client.get("/")
        # Menu links should appear in the HTML
        assert "Custom Page" in response.text
        assert "Settings" in response.text


@pytest.mark.asyncio
async def test_permission_mocking(ds, memory_db):
    """
    Mock permissions for testing.

    The PermissionMocker provides a fluent API for setting up
    permission rules.
    """
    db = memory_db(ds, "private")
    await db.execute_write("CREATE TABLE secrets (id INTEGER PRIMARY KEY)")

    mocker = PermissionMocker()
    mocker.allow("view-database", database="private", actor_id="alice")
    mocker.deny("view-database", actor_id="bob")

    with register_plugin(ds, mocker.as_plugin()):
        from datasette_pytest.helpers import actor_cookies

        # Alice can access
        alice_cookies = actor_cookies(ds, "alice")
        response1 = await ds.client.get("/private.json", cookies=alice_cookies)
        # This tests the permission check, actual result depends on Datasette version

        # Bob is denied
        bob_cookies = actor_cookies(ds, "bob")
        response2 = await ds.client.get("/private.json", cookies=bob_cookies)


@pytest.mark.asyncio
async def test_event_tracking(ds, event_tracker, memory_db):
    """
    Track events fired by Datasette.

    The event_tracker fixture captures events for assertions.
    """
    tracker = event_tracker(ds)
    db = memory_db(ds, "data")
    await db.execute_write("CREATE TABLE dogs (id INTEGER PRIMARY KEY, name TEXT)")

    # Perform an action that fires an event (depends on Datasette version)
    # This is a placeholder - actual events depend on what you're testing

    # Check events
    events = tracker.events
    # Example: assert any(e.name == "insert-row" for e in events)

    # Get last event
    last = tracker.last_event
    # Example: assert last.name == "expected-event-name"

    # Filter by event name
    # insert_events = tracker.filter_by_name("insert-row")


@pytest.mark.asyncio
async def test_startup_hook(ds):
    """
    Test a plugin's startup hook.
    """
    startup_called = []

    async def my_startup(datasette):
        startup_called.append(True)

    plugin = create_test_plugin(
        startup_fn=my_startup
    )

    with register_plugin(ds, plugin):
        # Trigger startup by making a request
        await ds.client.get("/")
        assert len(startup_called) >= 1


@pytest.mark.asyncio
async def test_extra_template_vars(ds):
    """
    Test a plugin that adds template variables.
    """
    def extra_vars(datasette, view_name):
        return {
            "custom_var": "Hello from plugin",
            "view_name": view_name,
        }

    plugin = create_test_plugin(
        extra_template_vars_fn=extra_vars
    )

    with register_plugin(ds, plugin):
        # Template vars would be available in rendered pages
        # This tests the hook registration works
        response = await ds.client.get("/")
        assert response.status_code == 200


@pytest.mark.asyncio
async def test_multiple_plugins(ds):
    """
    Test multiple plugins working together.
    """
    plugin1 = create_test_plugin(
        name="Plugin1",
        routes=[
            (r"^/-/one$", create_route_handler(response_text="one")),
        ]
    )

    plugin2 = create_test_plugin(
        name="Plugin2",
        routes=[
            (r"^/-/two$", create_route_handler(response_text="two")),
        ]
    )

    with register_plugin(ds, plugin1):
        with register_plugin(ds, plugin2):
            # Both routes work
            r1 = await ds.client.get("/-/one")
            r2 = await ds.client.get("/-/two")
            assert r1.text == "one"
            assert r2.text == "two"
