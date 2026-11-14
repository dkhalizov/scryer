//! Stream scan_mediainfo: Sonarr's `MediaInfoFormatter` fixtures against
//! Scryer's rename tokens (78 IDs, all mapped).
//!
//! Suites (each reads exactly one corpus file, so adopt mode cannot pick up
//! other streams' IDs):
//! - `son_scan_format_audio_codec`: FormatAudioCodecFixture, SON-SCAN-0060..0084
//! - `son_scan_format_video_codec`: FormatVideoCodecFixture, SON-SCAN-0085..0121
//! - `son_scan_format_dynamic_range`: FormatVideoDynamicRangeFixture, SON-SCAN-0122..0127
//! - `son_scan_format_dynamic_range_type`: FormatVideoDynamicRangeTypeFixture, SON-SCAN-0128..0137
//!
//! Why scryer-application and not scryer-mediainfo: scryer-mediainfo's
//! `normalize_codec_name` maps container codec ids to ffprobe-style names, and
//! `detect_hdr_format` is a `#[cfg(test)]` wrapper returning the probe's
//! `video_hdr_format` string; neither is a display label. The display labels are Scryer's rename tokens, built from a
//! `MediaFileAnalysis` plus the parsed release name, which is the same
//! `(mediaInfo, sceneName)` shape as the reference formatter. A dev-dependency
//! from scryer-mediainfo back to scryer-application would be a cycle.
//!
//! Scryer functions under test: `title_rename_tokens`,
//! `RenameMediaEvidence::from_analysis`, `resolve_release_labels_from_analysis`,
//! `scryer_media_types::normalize_audio_codec_for_release`,
//! `normalize_video_codec_for_release`, `dynamic_range_label` /
//! `dolby_vision_label`, `VideoCodec::parse`, `parse_release_metadata`.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | `media_info.audio_format` | `MediaFileAnalysis.audio_codec` and `audio_streams[0].codec` (ffprobe codec_name vocabulary) |
//! | `media_info.audio_profile` | `audio_profile` and `audio_streams[0].profile`, verbatim (missing -> None) |
//! | `media_info.video_format` | `MediaFileAnalysis.video_codec = VideoCodec::parse(video_format)`: the conversion build_media_file_analysis does (import/post_download_gate.rs:730-733), which is feature-gated and takes a scryer_mediainfo::MediaAnalysis, so it is repeated here through the public parser |
//! | `media_info.video_hdr_format` | `video_hdr_format` / `dovi_bl_compat_id` via `hdr_input` (below) |
//! | `scene_name` (null/missing -> "") | `parse_release_metadata(scene_name)` as parsed_current (untargeted path, D27) |
//! | FormatAudioCodec result | token `{audio_codec}` |
//! | FormatVideoCodec result | token `{video_codec}` |
//! | FormatVideoDynamicRange / ...Type result | token `{dynamic_range}` (Scryer has one dynamic-range token; same row as parity_naming.rs) |
//!
//! `hdr_input` gives the strings scryer-mediainfo records for the stream
//! Sonarr's GetHdrFormat (VideoFileInfoReader.cs:184) would classify that way.
//! The literals come from the `video_hdr_format` chain in `build_analysis`
//! (crates/scryer-mediainfo/src/lib.rs, Dolby Vision checked first, then
//! HDR10+, HDR10, HLG) and the HDR flag enrichment (`hdr.hdr10` match in the
//! same file). None -> nothing; Hlg10 -> "HLG" (lowercased, see below); Pq10 and Hdr10 -> "HDR10" (the
//! hdr10 flag is set for PQ + 10-bit + bt2020 without mastering metadata);
//! Hdr10Plus -> "HDR10+"; DolbyVision -> "Dolby Vision" compat 0;
//! DolbyVisionHdr10 and DolbyVisionHdr10Plus -> compat 1; DolbyVisionHlg ->
//! compat 4; DolbyVisionSdr -> compat 2. For DolbyVisionHdr10Plus the probe
//! keeps `hdr10plus` in `details.streams[].metadata.hdr`, but
//! `video_hdr_format` holds only "Dolby Vision"; the rename tokens read
//! `video_hdr_format` and the dovi ids, never `details`, so without a release
//! name saying HDR10+ the token is "DV HDR10" (SON-SCAN-0135 red).
//!
//! Greens that a reviewer could misread (all production output, none derived
//! from `expect`):
//! - SON-SCAN-0085 (MPEG2) is green through the release-name fallback:
//!   `VideoCodec::parse("mpeg2video")` is None, so "MPEG2" comes from
//!   `parse_release_metadata` of the scene `...MPEG2-NTb`. The reference gets
//!   it from the format alone; the probe-only twin SON-SCAN-0086 is red.
//! - SON-SCAN-0111 (rpza -> "") is green because Scryer leaves every unknown
//!   codec blank (twin SON-SCAN-0121 is red for the same reason); the
//!   reference blanks rpza through an explicit list.
//! - HLG / HDR10 greens: `dynamic_range_label` returns an unknown string
//!   unchanged (`format.trim()`), so feeding the exact probe literal would
//!   echo. `hdr_input` therefore passes them lowercased ("hlg", "hdr10",
//!   "hdr10+"); only the classifier branches turn those into "HLG" / "HDR10" /
//!   "HDR10+". The exact probe literals are covered on the producer side by
//!   scryer-mediainfo `son_scan_reader_hdr`. DV greens go through
//!   `dolby_vision_label` by compat id.
//!
//! Ignored keys: `media_info.audio_codec_id` (Scryer's analysis has no codec
//! tag field and reads Atmos from the profile; translating the tag into an
//! Atmos profile would inject the answer), `media_info.video_codec_id` (the
//! probe folds the FourCC into the codec name, so XviD/DivX/x264 tags cannot
//! be represented), `media_info.schema_revision` (Sonarr's stored-model
//! revision), `note`.
//!
//! Reds follow D29 (Scryer has the token, not the reference's value): e.g.
//! H.264 vs h264/x264, no WMA/MP2/VC1/VP6-8/WMV labels, DTS:X vs DTS-X, no
//! plain "HDR" token, HDR10+ vs HDR10Plus spelling.
//!
//! Unmapped (tests: []): none.

