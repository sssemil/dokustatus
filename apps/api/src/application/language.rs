#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLanguage {
    En,
    De,
}

impl UserLanguage {
    pub fn from_raw(raw: Option<&str>) -> Self {
        let candidate = raw
            .unwrap_or("de")
            .split(',')
            .next()
            .unwrap_or("de")
            .split(['-', '_'])
            .next()
            .unwrap_or("de")
            .to_lowercase();
        match candidate.as_str() {
            "en" => UserLanguage::En,
            _ => UserLanguage::De,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UserLanguage::En => "en",
            UserLanguage::De => "de",
        }
    }
}
