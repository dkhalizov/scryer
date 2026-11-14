//! The catalogue parses and follows the schema (design § Catalogue entry).

#[test]
fn catalogue_is_valid() {
    let errors = scryer_parity::Catalogue::load().validate();
    assert!(errors.is_empty(), "catalogue invalid:\n{}", errors.join("\n"));
}
