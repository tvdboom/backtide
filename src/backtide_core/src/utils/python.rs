//! Shared utilities to convert from/to Python objects.

use crate::config::interface::Config;
use crate::config::models::dataframe_library::DataFrameLibrary;
use crate::data::models::bar::Bar;
use pyo3::prelude::*;
use pyo3::types::{PyDict};
use std::path::PathBuf;

/// Build a DataFrame from a Python dict, using the configured backend.
pub fn dict_to_dataframe<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    match Config::get()?.data.dataframe_library {
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

/// Extract a 1d series from a Python object.
pub fn extract_1d_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    data.extract::<Vec<f64>>().or_else(|_| data.call_method0("to_numpy")?.extract::<Vec<f64>>())
}

/// Extract a 2d dataframe from a Python object.
pub fn extract_2d_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    data
        .extract::<Vec<Vec<f64>>>()
        .or_else(|_| data.call_method0("to_numpy")?.extract::<Vec<Vec<f64>>>())
}

/// Take a Python data object and return the corresponding bars.
pub fn extract_bars_from_python(df: &Bound<'_, PyAny>) -> PyResult<Vec<Bar>> {
    let extract_col = |name: &str| -> PyResult<Vec<f64>> {
        let col = df.get_item(name)?;
        extract_1d_from_python(&col)
    };

    let o = extract_col("open")?;
    let h = extract_col("high")?;
    let l = extract_col("low")?;
    let c = extract_col("close")?;
    let v = extract_col("volume").unwrap_or_else(|_| vec![0.0; c.len()]);

    Ok((0..c.len())
        .map(|i| Bar {
            open_ts: 0,
            close_ts: 0,
            open_ts_exchange: 0,
            open: o[i],
            high: h[i],
            low: l[i],
            close: c[i],
            adj_close: c[i],
            volume: v[i],
            n_trades: None,
        })
        .collect())
}

/// Convert data into the configured data backend format.
///
/// The result is shaped as (n_points, n_series), i.e., rows x columns.
/// Single-series return a 1-D array / single-column frame.
pub fn to_python(py: Python, series: Vec<Vec<f64>>) -> PyResult<Bound<PyAny>> {
    let backend = Config::get()?.data.dataframe_library;

    if series.len() == 1 {
        // Single series → 1-D
        let arr = series.into_iter().next().unwrap();
        match backend {
            DataFrameLibrary::Pandas => {
                let pd = py.import("pandas")?;
                pd.call_method1("Series", (&arr,))
            },
            DataFrameLibrary::Polars => {
                let pl = py.import("polars")?;
                pl.call_method1("Series", (&arr,))
            },
        }
    } else {
        // Multiple series → transpose to (n_points, n_series)
        let np = py.import("numpy")?;
        let arr_2d = np.call_method1("array", (series,))?;
        let arr_t = arr_2d.getattr("T")?;
        match backend {
            DataFrameLibrary::Pandas => {
                let pd = py.import("pandas")?;
                pd.call_method1("DataFrame", (&arr_t,))
            },
            DataFrameLibrary::Polars => {
                let pl = py.import("polars")?;
                pl.call_method1("from_numpy", (&arr_t,))
            },
        }
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
