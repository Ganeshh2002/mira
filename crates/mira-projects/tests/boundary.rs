//! The crate's whole job in Slice 0: sit between the shell and the repository
//! without reaching around it.

use mira_core::{MiraError, Result};
use mira_db::ProjectRepo;
use mira_projects::{ProjectService, Projects};

struct FakeRepo(Result<u32>);

impl ProjectRepo for FakeRepo {
    fn count(&self) -> Result<u32> {
        self.0.clone()
    }
}

#[test]
fn the_service_reports_what_the_repository_reports() {
    let service = Projects::new(FakeRepo(Ok(3)));
    assert_eq!(service.count().expect("count"), 3);
}

#[test]
fn a_repository_failure_reaches_the_caller_intact() {
    let failure = MiraError::external("SQLite", "disk I/O error");
    let service = Projects::new(FakeRepo(Err(failure.clone())));

    assert_eq!(
        service.count().expect_err("must not be swallowed"),
        failure,
        "the shell needs the real reason to show a person, not a generic error"
    );
}
