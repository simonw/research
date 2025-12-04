"""
Helpers for testing Datasette plugins.

These utilities help with registering test plugins, mocking plugin
behavior, and tracking plugin events.
"""

import contextlib
from typing import Any, Callable, Dict, List, Optional

from datasette import hookimpl


class TrackEventPlugin:
    """
    A plugin that tracks all events fired by Datasette.

    This is useful for asserting that specific events were fired
    during test execution.

    Usage:
        # In conftest.py
        @pytest.fixture(scope="session", autouse=True)
        def install_event_tracking():
            from datasette.plugins import pm
            from datasette_pytest import TrackEventPlugin

            pm.register(TrackEventPlugin(), name="TrackEventPlugin")
            yield
            pm.unregister(name="TrackEventPlugin")

        # In tests
        async def test_creates_event(ds):
            # ... perform action ...
            events = getattr(ds, "_tracked_events", [])
            assert any(e.name == "insert-row" for e in events)
    """

    __name__ = "TrackEventPlugin"

    @hookimpl
    def track_event(self, datasette, event):
        """Store events on the Datasette instance for later inspection."""
        if not hasattr(datasette, "_tracked_events"):
            datasette._tracked_events = []
        datasette._tracked_events.append(event)


@contextlib.contextmanager
def register_plugin(datasette, plugin, name: str = None):
    """
    Context manager for temporarily registering a plugin.

    The plugin is automatically unregistered when the context exits.

    Args:
        datasette: The Datasette instance
        plugin: The plugin instance to register
        name: Name for the plugin (defaults to plugin class name)

    Usage:
        class MyTestPlugin:
            __name__ = "MyTestPlugin"

            @hookimpl
            def register_routes(self):
                return [(r"^/test$", my_handler)]

        with register_plugin(ds, MyTestPlugin()):
            response = await ds.client.get("/test")
            assert response.status_code == 200
    """
    if name is None:
        name = getattr(plugin, "__name__", plugin.__class__.__name__)

    # Ensure plugin has __name__ attribute
    if not hasattr(plugin, "__name__"):
        plugin.__name__ = name

    datasette.pm.register(plugin, name=name)
    try:
        yield plugin
    finally:
        datasette.pm.unregister(name=name)


def create_test_plugin(
    name: str = "TestPlugin",
    routes: List[tuple] = None,
    startup_fn: Callable = None,
    extra_template_vars_fn: Callable = None,
    actor_from_request_fn: Callable = None,
    permission_checks: Dict[str, bool] = None,
    menu_links: List[Dict] = None,
    table_actions: List[Dict] = None,
    database_actions: List[Dict] = None,
    **hook_impls
):
    """
    Factory function to create test plugins with common hook implementations.

    This makes it easy to create plugins inline in tests without
    boilerplate.

    Args:
        name: Name for the plugin
        routes: List of (regex, handler) tuples for register_routes
        startup_fn: Async function to call on startup
        extra_template_vars_fn: Function returning extra template variables
        actor_from_request_fn: Function to determine actor from request
        permission_checks: Dict mapping (action, resource) -> allow boolean
        menu_links: List of menu link dicts
        table_actions: List of table action dicts
        database_actions: List of database action dicts
        **hook_impls: Additional hook implementations as keyword args

    Returns:
        A plugin instance ready to be registered

    Usage:
        plugin = create_test_plugin(
            routes=[
                (r"^/hello$", lambda: Response.text("Hello"))
            ],
            menu_links=[
                {"href": "/hello", "label": "Say Hello"}
            ]
        )

        with register_plugin(ds, plugin):
            response = await ds.client.get("/hello")
    """

    class GeneratedTestPlugin:
        __name__ = name

    plugin = GeneratedTestPlugin()

    # Register routes hook
    if routes:
        @hookimpl
        def register_routes():
            return routes
        plugin.register_routes = register_routes

    # Startup hook
    if startup_fn:
        @hookimpl
        def startup(datasette):
            return startup_fn(datasette)
        plugin.startup = startup

    # Extra template vars hook
    if extra_template_vars_fn:
        @hookimpl
        def extra_template_vars(datasette, view_name):
            return extra_template_vars_fn(datasette, view_name)
        plugin.extra_template_vars = extra_template_vars

    # Actor from request hook
    if actor_from_request_fn:
        @hookimpl
        def actor_from_request(datasette, request):
            return actor_from_request_fn(datasette, request)
        plugin.actor_from_request = actor_from_request

    # Permission allowed hook for simple permission mocking
    if permission_checks is not None:
        @hookimpl
        def permission_allowed(datasette, actor, action, resource):
            key = (action, resource) if resource else action
            if key in permission_checks:
                return permission_checks[key]
            # Also check just action without resource
            if action in permission_checks:
                return permission_checks[action]
            return None
        plugin.permission_allowed = permission_allowed

    # Menu links hook
    if menu_links:
        @hookimpl
        def menu_links_hook(datasette, actor, request):
            return menu_links
        plugin.menu_links = menu_links_hook

    # Table actions hook
    if table_actions:
        @hookimpl
        def table_actions_hook(datasette, actor, database, table):
            return table_actions
        plugin.table_actions = table_actions_hook

    # Database actions hook
    if database_actions:
        @hookimpl
        def database_actions_hook(datasette, actor, database):
            return database_actions
        plugin.database_actions = database_actions_hook

    # Handle additional hooks passed as kwargs
    for hook_name, impl in hook_impls.items():
        decorated = hookimpl(impl)
        setattr(plugin, hook_name, decorated)

    return plugin


