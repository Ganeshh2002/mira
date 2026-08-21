//! The crate's boundary: sit between the shell and the repository without
//! reaching around it, and without editorialising what the repository says.
//!
//! A database failure is one of the states Slice 1 must handle. The rule is that
//! the reason survives the trip: the shell shows a person what actually happened,
//! not a generic "something went wrong" invented on the way up.

use std::path::{Path, PathBuf};

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_db::{NewProject, ProjectRepo};
use mira_fs::PathMatching;
use mira_git::{
    ChangedFiles, CommitGraph, CommitId, CommitLookup, CommitPage, DiffScope, FileDiff,
    FileHistory, FileSubject, GitOverview, GitProvider,
};
use mira_projects::{ProjectService, Projects};
use tempfile::TempDir;

/// A repository whose every operation fails the same way.
struct BrokenRepo(MiraError);

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

struct NoGit;

impl GitProvider for NoGit {
    fn discover(&self, _start: &Path) -> Option<PathBuf> {
        None
    }
    fn overview(&self, _root: &Path) -> GitOverview {
        GitOverview::NotARepository
    }
    fn history(&self, _root: &Path, _from: Option<&CommitId>) -> CommitPage {
        CommitPage::NotARepository
    }
    fn commit(&self, _root: &Path, _id: &CommitId) -> CommitLookup {
        CommitLookup::NotARepository
    }
    fn graph(&self, _root: &Path, _from: Option<&CommitId>) -> CommitGraph {
        CommitGraph::NotARepository
    }
    fn changed_files(&self, _root: &Path, _scope: &DiffScope) -> ChangedFiles {
        ChangedFiles::NotARepository
    }
    fn file_diff(&self, _root: &Path, _scope: &DiffScope, _at: u32) -> FileDiff {
        FileDiff::NotARepository
    }
    fn file_history(
        &self,
        _root: &Path,
        _subject: &FileSubject,
        _from: Option<&CommitId>,
    ) -> FileHistory {
        FileHistory::NotARepository
    }
}

fn broken(failure: &MiraError) -> Projects<BrokenRepo, NoGit> {
    Projects::new(
        BrokenRepo(failure.clone()),
        NoGit,
        PathMatching::CaseSensitive,
    )
}

#[test]
fn a_database_failure_while_listing_reaches_the_caller_intact() {
    let failure = MiraError::external("SQLite", "disk I/O error");

    assert_eq!(
        broken(&failure).list().expect_err("must not be swallowed"),
        failure,
        "the shell needs the real reason to show a person, not a generic error"
    );
}

#[test]
fn a_database_failure_while_adding_reaches_the_caller_intact() {
    let dir = TempDir::new().expect("tempdir");
    let failure = MiraError::external("SQLite", "database is locked");

    assert_eq!(
        broken(&failure)
            .add(dir.path(), 1)
            .expect_err("must not be swallowed"),
        failure
    );
}

#[test]
fn a_database_failure_while_opening_reaches_the_caller_intact() {
    let failure = MiraError::external("SQLite", "attempt to write a readonly database");

    assert_eq!(
        broken(&failure)
            .open(ProjectId::new(1), 1)
            .expect_err("must not be swallowed"),
        failure
    );
}
