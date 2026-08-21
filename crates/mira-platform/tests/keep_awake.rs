//! Keep Awake, against a double rather than against the machine.
//!
//! **No test here changes a real power setting.** [`Counting`] stands in for the
//! operating system and records what it was asked to do, which is what makes the
//! lifecycle rules — one request at a time, a deadline that ends it, a release on
//! shutdown, a refusal where the platform cannot — assertable on any machine and
//! without waiting for half an hour to pass.
//!
//! The clock is a parameter for the same reason. Expiry is arithmetic on a
//! timestamp, so "thirty minutes later" is a number in a test rather than a sleep.

use std::sync::atomic::{AtomicUsize, Ordering};

use mira_core::{Capability, CapabilityReport, CapabilityStatus, MiraError};
use mira_platform::{
    DisplayServer, EnvFacts, Inhibit, KeepAwake, KeepAwakeHost, KeepAwakeSpan, KeepAwakeState,
    LinuxPackaging, Os, Platform, PlatformCapabilities,
};

/// A stand-in for the operating system's power interface.
///
/// Counts rather than acts. `engaged` is the property that matters most: a second
/// request taken out while the first is held would be the duplicate inhibitor this
/// double exists to catch.
#[derive(Debug, Default)]
struct Counting {
    engaged: AtomicUsize,
    engagements: AtomicUsize,
    releases: AtomicUsize,
    refuse: bool,
}

impl Counting {
    fn refusing() -> Self {
        Self {
            refuse: true,
            ..Self::default()
        }
    }

    fn engagements(&self) -> usize {
        self.engagements.load(Ordering::Relaxed)
    }

    fn releases(&self) -> usize {
        self.releases.load(Ordering::Relaxed)
    }
}

