//! fork (P24): disc images (arr RawDiskSpecification + Radarr BRDISKRegex)
//! and HD DVD as the Bluray family.

use scryer_release_parser::{
    ContextFacetHint, ContextTitle, ParsedReleaseMetadata, ReleaseParseContext, ReleaseSource,
    best_parse_for_target,
};

fn parse(raw: &str) -> ParsedReleaseMetadata {
    let target = ReleaseParseContext {
        facet_hint: ContextFacetHint::Movie,
        title: ContextTitle {
            name: "Movie Title".into(),
        },
        aliases: vec![],
        known_years: vec![],
        imdb_ids: vec![],
        episodes: vec![],
    };
    best_parse_for_target(raw, &target)
}

#[test]
fn disc_image_names_are_flagged() {
    for (raw, source) in [
        ("Movie.Title.1996.Bluray.ISO", None),
        ("Movie.Title.1997.1080p.NL.BD-50", None),
        ("Movie Title 2009 3D BD 2009 UNTOUCHED", None),
        (
            "BD25.Movie.Title.1994.1080p.DTS-HD",
            Some(ReleaseSource::BrDisk),
        ),
        (
            "Movie.Title.1982.1080p.HD.DVD.VC-1.DD+.5.1",
            Some(ReleaseSource::BluRay),
        ),
        (
            "Movie Title 1976 2160p UHD Blu-ray DTS-HD MA 5.1 DV HDR HEVC-UNTOUCHED",
            None,
        ),
        (
            "Some.Movie.2005.PAL.MDVDR-SOMegRoUP",
            Some(ReleaseSource::Dvd),
        ),
        (
            "Some.Movie.Magic.Rainbow.2007.DVD-R.NTSC",
            Some(ReleaseSource::Dvd),
        ),
        ("Movie.Title.2010.3xDVD9.PAL-GRP", Some(ReleaseSource::Dvd)),
        ("Movie.Title.2010.DVD5.PAL-GRP", Some(ReleaseSource::Dvd)),
        (
            "Movie.2024.2160p.UHD.BluRay.HEVC.TrueHD.7.1.Atmos-GRP",
            None,
        ),
        ("The German 2021 Bluray AVC", None),
        ("Movie.Title.2001.Disc.1.1080p.BluRay-GRP", None),
        ("Movie.Title.2001.FULL.BLURAY-GRP", None),
        ("Movie.Title.2001.COMPLETE.UHD.BLURAY-GRP", None),
    ] {
        let parsed = parse(raw);
        assert!(parsed.is_bd_disk, "{raw}: not flagged");
        if let Some(source) = source {
            assert_eq!(parsed.source, Some(source), "{raw}");
        }
    }
}

#[test]
fn encodes_and_titles_are_not_discs() {
    for raw in [
        "Discovery.2019.1080p.BluRay.x264",
        "Movie.2010.DVDRip.XviD",
        "Movie.2010.DVD.x264",
        "Movie.2019.2160p.BluRay.x265",
        "Movie.2019.1080p.BluRay.HEVC",
        "Movie.2019.1080p.BluRay.REMUX.AVC",
        "Movie.2019.720p.BluRay.AVC",
        "Completely.Different.Title.2024.1080p.BluRay",
        "Iso.2019.1080p.WEB-DL.x264",
        "Movie.2019.German.DL.1080p.BluRay.AVC",
        "[Groupi] Titles (3D) [BD 1080p AAC]",
        "Movie.2019.DVDRemux",
        "Movie.2019.DVDRip.x264-PLAiD",
        "Movie.2019.DVD 5.x264",
    ] {
        assert!(!parse(raw).is_bd_disk, "{raw}: wrongly flagged");
    }
}

#[test]
fn hd_dvd_is_the_bluray_family() {
    for raw in [
        "Movie.Name.S01E01.33.720p.HDDVD.x264-SiNNERS.mkv",
        "Movie.Title.2005.1080p.HDDVDRip.x264",
    ] {
        let parsed = parse(raw);
        assert_eq!(parsed.source, Some(ReleaseSource::BluRay), "{raw}");
        assert!(!parsed.is_bd_disk, "{raw}");
    }
}
