//! Replays the minimized `scan_path` fuzz crashes in
//! `parity/fuzz/artifacts/scan_path/` through the library scan's filename
//! parser (ledger P12, P14). The allocator refuses any single allocation over
//! 64 MiB, so an unbounded episode range aborts the test instead of eating
//! the machine's memory.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;

use scryer_application::parity_seams::{
    LibraryFilenameParse, LibraryFilenameParseInput, LibraryFilenameParseMode,
    parse_library_filename,
};

struct Capped;

const MAX_ALLOCATION: usize = 64 << 20;

// SAFETY: forwards to the system allocator; refusing returns null, which the
// standard library turns into an allocation-failure abort.
unsafe impl GlobalAlloc for Capped {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > MAX_ALLOCATION {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Capped = Capped;

const ROOT: &str = "/library/tv";

/// The fuzz target's input is `(title_scan, rooted, relative)` as decoded by
/// `arbitrary`: one byte per bool (low bit), the rest is the path.
fn replay(data: &[u8]) -> LibraryFilenameParse {
    let (title_scan, rooted) = (data[0] & 1 == 1, data[1] & 1 == 1);
    let relative = std::str::from_utf8(&data[2..]).expect("utf-8 path");
    let full = format!("{ROOT}/{relative}");
    let path = Path::new(if rooted { full.as_str() } else { relative });
    let mut input = LibraryFilenameParseInput::title_only(path, rooted.then(|| Path::new(ROOT)));
    if title_scan {
        input.mode = LibraryFilenameParseMode::TitleScan;
    }
    parse_library_filename(&input)
}

fn artifact(name: &str) -> Vec<u8> {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fuzz/artifacts/scan_path"
    );
    std::fs::read(format!("{dir}/{name}")).expect("fuzz artifact")
}

#[test]
fn scan_path_wide_range_does_not_allocate_the_whole_range() {
    replay(&artifact("oom-range-1-4000000000"));
}

/// A season of five digits is not a season (ledger P14).
#[test]
fn scan_path_five_digit_season_is_not_read() {
    let parse = replay(&artifact("crash-season-22332x3"));
    for episode in [
        parse.parsed_release.episode.as_ref(),
        parse.episode_identity.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(
            episode.season.is_none_or(|season| season < 10_000),
            "{episode:?}"
        );
        assert!(
            episode.season_numbers.iter().all(|season| *season < 10_000),
            "{episode:?}"
        );
    }
}
