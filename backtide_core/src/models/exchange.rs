//! Exchange definition.

use crate::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString};

/// A stock exchange identified by its ISO 10383 MIC (Market Identifier Code).
#[pyclass(skip_from_py_object)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumIter,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(ascii_case_insensitive)]
pub enum Exchange {
    XAMS,
    XASE,
    XASX,
    XBOM,
    XBRU,
    XCSE,
    XETR,
    XHEL,
    XHKG,
    XJPX,
    XKRX,
    XLIS,
    XLON,
    XMAD,
    XMIL,
    XNAS,
    XNCM,
    XNGS,
    XNSE,
    #[default]
    XNYS,
    XNZE,
    XOSL,
    XPAR,
    XSES,
    XSHE,
    XSHG,
    XSTO,
    XSWX,
    XTAI,
    XWBO,
}

impl Exchange {
    /// Get the full name, country, city, yahoo code and currency per exchange
    fn data(&self) -> (&'static str, &'static str, &'static str, &'static str, Currency) {
        use Currency::*;
        use Exchange::*;
        match self {
            XAMS => ("Euronext Amsterdam", "Netherlands", "Amsterdam", "AMS", EUR),
            XASE => ("NYSE American", "United States", "New York", "ASE", USD),
            XASX => ("Australian Securities Exchange", "Australia", "Sydney", "ASX", AUD),
            XBOM => ("Bombay Stock Exchange", "India", "Mumbai", "BSE", INR),
            XBRU => ("Euronext Brussels", "Belgium", "Brussels", "BRU", EUR),
            XCSE => ("Nasdaq Copenhagen", "Denmark", "Copenhagen", "CPH", DKK),
            XETR => ("XETRA", "Germany", "Frankfurt", "GER", EUR),
            XHEL => ("Nasdaq Helsinki", "Finland", "Helsinki", "HEL", EUR),
            XHKG => ("Hong Kong Stock Exchange", "Hong Kong", "Hong Kong", "HKG", HKD),
            XJPX => ("Japan Exchange Group", "Japan", "Tokyo", "JPX", JPY),
            XKRX => ("Korea Exchange", "South Korea", "Seoul", "KSC", KRW),
            XLIS => ("Euronext Lisbon", "Portugal", "Lisbon", "LIS", EUR),
            XLON => ("London Stock Exchange", "United Kingdom", "London", "LSE", GBP),
            XMAD => ("Bolsa de Madrid", "Spain", "Madrid", "MCE", EUR),
            XMIL => ("Borsa Italiana", "Italy", "Milan", "MIL", EUR),
            XNAS => ("NASDAQ Global Select Market", "United States", "New York", "NMS", USD),
            XNCM => ("NASDAQ Capital Market", "United States", "New York", "NCM", USD),
            XNGS => ("NASDAQ Global Market", "United States", "New York", "NGM", USD),
            XNSE => ("National Stock Exchange of India", "India", "Mumbai", "NSI", INR),
            XNYS => ("New York Stock Exchange", "United States", "New York", "NYQ", USD),
            XNZE => ("New Zealand Exchange", "New Zealand", "Wellington", "NZE", NZD),
            XOSL => ("Oslo Børs", "Norway", "Oslo", "OSL", NOK),
            XPAR => ("Euronext Paris", "France", "Paris", "PAR", EUR),
            XSES => ("Singapore Exchange", "Singapore", "Singapore", "SGX", SGD),
            XSHE => ("Shenzhen Stock Exchange", "China", "Shenzhen", "SHZ", CNY),
            XSHG => ("Shanghai Stock Exchange", "China", "Shanghai", "SHH", CNY),
            XSTO => ("Nasdaq Stockholm", "Sweden", "Stockholm", "STO", SEK),
            XSWX => ("SIX Swiss Exchange", "Switzerland", "Zurich", "SWX", CHF),
            XTAI => ("Taiwan Stock Exchange", "Taiwan", "Taipei", "TAI", TWD),
            XWBO => ("Vienna Stock Exchange", "Austria", "Vienna", "VIE", EUR),
        }
    }

    pub fn name(&self) -> &'static str {
        self.data().0
    }
    pub fn country(&self) -> &'static str {
        self.data().1
    }
    pub fn city(&self) -> &'static str {
        self.data().2
    }
    pub fn yahoo_code(&self) -> &'static str {
        self.data().3
    }
    pub fn currency(&self) -> Currency {
        self.data().4
    }
}

#[pymethods]
impl Exchange {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Exchange {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("unknown exchange {s:?}")))
    }
}
