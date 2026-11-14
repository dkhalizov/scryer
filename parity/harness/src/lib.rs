//! Parity net support (notes/11-parity-net-design.md): load and validate the
//! catalogue, run a suite under the ratchet, write per-suite results for the
//! report. Test-only: no product crate depends on this outside dev-deps.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::path::{Path, PathBuf};

pub const APPS: &[&str] = &["SON", "RAD", "PRW", "BAZ", "SER", "MNT", "QBT"];
pub const AREAS: &[&str] = &[
    "parse",
    "decision",
    "quality",
    "naming",
    "import",
    "scan",
    "torrent",
    "requests",
    "maintenance",
    "subtitles",
    "indexers",
    "notify",
];

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Unmapped,
    Red,
    Green,
    Waived,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Reference,
    Captured,
    Config,
    Observed,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub behaviour: String,
    pub source: Source,
    #[serde(rename = "ref")]
    pub reference: String,
    pub in_scope: bool,
    pub status: Status,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub tests: Vec<String>,
}

pub struct Catalogue {
    /// (area, entry), in file then entry order.
    pub entries: Vec<(String, Entry)>,
}

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn parity_dir() -> PathBuf {
    root().join("parity")
}

/// Reads a JSON file under `parity/` (e.g. `corpus/reference/sonarr/parse/x.json`).
pub fn corpus<T: serde::de::DeserializeOwned>(rel: &str) -> T {
    let path = parity_dir().join(rel);
    // A checkout without the private captured corpus (D45): hand back an empty
    // list or map so the suite reaches `Suite::open` and skips as designed.
    if rel.starts_with("corpus/captured/") && !parity_dir().join("corpus/captured").is_dir() {
        return empty_corpus(rel);
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn empty_corpus<T: serde::de::DeserializeOwned>(rel: &str) -> T {
    ["[]", "{}"]
        .iter()
        .find_map(|empty| serde_json::from_str(empty).ok())
        .unwrap_or_else(|| {
            panic!("{rel}: private corpus absent and the type is neither a list nor a map")
        })
}

impl Catalogue {
    /// Loads `parity/catalogue/<area>[.<part>].yaml`. Panics on unreadable or
    /// malformed files; call `validate` for the schema rules.
    pub fn load() -> Self {
        let dir = parity_dir().join("catalogue");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        files.sort();
        let mut entries = Vec::new();
        for file in files {
            let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
            let area = stem.split('.').next().unwrap().to_string();
            let text = std::fs::read_to_string(&file).unwrap();
            let list: Option<Vec<Entry>> = serde_norway::from_str(&text)
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            for entry in list.unwrap_or_default() {
                entries.push((area.clone(), entry));
            }
        }
        Self { entries }
    }

    /// Schema rules from the design. Returns every problem found.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut seen = BTreeSet::new();
        for (area, e) in &self.entries {
            let mut err = |msg: String| errors.push(format!("{}: {msg}", e.id));
            if !AREAS.contains(&area.as_str()) {
                err(format!("unknown area file `{area}`"));
            }
            match e.id.split('-').collect::<Vec<_>>()[..] {
                [app, id_area, n]
                    if APPS.contains(&app)
                        && id_area == area.to_uppercase()
                        && n.len() >= 4
                        && n.bytes().all(|b| b.is_ascii_digit()) => {}
                _ => err(format!("id must be <APP>-{}-<nnnn>, APP in {APPS:?}", area.to_uppercase())),
            }
            if !seen.insert(e.id.clone()) {
                err("duplicate id".into());
            }
            if e.behaviour.trim().is_empty() || e.reference.trim().is_empty() {
                err("behaviour and ref are required".into());
            }
            if e.status == Status::Waived && e.decision.as_deref().is_none_or(|d| d.trim().is_empty()) {
                err("waived needs a decision".into());
            }
            match (e.status, e.tests.is_empty()) {
                (Status::Unmapped, false) => err("unmapped entries list no tests".into()),
                (Status::Red | Status::Green, true) => err("red/green entries need tests".into()),
                _ => {}
            }
            if !e.in_scope && matches!(e.status, Status::Red | Status::Green) {
                err("out-of-scope entries are unmapped or waived".into());
            }
            for t in &e.tests {
                if parse_test(t).is_none() {
                    err(format!("test `{t}` must be <crate>::parity_<name>::<fn>"));
                }
            }
        }
        errors
    }

    pub fn suite_entries(&self, suite: &str) -> BTreeMap<String, Status> {
        self.entries
            .iter()
            .filter(|(_, e)| e.tests.iter().any(|t| t == suite))
            .map(|(_, e)| (e.id.clone(), e.status))
            .collect()
    }
}

/// `<crate>::parity_<name>::<fn>` → (crate, test binary, fn).
pub fn parse_test(t: &str) -> Option<(&str, &str, &str)> {
    match t.split("::").collect::<Vec<_>>()[..] {
        [krate, bin, f] if !krate.is_empty() && bin.starts_with("parity_") && !f.is_empty() => Some((krate, bin, f)),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CaseResult {
    pub id: String,
    pub status: Status,
    pub pass: bool,
    pub detail: String,
    pub violation: Option<String>,
}

/// One harness test under the ratchet. Record one outcome per catalogue ID
/// mapped to this suite, then call `finish`.
pub struct Suite {
    name: String,
    expected: BTreeMap<String, Status>,
    results: BTreeMap<String, (bool, String)>,
    extra: Vec<String>,
    /// PARITY_ADOPT=1: in-scope `unmapped` IDs this run may also record; their
    /// outcomes become first statuses via `parity/adopt.py`. No ratchet panic.
    adoptable: BTreeSet<String>,
}

impl Suite {
    /// `name` is the catalogue `tests:` value, e.g.
    /// `scryer-application::parity_parse::son_parse`.
    pub fn new(name: &str) -> Self {
        assert!(parse_test(name).is_some(), "bad suite name {name}");
        let catalogue = Catalogue::load();
        let errors = catalogue.validate();
        assert!(errors.is_empty(), "catalogue invalid:\n{}", errors.join("\n"));
        let expected = catalogue.suite_entries(name);
        let adoptable: BTreeSet<String> = if std::env::var_os("PARITY_ADOPT").is_some() {
            catalogue
                .entries
                .iter()
                .filter(|(_, e)| e.in_scope && e.status == Status::Unmapped)
                .map(|(_, e)| e.id.clone())
                .collect()
        } else {
            BTreeSet::new()
        };
        assert!(
            !expected.is_empty() || !adoptable.is_empty(),
            "suite {name} has no catalogue entries (vacuous)"
        );
        Self { name: name.to_string(), expected, results: BTreeMap::new(), extra: Vec::new(), adoptable }
    }

    /// Like `new`, but `None` when the suite has no catalogue entries and the
    /// private captured corpus is absent (a checkout without it, D45). With the
    /// corpus present an empty suite still panics in `new` (D30).
    pub fn open(name: &str) -> Option<Self> {
        let captured = parity_dir().join("corpus/captured").is_dir();
        if !captured && std::env::var_os("PARITY_ADOPT").is_none() && Catalogue::load().suite_entries(name).is_empty() {
            eprintln!("skipped {name}: no catalogue entries and no captured corpus");
            return None;
        }
        Some(Self::new(name))
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.expected.keys().map(String::as_str)
    }

    /// Whether this run should execute the case: mapped to this suite, or
    /// adoptable in PARITY_ADOPT mode. Skip the rest (out of scope, other suites).
    pub fn wants(&self, id: &str) -> bool {
        self.expected.contains_key(id) || self.adoptable.contains(id)
    }

    pub fn record(&mut self, id: &str, pass: bool, detail: impl Into<String>) {
        if self.results.insert(id.to_string(), (pass, detail.into())).is_some() {
            self.extra.push(format!("{id}: recorded twice"));
        }
    }

    /// Records pass when `expected == actual`, with both in the detail otherwise.
    pub fn check<T: PartialEq + Debug>(&mut self, id: &str, expected: T, actual: T) {
        let pass = expected == actual;
        let detail = if pass { String::new() } else { format!("expected {expected:?}, got {actual:?}") };
        self.record(id, pass, detail);
    }

    /// Writes `parity/report/results/<suite>.json`, then panics on any
    /// ratchet violation (see `verdict`).
    pub fn finish(self) {
        let (out, violations) = self.verdict();
        let dir = parity_dir().join("report/results");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(format!("{}.json", self.name.replace("::", "__")));
        std::fs::write(&file, serde_json::to_string_pretty(&out).unwrap()).unwrap();
        if !self.adoptable.is_empty() {
            eprintln!("PARITY_ADOPT: {} results written to {}; violations not enforced:\n{}", out.len(), file.display(), violations.join("\n"));
            return;
        }
        assert!(violations.is_empty(), "ratchet ({}):\n{}", self.name, violations.join("\n"));
    }

    /// Violations: green that fails, red that passes, an ID not run, an ID
    /// run that the catalogue does not map to this suite, a double record.
    fn verdict(&self) -> (Vec<CaseResult>, Vec<String>) {
        let mut violations = self.extra.clone();
        let mut out = Vec::new();
        for (id, status) in &self.expected {
            let (pass, detail, violation) = match self.results.get(id) {
                None => (false, String::new(), Some("not run".to_string())),
                Some((pass, detail)) => {
                    let v = match (status, pass) {
                        (Status::Green, false) => Some(format!("green but fails: {detail}")),
                        (Status::Red, true) => Some("red but passes: flip it to green in the catalogue".into()),
                        (Status::Unmapped, _) => Some("unmapped entry has a test".into()),
                        _ => None,
                    };
                    (*pass, detail.clone(), v)
                }
            };
            if let Some(v) = &violation {
                violations.push(format!("{id}: {v}"));
            }
            out.push(CaseResult { id: id.clone(), status: *status, pass, detail, violation });
        }
        for (id, (pass, detail)) in self.results.iter().filter(|(id, _)| !self.expected.contains_key(*id)) {
            if self.adoptable.contains(id) {
                out.push(CaseResult { id: id.clone(), status: Status::Unmapped, pass: *pass, detail: detail.clone(), violation: None });
            } else {
                violations.push(format!("{id}: run but not mapped to {} in the catalogue", self.name));
            }
        }
        (out, violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_private_corpus_reads_as_empty_list_or_map() {
        assert!(empty_corpus::<Vec<String>>("x").is_empty());
        assert!(empty_corpus::<BTreeMap<String, u8>>("x").is_empty());
        assert!(empty_corpus::<serde_json::Value>("x").as_array().is_some_and(|a| a.is_empty()));
    }

    #[test]
    #[should_panic(expected = "neither a list nor a map")]
    fn absent_private_corpus_of_another_shape_panics_clearly() {
        empty_corpus::<String>("x");
    }

    fn suite(expected: &[(&str, Status)]) -> Suite {
        Suite {
            name: "x::parity_x::x".into(),
            expected: expected.iter().map(|(i, s)| (i.to_string(), *s)).collect(),
            results: BTreeMap::new(),
            extra: Vec::new(),
            adoptable: BTreeSet::new(),
        }
    }

    #[test]
    fn ratchet_flags_every_mismatch() {
        let mut s = suite(&[
            ("SON-PARSE-0001", Status::Green),
            ("SON-PARSE-0002", Status::Red),
            ("SON-PARSE-0003", Status::Green),
            ("SON-PARSE-0004", Status::Red),
            ("SON-PARSE-0005", Status::Waived),
            ("SON-PARSE-0006", Status::Green),
        ]);
        s.check("SON-PARSE-0001", 1, 1); // green passes: ok
        s.check("SON-PARSE-0002", 1, 2); // red fails: ok
        s.check("SON-PARSE-0003", 1, 2); // green fails: violation
        s.check("SON-PARSE-0004", 2, 2); // red passes: violation
        s.check("SON-PARSE-0005", 1, 2); // waived: ok either way
        // 0006 not run: violation
        s.check("SON-PARSE-0099", 1, 1); // not mapped: violation
        s.check("SON-PARSE-0001", 1, 1); // twice: violation
        let (out, v) = s.verdict();
        assert_eq!(out.len(), 6);
        let ids: Vec<&str> = v.iter().map(|m| &m[..14]).collect();
        assert_eq!(ids, ["SON-PARSE-0001", "SON-PARSE-0003", "SON-PARSE-0004", "SON-PARSE-0006", "SON-PARSE-0099"]);
    }

    #[test]
    fn adopt_mode_accepts_unmapped_ids() {
        let mut s = suite(&[("SON-PARSE-0001", Status::Green)]);
        s.adoptable.insert("SON-PARSE-0002".into());
        assert!(s.wants("SON-PARSE-0002") && !s.wants("SON-PARSE-0003"));
        s.check("SON-PARSE-0001", 1, 1);
        s.check("SON-PARSE-0002", 1, 2);
        s.check("SON-PARSE-0003", 1, 1); // neither mapped nor adoptable
        let (out, v) = s.verdict();
        assert_eq!(out.len(), 2);
        assert_eq!(v.len(), 1);
        assert!(v[0].starts_with("SON-PARSE-0003"));
    }

    #[test]
    fn clean_suite_has_no_violations() {
        let mut s = suite(&[("SON-PARSE-0001", Status::Green), ("SON-PARSE-0002", Status::Red)]);
        s.check("SON-PARSE-0001", "a", "a");
        s.check("SON-PARSE-0002", "a", "b");
        assert!(s.verdict().1.is_empty());
    }

    #[test]
    fn validator_rules() {
        let e = |id: &str, status: Status, tests: &[&str], decision: Option<&str>| {
            ("parse".to_string(), Entry {
                id: id.into(),
                behaviour: "b".into(),
                source: Source::Reference,
                reference: "r".into(),
                in_scope: true,
                status,
                decision: decision.map(Into::into),
                tests: tests.iter().map(|t| t.to_string()).collect(),
            })
        };
        let t = &["c::parity_parse::f"][..];
        let good = Catalogue { entries: vec![
            e("SON-PARSE-0001", Status::Green, t, None),
            e("RAD-PARSE-0001", Status::Unmapped, &[], None),
            e("SON-PARSE-0002", Status::Waived, &[], Some("why")),
        ]};
        assert_eq!(good.validate(), Vec::<String>::new());
        let bad = Catalogue { entries: vec![
            e("SON-PARSE-0001", Status::Green, t, None),
            e("SON-PARSE-0001", Status::Red, t, None),        // duplicate
            e("XXX-PARSE-0003", Status::Red, t, None),        // app
            e("SON-NAMING-0004", Status::Red, t, None),       // area mismatch
            e("SON-PARSE-5", Status::Red, t, None),           // digits
            e("SON-PARSE-0006", Status::Waived, &[], None),   // no decision
            e("SON-PARSE-0007", Status::Unmapped, t, None),   // unmapped with tests
            e("SON-PARSE-0008", Status::Green, &[], None),    // green without tests
            e("SON-PARSE-0009", Status::Red, &["bad"], None), // test name
        ]};
        assert_eq!(bad.validate().len(), 8);
    }
}
