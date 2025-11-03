import pytest

import h3o_python as h3o


@pytest.fixture(scope="module")
def golden_cell() -> int:
    """Golden cell index for downtown San Francisco at resolution 9."""
    return 617700169958293503


def test_latlng_to_cell_known_value(golden_cell: int) -> None:
    cell = h3o.latlng_to_cell(37.775938728915946, -122.41795063018799, 9)
    assert cell == golden_cell


def test_cell_to_latlng_roundtrip(golden_cell: int) -> None:
    lat, lng = h3o.cell_to_latlng(golden_cell)
    # The cell center should be close to the original coordinate (within ~100 m).
    assert lat == pytest.approx(37.776, abs=0.01)
    assert lng == pytest.approx(-122.418, abs=0.01)


def test_grid_disk_neighbors(golden_cell: int) -> None:
    neighbors = h3o.grid_disk(golden_cell, 1)
    assert golden_cell in neighbors
    assert len(neighbors) == 7


def test_string_conversions(golden_cell: int) -> None:
    as_string = h3o.cell_to_string(golden_cell)
    assert h3o.string_to_cell(as_string) == golden_cell


def test_neighbor_relationship(golden_cell: int) -> None:
    neighbors = h3o.grid_disk(golden_cell, 1)
    # Remove the cell itself to probe adjacency checks.
    neighbor = next(value for value in neighbors if value != golden_cell)
    assert h3o.are_neighbors(golden_cell, neighbor)


def test_distance_km() -> None:
    paris = (48.864716, 2.349014)
    shanghai = (31.224361, 121.46917)
    distance = h3o.great_circle_distance_km(*paris, *shanghai)
    assert distance == pytest.approx(9262.5475, rel=1e-6)


def test_average_hexagon_area() -> None:
    assert h3o.average_hexagon_area_km2(5) == pytest.approx(252.90386, rel=1e-6)
