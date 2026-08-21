//! Keep Awake's lifecycle through the application shell.
//!
//! `mira-platform` proves the policy against a double, and `mira-scheduler` proves
//! the one-shot timer against a paused clock. This proves the seam between them:
//! that choosing thirty minutes really does release the lock thirty minutes later,
//! that changing your mind moves the deadline rather than stacking a second one,
//! and that quitting gives the machine back.
//!
//! **No test here changes a real power setting.** [`Counting`] stands in for the
//! operating system, and time is paused — a half-hour span costs no wall clock.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mira_core::Result;
use mira_lib::awake::Awake;
use mira_platform::{
    DisplayServer, EnvFacts, Inhibit, KeepAwakeSpan, KeepAwakeState, LinuxPackaging, Os, Platform,
};

/// A stand-in for the operating system's power interface.
///
/// A newtype over a shared counter rather than the counter itself, because the
/// test keeps a handle on the counts while `Awake` owns the inhibitor.
#[derive(Debug, Default, Clone)]
struct Counting(Arc<Counts>);

#[derive(Debug, Default)]
struct Counts {
    engaged: AtomicUsize,
    engagements: AtomicUsize,
    releases: AtomicUsize,
}

impl Counting {
    fn engagements(&self) -> usize {
        self.0.engagements.load(Ordering::SeqCst)
    }

    fn releases(&self) -> usize {
        self.0.releases.load(Ordering::SeqCst)
    }
}

impl Inhibit for Counting {
    fn engage(&self) -> Result<()> {
        assert_eq!(
            self.0.engaged.load(Ordering::SeqCst),
            0,
            "a second request was taken out while the first was still held"
        );
        self.0.engaged.store(1, Ordering::SeqCst);
        self.0.engagements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn release(&self) {
        if self.0.engaged.swap(0, Ordering::SeqCst) == 1 {
            self.0.releases.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn engaged(&self) -> bool {
        self.0.engaged.load(Ordering::SeqCst) == 1
    }
}

/// A machine where Keep Awake works. Windows and Linux are covered by
/// `mira-platform`'s capability tests; this file is about the lifecycle.
fn able() -> Platform {
    Platform::from_facts(EnvFacts {
        os: Os::MacOs,
        display_server: DisplayServer::Quartz,
        has_logind: false,
        packaging: LinuxPackaging::NotApplicable,
    })
}

/// Keep Awake, plus the counters behind it and the times it announced a change.
fn wired() -> (Awake<Counting>, Counting, Arc<AtomicUsize>) {
    let counting = Counting::default();
    let announced = Arc::new(AtomicUsize::new(0));
    let told = Arc::clone(&announced);

    let awake = Awake::with(
        able(),
        counting.clone(),
        Arc::new(move || {
            told.fetch_add(1, Ordering::SeqCst);
        }),
    );

    (awake, counting, announced)
}

#[tokio::test(start_paused = true)]
async fn a_span_releases_itself_when_its_time_is_up() {
    let (awake, counting, announced) = wired();

    awake.set(KeepAwakeSpan::ThirtyMinutes).expect("enable");
    assert!(matches!(awake.state(), KeepAwakeState::On { .. }));

    tokio::time::sleep(Duration::from_secs(29 * 60)).await;
    assert!(
        matches!(awake.state(), KeepAwakeState::On { .. }),
        "still held a minute before it is due"
    );
    assert_eq!(counting.releases(), 0);

    tokio::time::sleep(Duration::from_secs(2 * 60)).await;

    assert_eq!(awake.state(), KeepAwakeState::Off);
    assert_eq!(
        counting.releases(),
        1,
        "the operating system was told, not just the record"
    );
    assert_eq!(
        announced.load(Ordering::SeqCst),
        1,
        "and the interface was told, because nobody pressed anything"
    );
}

#[tokio::test(start_paused = true)]
async fn changing_your_mind_moves_the_deadline_instead_of_stacking_one() {
    let (awake, counting, _) = wired();

    awake.set(KeepAwakeSpan::ThirtyMinutes).expect("enable");
    tokio::time::sleep(Duration::from_secs(20 * 60)).await;
    awake.set(KeepAwakeSpan::OneHour).expect("extend");

    // The first deadline would have fired here. It was disarmed when the second
    // was armed, so the lock is still held.
    tokio::time::sleep(Duration::from_secs(15 * 60)).await;
    assert!(
        matches!(awake.state(), KeepAwakeState::On { .. }),
        "the extended span is what is in force"
    );
    assert_eq!(counting.engagements(), 1, "one request throughout");
    assert_eq!(counting.releases(), 0);

    tokio::time::sleep(Duration::from_secs(46 * 60)).await;
    assert_eq!(awake.state(), KeepAwakeState::Off);
}

#[tokio::test(start_paused = true)]
async fn turning_it_off_disarms_the_deadline_as_well_as_the_lock() {
    let (awake, counting, announced) = wired();

    awake.set(KeepAwakeSpan::ThirtyMinutes).expect("enable");
    awake.set(KeepAwakeSpan::Off).expect("disable");
    assert_eq!(counting.releases(), 1);

    // A deadline left armed would fire into a lock that is already gone and
    // announce a change that did not happen.
    tokio::time::sleep(Duration::from_secs(60 * 60)).await;

    assert_eq!(awake.state(), KeepAwakeState::Off);
    assert_eq!(counting.releases(), 1, "released once, not twice");
    assert_eq!(
        announced.load(Ordering::SeqCst),
        0,
        "nothing changed on its own, so nothing was announced"
    );
}

#[tokio::test(start_paused = true)]
async fn a_span_without_an_end_arms_nothing() {
    let (awake, counting, announced) = wired();

    awake.set(KeepAwakeSpan::UntilTurnedOff).expect("enable");

    tokio::time::sleep(Duration::from_secs(400 * 24 * 60 * 60)).await;

    assert!(matches!(
        awake.state(),
        KeepAwakeState::On {
            span: KeepAwakeSpan::UntilTurnedOff,
            until: None
        }
    ));
    assert_eq!(counting.releases(), 0);
    assert_eq!(announced.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn quitting_releases_the_lock_and_the_deadline() {
    let (awake, counting, announced) = wired();

    awake.set(KeepAwakeSpan::OneHour).expect("enable");
    awake.shutdown();

    assert_eq!(counting.releases(), 1, "the machine is given back at once");

    // And the armed deadline went with it, so nothing fires into a dead process's
    // worth of state.
    tokio::time::sleep(Duration::from_secs(2 * 60 * 60)).await;
    assert_eq!(counting.releases(), 1);
    assert_eq!(announced.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn quitting_twice_is_harmless() {
    let (awake, counting, _) = wired();

    awake.shutdown();
    awake.shutdown();

    assert_eq!(awake.state(), KeepAwakeState::Off);
    assert_eq!(counting.releases(), 0, "nothing was held to release");
}
