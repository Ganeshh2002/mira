//! The crate's whole job in Slice 0: sit between the shell and the repository
//! without reaching around it.

use mira_core::{MiraError, Result};
use mira_db::WorkspaceRepo;
use mira_workspaces::{WorkspaceService, Workspaces};

struct FakeRepo(Result<u32>);

impl WorkspaceRepo for FakeRepo {
    fn count(&self) -> Result<u32> {
        self.0.clone()
    }
}

#[test]
fn the_service_reports_what_the_repository_reports() {
    let service = Workspaces::new(FakeRepo(Ok(7)));
    assert_eq!(service.count().expect("count"), 7);
}

#[test]
fn a_repository_failure_reaches_the_caller_intact() {
    let failure = MiraError::NotFound {
        what: "the workspaces table".to_owned(),
    };
    let service = Workspaces::new(FakeRepo(Err(failure.clone())));
    assert_eq!(service.count().expect_err("must not be swallowed"), failure);
}
