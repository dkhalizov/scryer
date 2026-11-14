//! Parity net, import area, specification fixtures (stream import_specs).
//!
//! Suites:
//! - `scryer-application::parity_import::son_import_specs`:
//!   Sonarr Upgrade (SON-IMPORT-0215..0228, 0231..0236), MatchesGrab
//!   (0206..0209), MatchesFolder (0192, 0193, 0195).
//! - `scryer-application::parity_import::rad_import_specs`:
//!   Radarr Upgrade (RAD-IMPORT-0246..0255) and FreeSpace RAD-IMPORT-0216.
//!
//! Scryer code under test:
//! - Upgrade: `evaluate_admission` with `AdmissionPolicy::not_a_downgrade()`,
//!   the call of import/decide.rs:366 `evaluate_import_admission`, with the
//!   candidate tier from `quality_tier_index` (post_download_gate.rs:2241) and
//!   `CandidateFacts::with_profile_policy` as decide.rs does.
//! - MatchesGrab / MatchesFolder: `resolved_episode_ids_are_within_expected`
//!   (import/workflow/series.rs:886), the gate applied at series.rs:1757.
//!   Only that subset gate is checked, on the reference's already resolved
//!   episode ids; Scryer's episode resolution and pack routing are not.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | profile qualities (Sonarr `qualities`, Radarr allowed `profile_items`) | `criteria.quality_tiers` = distinct tiers, highest resolution first |
//! | quality name | tier: Unknown/CAM/TELECINE/DVDSCR none; SDTV, DVD, DVD-R, REGIONAL, WEBDL-480p, Bluray-480p 480P; Bluray-576p 576P; HDTV/WEBDL/Bluray-720p 720P; HDTV-1080p, Raw-HD, WEBDL/Bluray-1080p, BR-DISK 1080P; HDTV/WEBDL/Bluray-2160p 2160P |
//! | revision version v (Radarr `{version, real}`) | revision v - 1 (`real` > 0 has no field: recorded red) |
//! | DownloadPropersAndRepacks PreferAndUpgrade (default) / DoNotPrefer | `set_ignore_proper_repack(false / true)` |
//! | custom format score | candidate: `local_episode_custom_format_score` (absent = 0, as the reference's unset `CustomFormatScore`), Radarr `file_custom_format_score`; incumbent: sum of profile `format_items` over `existing_file_custom_formats`, Radarr `existing_file.custom_format_score` |
//! | Sonarr episodes[i] | episode id `ep{i}`, scope `Episodes(all)` |
//! | existing file (episode_file_id > 0 and a record) | one `Incumbent` per distinct file id, covering the episodes sharing it |
//! | Radarr movie_file_id > 0 and existing_file | one `Incumbent`, scope `Title` |
//! | missing file record | no incumbent (the reference accepts) |
//! | spec accepted | `verdict.rejection().is_none()` |
//! | MatchesGrab episodes, grabbed_release_episode_ids | target ids, expected set (as strings) |
//! | MatchesFolder episodes_for_file_info, episodes_for_folder_info ids | target ids, expected set |
//! | FreeSpace SkipFreeSpaceCheckWhenImporting = true (RAD-IMPORT-0216) | no Scryer setting; `disk_space_verdict_for_measurement(Some(0), 0)` (import/checks.rs:97, a disk reporting 0 bytes free, size 0 = LocalMovie.Size default) is compared with `accepted`; its 500 MiB reserve rejects, so red (D29) |
//!
//! With no group on either side the pair is unrelated (decision_pipeline.rs
//! `Facts::is_related_to`), so a higher revision is never preferred, but a
//! lower one of the same tier is refused unless the candidate ignores
//! revisions (P18): SON-IMPORT-0226 / RAD-IMPORT-0249 refuse the v1, and
//! RAD-IMPORT-0250/0251 (`set_ignore_proper_repack`) admit it.
//!
//! Scores are custom-format scores only (the parity_decision convention):
//! size, rule and other canonical score terms are 0 on both sides, so a
//! same-tier pair is decided by the ladder and the CF delta alone. In
//! production decide_import scores the candidate canonically and the
//! incumbent by its landed total (canonical_context.rs incumbent_bar), where
//! non-CF terms can break such a tie.
//!
//! Ignored keys: languages (neutral rank), local_episode.path, series.type,
//! local_episode_file_info, local_episode_custom_formats (the reference reads
//! only the score), profile.allowed, `spec` (dispatch only), notes. `path`,
//! `file_info`, `folder_info`, `download_client_item`: not read; they were
//! used only to choose the explicit ID lists (cases with a grab; folders that
//! are not full-season and name one episode).
//! No release group on either side, on purpose: the fixtures name none.
//!
//! Unmapped (tests: []), with reasons:
//! - AlreadyImported (SON-IMPORT-0158..0162, RAD-IMPORT-0203..0207): a
//!   download-history rule; Scryer keeps tracked-download state in
//!   repositories (result_state.rs:421), no pure function takes a history.
//! - EpisodeTitle (SON-IMPORT-0163..0166): no TBA or title-required concept.
//! - FullSeason (SON-IMPORT-0182..0184): no import check rejects a full-season
//!   file; it only routes to pack verification (series_plan.rs:46).
//! - HasAudioTrack (SON-IMPORT-0185..0187, RAD-IMPORT-0223..0225): the probe
//!   gate rejects only a missing video codec or duration;
//!   classify_required_audio (media/audio_requirements.rs:379, empty check at
//!   :389) deliberately never rejects a file with no audio tracks.
//! - GrabbedReleaseQuality (RAD-IMPORT-0217..0222): needs a
//!   ResolvedScoringContext, which has no constructor.
//! - Radarr MatchesGrab (RAD-IMPORT-0226..0230): the identity gate needs
//!   repositories (AppUseCase).
//! - NotMultiPart (RAD-IMPORT-0231..0240): no multi-part check on the import
//!   path.
//! - existing_file skips (SON-IMPORT-0188, 0203, 0210, 0179, RAD-IMPORT-0214,
//!   0241): Scryer's scan never runs import checks.
//! - FreeSpace which-folder-is-measured (SON-IMPORT-0176, RAD-IMPORT-0211):
//!   Scryer measures the nearest existing ancestor of the destination.
//! - No grab / no parse skips (SON-IMPORT-0189, 0190, 0204, 0205): the skip is
//!   inside the async import flow, the seamed predicate is never called.
//! - MatchesFolder full-season or multi-episode folders (SON-IMPORT-0191,
//!   0194, 0196..0199, 0201, 0202): pack members go through
//!   VerifiedEpisodePack (private, built by the async plan); SON-IMPORT-0200
//!   (special episodes) needs the async series import.
//! - detect_sample RAD-IMPORT-0159: probe count only.

