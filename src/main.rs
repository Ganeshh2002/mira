use std::path::Path;
use std::time::Instant;

use mira_git::{CommitGraph, GitProvider, Libgit2};

fn main() {
    let repo = Path::new("/Users/ganeshh/mira");

    let started = Instant::now();
    let page = Libgit2.graph(repo, None);
    let took = started.elapsed();

    match page {
        CommitGraph::Ready { head, rows, next, shallow, lanes, collapsed, refs_truncated } => {
            println!("head    {head:?}");
            println!("lanes   {lanes}  collapsed {collapsed}  shallow {shallow}  refs cut {refs_truncated}");
            println!("read    {} rows in {:.2} ms   next {:?}", rows.len(), took.as_secs_f64() * 1000.0, next.is_some());
            println!();
            println!("  lane  kind     edges                     refs                  subject");
            println!("  ----  -------  ------------------------  --------------------  -------");
            for row in &rows {
                let edges: Vec<String> = row
                    .edges
                    .iter()
                    .map(|e| format!("{:?}->{}", e.kind, e.to))
                    .collect();
                let refs: Vec<String> = row
                    .refs
                    .iter()
                    .map(|r| format!("{:?}:{}", r.kind, r.name))
                    .collect();
                println!(
                    "  {:>4}  {:<7}  {:<24}  {:<20}  {}",
                    row.lane,
                    format!("{:?}", row.kind),
                    edges.join(" "),
                    refs.join(" "),
                    &row.commit.subject.chars().take(46).collect::<String>(),
                );
            }
            println!();
            println!("continuing per row: {:?}", rows.iter().map(|r| r.continuing.clone()).collect::<Vec<_>>());
        }
        other => println!("unexpected: {other:?}"),
    }
}
