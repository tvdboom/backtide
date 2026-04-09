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
#[pyclass(skip_from_py_object)]
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

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self) -> u64 {
        *self as u64
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
