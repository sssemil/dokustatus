use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;

use crate::application::language::UserLanguage;

#[derive(Debug, Deserialize)]
struct StatusCallout {
    title: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Dictionary {
    #[serde(rename = "statusCallouts")]
    status_callouts: HashMap<String, StatusCallout>,
}

static RAW_EN: &str = include_str!("../../dictionaries/en.json");
static RAW_DE: &str = include_str!("../../dictionaries/de.json");

static DICT_EN: Lazy<Dictionary> =
    Lazy::new(|| serde_json::from_str(RAW_EN).expect("failed to parse en dictionary"));

static DICT_DE: Lazy<Dictionary> =
    Lazy::new(|| serde_json::from_str(RAW_DE).expect("failed to parse de dictionary"));

static DICT_EN_VALUE: Lazy<Value> =
    Lazy::new(|| serde_json::from_str(RAW_EN).expect("failed to parse en dictionary json"));
static DICT_DE_VALUE: Lazy<Value> =
    Lazy::new(|| serde_json::from_str(RAW_DE).expect("failed to parse de dictionary json"));

#[derive(Debug, Clone)]
pub struct StatusCopy {
    pub title: String,
    pub message: Option<String>,
}

pub fn status_copy(lang: UserLanguage, status: Option<&str>) -> StatusCopy {
    let code = status.unwrap_or("UNBEKANNT");
    let dict = match lang {
        UserLanguage::En => &*DICT_EN,
        UserLanguage::De => &*DICT_DE,
    };
    let entry = dict
        .status_callouts
        .get(code)
        .or_else(|| dict.status_callouts.get("default"));
    let (title, message) = entry
        .map(|c| {
            (
                c.title.replace("{status}", code),
                c.message.as_ref().map(|m| m.replace("{status}", code)),
            )
        })
        .unwrap_or_else(|| (code.to_string(), None));
    StatusCopy { title, message }
}

pub fn dictionary_json(lang: UserLanguage) -> Value {
    match lang {
        UserLanguage::En => DICT_EN_VALUE.clone(),
        UserLanguage::De => DICT_DE_VALUE.clone(),
    }
}

pub fn t(lang: UserLanguage, path: &str) -> String {
    let dict = match lang {
        UserLanguage::En => &*DICT_EN_VALUE,
        UserLanguage::De => &*DICT_DE_VALUE,
    };
    let mut cursor = dict;
    for segment in path.split('.') {
        if let Some(v) = cursor.get(segment) {
            cursor = v;
        } else {
            return path.to_string();
        }
    }
    cursor.as_str().unwrap_or(path).to_string()
}

pub fn status_label(lang: UserLanguage) -> String {
    t(lang, "labels.status")
}

pub fn pickup_label(lang: UserLanguage) -> String {
    t(lang, "labels.pickup")
}

pub fn doc_type_label(lang: UserLanguage) -> String {
    t(lang, "labels.docType")
}