use std::collections::HashSet;

use scryer_application::parity_seams::{
    AdmissionPolicy, AdmissionScope, AdmissionSubject, CandidateFacts, Incumbent,
    evaluate_admission, quality_tier_index, resolved_episode_ids_are_within_expected,
};
use scryer_application::{
    AudioLanguageRank, ImportVerdict, QualityProfile, builtin_1080p_profile, disk_space_verdict_for_measurement,
};
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

const SON_UPGRADE: &[&str] = &[
    "SON-IMPORT-0215", "SON-IMPORT-0216", "SON-IMPORT-0217", "SON-IMPORT-0218",
    "SON-IMPORT-0219", "SON-IMPORT-0220", "SON-IMPORT-0221", "SON-IMPORT-0222",
    "SON-IMPORT-0223", "SON-IMPORT-0224", "SON-IMPORT-0225", "SON-IMPORT-0226",
    "SON-IMPORT-0227", "SON-IMPORT-0228", "SON-IMPORT-0231", "SON-IMPORT-0232",
    "SON-IMPORT-0233", "SON-IMPORT-0234", "SON-IMPORT-0235", "SON-IMPORT-0236",
];
const SON_GRAB: &[&str] = &["SON-IMPORT-0206", "SON-IMPORT-0207", "SON-IMPORT-0208", "SON-IMPORT-0209"];
const SON_FOLDER: &[&str] = &["SON-IMPORT-0192", "SON-IMPORT-0193", "SON-IMPORT-0195"];
const RAD_IDS: &[&str] = &[
    "RAD-IMPORT-0246", "RAD-IMPORT-0247", "RAD-IMPORT-0248", "RAD-IMPORT-0249",
    "RAD-IMPORT-0250", "RAD-IMPORT-0251", "RAD-IMPORT-0252", "RAD-IMPORT-0253",
    "RAD-IMPORT-0254", "RAD-IMPORT-0255", "RAD-IMPORT-0216",
];
const CREATED_AT: &str = "2026-01-15T12:00:00Z";

