use pyo3::prelude::*;

// Import memchr crate functions with explicit paths
use ::memchr as memchr_crate;

/// Find the first occurrence of a byte in a haystack.
/// Returns the index of the first occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memchr")]
fn py_memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memchr(needle, haystack)
}

/// Find the first occurrence of either of two bytes in a haystack.
/// Returns the index of the first occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memchr2")]
fn py_memchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memchr2(needle1, needle2, haystack)
}

/// Find the first occurrence of any of three bytes in a haystack.
/// Returns the index of the first occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memchr3")]
fn py_memchr3(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memchr3(needle1, needle2, needle3, haystack)
}

/// Find the last occurrence of a byte in a haystack (reverse search).
/// Returns the index of the last occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memrchr")]
fn py_memrchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memrchr(needle, haystack)
}

/// Find the last occurrence of either of two bytes in a haystack (reverse search).
/// Returns the index of the last occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memrchr2")]
fn py_memrchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memrchr2(needle1, needle2, haystack)
}

/// Find the last occurrence of any of three bytes in a haystack (reverse search).
/// Returns the index of the last occurrence, or None if not found.
#[pyfunction]
#[pyo3(name = "memrchr3")]
fn py_memrchr3(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Option<usize> {
    memchr_crate::memrchr3(needle1, needle2, needle3, haystack)
}

/// Find all occurrences of a byte in a haystack.
/// Returns a list of all indices where the byte was found.
#[pyfunction]
#[pyo3(name = "memchr_iter")]
fn py_memchr_iter(needle: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memchr_iter(needle, haystack).collect()
}

/// Find all occurrences of either of two bytes in a haystack.
/// Returns a list of all indices where either byte was found.
#[pyfunction]
#[pyo3(name = "memchr2_iter")]
fn py_memchr2_iter(needle1: u8, needle2: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memchr2_iter(needle1, needle2, haystack).collect()
}

/// Find all occurrences of any of three bytes in a haystack.
/// Returns a list of all indices where any of the bytes was found.
#[pyfunction]
#[pyo3(name = "memchr3_iter")]
fn py_memchr3_iter(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memchr3_iter(needle1, needle2, needle3, haystack).collect()
}

/// Find all occurrences of a byte in a haystack in reverse order.
/// Returns a list of all indices where the byte was found, from last to first.
#[pyfunction]
#[pyo3(name = "memrchr_iter")]
fn py_memrchr_iter(needle: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memrchr_iter(needle, haystack).collect()
}

/// Find all occurrences of either of two bytes in a haystack in reverse order.
/// Returns a list of all indices where either byte was found, from last to first.
#[pyfunction]
#[pyo3(name = "memrchr2_iter")]
fn py_memrchr2_iter(needle1: u8, needle2: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memrchr2_iter(needle1, needle2, haystack).collect()
}

/// Find all occurrences of any of three bytes in a haystack in reverse order.
/// Returns a list of all indices where any byte was found, from last to first.
#[pyfunction]
#[pyo3(name = "memrchr3_iter")]
fn py_memrchr3_iter(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memrchr3_iter(needle1, needle2, needle3, haystack).collect()
}

/// Find the first occurrence of a substring in a haystack.
/// Returns the index of the first occurrence, or None if not found.
#[pyfunction]
fn memmem_find(needle: &[u8], haystack: &[u8]) -> Option<usize> {
    memchr_crate::memmem::find(haystack, needle)
}

/// Find the last occurrence of a substring in a haystack (reverse search).
/// Returns the index of the last occurrence, or None if not found.
#[pyfunction]
fn memmem_rfind(needle: &[u8], haystack: &[u8]) -> Option<usize> {
    memchr_crate::memmem::rfind(haystack, needle)
}

/// Find all occurrences of a substring in a haystack.
/// Returns a list of all indices where the substring starts.
#[pyfunction]
fn memmem_find_iter(needle: &[u8], haystack: &[u8]) -> Vec<usize> {
    memchr_crate::memmem::find_iter(haystack, needle).collect()
}

/// A precompiled substring finder for repeated searches.
/// This is more efficient when searching for the same needle in multiple haystacks.
#[pyclass]
struct Finder {
    needle: Vec<u8>,
}

#[pymethods]
impl Finder {
    /// Create a new Finder with the given needle.
    #[new]
    fn new(needle: &[u8]) -> Self {
        Finder {
            needle: needle.to_vec(),
        }
    }

    /// Find the first occurrence of the needle in the haystack.
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        let finder = memchr_crate::memmem::Finder::new(&self.needle);
        finder.find(haystack)
    }

    /// Find all occurrences of the needle in the haystack.
    fn find_iter(&self, haystack: &[u8]) -> Vec<usize> {
        let finder = memchr_crate::memmem::Finder::new(&self.needle);
        finder.find_iter(haystack).collect()
    }

    /// Get the needle as bytes.
    fn needle(&self) -> Vec<u8> {
        self.needle.clone()
    }
}

/// A precompiled substring finder for reverse searches.
#[pyclass]
struct FinderRev {
    needle: Vec<u8>,
}

#[pymethods]
impl FinderRev {
    /// Create a new FinderRev with the given needle.
    #[new]
    fn new(needle: &[u8]) -> Self {
        FinderRev {
            needle: needle.to_vec(),
        }
    }

    /// Find the last occurrence of the needle in the haystack.
    fn rfind(&self, haystack: &[u8]) -> Option<usize> {
        let finder = memchr_crate::memmem::FinderRev::new(&self.needle);
        finder.rfind(haystack)
    }

    /// Get the needle as bytes.
    fn needle(&self) -> Vec<u8> {
        self.needle.clone()
    }
}

/// Python bindings for the memchr Rust library.
///
/// This module provides high-performance byte and substring search functions
/// using SIMD optimizations on supported platforms (x86_64, aarch64, wasm32).
#[pymodule]
fn _pymemchr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Single byte search
    m.add_function(wrap_pyfunction!(py_memchr, m)?)?;
    m.add_function(wrap_pyfunction!(py_memchr2, m)?)?;
    m.add_function(wrap_pyfunction!(py_memchr3, m)?)?;

    // Reverse single byte search
    m.add_function(wrap_pyfunction!(py_memrchr, m)?)?;
    m.add_function(wrap_pyfunction!(py_memrchr2, m)?)?;
    m.add_function(wrap_pyfunction!(py_memrchr3, m)?)?;

    // Iterator functions
    m.add_function(wrap_pyfunction!(py_memchr_iter, m)?)?;
    m.add_function(wrap_pyfunction!(py_memchr2_iter, m)?)?;
    m.add_function(wrap_pyfunction!(py_memchr3_iter, m)?)?;
    m.add_function(wrap_pyfunction!(py_memrchr_iter, m)?)?;
    m.add_function(wrap_pyfunction!(py_memrchr2_iter, m)?)?;
    m.add_function(wrap_pyfunction!(py_memrchr3_iter, m)?)?;

    // Substring search
    m.add_function(wrap_pyfunction!(memmem_find, m)?)?;
    m.add_function(wrap_pyfunction!(memmem_rfind, m)?)?;
    m.add_function(wrap_pyfunction!(memmem_find_iter, m)?)?;

    // Classes
    m.add_class::<Finder>()?;
    m.add_class::<FinderRev>()?;

    Ok(())
}