impl Inhibit for Counting {
    fn engage(&self) -> mira_core::Result<()> {
        if self.refuse {
            return Err(MiraError::External {
                source: "the operating system".to_owned(),
                detail: "The power request was refused.".to_owned(),
            });
        }
        assert_eq!(
            self.engaged.load(Ordering::Relaxed),
            0,
            "a second request was taken out while the first was still held"
        );
        self.engaged.store(1, Ordering::Relaxed);
        self.engagements.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn release(&self) {
        if self.engaged.swap(0, Ordering::Relaxed) == 1 {
            self.releases.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn engaged(&self) -> bool {
        self.engaged.load(Ordering::Relaxed) == 1
    }
}

/// A platform whose every capability resolves the way this test wants.
#[derive(Debug, Clone)]
struct Fixed(CapabilityStatus);

impl PlatformCapabilities for Fixed {
    fn status(&self, _capability: Capability) -> CapabilityStatus {
        self.0.clone()
    }

    fn report(&self) -> Vec<CapabilityReport> {
        vec![CapabilityReport::new(Capability::KeepAwake, self.0.clone())]
    }
}

fn able() -> Fixed {
    Fixed(CapabilityStatus::Full)
}

fn unable() -> Fixed {
    Fixed(CapabilityStatus::Unavailable {
        reason: "Mira does not yet hold a systemd-logind sleep inhibitor".to_owned(),
        fallback: Some("your desktop's power settings".to_owned()),
    })
}

/// A fixed moment. Any epoch would do.
const NOW: i64 = 1_800_000_000;

fn facts(os: Os) -> EnvFacts {
    EnvFacts {
        os,
        display_server: match os {
            Os::MacOs => DisplayServer::Quartz,
            Os::Windows => DisplayServer::Dwm,
            Os::Linux => DisplayServer::Wayland,
        },
        has_logind: false,
        packaging: LinuxPackaging::NotApplicable,
    }
}

// ── Off by default ───────────────────────────────────────────────────────────

#[test]
fn nothing_is_held_until_somebody_asks() {
    let keep = KeepAwake::with(able(), Counting::default());

    assert_eq!(keep.state(), KeepAwakeState::Off);
}

// ── Enabling ─────────────────────────────────────────────────────────────────

#[test]
fn turning_it_on_takes_out_exactly_one_request() {
    let keep = KeepAwake::with(able(), Counting::default());

    let state = keep
        .set(KeepAwakeSpan::UntilTurnedOff, NOW)
        .expect("the capability is available");

    assert_eq!(
        state,
        KeepAwakeState::On {
            span: KeepAwakeSpan::UntilTurnedOff,
            until: None
        }
    );
    assert_eq!(keep.state(), state, "and reading it back says the same");
}

#[test]
fn a_span_with_an_end_reports_when_it_ends() {
    let keep = KeepAwake::with(able(), Counting::default());

    assert_eq!(
        keep.set(KeepAwakeSpan::ThirtyMinutes, NOW).expect("set"),
        KeepAwakeState::On {
            span: KeepAwakeSpan::ThirtyMinutes,
            until: Some(NOW + 30 * 60)
        }
    );
    assert_eq!(
        keep.set(KeepAwakeSpan::OneHour, NOW).expect("set"),
        KeepAwakeState::On {
            span: KeepAwakeSpan::OneHour,
            until: Some(NOW + 60 * 60)
        }
    );
}

#[test]
fn asking_twice_never_stacks_a_second_request() {
    // The duplicate-inhibitor test. `Counting::engage` asserts it too, from the
    // other side: this is what a second `caffeinate` process would look like, and
    // there must never be one.
    let counting = Counting::default();
    let keep = KeepAwake::with(able(), &counting);

    for _ in 0..5 {
        keep.set(KeepAwakeSpan::UntilTurnedOff, NOW).expect("set");
    }
    keep.set(KeepAwakeSpan::ThirtyMinutes, NOW).expect("set");
    keep.set(KeepAwakeSpan::OneHour, NOW).expect("set");

    assert_eq!(
        keep.state(),
        KeepAwakeState::On {
            span: KeepAwakeSpan::OneHour,
            until: Some(NOW + 60 * 60)
        },
        "changing the span moves the deadline and keeps the one request"
    );
    assert_eq!(
        counting.engagements(),
        1,
        "seven requests to stay awake, one request to the operating system"
    );
    assert_eq!(counting.releases(), 0, "and nothing released in between");
}

#[test]
fn a_platform_that_refuses_leaves_nothing_held() {
    let refusing = Counting::refusing();
    let keep = KeepAwake::with(able(), &refusing);

    let refused = keep.set(KeepAwakeSpan::OneHour, NOW);

    assert!(matches!(refused, Err(MiraError::External { .. })));
    assert_eq!(
        keep.state(),
        KeepAwakeState::Off,
        "a refusal must not leave the interface saying it is on"
    );
    assert_eq!(refusing.engagements(), 0);
}

// ── Disabling ────────────────────────────────────────────────────────────────

#[test]
fn turning_it_off_releases_the_request() {
    let counting = Counting::default();
    let keep = KeepAwake::with(able(), &counting);
    keep.set(KeepAwakeSpan::UntilTurnedOff, NOW).expect("set");

    assert_eq!(
        keep.set(KeepAwakeSpan::Off, NOW).expect("off"),
        KeepAwakeState::Off
    );
    assert_eq!(keep.state(), KeepAwakeState::Off);
    assert_eq!(counting.engagements(), 1);
    assert_eq!(counting.releases(), 1, "what was taken out was given back");
}

#[test]
fn turning_it_off_twice_is_not_an_error() {
    let keep = KeepAwake::with(able(), Counting::default());

    keep.set(KeepAwakeSpan::Off, NOW).expect("off");
    keep.set(KeepAwakeSpan::Off, NOW).expect("off again");

    assert_eq!(keep.state(), KeepAwakeState::Off);
}

// ── The timeout ──────────────────────────────────────────────────────────────

#[test]
fn a_span_ends_by_itself_when_its_time_is_up() {
    let keep = KeepAwake::with(able(), Counting::default());
    keep.set(KeepAwakeSpan::ThirtyMinutes, NOW).expect("set");

    assert!(
        matches!(keep.expire(NOW + 29 * 60), KeepAwakeState::On { .. }),
        "still on a minute before it is due"
    );
    assert_eq!(
        keep.expire(NOW + 30 * 60),
        KeepAwakeState::Off,
        "and off the moment it is"
    );
}

#[test]
fn expiry_releases_the_operating_system_request_and_not_just_the_record() {
    // The failure this guards against is the worst one available: the interface
    // says Off while the machine is still pinned awake.
    let counting = Counting::default();
    let keep = KeepAwake::with(able(), &counting);
    keep.set(KeepAwakeSpan::OneHour, NOW).expect("set");

    let after = keep.expire(NOW + 60 * 60 + 1);

    assert_eq!(after, KeepAwakeState::Off);
    assert_eq!(keep.state(), KeepAwakeState::Off);
    assert_eq!(
        counting.releases(),
        1,
        "the operating system was told, not just the record"
    );
}

#[test]
fn a_span_that_does_not_end_never_expires() {
    let keep = KeepAwake::with(able(), Counting::default());
    keep.set(KeepAwakeSpan::UntilTurnedOff, NOW).expect("set");

    assert!(matches!(
        keep.expire(NOW + 400 * 24 * 60 * 60),
        KeepAwakeState::On { until: None, .. }
    ));
}

#[test]
fn expiring_when_nothing_is_held_does_nothing() {
    let keep = KeepAwake::with(able(), Counting::default());

    assert_eq!(keep.expire(NOW), KeepAwakeState::Off);
}

// ── Shutdown ─────────────────────────────────────────────────────────────────

#[test]
fn quitting_releases_whatever_was_held() {
    let counting = Counting::default();
    let keep = KeepAwake::with(able(), &counting);
    keep.set(KeepAwakeSpan::UntilTurnedOff, NOW).expect("set");

    keep.release();

    assert_eq!(keep.state(), KeepAwakeState::Off);
    assert_eq!(counting.releases(), 1);
}

#[test]
fn quitting_with_nothing_held_is_harmless() {
    let keep = KeepAwake::with(able(), Counting::default());

    keep.release();
    keep.release();

    assert_eq!(keep.state(), KeepAwakeState::Off);
}

#[test]
fn nothing_survives_the_process_that_held_it() {
    // A new `KeepAwake` is a new process, and it starts Off. The lock lives in
    // memory and in an operating-system request owned by this process; there is
    // no row, no file, and no way for it to come back on restart
    // (`data-model.md` §1 rule 2, and ADR-0014).
    let first = KeepAwake::with(able(), Counting::default());
    first.set(KeepAwakeSpan::UntilTurnedOff, NOW).expect("set");
    drop(first);

    let restarted = KeepAwake::with(able(), Counting::default());
    assert_eq!(restarted.state(), KeepAwakeState::Off);
}

// ── Where the platform cannot ────────────────────────────────────────────────

#[test]
fn an_unavailable_platform_reports_the_reason_rather_than_off() {
    let keep = KeepAwake::with(unable(), Counting::default());

    match keep.state() {
        KeepAwakeState::Unavailable { reason, fallback } => {
            assert!(reason.contains("logind"));
            assert_eq!(fallback.as_deref(), Some("your desktop's power settings"));
        }
        other => panic!("expected an unavailable state, got {other:?}"),
    }
}

#[test]
fn an_unavailable_platform_is_never_asked_to_do_anything() {
    let counting = Counting::default();
    let keep = KeepAwake::with(unable(), &counting);

    let refused = keep.set(KeepAwakeSpan::OneHour, NOW);

    assert!(matches!(
        refused,
        Err(MiraError::Unsupported {
            capability: Capability::KeepAwake,
            ..
        })
    ));
    assert!(matches!(keep.state(), KeepAwakeState::Unavailable { .. }));
    assert_eq!(
        counting.engagements(),
        0,
        "the capability check comes before the operating system, not after it"
    );
}

// ── The capability, per platform ─────────────────────────────────────────────

#[test]
fn the_capability_matrix_says_what_this_slice_actually_built() {
    // `platform-abstraction.md` §5 has a row for Keep Awake, and roadmap rule 4
    // says a capability is declared honestly in the slice that introduces it.
    // This is that declaration, in a form that fails the build if the code and the
    // document drift apart.
    assert_eq!(
        Platform::from_facts(facts(Os::MacOs)).status(Capability::KeepAwake),
        CapabilityStatus::Full
    );

    for absent in [Os::Windows, Os::Linux] {
        let status = Platform::from_facts(facts(absent)).status(Capability::KeepAwake);
        match status {
            CapabilityStatus::Unavailable { reason, fallback } => {
                assert!(!reason.is_empty(), "{absent:?} must say why");
                assert!(
                    fallback.is_some(),
                    "{absent:?} has power settings of its own; say so"
                );
            }
            other => panic!("expected {absent:?} to be unavailable, got {other:?}"),
        }
    }
}

#[test]
fn the_reasons_are_written_for_a_person_and_never_promise_a_workaround() {
    // §2 rule 3: every reason is user-facing copy. It must also not name a
    // program Mira could run on the user's behalf — Keep Awake is a native power
    // request, and the absence of `caffeinate`, `powercfg` and `systemd-inhibit`
    // from this product is the point (ADR-0014).
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        let status = Platform::from_facts(facts(os)).status(Capability::KeepAwake);
        let words = match &status {
            CapabilityStatus::Full => String::new(),
            CapabilityStatus::Degraded { reason, detail } => format!("{reason} {detail}"),
            CapabilityStatus::Unavailable { reason, fallback } => {
                format!("{reason} {}", fallback.clone().unwrap_or_default())
            }
        };

        for shell_shaped in ["caffeinate", "powercfg", "systemd-inhibit", "xdotool"] {
            assert!(
                !words.contains(shell_shaped),
                "{os:?} names {shell_shaped}, which Mira neither runs nor recommends running"
            );
        }
    }
}

// ── The vocabulary ───────────────────────────────────────────────────────────

#[test]
fn a_span_is_one_of_exactly_four_words() {
    use std::collections::BTreeSet;

    let spans: BTreeSet<String> = KeepAwakeSpan::ALL
        .iter()
        .map(|span| serde_json::to_string(span).expect("serialise"))
        .collect();

    assert_eq!(
        spans,
        BTreeSet::from([
            "\"off\"".to_owned(),
            "\"thirtyMinutes\"".to_owned(),
            "\"oneHour\"".to_owned(),
            "\"untilTurnedOff\"".to_owned(),
        ])
    );
}

#[test]
fn no_span_can_be_asked_for_a_duration_of_its_own() {
    // The reason spans are words and not numbers: there is nothing here a caller
    // could inflate. The longest a lock can last without being renewed is an hour.
    for span in KeepAwakeSpan::ALL {
        if let Some(window) = span.duration() {
            assert!(window.as_secs() <= 60 * 60);
        }
    }

    for refused in [
        serde_json::json!(3600),
        serde_json::json!("forever"),
        serde_json::json!("1h"),
        serde_json::json!({ "minutes": 999_999 }),
        serde_json::json!(null),
    ] {
        assert!(
            serde_json::from_value::<KeepAwakeSpan>(refused.clone()).is_err(),
            "{refused} must not deserialise into a span"
        );
    }
}
