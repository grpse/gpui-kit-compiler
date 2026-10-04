use crate::{Entry, allocated_bytes};
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct SearchUpdate {
    pub visited: u64,
    pub folders: u64,
    pub skipped: u64,
    pub matches: u64,
    pub results: Vec<Entry>,
}

/// Unicode-aware subsequence matching; contiguous and word-boundary matches rank first.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i64> {
    let query = query.to_lowercase().chars().collect::<Vec<_>>();
    let candidate = candidate.to_lowercase().chars().collect::<Vec<_>>();
    if query.is_empty() {
        return Some(0);
    }
    let mut next = 0;
    let mut score = 0;
    let mut previous: Option<usize> = None;
    for (index, &character) in candidate.iter().enumerate() {
        if character != query[next] {
            continue;
        }
        score += 10;
        if let Some(previous) = previous {
            if previous + 1 == index {
                score += 24;
            } else {
                score -= (index - previous - 1).min(30) as i64;
            }
        }
        if index == 0 || !candidate[index - 1].is_alphanumeric() {
            score += 20;
        }
        if next == 0 {
            score -= index as i64;
        }
        previous = Some(index);
        next += 1;
        if next == query.len() {
            return Some(score - candidate.len() as i64 / 4);
        }
    }
    None
}

/// Enumerate and rank on a worker. Never follow directory links; retain at most 500 results.
pub fn search(
    root: &Path,
    query: &str,
    recursive: bool,
    hidden: bool,
    cancelled: Arc<AtomicBool>,
    mut report: impl FnMut(SearchUpdate),
) -> Result<SearchUpdate, String> {
    let mut pending = vec![(root.to_owned(), 0usize)];
    let mut update = SearchUpdate::default();
    let mut ranked: Vec<(i64, Entry)> = vec![];
    let mut last_report = Instant::now();
    fn snapshot(update: &SearchUpdate, ranked: &mut Vec<(i64, Entry)>) -> SearchUpdate {
        ranked.sort_by(|(a, left), (b, right)| b.cmp(a).then_with(|| left.path.cmp(&right.path)));
        ranked.truncate(500);
        SearchUpdate {
            results: ranked.iter().map(|(_, entry)| entry.clone()).collect(),
            ..update.clone()
        }
    }
    while let Some((folder, depth)) = pending.pop() {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Search cancelled".into());
        }
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) if depth == 0 => return Err(format!("{}: {error}", folder.display())),
            Err(_) => {
                update.skipped += 1;
                continue;
            }
        };
        update.folders += 1;
        if last_report.elapsed() >= Duration::from_millis(100) {
            report(snapshot(&update, &mut ranked));
            last_report = Instant::now();
        }
        for entry in entries {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Search cancelled".into());
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    update.skipped += 1;
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                continue;
            }
            update.visited += 1;
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    update.skipped += 1;
                    continue;
                }
            };
            let is_symlink = metadata.file_type().is_symlink();
            let is_dir = metadata.is_dir();
            if recursive && is_dir {
                if depth < 256 {
                    pending.push((path.clone(), depth + 1));
                } else {
                    update.skipped += 1;
                }
            }
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
            let score = fuzzy_score(query, &name)
                .map(|score| score + 80)
                .or_else(|| fuzzy_score(query, &relative));
            if let Some(score) = score {
                update.matches += 1;
                ranked.push((
                    score,
                    Entry {
                        path,
                        name,
                        is_dir,
                        is_symlink,
                        bytes: metadata.len(),
                        allocated: allocated_bytes(&metadata),
                        modified: metadata.modified().ok(),
                    },
                ));
                if ranked.len() >= 1000 {
                    let _ = snapshot(&update, &mut ranked);
                }
            }
            if last_report.elapsed() >= Duration::from_millis(100) {
                report(snapshot(&update, &mut ranked));
                last_report = Instant::now();
            }
        }
    }
    let result = snapshot(&update, &mut ranked);
    report(result.clone());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranking_prefers_contiguous_names_and_handles_unicode() {
        assert!(fuzzy_score("rsm", "resume.md").is_some());
        assert!(fuzzy_score("rés", "Résumé.pdf").is_some());
        assert!(fuzzy_score("abc", "acb").is_none());
        assert!(
            fuzzy_score("read", "README.md") > fuzzy_score("read", "really-extra-awesome-document")
        );
    }

    #[test]
    fn recursive_search_respects_hidden_subtrees_scope_links_and_cancellation() {
        let root = std::env::temp_dir().join(format!("explorer-search-{}", std::process::id()));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("README.md"), "a").unwrap();
        fs::write(root.join("nested/README.md"), "b").unwrap();
        fs::write(root.join(".hidden/README.md"), "c").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let run = |recursive, hidden| {
            search(
                &root,
                "rdm",
                recursive,
                hidden,
                Arc::new(AtomicBool::new(false)),
                |_| {},
            )
            .unwrap()
        };
        assert_eq!(run(false, false).matches, 1);
        assert_eq!(run(true, false).matches, 2);
        assert_eq!(run(true, true).matches, 3);
        assert!(
            search(
                &root,
                "rdm",
                true,
                true,
                Arc::new(AtomicBool::new(true)),
                |_| {}
            )
            .is_err()
        );
        for i in 0..550 {
            fs::write(root.join(format!("match-{i}")), "").unwrap();
        }
        let bounded = search(
            &root,
            "match",
            false,
            false,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        assert_eq!(bounded.matches, 550);
        assert_eq!(bounded.results.len(), 500);
        fs::remove_dir_all(root).unwrap();
    }
}
