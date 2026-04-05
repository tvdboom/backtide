use crate::data::models::country::Country;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// An ISO 4217 currency tied to a specific country or supranational union.
///
/// Variant names are identical to their 3-letter ISO codes.
///
/// Attributes
/// ----------
/// name : str
///     The human-readable name of the currency.
///
/// symbol : str
///     The currency symbol as a UTF-8 string (e.g., `$`, `€`, `₺`).
///
/// country : [`Country`]
///     The country that issues this currency.
///
/// decimals : int
///     The number of decimal places conventionally used when displaying
///     amounts in this currency, per ISO 4217.
///
/// symbol_prefix : bool
///     Returns `true` if the currency symbol is conventionally placed before
///     the numeric amount, or `false` if it follows the amount.
/// 
/// See Also
/// --------
/// - backtide.data:Country
/// - backtide.data:Exchange
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, module = "backtide.data")]
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
#[allow(clippy::upper_case_acronyms)]
pub enum Currency {
    AED,
    AFN,
    ALL,
    AMD,
    AOA,
    ARS,
    AUD,
    AZN,
    BAM,
    BDT,
    BGN,
    BHD,
    BND,
    BOB,
    BRL,
    CAD,
    CHF,
    CLP,
    CNY,
    COP,
    CRC,
    CZK,
    DKK,
    DOP,
    DZD,
    EGP,
    EUR,
    FJD,
    GBP,
    GEL,
    GHS,
    GTQ,
    HKD,
    HNL,
    HUF,
    IDR,
    ILS,
    INR,
    IQD,
    ISK,
    JMD,
    JOD,
    JPY,
    KES,
    KRW,
    KWD,
    KYD,
    KZT,
    LBP,
    LKR,
    LYD,
    MAD,
    MDL,
    MKD,
    MNT,
    MOP,
    MUR,
    MVR,
    MXN,
    MYR,
    MZN,
    NAD,
    NGN,
    NIO,
    NOK,
    NPR,
    NZD,
    OMR,
    PEN,
    PGK,
    PHP,
    PKR,
    PLN,
    PYG,
    QAR,
    RON,
    RSD,
    RUB,
    RWF,
    SAR,
    SCR,
    SEK,
    SGD,
    SRD,
    THB,
    TND,
    TRY,
    TTD,
    TWD,
    TZS,
    UAH,
    UGX,
    #[default]
    USD,
    UYU,
    UZS,
    VND,
    YER,
    ZAR,
    ZMW,
}