use std::path::Path;

use scryer_application::parity_seams::{RenameMediaEvidence, title_rename_tokens};
use scryer_application::{
    AudioStreamDetail, MediaFileAnalysis, VideoCodec, parse_release_metadata,
};
use scryer_domain::{MediaFacet, Title};
use scryer_parity::Suite;
use serde_json::{Value, json};

fn title() -> Title {
    Title {
        id: "title-1".into(),
        library_id: scryer_domain::default_library_id_for_facet(&MediaFacet::Series),
        name: "Series".into(),
        facet: MediaFacet::Series,
        monitored: true,
        tags: vec![],
        external_ids: vec![],
        root_folder_id: scryer_domain::root_folder_id_for_path("/library"),
        created_by: None,
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).expect("epoch"),
        year: None,
        overview: None,
        poster_url: None,
        poster_source_url: None,
        background_url: None,
        background_source_url: None,
        sort_title: None,
        catalog_sort_key: String::new(),
        slug: None,
        imdb_id: None,
        runtime_minutes: None,
        popularity: None,
        canonical_tags: vec![],
        content_status: None,
        language: None,
        first_aired: None,
        network: None,
        studio: None,
        country: None,
        aliases: vec![],
        tagged_aliases: vec![],
        metadata_language: None,
        metadata_fetched_at: None,
        min_availability: None,
        digital_release_date: None,
        movie_release_dates: None,
        folder_path: None,
    }
}

/// The named token of Scryer's rename tokens for one analysis and scene name.
fn token(analysis: &MediaFileAnalysis, input: &Value, name: &str) -> Value {
    let scene = input["given"]["scene_name"].as_str().unwrap_or("");
    let parsed = parse_release_metadata(scene);
    let (tokens, _) = title_rename_tokens(
        &title(),
        Some(RenameMediaEvidence::from_analysis(analysis)),
        &parsed,
        "mkv",
    );
    json!(tokens.get(name).cloned().expect("token present"))
}