class PermissionMocker:
    """
    Helper for mocking Datasette permissions in tests.

    Provides a clean way to set up permission rules for testing
    without creating full plugin implementations.

    Usage:
        mocker = PermissionMocker()
        mocker.allow("edit-schema", database="mydb")
        mocker.deny("drop-table")

        with register_plugin(ds, mocker.as_plugin()):
            # edit-schema is now allowed for mydb
            # drop-table is denied everywhere
            response = await ds.client.get("/-/edit-schema/mydb")
            assert response.status_code == 200
    """

    def __init__(self):
        self._rules: List[Dict] = []

    def allow(
        self,
        action: str,
        actor_id: str = None,
        database: str = None,
        resource: str = None,
    ):
        """Allow an action, optionally scoped to actor/database/resource."""
        self._rules.append({
            "action": action,
            "actor_id": actor_id,
            "database": database,
            "resource": resource,
            "allow": True,
        })
        return self

    def deny(
        self,
        action: str,
        actor_id: str = None,
        database: str = None,
        resource: str = None,
    ):
        """Deny an action, optionally scoped to actor/database/resource."""
        self._rules.append({
            "action": action,
            "actor_id": actor_id,
            "database": database,
            "resource": resource,
            "allow": False,
        })
        return self

    def _matches(self, rule: Dict, action: str, actor, database: str, resource: str) -> bool:
        """Check if a rule matches the given context."""
        if rule["action"] != action:
            return False

        if rule["actor_id"] is not None:
            actor_id = actor.get("id") if actor else None
            if actor_id != rule["actor_id"]:
                return False

        if rule["database"] is not None:
            if database != rule["database"]:
                return False

        if rule["resource"] is not None:
            if resource != rule["resource"]:
                return False

        return True

    def as_plugin(self, name: str = "PermissionMocker"):
        """
        Create a plugin instance from the defined rules.

        Returns:
            A plugin ready to be registered with register_plugin()
        """
        rules = self._rules

        class PermissionMockerPlugin:
            __name__ = name

            @hookimpl
            def permission_allowed(self, datasette, actor, action, resource):
                # Parse resource into database and table/resource
                database = None
                table_resource = None
                if isinstance(resource, tuple):
                    if len(resource) >= 1:
                        database = resource[0]
                    if len(resource) >= 2:
                        table_resource = resource[1]
                elif isinstance(resource, str):
                    database = resource

                for rule in rules:
                    if self._matches_rule(rule, action, actor, database, table_resource):
                        return rule["allow"]
                return None

            def _matches_rule(self, rule, action, actor, database, resource):
                if rule["action"] != action:
                    return False

                if rule["actor_id"] is not None:
                    actor_id = actor.get("id") if actor else None
                    if actor_id != rule["actor_id"]:
                        return False

                if rule["database"] is not None:
                    if database != rule["database"]:
                        return False

                if rule["resource"] is not None:
                    if resource != rule["resource"]:
                        return False

                return True

        return PermissionMockerPlugin()


def create_route_handler(
    response_text: str = None,
    response_json: Any = None,
    response_html: str = None,
    status_code: int = 200,
    headers: Dict[str, str] = None,
):
    """
    Create a simple route handler for testing.

    Args:
        response_text: Plain text response body
        response_json: JSON response body (will be serialized)
        response_html: HTML response body
        status_code: HTTP status code
        headers: Additional response headers

    Returns:
        An async handler function for use with register_routes

    Usage:
        plugin = create_test_plugin(
            routes=[
                (r"^/api/test$", create_route_handler(
                    response_json={"status": "ok"}
                )),
                (r"^/error$", create_route_handler(
                    response_text="Error occurred",
                    status_code=500
                ))
            ]
        )
    """
    from datasette.utils.asgi import Response
    import json

    async def handler(datasette, request):
        if response_json is not None:
            return Response.json(response_json, status=status_code, headers=headers)
        elif response_html is not None:
            return Response.html(response_html, status=status_code, headers=headers)
        else:
            return Response.text(
                response_text or "",
                status=status_code,
                headers=headers
            )

    return handler
