//! The crate's boundary: sit between the shell and the repository without
//! reaching around it, and without editorialising what the repository says.
//!
//! The risk this file exists for is specific. `create` asks the repository for
//! the project's existing workspaces to refuse a duplicate name. If that read
//! fails and the failure is treated as "no workspaces, so no duplicate", Mira
//! would go on to attempt the insert and report a constraint violation — or
//! worse, succeed against a database that was only briefly unhappy. The reason
//! has to survive the trip.

use mira_core::action::ActionId;
use mira_core::service::{Port, WatchedService};
use mira_core::{
    AppId, AppKind, MiraError, Project, ProjectId, Result, Workspace, WorkspaceId,
    WorkspaceServiceId,
};
use mira_db::{NewProject, NewWorkspace, ProjectRepo, WorkspaceRepo};
use mira_workspaces::{WorkspaceService, Workspaces};

/// A repository whose every operation fails the same way.
struct BrokenRepo(MiraError);

impl WorkspaceRepo for BrokenRepo {
    fn count(&self) -> Result<u32> {
        Err(self.0.clone())
    }
    fn list_for(&self, _project_id: ProjectId) -> Result<Vec<Workspace>> {
        Err(self.0.clone())
    }
    fn get_workspace(&self, _id: WorkspaceId) -> Result<Workspace> {
        Err(self.0.clone())
    }
    fn create(&self, _new: &NewWorkspace, _now: i64) -> Result<Workspace> {
        Err(self.0.clone())
    }
    fn rename_workspace(
        &self,
        _id: WorkspaceId,
        _name: &str,
        _description: Option<&str>,
        _now: i64,
    ) -> Result<()> {
        Err(self.0.clone())
    }
    fn touch_workspace(&self, _id: WorkspaceId, _now: i64) -> Result<()> {
        Err(self.0.clone())
    }
    fn remove_workspace(&self, _id: WorkspaceId) -> Result<()> {
        Err(self.0.clone())
    }
    fn set_workspace_applications(
        &self,
        _id: WorkspaceId,
        _kinds: &[AppKind],
        _now: i64,
    ) -> Result<()> {
        Err(self.0.clone())
    }

    fn set_workspace_preference(
        &self,
        _id: WorkspaceId,
        _kind: AppKind,
        _application: Option<&AppId>,
        _now: i64,
    ) -> Result<()> {
        Err(self.0.clone())
    }
    fn workspace_services(&self, _id: WorkspaceId) -> Result<Vec<WatchedService>> {
        Err(self.0.clone())
    }
    fn watch_service(&self, _id: WorkspaceId, _port: Port, _now: i64) -> Result<WatchedService> {
        Err(self.0.clone())
    }
    fn forget_service(&self, _id: WorkspaceId, _service: WorkspaceServiceId) -> Result<()> {
        Err(self.0.clone())
    }
    fn workspace_service(
        &self,
        _id: WorkspaceId,
        _service: WorkspaceServiceId,
    ) -> Result<WatchedService> {
        Err(self.0.clone())
    }
    fn workspace_actions(&self, _id: WorkspaceId) -> Result<Vec<ActionId>> {
        Err(self.0.clone())
    }
    fn set_workspace_action(
        &self,
        _id: WorkspaceId,
        _action: &ActionId,
        _wanted: bool,
        _now: i64,
    ) -> Result<()> {
        Err(self.0.clone())
    }
}

impl ProjectRepo for BrokenRepo {
    fn count(&self) -> Result<u32> {
        Err(self.0.clone())
    }
    fn list(&self) -> Result<Vec<Project>> {
        Err(self.0.clone())
    }
    fn get(&self, _id: ProjectId) -> Result<Project> {
        Err(self.0.clone())
    }
    fn find_by_root(&self, _root_path: &str) -> Result<Option<Project>> {
        Err(self.0.clone())
    }
    fn insert(&self, _new: &NewProject, _now: i64) -> Result<Project> {
        Err(self.0.clone())
    }
    fn touch_opened(&self, _id: ProjectId, _now: i64) -> Result<()> {
        Err(self.0.clone())
    }
    fn remove(&self, _id: ProjectId) -> Result<()> {
        Err(self.0.clone())
    }
}

fn broken(failure: &MiraError) -> Workspaces<BrokenRepo> {
    Workspaces::new(BrokenRepo(failure.clone()))
}

#[test]
fn a_database_failure_while_listing_reaches_the_caller_intact() {
    let failure = MiraError::external("SQLite", "disk I/O error");

    assert_eq!(
        broken(&failure)
            .list_for(ProjectId::new(1))
            .expect_err("must not be swallowed"),
        failure,
        "the shell needs the real reason to show a person, not a generic error"
    );
}

#[test]
fn a_database_failure_while_creating_is_not_mistaken_for_an_absent_duplicate() {
    // The failure mode this whole file is about: a read that fails must not read
    // as "nothing found".
    let failure = MiraError::external("SQLite", "database is locked");

    assert_eq!(
        broken(&failure)
            .create(ProjectId::new(1), "Web", None, 1)
            .expect_err("must not be swallowed"),
        failure
    );
}

#[test]
fn a_database_failure_while_opening_reaches_the_caller_intact() {
    let failure = MiraError::external("SQLite", "attempt to write a readonly database");

    assert_eq!(
        broken(&failure)
            .open(WorkspaceId::new(1), 1)
            .expect_err("must not be swallowed"),
        failure
    );
}

#[test]
fn a_blank_name_is_refused_before_the_database_is_asked_anything() {
    // Validation first: a name Mira will not accept should not cost a query, and
    // the message should be about the name rather than about SQLite.
    let failure = MiraError::external("SQLite", "disk I/O error");

    match broken(&failure).create(ProjectId::new(1), "   ", None, 1) {
        Err(MiraError::Invalid { field, .. }) => assert_eq!(field, "name"),
        other => panic!("expected Invalid, got {other:?}"),
    }
}