/// Quality name -> Scryer tier label. An unknown name is a harness bug.
fn tier(name: &str) -> Option<&'static str> {
    match name {
        "Unknown" | "CAM" | "TELECINE" | "DVDSCR" => None,
        "SDTV" | "DVD" | "DVD-R" | "REGIONAL" | "WEBDL-480p" | "Bluray-480p" => Some("480P"),
        "Bluray-576p" => Some("576P"),
        "HDTV-720p" | "WEBDL-720p" | "Bluray-720p" => Some("720P"),
        "HDTV-1080p" | "Raw-HD" | "WEBDL-1080p" | "Bluray-1080p" | "BR-DISK" => Some("1080P"),
        "HDTV-2160p" | "WEBDL-2160p" | "Bluray-2160p" => Some("2160P"),
        other => panic!("unknown quality {other}"),
    }
}

/// Profile with the distinct tiers of `names`, highest resolution first.
fn profile(names: &[&str], ignore_revisions: bool) -> QualityProfile {
    let mut tiers: Vec<&str> = Vec::new();
    for t in names.iter().filter_map(|n| tier(n)) {
        if !tiers.contains(&t) {
            tiers.push(t);
        }
    }
    tiers.sort_by_key(|t| std::cmp::Reverse(t.trim_end_matches('P').parse::<u32>().unwrap()));
    let mut p = builtin_1080p_profile();
    p.criteria.quality_tiers = tiers.iter().map(|t| t.to_string()).collect();
    p.criteria.set_ignore_proper_repack(ignore_revisions);
    p
}

fn incumbent(p: &QualityProfile, n: usize, quality: &str, revision: i64, score: i32, covers: Vec<String>) -> Incumbent {
    Incumbent {
        tier_index: quality_tier_index(&p.criteria, tier(quality)),
        revision: revision as i32 - 1,
        file_id: format!("file{n}"),
        file_path: format!("/parity/file{n}.mkv"),
        release_group: None,
        score,
        language: AudioLanguageRank::default(),
        covers,
        created_at: CREATED_AT.into(),
    }
}

fn accepted(
    p: &QualityProfile,
    scope: AdmissionScope,
    incumbents: Vec<Incumbent>,
    quality: &str,
    revision: i64,
    score: i32,
) -> bool {
    let candidate = CandidateFacts::new(quality_tier_index(&p.criteria, tier(quality)), revision as i32 - 1, score)
        .with_profile_policy(None, &p.criteria);
    let subject = AdmissionSubject::new(scope, incumbents.into_iter().map(|i| (i, true)));
    evaluate_admission(&subject, candidate, &AdmissionPolicy::not_a_downgrade())
        .rejection()
        .is_none()
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

fn son_upgrade(g: &Value) -> bool {
    let ignore = g["config"]["download_propers_and_repacks"].as_str() == Some("DoNotPrefer");
    let names = strs(&g["series"]["profile"]["qualities"]);
    let p = profile(&names, ignore);
    let weight = |formats: &Value| -> i32 {
        strs(formats)
            .iter()
            .map(|f| {
                g["series"]["profile"]["format_items"]
                    .as_array()
                    .and_then(|items| items.iter().find(|i| i["format"] == *f))
                    .map_or(0, |i| i["score"].as_i64().unwrap_or(0) as i32)
            })
            .sum()
    };
    let existing_score = weight(&g["existing_file_custom_formats"]);
    let local = &g["local_episode"];
    let score = g["local_episode_custom_format_score"].as_i64().unwrap_or(0) as i32;
    let episodes = g["episodes"].as_array().expect("episodes");
    let ids: Vec<String> = (0..episodes.len()).map(|i| format!("ep{i}")).collect();
    let mut incumbents: Vec<Incumbent> = Vec::new();
    let mut seen: Vec<i64> = Vec::new();
    for ep in episodes {
        let fid = ep["episode_file_id"].as_i64().unwrap_or(0);
        if fid <= 0 || ep["file"].is_null() || seen.contains(&fid) {
            continue;
        }
        seen.push(fid);
        let covers = episodes
            .iter()
            .enumerate()
            .filter(|(_, e)| e["episode_file_id"].as_i64() == Some(fid))
            .map(|(j, _)| format!("ep{j}"))
            .collect();
        let f = &ep["file"];
        incumbents.push(incumbent(
            &p,
            fid as usize,
            f["quality"].as_str().expect("file quality"),
            f["revision"].as_i64().unwrap_or(1),
            existing_score,
            covers,
        ));
    }
    accepted(
        &p,
        AdmissionScope::Episodes(ids),
        incumbents,
        local["quality"].as_str().expect("quality"),
        local["revision"].as_i64().unwrap_or(1),
        score,
    )
}

fn id_set(v: &Value) -> Vec<String> {
    v.as_array().expect("ids").iter().map(|x| x.as_i64().expect("id").to_string()).collect()
}

fn ids_of(v: &Value) -> Vec<String> {
    v.as_array().expect("episodes").iter().map(|e| e["id"].as_i64().expect("id").to_string()).collect()
}

fn within(target: Vec<String>, expected: Vec<String>) -> bool {
    resolved_episode_ids_are_within_expected(&target, &expected.into_iter().collect::<HashSet<_>>())
}

fn son_matches_grab(g: &Value) -> bool {
    within(id_set(&g["episodes"]), id_set(&g["grabbed_release_episode_ids"]))
}

fn son_matches_folder(g: &Value) -> bool {
    within(ids_of(&g["episodes_for_file_info"]), ids_of(&g["episodes_for_folder_info"]))
}

#[test]
fn son_import_specs() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::son_import_specs") else { return };
    let dir = "corpus/reference/sonarr/import/MediaFiles.EpisodeImport.Specifications.";
    let groups: [(&str, &[&str], fn(&Value) -> bool); 3] = [
        ("UpgradeSpecificationFixture.json", SON_UPGRADE, son_upgrade),
        ("MatchesGrabSpecificationFixture.json", SON_GRAB, son_matches_grab),
        ("MatchesFolderSpecificationFixture.json", SON_FOLDER, son_matches_folder),
    ];
    for (file, ids, handler) in groups {
        let cases: Vec<Value> = corpus(&format!("{dir}{file}"));
        for case in &cases {
            let id = case["id"].as_str().expect("case id");
            if !ids.contains(&id) || !suite.wants(id) {
                continue;
            }
            let actual = handler(&case["input"]["given"]);
            suite.check(id, json!({"accepted": case["expect"]["accepted"]}), json!({"accepted": actual}));
        }
    }
    suite.finish();
}

