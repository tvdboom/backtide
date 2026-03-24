//! ISO 4217 currency definitions.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString};
use crate::ingestion::provider::Provider;

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
            AED => ("United Arab Emirates dirham", "United Arab Emirates", "🇦🇪", 2, "د.إ", false),
            AFN => ("Afghan afghani", "Afghanistan", "🇦🇫", 2, "؋", true),
            ALL => ("Albanian lek", "Albania", "🇦🇱", 2, "L", false),
            AMD => ("Armenian dram", "Armenia", "🇦🇲", 2, "֏", false),
            AOA => ("Angolan kwanza", "Angola", "🇦🇴", 2, "Kz", false),
            ARS => ("Argentine peso", "Argentina", "🇦🇷", 2, "$", true),
            AUD => ("Australian dollar", "Australia", "🇦🇺", 2, "$", true),
            AZN => ("Azerbaijani manat", "Azerbaijan", "🇦🇿", 2, "₼", false),
            BAM => (
                "Bosnia and Herzegovina convertible mark",
                "Bosnia and Herzegovina",
                "🇧🇦",
                2,
                "KM",
                false,
            ),
            BDT => ("Bangladeshi taka", "Bangladesh", "🇧🇩", 2, "৳", true),
            BGN => ("Bulgarian lev", "Bulgaria", "🇧🇬", 2, "лв", false),
            BHD => ("Bahraini dinar", "Bahrain", "🇧🇭", 3, "BD", true),
            BND => ("Brunei dollar", "Brunei", "🇧🇳", 2, "$", true),
            BOB => ("Boliviano", "Bolivia", "🇧🇴", 2, "Bs.", true),
            BRL => ("Brazilian real", "Brazil", "🇧🇷", 2, "R$", true),
            CAD => ("Canadian dollar", "Canada", "🇨🇦", 2, "$", true),
            CHF => ("Swiss franc", "Switzerland", "🇨🇭", 2, "Fr.", true),
            CLP => ("Chilean peso", "Chile", "🇨🇱", 0, "$", true),
            CNY => ("Chinese yuan", "China", "🇨🇳", 2, "¥", true),
            COP => ("Colombian peso", "Colombia", "🇨🇴", 2, "$", true),
            CRC => ("Costa Rican colon", "Costa Rica", "🇨🇷", 2, "₡", true),
            CZK => ("Czech koruna", "Czech Republic", "🇨🇿", 2, "Kč", false),
            DKK => ("Danish krone", "Denmark", "🇩🇰", 2, "kr", false),
            DOP => ("Dominican peso", "Dominican Republic", "🇩🇴", 2, "RD$", true),
            DZD => ("Algerian dinar", "Algeria", "🇩🇿", 2, "دج", false),
            EGP => ("Egyptian pound", "Egypt", "🇪🇬", 2, "E£", true),
            EUR => ("Euro", "Europe", "🇪🇺", 2, "€", false),
            FJD => ("Fiji dollar", "Fiji", "🇫🇯", 2, "FJ$", true),
            GBP => ("Pound sterling", "United Kingdom", "🇬🇧", 2, "£", true),
            GEL => ("Georgian lari", "Georgia", "🇬🇪", 2, "₾", false),
            GHS => ("Ghanaian cedi", "Ghana", "🇬🇭", 2, "₵", false),
            GTQ => ("Guatemalan quetzal", "Guatemala", "🇬🇹", 2, "Q", true),
            HKD => ("Hong Kong dollar", "Hong Kong", "🇭🇰", 2, "HK$", true),
            HNL => ("Honduran lempira", "Honduras", "🇭🇳", 2, "L", true),
            HUF => ("Hungarian forint", "Hungary", "🇭🇺", 2, "Ft", false),
            IDR => ("Indonesian rupiah", "Indonesia", "🇮🇩", 2, "Rp", true),
            ILS => ("Israeli new shekel", "Israel", "🇮🇱", 2, "₪", true),
            INR => ("Indian rupee", "India", "🇮🇳", 2, "₹", true),
            IQD => ("Iraqi dinar", "Iraq", "🇮🇶", 3, "ع.د", false),
            ISK => ("Icelandic króna", "Iceland", "🇮🇸", 0, "kr", false),
            JMD => ("Jamaican dollar", "Jamaica", "🇯🇲", 2, "J$", true),
            JOD => ("Jordanian dinar", "Jordan", "🇯🇴", 3, "JD", true),
            JPY => ("Japanese yen", "Japan", "🇯🇵", 0, "¥", true),
            KES => ("Kenyan shilling", "Kenya", "🇰🇪", 2, "KSh", true),
            KRW => ("South Korean won", "South Korea", "🇰🇷", 0, "₩", true),
            KWD => ("Kuwaiti dinar", "Kuwait", "🇰🇼", 3, "KD", true),
            KYD => ("Cayman Islands dollar", "Cayman Islands", "🇰🇾", 2, "CI$", true),
            KZT => ("Kazakhstani tenge", "Kazakhstan", "🇰🇿", 2, "₸", false),
            LBP => ("Lebanese pound", "Lebanon", "🇱🇧", 0, "ل.ل", false),
            LKR => ("Sri Lankan rupee", "Sri Lanka", "🇱🇰", 2, "Rs", true),
            LYD => ("Libyan dinar", "Libya", "🇱🇾", 3, "LD", true),
            MAD => ("Moroccan dirham", "Morocco", "🇲🇦", 2, "د.م.", false),
            MDL => ("Moldovan leu", "Moldova", "🇲🇩", 2, "L", false),
            MKD => ("Macedonian denar", "North Macedonia", "🇲🇰", 0, "ден", false),
            MNT => ("Mongolian tugrik", "Mongolia", "🇲🇳", 2, "₮", false),
            MOP => ("Macanese pataca", "Macau", "🇲🇴", 2, "MOP$", true),
            MUR => ("Mauritian rupee", "Mauritius", "🇲🇺", 2, "Rs", true),
            MVR => ("Maldivian rufiyaa", "Maldives", "🇲🇻", 2, "Rf", false),
            MXN => ("Mexican peso", "Mexico", "🇲🇽", 2, "$", true),
            MYR => ("Malaysian ringgit", "Malaysia", "🇲🇾", 2, "RM", true),
            MZN => ("Mozambican metical", "Mozambique", "🇲🇿", 2, "MT", false),
            NAD => ("Namibian dollar", "Namibia", "🇳🇦", 2, "N$", true),
            NGN => ("Nigerian naira", "Nigeria", "🇳🇬", 2, "₦", true),
            NIO => ("Nicaraguan córdoba", "Nicaragua", "🇳🇮", 2, "C$", true),
            NOK => ("Norwegian krone", "Norway", "🇳🇴", 2, "kr", false),
            NPR => ("Nepalese rupee", "Nepal", "🇳🇵", 2, "Rs", true),
            NZD => ("New Zealand dollar", "New Zealand", "🇳🇿", 2, "$", true),
            OMR => ("Omani rial", "Oman", "🇴🇲", 3, "ر.ع.", false),
            PEN => ("Peruvian nuevo sol", "Peru", "🇵🇪", 2, "S/", true),
            PGK => ("Papua New Guinean kina", "Papua New Guinea", "🇵🇬", 2, "K", true),
            PHP => ("Philippine peso", "Philippines", "🇵🇭", 2, "₱", true),
            PKR => ("Pakistani rupee", "Pakistan", "🇵🇰", 2, "Rs", true),
            PLN => ("Polish złoty", "Poland", "🇵🇱", 2, "zł", false),
            PYG => ("Paraguayan guaraní", "Paraguay", "🇵🇾", 0, "₲", false),
            QAR => ("Qatari riyal", "Qatar", "🇶🇦", 2, "QR", true),
            RON => ("Romanian new leu", "Romania", "🇷🇴", 2, "lei", false),
            RSD => ("Serbian dinar", "Serbia", "🇷🇸", 2, "din", false),
            RUB => ("Russian rouble", "Russia", "🇷🇺", 2, "₽", false),
            RWF => ("Rwandan franc", "Rwanda", "🇷🇼", 0, "Fr", false),
            SAR => ("Saudi riyal", "Saudi Arabia", "🇸🇦", 2, "ر.س", false),
            SCR => ("Seychelles rupee", "Seychelles", "🇸🇨", 2, "Rs", true),
            SEK => ("Swedish krona", "Sweden", "🇸🇪", 2, "kr", false),
            SGD => ("Singapore dollar", "Singapore", "🇸🇬", 2, "S$", true),
            SRD => ("Surinamese dollar", "Suriname", "🇸🇷", 2, "Sr$", true),
            THB => ("Thai baht", "Thailand", "🇹🇭", 2, "฿", true),
            TND => ("Tunisian dinar", "Tunisia", "🇹🇳", 3, "DT", false),
            TRY => ("Turkish lira", "Turkey", "🇹🇷", 2, "₺", true),
            TTD => ("Trinidad and Tobago dollar", "Trinidad and Tobago", "🇹🇹", 2, "TT$", true),
            TWD => ("New Taiwan dollar", "Taiwan", "🇹🇼", 2, "NT$", true),
            TZS => ("Tanzanian shilling", "Tanzania", "🇹🇿", 2, "TSh", true),
            UAH => ("Ukrainian hryvnia", "Ukraine", "🇺🇦", 2, "₴", false),
            UGX => ("Ugandan shilling", "Uganda", "🇺🇬", 2, "USh", true),
            USD => ("United States dollar", "United States", "🇺🇸", 2, "$", true),
            UYU => ("Uruguayan peso", "Uruguay", "🇺🇾", 2, "$U", true),
            UZS => ("Uzbekistan som", "Uzbekistan", "🇺🇿", 2, "сум", false),
            VND => ("Vietnamese dong", "Vietnam", "🇻🇳", 0, "₫", false),
            YER => ("Yemeni rial", "Yemen", "🇾🇪", 2, "﷼", false),
            ZAR => ("South African rand", "South Africa", "🇿🇦", 2, "R", true),
            ZMW => ("Zambian kwacha", "Zambia", "🇿🇲", 2, "ZK", true),
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

impl<'a, 'py> FromPyObject<'a, 'py> for Currency {
    type Error = PyErr;

    /// Parse the currency from a string.
    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("unknown currency {s:?}")))
    }
}
