"""
Example tests demonstrating basic usage of datasette-pytest fixtures.

These examples show the most common testing patterns for Datasette plugins.
"""

import pytest


@pytest.mark.asyncio
async def test_plugin_is_installed(ds):
    """
    Verify a plugin is installed.

    The `ds` fixture provides a simple in-memory Datasette instance
    with root enabled.
    """
    response = await ds.client.get("/-/plugins.json")
    assert response.status_code == 200
    # Check your plugin is in the list
    plugins = response.json()
    # assert any(p["name"] == "datasette-my-plugin" for p in plugins)


@pytest.mark.asyncio
async def test_memory_database(ds, memory_db):
    """
    Test with an in-memory database.

    The `memory_db` factory creates named in-memory databases.
    """
    # Create a database called "mydata"
    db = memory_db(ds, "mydata")

    # Populate it
    await db.execute_write("CREATE TABLE dogs (id INTEGER PRIMARY KEY, name TEXT)")
    await db.execute_write("INSERT INTO dogs (name) VALUES ('Cleo')")
    await db.execute_write("INSERT INTO dogs (name) VALUES ('Pancakes')")

    # Query via the API
    response = await ds.client.get("/mydata/dogs.json?_shape=array")
    assert response.status_code == 200
    assert response.json() == [
        {"id": 1, "name": "Cleo"},
        {"id": 2, "name": "Pancakes"},
    ]


@pytest.mark.asyncio
@pytest.mark.requires_sqlite_utils
async def test_with_prefilled_database(ds_with_db):
    """
    Test with a pre-populated database.

    The `ds_with_db` fixture accepts a dict of tables to create.
    """
    ds = ds_with_db({
        "dogs": [
            {"id": 1, "name": "Cleo", "age": 5},
            {"id": 2, "name": "Pancakes", "age": 4},
        ],
        "owners": [
            {"id": 1, "name": "Simon"},
        ]
    })

    # The data is immediately available
    response = await ds.client.get("/test/dogs.json?_shape=array")
    assert response.status_code == 200
    assert len(response.json()) == 2


@pytest.mark.asyncio
@pytest.mark.requires_sqlite_utils
async def test_custom_database_setup(ds_with_db):
    """
    Test with custom database setup using a function.

    Pass a callable for complex database setup.
    """
    def setup(db):
        db["dogs"].insert({"id": 1, "name": "Cleo"}, pk="id")
        db["dogs"].create_index(["name"])
        db.execute("CREATE VIEW happy_dogs AS SELECT * FROM dogs")

    ds = ds_with_db(setup)

    # Check the view exists
    response = await ds.client.get("/test.json")
    assert response.status_code == 200
    tables = {t["name"] for t in response.json()["tables"]}
    assert "dogs" in tables


@pytest.mark.asyncio
async def test_fresh_datasette_instances(fresh_ds):
    """
    Create multiple Datasette instances in one test.

    The `fresh_ds` factory creates new instances on demand.
    """
    # Create two differently configured instances
    ds1 = fresh_ds(settings={"max_returned_rows": 10})
    ds2 = fresh_ds(settings={"max_returned_rows": 100})

    # They have different settings
    settings1 = (await ds1.client.get("/-/settings.json")).json()
    settings2 = (await ds2.client.get("/-/settings.json")).json()

    assert settings1["max_returned_rows"] == 10
    assert settings2["max_returned_rows"] == 100


@pytest.mark.asyncio
async def test_home_page_loads(ds):
    """
    Simple smoke test that the home page loads.
    """
    response = await ds.client.get("/")
    assert response.status_code == 200
    assert "text/html" in response.headers.get("content-type", "")
