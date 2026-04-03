use crate::data::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// A country identified by its ISO 3166-1 alpha-3 code.
///
/// Variant names are identical to their 3-letter ISO 3166-1 alpha-3 codes.
///
/// Attributes
/// ----------
/// alpha2 : str
///     The ISO 3166-1 alpha-2 code.
///
/// alpha3 : str
///     /// The ISO 3166-1 alpha-3 code.
///
/// name : str
///     The name of the country.
///
/// flag : str
///     The Unicode regional-indicator flag emoji for the country.
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
pub enum Country {
    AFG,
    AGO,
    ALB,
    ARE,
    ARG,
    ARM,
    AUS,
    AUT,
    AZE,
    BDI,
    BEL,
    BEN,
    BGD,
    BGR,
    BHR,
    BIH,
    BLR,
    BOL,
    BRA,
    BRN,
    BTN,
    BWA,
    CAN,
    CHE,
    CHL,
    CHN,
    CIV,
    CMR,
    COD,
    COG,
    COL,
    CRI,
    CUB,
    CYM,
    CYP,
    CZE,
    DEU,
    DNK,
    DOM,
    DZA,
    ECU,
    EGY,
    ESP,
    EST,
    ETH,
    EUR,
    FIN,
    FJI,
    FRA,
    GBR,
    GEO,
    GHA,
    GRC,
    GTM,
    HKG,
    HND,
    HRV,
    HTI,
    HUN,
    IDN,
    IND,
    IRL,
    IRN,
    IRQ,
    ISL,
    ISR,
    ITA,
    JAM,
    JOR,
    JPN,
    KAZ,
    KEN,
    KGZ,
    KHM,
    KOR,
    KWT,
    LAO,
    LBN,
    LBY,
    LKA,
    LTU,
    LUX,
    LVA,
    MAC,
    MAR,
    MDA,
    MDV,
    MEX,
    MKD,
    MLT,
    MMR,
    MNE,
    MNG,
    MOZ,
    MRT,
    MUS,
    MYS,
    NAM,
    NGA,
    NIC,
    NLD,
    NOR,
    NPL,
    NZL,
    OMN,
    PAK,
    PAN,
    PER,
    PHL,
    PNG,
    POL,
    PRY,
    PRT,
    PSE,
    QAT,
    ROU,
    RUS,
    RWA,
    SAU,
    SDN,
    SEN,
    SGP,
    SLV,
    SRB,
    SUR,
    SVK,
    SVN,
    SWE,
    SYC,
    SYR,
    THA,
    TJK,
    TKM,
    TTO,
    TUN,
    TUR,
    TWN,
    TZA,
    UGA,
    UKR,
    URY,
    USA,
    UZB,
    VEN,
    VNM,
    YEM,
    ZAF,
    ZMB,
    ZWE,
}

