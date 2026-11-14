//! Stream scan_reader: Sonarr VideoFileInfoReaderFixture
//! (parity/corpus/reference/sonarr/scan/MediaFiles.MediaInfo.VideoFileInfoReaderFixture.json).
//!
//! Suites
//! - `son_scan_reader_hdr`: SON-SCAN-0043..0053 (GetHdrFormat). Scryer's HDR
//!   classification runs inside `build_analysis` (lib.rs, via
//!   `scryer_mediainfo::parity_seams`); a `RawTrack` is built from the case
//!   input and `MediaAnalysis.video_hdr_format` is compared.
//! - `son_scan_reader_file`: SON-SCAN-0040..0042 (GetRunTime, GetMediaInfo).
//!   Runs `analyze_catalog_file` on Sonarr's own sample, vendored byte-identical
//!   as `tests/parity_scan/media/H264_sample.mp4` (429076 bytes, sha256
//!   f4867d2a...2816; origin Sonarr@v4.0.20.3014
//!   src/NzbDrone.Core.Test/Files/Media/H264_sample.mp4, GPL-3).
//!
//! Translation, hdr suite (reference -> Scryer):
//! - bit_depth -> metadata.bit_depth
//! - colour_primaries (ffprobe name -> H.273 code in metadata.color.primaries):
//!   "" -> None, bt709 -> 1, bt2020 -> 9
//! - transfer_characteristics (-> metadata.color.transfer): "" -> None,
//!   bt709 -> 1, bt2020-10 -> 14, smpte2084 -> 16, arib-std-b67 -> 18
//! - side_data: SideData -> nothing; MasteringDisplayMetadata ->
//!   color.mastering_display = Some(default); ContentLightLevelMetadata ->
//!   color.content_light = Some(default); HdrDynamicMetadataSpmte2094 ->
//!   has_hdr10plus = true. Other names panic.
//! - hdr_format: None -> None, Hlg10 -> "HLG", Pq10 -> "PQ10" (Scryer has no
//!   PQ10 label, so 0049/0050 are red), Hdr10 -> "HDR10", Hdr10Plus -> "HDR10+".
//! 0051/0052 match through bt2020 primaries, not through the side data:
//! Scryer's HDR10 rule does not need mastering/light-level metadata.
//!
//! Translation, file suite (compared as one JSON object over the case's keys;
//! `a` = MediaAnalysis, `vs` = selected video stream of a.details
//! (metadata.id == selected_video_id, else the first video), `au` = first
//! audio stream of a.details):
//! - run_time_seconds (TimeSpan.Seconds, the seconds component) ->
//!   floor(details.duration_seconds) % 60. It reads details.duration_seconds
//!   (f64, 10.563 for the sample) because TimeSpan.Seconds truncates a precise
//!   duration. MediaAnalysis.duration_seconds, the runtime Scryer persists and
//!   uses (import, rules, NFO), rounds it (lib.rs `d.round()`), so Scryer's
//!   stored runtime for this file is 11 s, not 10.
//! - video_format -> a.video_codec; video_profile -> a.video_profile
//! - audio_format -> a.audio_codec; audio_profile -> a.audio_profile
//! - video_codec_id / audio_codec_id (codec tag) -> null: Scryer carries no codec tag
//! - audio_bitrate / video_bitrate -> stream metadata.bitrate_bps (exact bps)
//! - audio_channels, audio_languages, height, width, video_bit_depth,
//!   subtitles -> a.audio_channels, a.audio_languages, a.video_height,
//!   a.video_width, a.video_bit_depth, a.subtitle_languages
//! - audio_channel_positions -> au.metadata.channel_layout
//! - video_fps -> a.video_frame_rate parsed as f64 (expected as f64)
//! - video_colour_primaries / video_transfer_characteristics -> vs.metadata.color
//!   code (smpte170m -> 6, bt709 -> 1)
//! - title (format tag) -> null: Scryer reads no container title
//!
//! 0041/0042 compare one object over all keys. video_codec_id, audio_codec_id
//! and title are constants (null) in `actual_key`, so both cases stay red;
//! update `actual_key` if MediaAnalysis gains a codec tag or a container title.
//! While the cases are red, the other keys (all matching today: profile,
//! bitrates 125509/193694, 480x320, fps, colour codes 6/1, channel layout,
//! languages) have no ratchet protection; a regression in them shows only as
//! a change of the `detail` text in the results JSON.
//!
//! Ignored keys: `scan_type` (constant "Progressive" in the reference,
//! VideoFileInfoReader.cs:108, not read from the file).
//!
//! Unmapped (tests: []), with reasons:
//! - SON-SCAN-0054..0059 (DoviConfigurationRecord rows of GetHdrFormat):
//!   DEFERRED, not concept-absent. Scryer has the behaviour, and under D29
//!   these rows are mappable with seams only, in a scryer-application suite.
//!   scryer-mediainfo reports "Dolby Vision" plus the raw dovi_bl_compat_id;
//!   the base-layer flavour by compatibility id is composed in
//!   scryer-application (library/rename.rs dynamic_range_label ->
//!   dolby_vision_label). Comparing only the reader label here would make the
//!   compatibility-id dimension vacuous, and this crate cannot depend on
//!   scryer-application (cycle). The application suite needs scryer-mediainfo
//!   as a dev-dependency (today an optional dependency behind the
//!   runtime-media-analysis feature), a dvcC byte encoding of the compat id as
//!   documented harness glue, build_analysis (seamed here), and a seam on
//!   dynamic_range_label or build_media_file_analysis. Likely reds there:
//!   0056/0057 (dolby_vision_label takes HDR10+ only from the release name,
//!   parsed.is_hdr10plus, so it gives "DV HDR10").

