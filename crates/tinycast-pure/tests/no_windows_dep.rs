#[test]
fn pure_crate_toml_does_not_depend_on_windows() {
    let toml = include_str!("../Cargo.toml");
    assert!(
        !toml.lines().any(|l| l.contains("windows")),
        "tinycast-pure must not depend on the windows crate"
    );
}