fn mi<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input["given"]["media_info"][key].as_str()
}

fn audio_case(_: &Path, input: &Value, expect: &Value) -> Option<(Value, Value)> {
    let codec = mi(input, "audio_format").map(String::from);
    let profile = mi(input, "audio_profile").map(String::from);
    let analysis = MediaFileAnalysis {
        audio_codec: codec.clone(),
        audio_profile: profile.clone(),
        audio_streams: vec![AudioStreamDetail {
            codec,
            profile,
            channels: None,
            language: None,
            name: None,
            bitrate_kbps: None,
        }],
        ..Default::default()
    };
    Some((expect["result"].clone(), token(&analysis, input, "audio_codec")))
}

fn video_case(_: &Path, input: &Value, expect: &Value) -> Option<(Value, Value)> {
    let analysis = MediaFileAnalysis {
        video_codec: mi(input, "video_format").and_then(VideoCodec::parse),
        ..Default::default()
    };
    Some((expect["result"].clone(), token(&analysis, input, "video_codec")))
}

fn hdr_input(name: &str) -> (Option<&'static str>, Option<u8>) {
    match name {
        "None" => (None, None),
        // Lowercased probe literals (fork: anti-echo). dynamic_range_label
        // uppercases before matching, but its last branch returns an unknown
        // string unchanged, so the exact probe spelling would pass with the
        // HLG/HDR10/HDR10+ branches deleted. The lowercase spelling only
        // matches the canonical token if those branches run. The exact probe
        // literals are asserted by scryer-mediainfo son_scan_reader_hdr.
        "Hlg10" => (Some("hlg"), None),
        "Pq10" | "Hdr10" => (Some("hdr10"), None),
        "Hdr10Plus" => (Some("hdr10+"), None),
        "DolbyVision" => (Some("Dolby Vision"), Some(0)),
        "DolbyVisionHdr10" | "DolbyVisionHdr10Plus" => (Some("Dolby Vision"), Some(1)),
        "DolbyVisionHlg" => (Some("Dolby Vision"), Some(4)),
        "DolbyVisionSdr" => (Some("Dolby Vision"), Some(2)),
        other => panic!("unknown HdrFormat {other}"),
    }
}

fn hdr_case(_: &Path, input: &Value, expect: &Value) -> Option<(Value, Value)> {
    let (hdr, compat) = hdr_input(mi(input, "video_hdr_format").expect("video_hdr_format"));
    let analysis = MediaFileAnalysis {
        video_hdr_format: hdr.map(String::from),
        dovi_bl_compat_id: compat.map(Into::into),
        ..Default::default()
    };
    Some((expect["result"].clone(), token(&analysis, input, "dynamic_range")))
}

const DIR: &str = "corpus/reference/sonarr/scan/MediaFiles.MediaInfo.MediaInfoFormatterTests.";

fn suite(name: &str, file: &str, handler: super::Handler) {
    let Some(mut suite) = Suite::open(&format!("scryer-application::parity_scan::{name}")) else {
        return;
    };
    super::run(&mut suite, &format!("{DIR}{file}Fixture.json"), handler);
    suite.finish();
}

#[test]
fn son_scan_format_audio_codec() {
    suite("son_scan_format_audio_codec", "FormatAudioCodec", audio_case);
}

#[test]
fn son_scan_format_video_codec() {
    suite("son_scan_format_video_codec", "FormatVideoCodec", video_case);
}

#[test]
fn son_scan_format_dynamic_range() {
    suite("son_scan_format_dynamic_range", "FormatVideoDynamicRange", hdr_case);
}

#[test]
fn son_scan_format_dynamic_range_type() {
    suite("son_scan_format_dynamic_range_type", "FormatVideoDynamicRangeType", hdr_case);
}
