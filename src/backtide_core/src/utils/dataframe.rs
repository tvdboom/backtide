//! Shared DataFrame construction utilities.

use crate::config::interface::Config;
use crate::config::models::dataframe_library::DataFrameLibrary;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

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
