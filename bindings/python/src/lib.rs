mod client;

use pyo3::prelude::*;

use client::{PyPlaylist, PyTrack, RustifyClient};

#[pymodule]
fn _rustify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<RustifyClient>()?;
    m.add_class::<PyTrack>()?;
    m.add_class::<PyPlaylist>()?;
    Ok(())
}