use std::path::Path;

use scryer_media_types::{ContentLight, MasteringDisplay, StreamKind};
use scryer_mediainfo::parity_seams::{RawContainer, RawTrack, TrackKind, build_analysis};
use scryer_mediainfo::{MediaAnalysis, analyze_catalog_file};
use scryer_parity::Suite;
use serde_json::{Value, json};

const FILE: &str =
    "corpus/reference/sonarr/scan/MediaFiles.MediaInfo.VideoFileInfoReaderFixture.json";

fn cases(range: std::ops::RangeInclusive<u32>) -> Vec<Value> {
    let all: Vec<Value> = scryer_parity::corpus(FILE);
    all.into_iter()
        .filter(|c| {
            let id = c["id"].as_str().expect("id");
            id.strip_prefix("SON-SCAN-")
                .and_then(|n| n.parse::<u32>().ok())
                .is_some_and(|n| range.contains(&n))
        })
        .collect()
}

fn primaries_code(name: &str) -> Option<u32> {
    match name {
        "" => None,
        "bt709" => Some(1),
        "bt2020" => Some(9),
        other => panic!("unmapped colour_primaries {other}"),
    }
}

fn transfer_code(name: &str) -> Option<u32> {
    match name {
        "" => None,
        "bt709" => Some(1),
        "bt2020-10" => Some(14),
        "smpte2084" => Some(16),
        "arib-std-b67" => Some(18),
        other => panic!("unmapped transfer_characteristics {other}"),
    }
}

fn hdr_label(name: &str) -> Option<&'static str> {
    match name {
        "None" => None,
        "Hlg10" => Some("HLG"),
        "Pq10" => Some("PQ10"),
        "Hdr10" => Some("HDR10"),
        "Hdr10Plus" => Some("HDR10+"),
        other => panic!("unmapped hdr_format {other}"),
    }
}

#[test]
fn son_scan_reader_hdr() {
    let Some(mut suite) = Suite::open("scryer-mediainfo::parity_scan::son_scan_reader_hdr") else {
        return;
    };
    for case in cases(43..=53) {
        let id = case["id"].as_str().unwrap();
        let given = &case["input"]["given"];
        assert_eq!(case["input"]["op"], "VideoFileInfoReader.GetHdrFormat", "{id}");
        assert!(given.get("dv_bl_signal_compatibility_id").is_none(), "{id}");
        if !suite.wants(id) {
            continue;
        }
        let mut t = RawTrack {
            kind: TrackKind::Video,
            ..Default::default()
        };
        t.metadata.bit_depth = Some(given["bit_depth"].as_i64().unwrap() as i32);
        t.metadata.color.primaries = primaries_code(given["colour_primaries"].as_str().unwrap());
        t.metadata.color.transfer =
            transfer_code(given["transfer_characteristics"].as_str().unwrap());
        for sd in given["side_data"].as_array().unwrap() {
            match sd.as_str().unwrap() {
                "SideData" => {}
                "MasteringDisplayMetadata" => {
                    t.metadata.color.mastering_display = Some(MasteringDisplay::default())
                }
                "ContentLightLevelMetadata" => {
                    t.metadata.color.content_light = Some(ContentLight::default())
                }
                "HdrDynamicMetadataSpmte2094" => t.has_hdr10plus = true,
                other => panic!("{id}: unmapped side_data {other}"),
            }
        }
        let a = build_analysis(RawContainer {
            format_name: "parity".into(),
            duration_seconds: None,
            num_chapters: None,
            tracks: vec![t],
            details: Default::default(),
        });
        let expected = hdr_label(case["expect"]["hdr_format"].as_str().unwrap()).map(String::from);
        suite.check(id, expected, a.video_hdr_format);
    }
    suite.finish();
}

