use crate::data::models::asset::Asset;
use pyo3::pyclass;

#[pyclass(from_py_object, get_all, module = "backtide.data")]
#[derive(Clone, Debug)]
pub struct DownloadValidation {
    /// All assets including injected forex dependencies.
    pub assets: Vec<Asset>,
}
