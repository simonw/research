use std::str::FromStr;

use h3o::{CellIndex, LatLng, Resolution};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn parse_resolution(resolution: u8) -> PyResult<Resolution> {
    Resolution::try_from(resolution).map_err(|err| PyValueError::new_err(err.to_string()))
}

fn parse_latlng(lat: f64, lng: f64) -> PyResult<LatLng> {
    LatLng::new(lat, lng).map_err(|err| PyValueError::new_err(err.to_string()))
}

fn parse_cell(cell: u64) -> PyResult<CellIndex> {
    CellIndex::try_from(cell).map_err(|err| PyValueError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(text_signature = "(lat, lng, resolution)")]
fn latlng_to_cell(lat: f64, lng: f64, resolution: u8) -> PyResult<u64> {
    let latlng = parse_latlng(lat, lng)?;
    let resolution = parse_resolution(resolution)?;
    Ok(u64::from(latlng.to_cell(resolution)))
}

#[pyfunction]
#[pyo3(text_signature = "(cell)")]
fn cell_to_latlng(cell: u64) -> PyResult<(f64, f64)> {
    let cell = parse_cell(cell)?;
    let latlng = LatLng::from(cell);
    Ok((latlng.lat(), latlng.lng()))
}

#[pyfunction]
#[pyo3(text_signature = "(cell, k)")]
fn grid_disk(cell: u64, k: u32) -> PyResult<Vec<u64>> {
    let cell = parse_cell(cell)?;
    Ok(cell
        .grid_disk::<Vec<_>>(k)
        .into_iter()
        .map(u64::from)
        .collect())
}

#[pyfunction]
#[pyo3(text_signature = "(cell)")]
fn cell_area_km2(cell: u64) -> PyResult<f64> {
    let cell = parse_cell(cell)?;
    Ok(cell.area_km2())
}

#[pyfunction]
#[pyo3(text_signature = "(resolution)")]
fn average_hexagon_area_km2(resolution: u8) -> PyResult<f64> {
    let resolution = parse_resolution(resolution)?;
    Ok(resolution.area_km2())
}

#[pyfunction]
#[pyo3(text_signature = "(cell_a, cell_b)")]
fn are_neighbors(cell_a: u64, cell_b: u64) -> PyResult<bool> {
    let cell_a = parse_cell(cell_a)?;
    let cell_b = parse_cell(cell_b)?;
    cell_a
        .is_neighbor_with(cell_b)
        .map_err(|err| PyValueError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(text_signature = "(lat1, lng1, lat2, lng2)")]
fn great_circle_distance_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> PyResult<f64> {
    let src = parse_latlng(lat1, lng1)?;
    let dst = parse_latlng(lat2, lng2)?;
    Ok(src.distance_km(dst))
}

#[pyfunction]
#[pyo3(text_signature = "(value)")]
fn string_to_cell(value: &str) -> PyResult<u64> {
    let cell = CellIndex::from_str(value).map_err(|err| PyValueError::new_err(err.to_string()))?;
    Ok(u64::from(cell))
}

#[pyfunction]
#[pyo3(text_signature = "(cell)")]
fn cell_to_string(cell: u64) -> PyResult<String> {
    let cell = parse_cell(cell)?;
    Ok(cell.to_string())
}

#[pymodule]
fn _core(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(latlng_to_cell, m)?)?;
    m.add_function(wrap_pyfunction!(cell_to_latlng, m)?)?;
    m.add_function(wrap_pyfunction!(grid_disk, m)?)?;
    m.add_function(wrap_pyfunction!(cell_area_km2, m)?)?;
    m.add_function(wrap_pyfunction!(average_hexagon_area_km2, m)?)?;
    m.add_function(wrap_pyfunction!(are_neighbors, m)?)?;
    m.add_function(wrap_pyfunction!(great_circle_distance_km, m)?)?;
    m.add_function(wrap_pyfunction!(string_to_cell, m)?)?;
    m.add_function(wrap_pyfunction!(cell_to_string, m)?)?;
    Ok(())
}