fn color_code(name: &str) -> u32 {
    match name {
        "smpte170m" => 6,
        "bt709" => 1,
        other => panic!("unmapped colour name {other}"),
    }
}

fn first<'a>(a: &'a MediaAnalysis, kind: StreamKind) -> Option<&'a scryer_media_types::StreamDetail> {
    let streams = &a.details.streams;
    if kind == StreamKind::Video {
        if let Some(s) = streams.iter().find(|s| {
            s.kind == kind && s.metadata.id.is_some() && s.metadata.id == a.details.selected_video_id
        }) {
            return Some(s);
        }
    }
    streams.iter().find(|s| s.kind == kind)
}

/// Scryer's value for one reference expect key.
fn actual_key(key: &str, a: &MediaAnalysis) -> Value {
    let vs = first(a, StreamKind::Video);
    let au = first(a, StreamKind::Audio);
    match key {
        "run_time_seconds" => json!(a.details.duration_seconds.map(|d| (d.floor() as i64) % 60)),
        "video_format" => json!(a.video_codec),
        "video_profile" => json!(a.video_profile),
        "audio_format" => json!(a.audio_codec),
        "audio_profile" => json!(a.audio_profile),
        "video_codec_id" | "audio_codec_id" | "title" => Value::Null,
        "audio_bitrate" => json!(au.and_then(|s| s.metadata.bitrate_bps)),
        "video_bitrate" => json!(vs.and_then(|s| s.metadata.bitrate_bps)),
        "audio_channels" => json!(a.audio_channels),
        "audio_channel_positions" => json!(au.and_then(|s| s.metadata.channel_layout.clone())),
        "audio_languages" => json!(a.audio_languages),
        "height" => json!(a.video_height),
        "width" => json!(a.video_width),
        "subtitles" => json!(a.subtitle_languages),
        "video_fps" => json!(
            a.video_frame_rate
                .as_deref()
                .and_then(|f| f.parse::<f64>().ok())
        ),
        "video_bit_depth" => json!(a.video_bit_depth),
        "video_colour_primaries" => json!(vs.and_then(|s| s.metadata.color.primaries)),
        "video_transfer_characteristics" => json!(vs.and_then(|s| s.metadata.color.transfer)),
        other => panic!("unmapped expect key {other}"),
    }
}

fn expected_key(key: &str, v: &Value) -> Value {
    match key {
        "video_colour_primaries" | "video_transfer_characteristics" => {
            json!(color_code(v.as_str().unwrap()))
        }
        "video_fps" => json!(v.as_f64().unwrap()),
        _ => v.clone(),
    }
}

#[test]
fn son_scan_reader_file() {
    let Some(mut suite) = Suite::open("scryer-mediainfo::parity_scan::son_scan_reader_file") else {
        return;
    };
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity_scan/media/H264_sample.mp4");
    assert_eq!(std::fs::metadata(&sample).unwrap().len(), 429_076);
    let tmp = tempfile::tempdir().expect("tempdir");
    for case in cases(40..=42) {
        let id = case["id"].as_str().unwrap();
        let given = &case["input"]["given"];
        let op = case["input"]["op"].as_str().unwrap();
        assert!(
            op == "VideoFileInfoReader.GetRunTime" || op == "VideoFileInfoReader.GetMediaInfo",
            "{id}"
        );
        assert_eq!(given["file"], "src/NzbDrone.Core.Test/Files/Media/H264_sample.mp4", "{id}");
        if !suite.wants(id) {
            continue;
        }
        let path = if given.get("copied_to").is_some() {
            let p = tmp.path().join("H264_Pok\u{e9}mon.mkv");
            std::fs::copy(&sample, &p).expect("copy");
            p
        } else {
            sample.clone()
        };
        let mut expected = serde_json::Map::new();
        let mut actual = serde_json::Map::new();
        match analyze_catalog_file(&path) {
            Ok(a) => {
                for (k, v) in case["expect"].as_object().unwrap() {
                    if k == "scan_type" {
                        continue;
                    }
                    expected.insert(k.clone(), expected_key(k, v));
                    actual.insert(k.clone(), actual_key(k, &a));
                }
            }
            Err(e) => {
                expected = case["expect"].as_object().unwrap().clone();
                actual.insert("error".into(), json!(e.to_string()));
            }
        }
        suite.check(id, Value::Object(expected), Value::Object(actual));
    }
    suite.finish();
}
