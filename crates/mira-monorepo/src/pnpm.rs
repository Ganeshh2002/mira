//! The one key Mira reads out of `pnpm-workspace.yaml`.
//!
//! **Why a parser here rather than a YAML crate.** The file Mira needs is a
//! `packages:` list of strings. `serde_yaml`, the obvious dependency, is archived
//! and carries a RUSTSEC unmaintained advisory; taking it would put an
//! unmaintained parser for a general-purpose format on the path that reads files
//! out of repositories a user did not write. The forks are young. So Mira reads
//! the documented subset it needs and treats everything else as *no packages
//! declared* — which costs a user with an exotic file nothing but a missing
//! badge, and costs Mira no dependency at all.
//!
//! The subset, stated so it can be checked:
//!
//! - `packages:` followed by a block sequence of `- item` lines;
//! - `packages: [a, b]`, the flow form on one line;
//! - single quotes, double quotes, or none around each item;
//! - `#` comments, whole-line or trailing;
//! - any other key, and any nesting under it, ignored.
//!
//! Anything it cannot read yields `None`, and detection continues as if the file
//! were not there.

/// The `packages` entries of a `pnpm-workspace.yaml`, if it declares any.
#[must_use]
pub fn packages(source: &str) -> Option<Vec<String>> {
    let mut lines = source.lines();

    while let Some(line) = lines.next() {
        let Some(rest) = top_level_key(line, "packages") else {
            continue;
        };

        // `packages: [ … ]` — the whole list is on this line.
        if let Some(inline) = rest.strip_prefix('[') {
            let items: Vec<String> = inline
                .trim_end_matches(']')
                .split(',')
                .filter_map(|item| unquote(item.trim()))
                .collect();
            return (!items.is_empty()).then_some(items);
        }

        if !rest.is_empty() {
            // `packages: something` is a scalar, not a list. Not a shape Mira reads.
            return None;
        }

        let items: Vec<String> = lines.by_ref().map_while(sequence_item).flatten().collect();
        return (!items.is_empty()).then_some(items);
    }

    None
}

/// The value after `key:` when `line` declares that key at the top level.
fn top_level_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    if line.starts_with([' ', '\t', '-']) {
        return None;
    }
    let rest = strip_comment(line)
        .trim_end()
        .strip_prefix(key)?
        .trim_start();
    rest.strip_prefix(':').map(str::trim)
}

/// One item of an indented block sequence, or `None` once the sequence ends.
///
/// A blank line or a comment is part of the sequence and yields nothing; anything
/// else ends it, which is what keeps a later top-level key out of the list.
fn sequence_item(line: &str) -> Option<Option<String>> {
    let trimmed = strip_comment(line).trim();
    if trimmed.is_empty() {
        return Some(None);
    }
    let item = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix('-'))?;
    Some(unquote(item.trim()))
}

fn strip_comment(line: &str) -> &str {
    // A `#` inside quotes would be content, and a glob has no business containing
    // one, so the simple rule is the right one here.
    match line.find('#') {
        Some(at) => &line[..at],
        None => line,
    }
}

fn unquote(item: &str) -> Option<String> {
    let item = item
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            item.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap_or(item)
        .trim();

    (!item.is_empty()).then(|| item.to_owned())
}

#[cfg(test)]
mod tests {
    use super::packages;

    #[test]
    fn a_block_sequence_is_read() {
        let source = "packages:\n  - 'apps/*'\n  - \"packages/*\"\n  - tools/build\n";
        assert_eq!(
            packages(source).expect("packages"),
            ["apps/*", "packages/*", "tools/build"]
        );
    }

    #[test]
    fn a_flow_sequence_is_read() {
        assert_eq!(
            packages("packages: ['apps/*', \"libs/*\"]\n").expect("packages"),
            ["apps/*", "libs/*"]
        );
    }

    #[test]
    fn comments_are_not_packages() {
        let source =
            "# everything we ship\npackages:\n  # the apps\n  - 'apps/*' # and only these\n";
        assert_eq!(packages(source).expect("packages"), ["apps/*"]);
    }

    #[test]
    fn the_sequence_ends_at_the_next_key() {
        let source = "packages:\n  - 'apps/*'\nonlyBuiltDependencies:\n  - esbuild\n";
        assert_eq!(packages(source).expect("packages"), ["apps/*"]);
    }

    #[test]
    fn a_file_without_the_key_declares_nothing() {
        assert_eq!(packages("onlyBuiltDependencies:\n  - esbuild\n"), None);
        assert_eq!(packages(""), None);
    }

    #[test]
    fn an_empty_list_declares_nothing() {
        assert_eq!(packages("packages: []\n"), None);
        assert_eq!(packages("packages:\n"), None);
    }

    #[test]
    fn a_scalar_value_is_not_a_list() {
        assert_eq!(packages("packages: apps/*\n"), None);
    }

    #[test]
    fn a_nested_packages_key_is_not_the_top_level_one() {
        assert_eq!(packages("catalog:\n  packages:\n    - 'apps/*'\n"), None);
    }
}