impl Currency {
    /// Returns `(name, symbol, country, decimals, symbol_prefix)`.
    fn data(&self) -> (&'static str, &'static str, Country, u8, bool) {
        use Country::*;
        match self {
            Currency::AED => ("United Arab Emirates Dirham", "د.إ", ARE, 2, false),
            Currency::AFN => ("Afghani", "؋", AFG, 2, true),
            Currency::ALL => ("Lek", "L", ALB, 2, false),
            Currency::AMD => ("Dram", "֏", ARM, 2, false),
            Currency::AOA => ("Kwanza", "Kz", AGO, 2, false),
            Currency::ARS => ("Argentine Peso", "$", ARG, 2, true),
            Currency::AUD => ("Australian Dollar", "$", AUS, 2, true),
            Currency::AZN => ("Manat", "₼", AZE, 2, false),
            Currency::BAM => ("Convertible Mark", "KM", BIH, 2, false),
            Currency::BDT => ("Taka", "৳", BGD, 2, true),
            Currency::BGN => ("Lev", "лв", BGR, 2, false),
            Currency::BHD => ("Bahraini Dinar", "BD", BHR, 3, true),
            Currency::BND => ("Brunei Dollar", "$", BRN, 2, true),
            Currency::BOB => ("Boliviano", "Bs.", BOL, 2, true),
            Currency::BRL => ("Real", "R$", BRA, 2, true),
            Currency::CAD => ("Canadian Dollar", "$", CAN, 2, true),
            Currency::CHF => ("Swiss Franc", "Fr.", CHE, 2, true),
            Currency::CLP => ("Chilean Peso", "$", CHL, 0, true),
            Currency::CNY => ("Yuan", "¥", CHN, 2, true),
            Currency::COP => ("Colombian Peso", "$", COL, 2, true),
            Currency::CRC => ("Colon", "₡", CRI, 2, true),
            Currency::CZK => ("Koruna", "Kč", CZE, 2, false),
            Currency::DKK => ("Danish Krone", "kr", DNK, 2, false),
            Currency::DOP => ("Dominican Peso", "RD$", DOM, 2, true),
            Currency::DZD => ("Algerian Dinar", "دج", DZA, 2, false),
            Currency::EGP => ("Egyptian Pound", "E£", EGY, 2, true),
            Currency::EUR => ("Euro", "€", EUR, 2, false),
            Currency::FJD => ("Fiji Dollar", "FJ$", FJI, 2, true),
            Currency::GBP => ("Pound Sterling", "£", GBR, 2, true),
            Currency::GEL => ("Lari", "₾", GEO, 2, false),
            Currency::GHS => ("Cedi", "₵", GHA, 2, false),
            Currency::GTQ => ("Quetzal", "Q", GTM, 2, true),
            Currency::HKD => ("Hong Kong Dollar", "HK$", HKG, 2, true),
            Currency::HNL => ("Lempira", "L", HND, 2, true),
            Currency::HUF => ("Forint", "Ft", HUN, 2, false),
            Currency::IDR => ("Rupiah", "Rp", IDN, 2, true),
            Currency::ILS => ("New Shekel", "₪", ISR, 2, true),
            Currency::INR => ("Indian Rupee", "₹", IND, 2, true),
            Currency::IQD => ("Iraqi Dinar", "ع.د", IRQ, 3, false),
            Currency::ISK => ("Icelandic Króna", "kr", ISL, 0, false),
            Currency::JMD => ("Jamaican Dollar", "J$", JAM, 2, true),
            Currency::JOD => ("Jordanian Dinar", "JD", JOR, 3, true),
            Currency::JPY => ("Yen", "¥", JPN, 0, true),
            Currency::KES => ("Kenyan Shilling", "KSh", KEN, 2, true),
            Currency::KRW => ("Won", "₩", KOR, 0, true),
            Currency::KWD => ("Kuwaiti Dinar", "KD", KWT, 3, true),
            Currency::KYD => ("Cayman Islands Dollar", "CI$", CYM, 2, true),
            Currency::KZT => ("Tenge", "₸", KAZ, 2, false),
            Currency::LBP => ("Lebanese Pound", "ل.ل", LBN, 0, false),
            Currency::LKR => ("Sri Lankan Rupee", "Rs", LKA, 2, true),
            Currency::LYD => ("Libyan Dinar", "LD", LBY, 3, true),
            Currency::MAD => ("Moroccan Dirham", "د.م.", MAR, 2, false),
            Currency::MDL => ("Moldovan Leu", "L", MDA, 2, false),
            Currency::MKD => ("Denar", "ден", MKD, 0, false),
            Currency::MNT => ("Tugrik", "₮", MNG, 2, false),
            Currency::MOP => ("Pataca", "MOP$", MAC, 2, true),
            Currency::MUR => ("Mauritian Rupee", "Rs", MUS, 2, true),
            Currency::MVR => ("Rufiyaa", "Rf", MDV, 2, false),
            Currency::MXN => ("Mexican Peso", "$", MEX, 2, true),
            Currency::MYR => ("Ringgit", "RM", MYS, 2, true),
            Currency::MZN => ("Metical", "MT", MOZ, 2, false),
            Currency::NAD => ("Namibian Dollar", "N$", NAM, 2, true),
            Currency::NGN => ("Naira", "₦", NGA, 2, true),
            Currency::NIO => ("Córdoba", "C$", NIC, 2, true),
            Currency::NOK => ("Norwegian Krone", "kr", NOR, 2, false),
            Currency::NPR => ("Nepalese Rupee", "Rs", NPL, 2, true),
            Currency::NZD => ("New Zealand Dollar", "$", NZL, 2, true),
            Currency::OMR => ("Omani Rial", "ر.ع.", OMN, 3, false),
            Currency::PEN => ("Nuevo Sol", "S/", PER, 2, true),
            Currency::PGK => ("Kina", "K", PNG, 2, true),
            Currency::PHP => ("Philippine Peso", "₱", PHL, 2, true),
            Currency::PKR => ("Pakistani Rupee", "Rs", PAK, 2, true),
            Currency::PLN => ("Złoty", "zł", POL, 2, false),
            Currency::PYG => ("Guaraní", "₲", PRY, 0, false),
            Currency::QAR => ("Qatari Riyal", "QR", QAT, 2, true),
            Currency::RON => ("Romanian New Leu", "lei", ROU, 2, false),
            Currency::RSD => ("Serbian Dinar", "din", SRB, 2, false),
            Currency::RUB => ("Rouble", "₽", RUS, 2, false),
            Currency::RWF => ("Rwandan Franc", "Fr", RWA, 0, false),
            Currency::SAR => ("Saudi Riyal", "ر.س", SAU, 2, false),
            Currency::SCR => ("Seychelles Rupee", "Rs", SYC, 2, true),
            Currency::SEK => ("Swedish Krona", "kr", SWE, 2, false),
            Currency::SGD => ("Singapore Dollar", "S$", SGP, 2, true),
            Currency::SRD => ("Surinamese Dollar", "Sr$", SUR, 2, true),
            Currency::THB => ("Baht", "฿", THA, 2, true),
            Currency::TND => ("Tunisian Dinar", "DT", TUN, 3, false),
            Currency::TRY => ("Lira", "₺", TUR, 2, true),
            Currency::TTD => ("Trinidad and Tobago Dollar", "TT$", TTO, 2, true),
            Currency::TWD => ("New Taiwan Dollar", "NT$", TWN, 2, true),
            Currency::TZS => ("Tanzanian Shilling", "TSh", TZA, 2, true),
            Currency::UAH => ("Hryvnia", "₴", UKR, 2, false),
            Currency::UGX => ("Ugandan Shilling", "USh", UGA, 2, true),
            Currency::USD => ("United States Dollar", "$", USA, 2, true),
            Currency::UYU => ("Uruguayan Peso", "$U", URY, 2, true),
            Currency::UZS => ("Som", "сум", UZB, 2, false),
            Currency::VND => ("Dong", "₫", VNM, 0, false),
            Currency::YER => ("Yemeni Rial", "﷼", YEM, 2, false),
            Currency::ZAR => ("Rand", "R", ZAF, 2, true),
            Currency::ZMW => ("Kwacha", "ZK", ZMB, 2, true),
        }
    }
}