fn rad_upgrade(g: &Value) -> Result<bool, String> {
    let names: Vec<&str> = g["profile_items"]
        .as_array()
        .expect("profile_items")
        .iter()
        .filter(|i| i["allowed"].as_bool() == Some(true))
        .filter_map(|i| i["quality"].as_str())
        .collect();
    let p = profile(&names, g["propers"].as_str() == Some("DoNotPrefer"));
    let version = |r: &Value| r["version"].as_i64().unwrap_or(1);
    if g["file_revision"]["real"].as_i64().unwrap_or(0) > 0
        || g["existing_file"]["revision"]["real"].as_i64().unwrap_or(0) > 0
    {
        return Err("revision `real` has no Scryer field".into());
    }
    let existing = &g["existing_file"];
    let incumbents = if g["movie_file_id"].as_i64().unwrap_or(0) > 0 && !existing.is_null() {
        vec![incumbent(
            &p,
            1,
            existing["quality"].as_str().expect("existing quality"),
            version(&existing["revision"]),
            existing["custom_format_score"].as_i64().unwrap_or(0) as i32,
            Vec::new(),
        )]
    } else {
        Vec::new()
    };
    Ok(accepted(
        &p,
        AdmissionScope::Title,
        incumbents,
        g["file_quality"].as_str().expect("file_quality"),
        version(&g["file_revision"]),
        g["file_custom_format_score"].as_i64().unwrap_or(0) as i32,
    ))
}

#[test]
fn rad_import_specs() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::rad_import_specs") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/radarr/import/specifications.json");
    for case in &cases {
        let id = case["id"].as_str().expect("case id");
        if !RAD_IDS.contains(&id) || !suite.wants(id) {
            continue;
        }
        let g = &case["input"]["given"];
        match g["spec"].as_str() {
            Some("Upgrade") => match rad_upgrade(g) {
                Ok(actual) => suite.check(
                    id,
                    json!({"accepted": case["expect"]["accepted"]}),
                    json!({"accepted": actual}),
                ),
                Err(why) => suite.record(id, false, why),
            },
            Some("FreeSpace") => {
                // Reference: skip=true accepts whatever the disk reports. Scryer has no
                // such setting: a disk reporting 0 bytes free, size 0 (LocalMovie.Size
                // default), is judged by the production verdict, which keeps its reserve.
                assert_eq!(g["skip_free_space_check"], true);
                let accepted = matches!(disk_space_verdict_for_measurement(Some(0), 0), ImportVerdict::Accept);
                suite.check(id, json!({"accepted": case["expect"]["accepted"]}), json!({"accepted": accepted}));
            }
            other => panic!("{id}: unexpected spec {other:?}"),
        }
    }
    suite.finish();
}
