//! Forex pair definition.

use crate::data::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::fmt::{Display, Formatter};
use strum::{EnumIter, EnumString};

/// A standard forex currency pair.
///
/// Variant names are the conventional 6-character symbols (base + quote).
///
/// See Also
/// --------
/// - backtide.data:Country
/// - backtide.data:Currency
/// - backtide.data:Exchange
#[pyclass(skip_from_py_object, frozen, eq, hash)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    EnumIter,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(ascii_case_insensitive)]
#[allow(clippy::upper_case_acronyms)]
pub enum ForexPair {
    AUDCAD,
    AUDCHF,
    AUDJPY,
    AUDNZD,
    AUDUSD,
    CADJPY,
    CHFJPY,
    EURAUD,
    EURCAD,
    EURCHF,
    EURCZK,
    EURDKK,
    EURGBP,
    EURHUF,
    EURJPY,
    EURMXN,
    EURNOK,
    EURNZD,
    EURPLN,
    EURSEK,
    EURTRY,
    #[default]
    EURUSD,
    EURZAR,
    GBPAUD,
    GBPCAD,
    GBPCHF,
    GBPDKK,
    GBPJPY,
    GBPNOK,
    GBPNZD,
    GBPPLN,
    GBPSEK,
    GBPTRY,
    GBPUSD,
    GBPZAR,
    NZDCAD,
    NZDCHF,
    NZDJPY,
    NZDUSD,
    USDBRL,
    USDCAD,
    USDCHF,
    USDCNY,
    USDCZK,
    USDDKK,
    USDHKD,
    USDHUF,
    USDIDR,
    USDINR,
    USDJPY,
    USDKRW,
    USDMXN,
    USDMYR,
    USDNOK,
    USDPHP,
    USDPLN,
    USDRUB,
    USDSAR,
    USDSEK,
    USDSGD,
    USDTHB,
    USDTRY,
    USDTWD,
    USDZAR,
}

impl ForexPair {
    fn data(&self) -> (Currency, Currency) {
        use Currency::*;
        use ForexPair::*;
        match self {
            AUDCAD => (AUD, CAD),
            AUDCHF => (AUD, CHF),
            AUDJPY => (AUD, JPY),
            AUDNZD => (AUD, NZD),
            AUDUSD => (AUD, USD),
            CADJPY => (CAD, JPY),
            CHFJPY => (CHF, JPY),
            EURAUD => (EUR, AUD),
            EURCAD => (EUR, CAD),
            EURCHF => (EUR, CHF),
            EURCZK => (EUR, CZK),
            EURDKK => (EUR, DKK),
            EURGBP => (EUR, GBP),
            EURHUF => (EUR, HUF),
            EURJPY => (EUR, JPY),
            EURMXN => (EUR, MXN),
            EURNOK => (EUR, NOK),
            EURNZD => (EUR, NZD),
            EURPLN => (EUR, PLN),
            EURSEK => (EUR, SEK),
            EURTRY => (EUR, TRY),
            EURUSD => (EUR, USD),
            EURZAR => (EUR, ZAR),
            GBPAUD => (GBP, AUD),
            GBPCAD => (GBP, CAD),
            GBPCHF => (GBP, CHF),
            GBPDKK => (GBP, DKK),
            GBPJPY => (GBP, JPY),
            GBPNOK => (GBP, NOK),
            GBPNZD => (GBP, NZD),
            GBPPLN => (GBP, PLN),
            GBPSEK => (GBP, SEK),
            GBPTRY => (GBP, TRY),
            GBPUSD => (GBP, USD),
            GBPZAR => (GBP, ZAR),
            NZDCAD => (NZD, CAD),
            NZDCHF => (NZD, CHF),
            NZDJPY => (NZD, JPY),
            NZDUSD => (NZD, USD),
            USDBRL => (USD, BRL),
            USDCAD => (USD, CAD),
            USDCHF => (USD, CHF),
            USDCNY => (USD, CNY),
            USDCZK => (USD, CZK),
            USDDKK => (USD, DKK),
            USDHKD => (USD, HKD),
            USDHUF => (USD, HUF),
            USDIDR => (USD, IDR),
            USDINR => (USD, INR),
            USDJPY => (USD, JPY),
            USDKRW => (USD, KRW),
            USDMXN => (USD, MXN),
            USDMYR => (USD, MYR),
            USDNOK => (USD, NOK),
            USDPHP => (USD, PHP),
            USDPLN => (USD, PLN),
            USDRUB => (USD, RUB),
            USDSAR => (USD, SAR),
            USDSEK => (USD, SEK),
            USDSGD => (USD, SGD),
            USDTHB => (USD, THB),
            USDTRY => (USD, TRY),
            USDTWD => (USD, TWD),
            USDZAR => (USD, ZAR),
        }
    }

    pub fn base(&self) -> Currency {
        self.data().0
    }

    pub fn quote(&self) -> Currency {
        self.data().1
    }
}

impl Display for ForexPair {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base(), self.quote())
    }
}

#[pymethods]
impl ForexPair {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown forex pair: {s}")))
    }

    /// Support Python pickle serialization.
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (format!("{self:?}"),)))
    }
    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for ForexPair {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<ForexPair>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown forex pair {s:?}.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyInt, PyString};

    #[test]
    fn python_constructor_representation_and_extraction_cover_valid_and_invalid_values() {
        assert_eq!(ForexPair::new("EURUSD").unwrap(), ForexPair::EURUSD);
        assert!(ForexPair::new("invalid").is_err());
        assert_eq!(ForexPair::EURUSD.__repr__(), "EUR/USD");

        Python::attach(|py| {
            let direct = Py::new(py, ForexPair::GBPUSD).unwrap().into_bound(py).into_any();
            assert_eq!(direct.extract::<ForexPair>().unwrap(), ForexPair::GBPUSD);
            assert_eq!(
                PyString::new(py, "USDJPY").extract::<ForexPair>().unwrap(),
                ForexPair::USDJPY
            );
            assert!(PyString::new(py, "invalid").extract::<ForexPair>().is_err());
            assert!(PyInt::new(py, 1).extract::<ForexPair>().is_err());
        });
    }

    #[test]
    fn pickle_reconstructor_receives_parseable_symbol() {
        Python::attach(|py| {
            let (_, (symbol,)) = ForexPair::EURUSD.__reduce__(py).unwrap();

            assert_eq!(symbol, "EURUSD");
            assert_eq!(symbol.parse::<ForexPair>().unwrap(), ForexPair::EURUSD);
        });
    }
}