#[pymethods]
impl Currency {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown currency: {s}")))
    }

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Currency>().into_any();
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

    /// Return the default variant.
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::USD).unwrap()
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }

    /// The human-readable name of the currency.
    #[getter]
    pub fn name(&self) -> &'static str {
        self.data().0
    }

    /// The currency symbol as a UTF-8 string.
    #[getter]
    pub fn symbol(&self) -> &'static str {
        self.data().1
    }

    /// The country that issues this currency.
    #[getter]
    pub fn country(&self) -> Country {
        self.data().2
    }

    /// The number of decimal places conventionally used when displaying
    /// amounts in this currency, per ISO 4217.
    #[getter]
    pub fn decimals(&self) -> u8 {
        self.data().3
    }

    /// Returns `true` if the currency symbol is conventionally placed before
    /// the numeric amount, or `false` if it follows the amount.
    #[getter]
    pub fn symbol_prefix(&self) -> bool {
        self.data().4
    }

    /// Format an amount using this currency's symbol and placement convention.
    ///
    /// Parameters
    /// ----------
    /// amount : int | float
    ///     Amount to display.
    ///
    /// Returns
    /// -------
    /// str
    ///     Formatted amount with currency indicator.
    pub fn format(&self, amount: f64) -> String {
        let decimals = self.decimals() as usize;
        let symbol = self.symbol();
        if self.symbol_prefix() {
            format!("{symbol}{amount:.decimals$}")
        } else {
            format!("{amount:.decimals$} {symbol}")
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Currency {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, PyErr> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<Currency>() {
            return Ok(bound.borrow().clone());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown currency {s:?}.")))
    }
}
