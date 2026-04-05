use crate::data::models::country::Country;
use crate::data::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// A stock exchange.
///
/// Variant names are identical to their 4-letter MIC (Market Identifier Code) codes.
///
/// Attributes
/// ----------
/// mic : str
///     The ISO 10383 Market Identifier Code.
///
/// name : str
///     The official name of the exchange.
///
/// country : [Country]
///     The country where the exchange is located.
///
/// city : str
///     The city where the exchange is headquartered.
///
/// yahoo_code : str
///     The Yahoo Finance suffix used to qualify ticker symbols for this
///     exchange.
///
/// currency : [Currency]
///     The primary trading currency of the exchange.
/// 
/// See Also
/// --------
/// - backtide.data:Country
/// - backtide.data:Currency
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, module = "backtide.data")]
#[derive(
    Clone,
    Copy,
    Debug,
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
#[allow(clippy::upper_case_acronyms)]
pub enum Exchange {
    BVMF,
    XADS,
    XAMS,
    XASE,
    XATH,
    XASX,
    XBKK,
    XBOG,
    XBOM,
    XBRU,
    XBUD,
    XBUE,
    XCAI,
    XCOL,
    XCSE,
    XDFM,
    XDHA,
    XDUB,
    XETR,
    XHEL,
    XHKG,
    XICE,
    XIDX,
    XIST,
    XJPX,
    XKAR,
    XKLS,
    XKRX,
    XKUW,
    XLIM,
    XLIS,
    XLIT,
    XLON,
    XLUX,
    XMAD,
    XMEX,
    XMIL,
    XMOS,
    XNAS,
    XNCM,
    XNGS,
    XNSE,
    XNYS,
    XNZE,
    XOSL,
    XPAR,
    XPHS,
    XPRA,
    XRIS,
    XSAU,
    XSES,
    XSGO,
    XSHE,
    XSHG,
    XSTC,
    XSTO,
    XSWX,
    XTAI,
    XTAL,
    XTSX,
    XWAR,
    XWBO,
}

