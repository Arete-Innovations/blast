use crate::cata_log;
use once_cell::sync::Lazy;
use rocket::http::CookieJar;
use rocket::request::FlashMessage;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

#[derive(Serialize, Debug, Default)]
pub struct BaseContext {
    pub lang: Value,
    pub translations: Value,
    pub flash: Option<(String, String)>,
    pub title: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Context<T: Serialize = ()> {
    #[serde(flatten)]
    pub base: BaseContext,
    #[serde(flatten)]
    pub extra: T,
}

pub static TRANSLATIONS: Lazy<HashMap<String, Value>> = Lazy::new(|| {
    let mut map = HashMap::new();

    let en_data = match fs::read_to_string("src/assets/locale/en.json") {
        Ok(data) => {
            cata_log!(Info, "Successfully read en.json");
            data
        }
        Err(e) => {
            let msg = format!("Failed to read en.json: {}", e);
            cata_log!(Error, msg);
            "".to_string()
        }
    };

    let en_translations: Value = serde_json::from_str(&en_data).expect("Invalid JSON in en.json");
    map.insert("en".to_string(), en_translations);

    match fs::read_dir("build/sources/locale") {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    if let Some(lang_code) = path.file_stem().and_then(|os_str| os_str.to_str()) {
                        if lang_code == "en" {
                            continue;
                        }
                        match fs::read_to_string(&path) {
                            Ok(data) => match serde_json::from_str::<Value>(&data) {
                                Ok(trans) => {
                                    map.insert(lang_code.to_string(), trans);
                                    cata_log!(Info, format!("Loaded translations for language: {}", lang_code));
                                }
                                Err(_) => {
                                    let msg = format!("Invalid JSON in file: {:?}", path);
                                    cata_log!(Error, msg);
                                }
                            },
                            Err(e) => {
                                let msg = format!("Failed to read file {:?}: {}", path, e);
                                cata_log!(Error, msg);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            cata_log!(Warning, format!("No locale directory found: {}", e));
        }
    }
    map
});

pub fn get_translations(lang_code: &str) -> &Value {
    TRANSLATIONS.get(lang_code).unwrap_or_else(|| {
        cata_log!(Warning, format!("Language '{}' not found, falling back to 'en'", lang_code));
        TRANSLATIONS.get("en").expect("Default English translations missing")
    })
}

impl Context<()> {
    pub fn new(base: BaseContext) -> Self {
        Context { base, extra: () }
    }
}

impl BaseContext {
    pub fn build(page_key: &str, cookies: &CookieJar<'_>, flash: Option<FlashMessage<'_>>) -> Self {
        let lang_code = cookies.get("lang").map(|c| c.value().to_string()).unwrap_or_else(|| {
            cata_log!(Info, "No language cookie found, defaulting to 'en'");
            "en".to_string()
        });
        cata_log!(Info, format!("Using language code: {}", lang_code));
        let translations = get_translations(&lang_code).clone();
        cata_log!(Debug, "Translations loaded successfully");
        let lang = translations.clone();
        let flash = flash.map(|f| (f.kind().to_string(), f.message().to_string()));

        let title = translations
            .get("pages")
            .and_then(|pages| pages.get(page_key))
            .and_then(|page| page.get("title"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        if let Some(ref t) = title {
            cata_log!(Info, format!("Page title set to: {}", t));
        } else {
            cata_log!(Warning, format!("No title found for page key: {}", page_key));
        }
        BaseContext { lang, translations, flash, title }
    }

    pub fn with_extra<T: Serialize>(self, extra: T) -> Context<T> {
        Context { base: self, extra }
    }
}
