//! Shared utilities to convert from/to Python objects.

use crate::config::interface::Config;
use crate::config::models::DataFrameLibrary;
use crate::data::models::Bar;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::Path;

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
    data.extract::<Vec<Vec<f64>>>()
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

    let expected = c.len();
    if [o.len(), h.len(), l.len(), v.len()].into_iter().any(|length| length != expected) {
        return Err(PyValueError::new_err(
            "OHLCV columns must all contain the same number of rows.",
        ));
    }

    Ok(o.into_iter()
        .zip(h)
        .zip(l)
        .zip(c)
        .zip(v)
        .map(|((((open, high), low), close), volume)| Bar {
            open_ts: 0,
            close_ts: 0,
            open_ts_exchange: 0,
            open,
            high,
            low,
            close,
            adj_close: close,
            volume,
            n_trades: None,
        })
        .collect())
}

/// Convert data into the configured data backend format.
///
/// The result is shaped as (n_points, n_series), i.e., rows x columns.
/// Single-series return a 1-D array / single-column frame.
pub fn to_python<'py>(py: Python<'py>, data: &[Vec<f64>]) -> PyResult<Bound<'py, PyAny>> {
    let backend = Config::get()?.data.dataframe_library;

    if let [values] = data {
        // Single series → 1-D
        match backend {
            DataFrameLibrary::Pandas => {
                let pd = py.import("pandas")?;
                pd.call_method1("Series", (values,))
            },
            DataFrameLibrary::Polars => {
                let pl = py.import("polars")?;
                pl.call_method1("Series", (values,))
            },
        }
    } else {
        // Multiple series → transpose to (n_points, n_series)
        let np = py.import("numpy")?;
        let arr_2d = np.call_method1("array", (data,))?;
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
pub fn load_pickle(py: Python<'_>, path: &Path) -> PyResult<Py<PyAny>> {
    let builtins = py.import("builtins")?;
    let cloudpickle = py.import("cloudpickle")?;

    let f = builtins.call_method1("open", (path.to_string_lossy().to_string(), "rb"))?;
    let loaded = cloudpickle.call_method1("load", (&f,));
    let closed = f.call_method0("close");
    match loaded {
        Ok(obj) => {
            closed?;
            Ok(obj.unbind())
        },
        Err(error) => {
            let _ = closed;
            Err(error)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyDict;
    use tempfile::TempDir;

    #[test]
    fn extract_bars_rejects_unequal_column_lengths() {
        Python::attach(|py| {
            let data = PyDict::new(py);
            data.set_item("open", vec![1.0, 2.0]).unwrap();
            data.set_item("high", vec![2.0]).unwrap();
            data.set_item("low", vec![0.5, 1.5]).unwrap();
            data.set_item("close", vec![1.5, 2.5]).unwrap();

            let error = extract_bars_from_python(data.as_any()).unwrap_err();

            assert!(error.is_instance_of::<PyValueError>(py));
            assert!(error.to_string().contains("same number of rows"));
        });
    }

    #[test]
    fn multi_series_python_conversion_uses_the_configured_dataframe_backend() {
        Python::attach(|py| {
            let converted = to_python(py, &[vec![1.0, 2.0], vec![3.0, 4.0]]).unwrap();

            assert_eq!(converted.len().unwrap(), 2);
            assert_eq!(
                converted.getattr("shape").unwrap().extract::<(usize, usize)>().unwrap(),
                (2, 2)
            );
        });
    }

    #[test]
    fn load_pickle_returns_a_successfully_deserialized_object() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("valid.pkl");

        Python::attach(|py| {
            let builtins = py.import("builtins").unwrap();
            let cloudpickle = py.import("cloudpickle").unwrap();
            let file =
                builtins.call_method1("open", (path.to_string_lossy().to_string(), "wb")).unwrap();
            let value = PyDict::new(py);
            value.set_item("answer", 42).unwrap();
            cloudpickle.call_method1("dump", (&value, &file)).unwrap();
            file.call_method0("close").unwrap();

            let loaded = load_pickle(py, &path).unwrap();
            assert_eq!(loaded.bind(py).get_item("answer").unwrap().extract::<i32>().unwrap(), 42);
        });
    }

    #[test]
    fn load_pickle_closes_file_after_deserialization_error() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("invalid.pkl");
        std::fs::write(&path, b"not a pickle").unwrap();

        Python::attach(|py| assert!(load_pickle(py, &path).is_err()));

        std::fs::remove_file(path).unwrap();
    }
}