impl Country {
    fn data(&self) -> (&'static str, &'static str) {
        use Country::*;
        match self {
            AFG => ("Afghanistan", "AF"),
            AGO => ("Angola", "AO"),
            ALB => ("Albania", "AL"),
            ARE => ("United Arab Emirates", "AE"),
            ARG => ("Argentina", "AR"),
            ARM => ("Armenia", "AM"),
            AUS => ("Australia", "AU"),
            AUT => ("Austria", "AT"),
            AZE => ("Azerbaijan", "AZ"),
            BDI => ("Burundi", "BI"),
            BEL => ("Belgium", "BE"),
            BEN => ("Benin", "BJ"),
            BGD => ("Bangladesh", "BD"),
            BGR => ("Bulgaria", "BG"),
            BHR => ("Bahrain", "BH"),
            BIH => ("Bosnia and Herzegovina", "BA"),
            BLR => ("Belarus", "BY"),
            BOL => ("Bolivia", "BO"),
            BRA => ("Brazil", "BR"),
            BRN => ("Brunei", "BN"),
            BTN => ("Bhutan", "BT"),
            BWA => ("Botswana", "BW"),
            CAN => ("Canada", "CA"),
            CHE => ("Switzerland", "CH"),
            CHL => ("Chile", "CL"),
            CHN => ("China", "CN"),
            CIV => ("Côte d'Ivoire", "CI"),
            CMR => ("Cameroon", "CM"),
            COD => ("DR Congo", "CD"),
            COG => ("Republic of the Congo", "CG"),
            COL => ("Colombia", "CO"),
            CRI => ("Costa Rica", "CR"),
            CUB => ("Cuba", "CU"),
            CYM => ("Cayman Islands", "KY"),
            CYP => ("Cyprus", "CY"),
            CZE => ("Czech Republic", "CZ"),
            DEU => ("Germany", "DE"),
            DNK => ("Denmark", "DK"),
            DOM => ("Dominican Republic", "DO"),
            DZA => ("Algeria", "DZ"),
            ECU => ("Ecuador", "EC"),
            EGY => ("Egypt", "EG"),
            ESP => ("Spain", "ES"),
            EST => ("Estonia", "EE"),
            ETH => ("Ethiopia", "ET"),
            EUR => ("European Union", "EU"),
            FIN => ("Finland", "FI"),
            FJI => ("Fiji", "FJ"),
            FRA => ("France", "FR"),
            GBR => ("United Kingdom", "GB"),
            GEO => ("Georgia", "GE"),
            GHA => ("Ghana", "GH"),
            GRC => ("Greece", "GR"),
            GTM => ("Guatemala", "GT"),
            HKG => ("Hong Kong", "HK"),
            HND => ("Honduras", "HN"),
            HRV => ("Croatia", "HR"),
            HTI => ("Haiti", "HT"),
            HUN => ("Hungary", "HU"),
            IDN => ("Indonesia", "ID"),
            IND => ("India", "IN"),
            IRL => ("Ireland", "IE"),
            IRN => ("Iran", "IR"),
            IRQ => ("Iraq", "IQ"),
            ISL => ("Iceland", "IS"),
            ISR => ("Israel", "IL"),
            ITA => ("Italy", "IT"),
            JAM => ("Jamaica", "JM"),
            JOR => ("Jordan", "JO"),
            JPN => ("Japan", "JP"),
            KAZ => ("Kazakhstan", "KZ"),
            KEN => ("Kenya", "KE"),
            KGZ => ("Kyrgyzstan", "KG"),
            KHM => ("Cambodia", "KH"),
            KOR => ("South Korea", "KR"),
            KWT => ("Kuwait", "KW"),
            LAO => ("Laos", "LA"),
            LBN => ("Lebanon", "LB"),
            LBY => ("Libya", "LY"),
            LKA => ("Sri Lanka", "LK"),
            LTU => ("Lithuania", "LT"),
            LUX => ("Luxembourg", "LU"),
            LVA => ("Latvia", "LV"),
            MAC => ("Macau", "MO"),
            MAR => ("Morocco", "MA"),
            MDA => ("Moldova", "MD"),
            MDV => ("Maldives", "MV"),
            MEX => ("Mexico", "MX"),
            MKD => ("North Macedonia", "MK"),
            MLT => ("Malta", "MT"),
            MMR => ("Myanmar", "MM"),
            MNE => ("Montenegro", "ME"),
            MNG => ("Mongolia", "MN"),
            MOZ => ("Mozambique", "MZ"),
            MRT => ("Mauritania", "MR"),
            MUS => ("Mauritius", "MU"),
            MYS => ("Malaysia", "MY"),
            NAM => ("Namibia", "NA"),
            NGA => ("Nigeria", "NG"),
            NIC => ("Nicaragua", "NI"),
            NLD => ("Netherlands", "NL"),
            NOR => ("Norway", "NO"),
            NPL => ("Nepal", "NP"),
            NZL => ("New Zealand", "NZ"),
            OMN => ("Oman", "OM"),
            PAK => ("Pakistan", "PK"),
            PAN => ("Panama", "PA"),
            PER => ("Peru", "PE"),
            PHL => ("Philippines", "PH"),
            PNG => ("Papua New Guinea", "PG"),
            POL => ("Poland", "PL"),
            PRY => ("Paraguay", "PY"),
            PRT => ("Portugal", "PT"),
            PSE => ("Palestine", "PS"),
            QAT => ("Qatar", "QA"),
            ROU => ("Romania", "RO"),
            RUS => ("Russia", "RU"),
            RWA => ("Rwanda", "RW"),
            SAU => ("Saudi Arabia", "SA"),
            SDN => ("Sudan", "SD"),
            SEN => ("Senegal", "SN"),
            SGP => ("Singapore", "SG"),
            SLV => ("El Salvador", "SV"),
            SRB => ("Serbia", "RS"),
            SUR => ("Suriname", "SR"),
            SVK => ("Slovakia", "SK"),
            SVN => ("Slovenia", "SI"),
            SWE => ("Sweden", "SE"),
            SYC => ("Seychelles", "SC"),
            SYR => ("Syria", "SY"),
            THA => ("Thailand", "TH"),
            TJK => ("Tajikistan", "TJ"),
            TKM => ("Turkmenistan", "TM"),
            TTO => ("Trinidad and Tobago", "TT"),
            TUN => ("Tunisia", "TN"),
            TUR => ("Turkey", "TR"),
            TWN => ("Taiwan", "TW"),
            TZA => ("Tanzania", "TZ"),
            UGA => ("Uganda", "UG"),
            UKR => ("Ukraine", "UA"),
            URY => ("Uruguay", "UY"),
            USA => ("United States", "US"),
            UZB => ("Uzbekistan", "UZ"),
            VEN => ("Venezuela", "VE"),
            VNM => ("Vietnam", "VN"),
            YEM => ("Yemen", "YE"),
            ZAF => ("South Africa", "ZA"),
            ZMB => ("Zambia", "ZM"),
            ZWE => ("Zimbabwe", "ZW"),
        }
    }
}

#[pymethods]
impl Country {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string()
    }

    /// The ISO 3166-1 alpha-2 code.
    #[getter]
    pub fn alpha2(&self) -> &'static str {
        self.data().1
    }

    /// The ISO 3166-1 alpha-3 code.
    #[getter]
    pub fn alpha3(&self) -> String {
        self.to_string()
    }

    /// The name of the country.
    #[getter]
    pub fn name(&self) -> &'static str {
        self.data().0
    }

    /// The Unicode regional-indicator flag emoji for the country.
    #[getter]
    pub fn flag(&self) -> String {
        self.alpha2()
            .chars()
            .map(|c| char::from_u32(0x1F1E6 + (c as u32 - 'A' as u32)).unwrap())
            .collect()
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Country {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, PyErr> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<Country>() {
            return Ok(bound.borrow().clone());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown country {s:?}.")))
    }
}
