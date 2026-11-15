//! Shared by `parity_quality` and `parity_decision` (`#[path]` module, not a
//! test target): an arr profile's custom-format scores the way the migration
//! skill reaches them (`skills/scryer-migrate-from-arr/references/custom-formats.md`
//! steps 1, 3 and 4). Review the whole export with no scores, map each review
//! id's `[i]` prefix back to `formats[i].id`, score it from the profile's
//! `formatItems` (a format the profile does not list scores 0), translate again.

use std::collections::HashMap;

use scryer_application::arr_custom_format::{
    ArrCustomFormatTranslation, ArrFormatStatus, ArrSourceApplication, translate_arr_custom_formats,
};
use serde_json::Value;

pub(crate) fn translate_with_profile(
    source: ArrSourceApplication,
    formats: &[Value],
    profile: &Value,
) -> Result<ArrCustomFormatTranslation, String> {
    let paste = Value::from(formats.to_vec()).to_string();
    let refused = |e| format!("import refused: {e:?}");
    let review = translate_arr_custom_formats(source, &paste, &HashMap::new()).map_err(refused)?;
    let items = profile["formatItems"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let scores = review
        .formats
        .iter()
        .filter_map(|f| {
            let i: usize = f.id.strip_prefix('[')?.split_once(']')?.0.parse().ok()?;
            let arr_id = formats.get(i)?["id"].as_i64();
            let score = items
                .iter()
                .find(|item| item["format"].as_i64() == arr_id)
                .and_then(|item| item["score"].as_i64())
                .unwrap_or(0);
            Some((f.id.clone(), i32::try_from(score).ok()?))
        })
        .collect();
    translate_arr_custom_formats(source, &paste, &scores).map_err(refused)
}

/// The score the translated rule set gives format `name`: its
/// `score_entry["<ident>_<pos+1>"] := <score> if {` line (translate.rs). None
/// when the format is missing or disabled.
pub(crate) fn rule_score(t: &ArrCustomFormatTranslation, name: &str) -> Option<f64> {
    let pos = t.formats.iter().position(|f| f.name == name)?;
    if t.formats[pos].status != ArrFormatStatus::Translated {
        return None;
    }
    let key_end = format!("_{}\"] := ", pos + 1);
    let line = t
        .rego_source
        .lines()
        .find(|l| l.starts_with("score_entry[\"") && l.contains(&key_end))?;
    line.split_once(" := ")?
        .1
        .split_once(" if {")?
        .0
        .trim()
        .parse()
        .ok()
}
