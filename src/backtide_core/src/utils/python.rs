//! Shared utilities to convert from/to Python objects.

use crate::config::interface::Config;
use crate::config::models::dataframe_library::DataFrameLibrary;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;

/// Build a DataFrame from a Python dict, using the configured backend.
///
/// Inspects [`Config::data::dataframe_library`] and dispatches to the
/// matching library constructor (numpy, pandas, or polars).
pub fn dict_to_dataframe<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    match Config::get()?.data.dataframe_library {
        DataFrameLibrary::Numpy => {
            let np = py.import("numpy")?;
            let values: Vec<Bound<'py, PyAny>> = data.values().iter().collect();
            let values_list = PyList::new(py, &values)?;
            np.call_method1("column_stack", (values_list,))
        },
        DataFrameLibrary::Pandas => {
            let pd = py.import("pandas")?;
            pd.call_method1("DataFrame", (data,))
        },
        DataFrameLibrary::Polars => {
            let pl = py.import("polars")?;
            pl.call_method1("from_dict", (data,))
        },
    }
}

/// Load a Python object from a pickle file.
pub fn load_pickle(py: Python<'_>, path: &PathBuf) -> PyResult<Py<PyAny>> {
    let builtins = py.import("builtins")?;
    let cloudpickle = py.import("cloudpickle")?;

    let f = builtins.call_method1("open", (path.to_string_lossy().to_string(), "rb"))?;
    let obj = cloudpickle.call_method1("load", (&f,))?;
    f.call_method0("close")?;

    Ok(obj.unbind())
}
