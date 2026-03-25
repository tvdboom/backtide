//! Currency definition.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString};

/// An ISO 4217 currency tied to a specific country or supranational union.
///
/// Variant names are identical to their 3-letter ISO codes.
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
    #[default]
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
    USD,
    UYU,
    UZS,
    VND,
    YER,
    ZAR,
    ZMW,
}
impl Currency {
    fn data(&self) -> (&'static str, &'static str, &'static str, u8, &'static str, bool) {
        use Currency::*;
        match self {
            AED => ("United Arab Emirates Dirham", "United Arab Emirates", "🇦🇪", 2, "د.إ", false),
            AFN => ("Afghani", "Afghanistan", "🇦🇫", 2, "؋", true),
            ALL => ("Lek", "Albania", "🇦🇱", 2, "L", false),
            AMD => ("Dram", "Armenia", "🇦🇲", 2, "֏", false),
            AOA => ("Kwanza", "Angola", "🇦🇴", 2, "Kz", false),
            ARS => ("Argentine Peso", "Argentina", "🇦🇷", 2, "$", true),
            AUD => ("Australian Dollar", "Australia", "🇦🇺", 2, "$", true),
            AZN => ("Manat", "Azerbaijan", "🇦🇿", 2, "₼", false),
            BAM => ("Convertible Mark", "Bosnia and Herzegovina", "🇧🇦", 2, "KM", false),
            BDT => ("Taka", "Bangladesh", "🇧🇩", 2, "৳", true),
            BGN => ("Lev", "Bulgaria", "🇧🇬", 2, "лв", false),
            BHD => ("Bahraini Dinar", "Bahrain", "🇧🇭", 3, "BD", true),
            BND => ("Brunei Dollar", "Brunei", "🇧🇳", 2, "$", true),
            BOB => ("Boliviano", "Bolivia", "🇧🇴", 2, "Bs.", true),
            BRL => ("Real", "Brazil", "🇧🇷", 2, "R$", true),
            CAD => ("Canadian Dollar", "Canada", "🇨🇦", 2, "$", true),
            CHF => ("Swiss Franc", "Switzerland", "🇨🇭", 2, "Fr.", true),
            CLP => ("Chilean Peso", "Chile", "🇨🇱", 0, "$", true),
            CNY => ("Yuan", "China", "🇨🇳", 2, "¥", true),
            COP => ("Colombian Peso", "Colombia", "🇨🇴", 2, "$", true),
            CRC => ("Colon", "Costa Rica", "🇨🇷", 2, "₡", true),
            CZK => ("Koruna", "Czech Republic", "🇨🇿", 2, "Kč", false),
            DKK => ("Danish Krone", "Denmark", "🇩🇰", 2, "kr", false),
            DOP => ("Dominican Peso", "Dominican Republic", "🇩🇴", 2, "RD$", true),
            DZD => ("Algerian Dinar", "Algeria", "🇩🇿", 2, "دج", false),
            EGP => ("Egyptian Pound", "Egypt", "🇪🇬", 2, "E£", true),
            EUR => ("Euro", "Europe", "🇪🇺", 2, "€", false),
            FJD => ("Fiji Dollar", "Fiji", "🇫🇯", 2, "FJ$", true),
            GBP => ("Pound Sterling", "United Kingdom", "🇬🇧", 2, "£", true),
            GEL => ("Lari", "Georgia", "🇬🇪", 2, "₾", false),
            GHS => ("Cedi", "Ghana", "🇬🇭", 2, "₵", false),
            GTQ => ("Quetzal", "Guatemala", "🇬🇹", 2, "Q", true),
            HKD => ("Hong Kong Dollar", "Hong Kong", "🇭🇰", 2, "HK$", true),
            HNL => ("Lempira", "Honduras", "🇭🇳", 2, "L", true),
            HUF => ("Forint", "Hungary", "🇭🇺", 2, "Ft", false),
            IDR => ("Rupiah", "Indonesia", "🇮🇩", 2, "Rp", true),
            ILS => ("New Shekel", "Israel", "🇮🇱", 2, "₪", true),
            INR => ("Indian Rupee", "India", "🇮🇳", 2, "₹", true),
            IQD => ("Iraqi Dinar", "Iraq", "🇮🇶", 3, "ع.د", false),
            ISK => ("Icelandic Króna", "Iceland", "🇮🇸", 0, "kr", false),
            JMD => ("Jamaican Dollar", "Jamaica", "🇯🇲", 2, "J$", true),
            JOD => ("Jordanian Dinar", "Jordan", "🇯🇴", 3, "JD", true),
            JPY => ("Yen", "Japan", "🇯🇵", 0, "¥", true),
            KES => ("Kenyan Shilling", "Kenya", "🇰🇪", 2, "KSh", true),
            KRW => ("Won", "South Korea", "🇰🇷", 0, "₩", true),
            KWD => ("Kuwaiti Dinar", "Kuwait", "🇰🇼", 3, "KD", true),
            KYD => ("Cayman Islands Dollar", "Cayman Islands", "🇰🇾", 2, "CI$", true),
            KZT => ("Tenge", "Kazakhstan", "🇰🇿", 2, "₸", false),
            LBP => ("Lebanese Pound", "Lebanon", "🇱🇧", 0, "ل.ل", false),
            LKR => ("Sri Lankan Rupee", "Sri Lanka", "🇱🇰", 2, "Rs", true),
            LYD => ("Libyan Dinar", "Libya", "🇱🇾", 3, "LD", true),
            MAD => ("Moroccan Dirham", "Morocco", "🇲🇦", 2, "د.م.", false),
            MDL => ("Moldovan Leu", "Moldova", "🇲🇩", 2, "L", false),
            MKD => ("Denar", "North Macedonia", "🇲🇰", 0, "ден", false),
            MNT => ("Tugrik", "Mongolia", "🇲🇳", 2, "₮", false),
            MOP => ("Pataca", "Macau", "🇲🇴", 2, "MOP$", true),
            MUR => ("Mauritian Rupee", "Mauritius", "🇲🇺", 2, "Rs", true),
            MVR => ("Rufiyaa", "Maldives", "🇲🇻", 2, "Rf", false),
            MXN => ("Mexican Peso", "Mexico", "🇲🇽", 2, "$", true),
            MYR => ("Ringgit", "Malaysia", "🇲🇾", 2, "RM", true),
            MZN => ("Metical", "Mozambique", "🇲🇿", 2, "MT", false),
            NAD => ("Namibian Dollar", "Namibia", "🇳🇦", 2, "N$", true),
            NGN => ("Naira", "Nigeria", "🇳🇬", 2, "₦", true),
            NIO => ("Córdoba", "Nicaragua", "🇳🇮", 2, "C$", true),
            NOK => ("Norwegian Krone", "Norway", "🇳🇴", 2, "kr", false),
            NPR => ("Nepalese Rupee", "Nepal", "🇳🇵", 2, "Rs", true),
            NZD => ("New Zealand Dollar", "New Zealand", "🇳🇿", 2, "$", true),
            OMR => ("Omani Rial", "Oman", "🇴🇲", 3, "ر.ع.", false),
            PEN => ("Nuevo Sol", "Peru", "🇵🇪", 2, "S/", true),
            PGK => ("Kina", "Papua New Guinea", "🇵🇬", 2, "K", true),
            PHP => ("Philippine Peso", "Philippines", "🇵🇭", 2, "₱", true),
            PKR => ("Pakistani Rupee", "Pakistan", "🇵🇰", 2, "Rs", true),
            PLN => ("Złoty", "Poland", "🇵🇱", 2, "zł", false),
            PYG => ("Guaraní", "Paraguay", "🇵🇾", 0, "₲", false),
            QAR => ("Qatari Riyal", "Qatar", "🇶🇦", 2, "QR", true),
            RON => ("Romanian New Leu", "Romania", "🇷🇴", 2, "lei", false),
            RSD => ("Serbian Dinar", "Serbia", "🇷🇸", 2, "din", false),
            RUB => ("Rouble", "Russia", "🇷🇺", 2, "₽", false),
            RWF => ("Rwandan Franc", "Rwanda", "🇷🇼", 0, "Fr", false),
            SAR => ("Saudi Riyal", "Saudi Arabia", "🇸🇦", 2, "ر.س", false),
            SCR => ("Seychelles Rupee", "Seychelles", "🇸🇨", 2, "Rs", true),
            SEK => ("Swedish Krona", "Sweden", "🇸🇪", 2, "kr", false),
            SGD => ("Singapore Dollar", "Singapore", "🇸🇬", 2, "S$", true),
            SRD => ("Surinamese Dollar", "Suriname", "🇸🇷", 2, "Sr$", true),
            THB => ("Baht", "Thailand", "🇹🇭", 2, "฿", true),
            TND => ("Tunisian Dinar", "Tunisia", "🇹🇳", 3, "DT", false),
            TRY => ("Lira", "Turkey", "🇹🇷", 2, "₺", true),
            TTD => ("Trinidad and Tobago Dollar", "Trinidad and Tobago", "🇹🇹", 2, "TT$", true),
            TWD => ("New Taiwan Dollar", "Taiwan", "🇹🇼", 2, "NT$", true),
            TZS => ("Tanzanian Shilling", "Tanzania", "🇹🇿", 2, "TSh", true),
            UAH => ("Hryvnia", "Ukraine", "🇺🇦", 2, "₴", false),
            UGX => ("Ugandan Shilling", "Uganda", "🇺🇬", 2, "USh", true),
            USD => ("United States Dollar", "United States", "🇺🇸", 2, "$", true),
            UYU => ("Uruguayan Peso", "Uruguay", "🇺🇾", 2, "$U", true),
            UZS => ("Som", "Uzbekistan", "🇺🇿", 2, "сум", false),
            VND => ("Dong", "Vietnam", "🇻🇳", 0, "₫", false),
            YER => ("Yemeni Rial", "Yemen", "🇾🇪", 2, "﷼", false),
            ZAR => ("Rand", "South Africa", "🇿🇦", 2, "R", true),
            ZMW => ("Kwacha", "Zambia", "🇿🇲", 2, "ZK", true),
        }
    }

    pub fn name(&self) -> &'static str {
        self.data().0
    }
    pub fn country(&self) -> &'static str {
        self.data().1
    }
    pub fn flag(&self) -> &'static str {
        self.data().2
    }
    pub fn decimals(&self) -> u8 {
        self.data().3
    }
    pub fn symbol(&self) -> &'static str {
        self.data().4
    }
    pub fn symbol_prefix(&self) -> bool {
        self.data().5
    }

    /// Format an amount using this currency's symbol and placement convention.
    /// e.g. `USD.format(300.0)` → `"$300.00"`, `SEK.format(300.0)` → `"300.00 kr"`
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

#[pymethods]
impl Currency {
    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Currency {
    type Error = PyErr;

    /// Parse the currency from a string.
    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("unknown currency {s:?}")))
    }
}
