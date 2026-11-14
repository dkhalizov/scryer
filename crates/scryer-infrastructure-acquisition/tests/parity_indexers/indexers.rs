//! Parity net, indexers stream (Prowlarr IndexerCapabilitiesCategoriesFixture).
//!
//! Suite: `scryer-infrastructure-acquisition::parity_indexers::prw_indexers_caps_categories`
//! IDs: PRW-INDEXERS-0001..0003 (op `capabilities.supported_categories`).
//!
//! Scryer code under test: `merge_source` (scryer-application
//! indexer_clients/caps.rs:125, via parity_seams), which filters an indexer's
//! declared category model by the client's per-indexer grant categories using
//! the `category_contains` rule (catalog/interactive_release_search/
//! scope_filter.rs:19: a thousand-wide parent contains its subcategories).
//!
//! Translation table (reference -> Scryer):
//!
//! | Prowlarr | Scryer |
//! |---|---|
//! | indexer definition `AddCategoryMapping(tracker, Newznab.X, desc)` | one entry of the declared category model: value = `newznab_id` decimal string, Numeric, label = `description` |
//! | `SupportedCategories(categories)` / app `SyncCategories` | the grant categories (`allowed` of `merge_source`) |
//! | supported categories returned | the keys `merge_source` keeps in the `categories` map |
//! | `supported.Should().HaveCount(n)` | `categories.len() == expect.count` |
//!
//! Ignored input keys: `mappings[].tracker_category` (Scryer has no tracker
//! category id) and `mappings[].newznab_category` (enum name, redundant with
//! `newznab_id`).
//!
//! Known difference, harmless for the count: Prowlarr's tree adds the implicit
//! parent 2000 (0001 returns [2000]); Scryer keeps the indexer's own 2030 under
//! the 2000 grant. Only the count is compared, as the fixture does. The two
//! sides count from opposite directions: Prowlarr counts the requested ids
//! found in its tree (IndexerCapabilitiesCategories.cs:124), Scryer counts the
//! indexer categories the grant contains. The counts match only for these
//! single-mapping fixtures; with several children under a granted parent, or
//! a request naming both parent and child, they would differ.
//!
//! Unmapped (tests: []), with reasons:
//! - PRW-INDEXERS-0004..0006 (GetTrackerCategories, MapTrackerCatToNewznab,
//!   MapTrackerCatDescToNewznab): Prowlarr keeps a tracker-category table
//!   (tracker id + description -> Newznab id) and synthesises custom 100000+id
//!   categories. Scryer's category model (IndexerCategoryDescriptor {value,
//!   label, value_kind, facets}) is a flat list of what an indexer exposes,
//!   with no tracker-to-Newznab mapping and no custom-category synthesis;
//!   plugins map provider codes inside the plugin. Scryer can carry
//!   provider-native string codes (value_kind String), but nothing relates
//!   them or their descriptions to Newznab ids, which 0005 and 0006 need.
//!   0004 mapped onto the declared model would echo its input back, so it
//!   would not test anything.

use std::collections::BTreeMap;

use scryer_application::parity_seams::merge_source;
use scryer_domain::{
    IndexerCapsSnapshot, IndexerCategoryDescriptor, IndexerCategoryModel, IndexerCategoryValueKind,
    IndexerProviderCapabilities,
};
use scryer_parity::Suite;
use serde_json::Value;

#[test]
fn prw_indexers_caps_categories() {
    let Some(mut suite) = Suite::open(
        "scryer-infrastructure-acquisition::parity_indexers::prw_indexers_caps_categories",
    ) else {
        return;
    };
    let cases: Vec<Value> = scryer_parity::corpus(
        "corpus/reference/prowlarr/indexers/IndexerCapabilitiesCategoriesFixture.json",
    );
    for case in cases {
        if case["input"]["op"] != "capabilities.supported_categories" {
            continue;
        }
        let id = case["id"].as_str().expect("case id");
        if !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let categories = super::list(&given["mappings"])
            .iter()
            .map(|m| IndexerCategoryDescriptor {
                value: m["newznab_id"].as_i64().expect("newznab_id").to_string(),
                label: m["description"].as_str().map(str::to_string),
                value_kind: IndexerCategoryValueKind::Numeric,
                facets: vec![],
            })
            .collect();
        let declared = IndexerProviderCapabilities {
            category_model: Some(IndexerCategoryModel {
                value_kinds: vec![IndexerCategoryValueKind::Numeric],
                categories,
                ..Default::default()
            }),
            ..Default::default()
        };
        let allowed: Vec<String> = super::ints(Some(&given["categories"]))
            .iter()
            .map(|c| c.to_string())
            .collect();
        let mut kept = BTreeMap::<u32, IndexerCategoryDescriptor>::new();
        merge_source(
            &mut IndexerCapsSnapshot::default(),
            &mut kept,
            &declared,
            None,
            Some(&allowed),
        );
        let expected = case["expect"]["count"].as_u64().expect("count") as usize;
        suite.check(id, expected, kept.len());
    }
    suite.finish();
}