impl Exchange {
    fn data(&self) -> (&'static str, Country, &'static str, &'static str, Currency) {
        use Country::*;
        use Currency::*;
        use Exchange::*;
        match self {
            BVMF => ("B3", BRA, "São Paulo", "SA", BRL),
            XADS => ("Abu Dhabi Securities Exchange", ARE, "Abu Dhabi", "AD", AED),
            XAMS => ("Euronext Amsterdam", NLD, "Amsterdam", "AMS", Currency::EUR),
            XASE => ("NYSE American", USA, "New York", "ASE", USD),
            XATH => ("Athens Exchange", GRC, "Athens", "AT", Currency::EUR),
            XASX => ("Australian Securities Exchange", AUS, "Sydney", "ASX", AUD),
            XBKK => ("Stock Exchange of Thailand", THA, "Bangkok", "BK", THB),
            XBOG => ("Colombia Stock Exchange", COL, "Bogotá", "CL", COP),
            XBOM => ("Bombay Stock Exchange", IND, "Mumbai", "BSE", INR),
            XBRU => ("Euronext Brussels", BEL, "Brussels", "BRU", Currency::EUR),
            XBUD => ("Budapest Stock Exchange", HUN, "Budapest", "BD", HUF),
            XBUE => ("Buenos Aires Stock Exchange", ARG, "Buenos Aires", "BA", ARS),
            XCAI => ("Egyptian Exchange", EGY, "Cairo", "CA", EGP),
            XCOL => ("Colombo Stock Exchange", LKA, "Colombo", "CM", LKR),
            XCSE => ("Nasdaq Copenhagen", DNK, "Copenhagen", "CPH", DKK),
            XDFM => ("Dubai Financial Market", ARE, "Dubai", "DU", AED),
            XDHA => ("Dhaka Stock Exchange", BGD, "Dhaka", "DH", BDT),
            XDUB => ("Euronext Dublin", IRL, "Dublin", "IR", Currency::EUR),
            XETR => ("XETRA", DEU, "Frankfurt", "GER", Currency::EUR),
            XHEL => ("Nasdaq Helsinki", FIN, "Helsinki", "HEL", Currency::EUR),
            XHKG => ("Hong Kong Stock Exchange", HKG, "Hong Kong", "HKG", HKD),
            XICE => ("Nasdaq Iceland", ISL, "Reykjavik", "IC", ISK),
            XIDX => ("Indonesia Stock Exchange", IDN, "Jakarta", "JK", IDR),
            XIST => ("Borsa Istanbul", TUR, "Istanbul", "IST", TRY),
            XJPX => ("Japan Exchange Group", JPN, "Tokyo", "JPX", JPY),
            XKAR => ("Pakistan Stock Exchange", PAK, "Karachi", "KA", PKR),
            XKLS => ("Bursa Malaysia", MYS, "Kuala Lumpur", "KL", MYR),
            XKRX => ("Korea Exchange", KOR, "Seoul", "KSC", KRW),
            XKUW => ("Kuwait Stock Exchange", KWT, "Kuwait City", "KW", KWD),
            XLIM => ("Lima Stock Exchange", PER, "Lima", "LM", PEN),
            XLIS => ("Euronext Lisbon", PRT, "Lisbon", "LIS", Currency::EUR),
            XLIT => ("Nasdaq Vilnius", LTU, "Vilnius", "VS", Currency::EUR),
            XLON => ("London Stock Exchange", GBR, "London", "LSE", GBP),
            XLUX => ("Luxembourg Stock Exchange", LUX, "Luxembourg", "LU", Currency::EUR),
            XMAD => ("Bolsa de Madrid", ESP, "Madrid", "MCE", Currency::EUR),
            XMEX => ("Mexican Stock Exchange", MEX, "Mexico City", "MX", MXN),
            XMIL => ("Borsa Italiana", ITA, "Milan", "MIL", Currency::EUR),
            XMOS => ("Moscow Exchange", RUS, "Moscow", "ME", RUB),
            XNAS => ("NASDAQ Global Select Market", USA, "New York", "NMS", USD),
            XNCM => ("NASDAQ Capital Market", USA, "New York", "NCM", USD),
            XNGS => ("NASDAQ Global Market", USA, "New York", "NGM", USD),
            XNSE => ("National Stock Exchange of India", IND, "Mumbai", "NSI", INR),
            XNYS => ("New York Stock Exchange", USA, "New York", "NYQ", USD),
            XNZE => ("New Zealand Exchange", NZL, "Wellington", "NZE", NZD),
            XOSL => ("Oslo Børs", NOR, "Oslo", "OSL", NOK),
            XPAR => ("Euronext Paris", FRA, "Paris", "PAR", Currency::EUR),
            XPHS => ("Philippine Stock Exchange", PHL, "Manila", "PS", PHP),
            XPRA => ("Prague Stock Exchange", CZE, "Prague", "PR", CZK),
            XRIS => ("Nasdaq Riga", LVA, "Riga", "RG", Currency::EUR),
            XSAU => ("Saudi Exchange", SAU, "Riyadh", "SR", SAR),
            XSES => ("Singapore Exchange", SGP, "Singapore", "SGX", SGD),
            XSGO => ("Santiago Stock Exchange", CHL, "Santiago", "SN", CLP),
            XSHE => ("Shenzhen Stock Exchange", CHN, "Shenzhen", "SHZ", CNY),
            XSHG => ("Shanghai Stock Exchange", CHN, "Shanghai", "SHH", CNY),
            XSTC => ("Ho Chi Minh Stock Exchange", VNM, "Ho Chi Minh City", "VN", VND),
            XSTO => ("Nasdaq Stockholm", SWE, "Stockholm", "STO", SEK),
            XSWX => ("SIX Swiss Exchange", CHE, "Zurich", "SWX", CHF),
            XTAI => ("Taiwan Stock Exchange", TWN, "Taipei", "TAI", TWD),
            XTAL => ("Nasdaq Tallinn", EST, "Tallinn", "TL", Currency::EUR),
            XTSX => ("Toronto Stock Exchange", CAN, "Toronto", "TO", CAD),
            XWAR => ("Warsaw Stock Exchange", POL, "Warsaw", "WA", PLN),
            XWBO => ("Vienna Stock Exchange", AUT, "Vienna", "VIE", Currency::EUR),
        }
    }
}

impl Exchange {
    /// Look up an [`Exchange`] by its Yahoo Finance suffix code.
    pub fn from_yahoo_code(code: &str) -> Option<Self> {
        Self::iter().find(|ex| ex.yahoo_code() == code)
    }
}

#[pymethods]
impl Exchange {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown exchange: {s}")))
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

    /// The ISO 10383 Market Identifier Code.
    #[getter]
    pub fn mic(&self) -> String {
        self.to_string()
    }

    /// The official name of the exchange.
    #[getter]
    pub fn name(&self) -> &'static str {
        self.data().0
    }

    /// The country where the exchange is located.
    #[getter]
    pub fn country(&self) -> Country {
        self.data().1
    }

    /// The city where the exchange is headquartered.
    #[getter]
    pub fn city(&self) -> &'static str {
        self.data().2
    }

    /// The Yahoo Finance suffix used to qualify ticker symbols for this exchange.
    #[getter]
    pub fn yahoo_code(&self) -> &'static str {
        self.data().3
    }

    /// The primary trading currency of the exchange.
    #[getter]
    pub fn currency(&self) -> Currency {
        self.data().4
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Exchange {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, PyErr> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<Exchange>() {
            return Ok(bound.borrow().clone());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown exchange {s:?}.")))
    }
}
