//! Fast, non-blocking deletion with live progress.
//!
//! Deleting a modpack build means removing thousands of small files —
//! done on the UI thread it froze the window for seconds. Now a folder is
//! first *renamed* into a trash folder on the same drive (instant, so the
//! build disappears from every list right away), and a background worker
//! removes the files on several threads while the UI reads the counters
//! below and draws a progress card. Leftovers of an interrupted run are
//! picked up on the next start.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// What the progress card shows.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    /// «Сборка Fabulously Optimized» — what is being removed now.
    pub title: String,
    pub done_files: u64,
    pub total_files: u64,
    pub freed: u64,
    /// Items still waiting after the current one.
    pub queued: usize,
    /// True while files are being counted (total not known yet).
    pub counting: bool,
}

struct State {
    queue: Mutex<VecDeque<(String, Vec<PathBuf>)>>,
    title: Mutex<String>,
    busy: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    freed: AtomicU64,
    counting: AtomicBool,
    /// Bumped on every finished item: the UI refreshes its lists.
    finished: AtomicU64,
    /// Bytes freed by the last completed batch (for the final toast).
    last_freed: AtomicU64,
}

fn state() -> &'static State {
    static S: OnceLock<State> = OnceLock::new();
    S.get_or_init(|| State {
        queue: Mutex::new(VecDeque::new()),
        title: Mutex::new(String::new()),
        busy: AtomicBool::new(false),
        done: AtomicU64::new(0),
        total: AtomicU64::new(0),
        freed: AtomicU64::new(0),
        counting: AtomicBool::new(false),
        finished: AtomicU64::new(0),
        last_freed: AtomicU64::new(0),
    })
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Trash folder on the same drive as `path`, so the rename never copies.
fn trash_for(path: &Path) -> PathBuf {
    let data = crate::config::data_dir();
    if path.starts_with(&data) {
        data.join(".trash")
    } else {
        path.parent().map(|p| p.join(".sad-trash")).unwrap_or_else(|| data.join(".trash"))
    }
}

/// Moves `path` to `.trash` and queues the tomb for background deletion.
/// Returns `false` if `path` still exists and could not be renamed (e.g. locked).
pub(crate) fn try_trash(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    let trash = trash_for(path);
    let _ = std::fs::create_dir_all(&trash);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tomb = trash.join(format!("{stamp}-{name}"));
    if std::fs::rename(path, &tomb).is_ok() {
        lock(&state().queue).push_back(("Старые файлы пака".into(), vec![tomb]));
        ensure_worker();
        true
    } else {
        false
    }
}

/// Moves `path` out of the way instantly and queues it for removal.
/// Returns false when there was nothing to remove.
pub fn queue(title: impl Into<String>, path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let trash = trash_for(path);
    let _ = std::fs::create_dir_all(&trash);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tomb = trash.join(format!("{stamp}-{name}"));
    // Locked files (a running game) make the rename fail: the folder is
    // then removed in place — still in the background.
    let target = if std::fs::rename(path, &tomb).is_ok() { tomb } else { path.to_path_buf() };
    lock(&state().queue).push_back((title.into(), vec![target]));
    ensure_worker();
    true
}

/// Queues loose files (e.g. every mod of a build) for background removal
/// without renaming: lists update live as the files go away.
pub fn queue_files(title: impl Into<String>, files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    lock(&state().queue).push_back((title.into(), files));
    ensure_worker();
}

/// Leftovers of a run that was cut short (window closed mid-delete).
pub fn sweep_leftovers() {
    let data = crate::config::data_dir();
    let trash = data.join(".trash");
    let Ok(rd) = std::fs::read_dir(&trash) else { return };
    let mut any = false;
    for e in rd.flatten() {
        lock(&state().queue).push_back(("Остатки прошлого удаления".into(), vec![e.path()]));
        any = true;
    }
    if any {
        ensure_worker();
    }
}

/// Counter of finished items; compare with a remembered value to know
/// when to refresh lists.
pub fn finished_count() -> u64 {
    state().finished.load(Ordering::Relaxed)
}

/// Bytes freed by the batch that just completed.
pub fn last_freed() -> u64 {
    state().last_freed.load(Ordering::Relaxed)
}

pub fn progress() -> Option<Progress> {
    let s = state();
    if !s.busy.load(Ordering::Relaxed) {
        return None;
    }
    Some(Progress {
        title: lock(&s.title).clone(),
        done_files: s.done.load(Ordering::Relaxed),
        total_files: s.total.load(Ordering::Relaxed),
        freed: s.freed.load(Ordering::Relaxed),
        queued: lock(&s.queue).len(),
        counting: s.counting.load(Ordering::Relaxed),
    })
}

fn ensure_worker() {
    let s = state();
    if s.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    s.freed.store(0, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("sad-remove".into())
        .stack_size(512 * 1024)
        .spawn(|| {
            let s = state();
            loop {
                let next = lock(&s.queue).pop_front();
                let Some((title, paths)) = next else {
                    s.last_freed.store(s.freed.load(Ordering::Relaxed), Ordering::Relaxed);
                    s.busy.store(false, Ordering::SeqCst);
                    // A queue() that raced with the exit restarts the worker.
                    if !lock(&s.queue).is_empty() && !s.busy.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    s.finished.fetch_add(1, Ordering::Relaxed);
                    return;
                };
                *lock(&s.title) = title;
                s.done.store(0, Ordering::Relaxed);
                s.total.store(0, Ordering::Relaxed);
                for path in &paths {
                    remove_tree(path, &s.done, &s.total, &s.freed, &s.counting);
                }
                s.finished.fetch_add(1, Ordering::Relaxed);
            }
        })
        .ok();
}

/// Removes `root` (file or folder) on several threads, adding files and
/// bytes to the given counters.
pub fn remove_tree(root: &Path, done: &AtomicU64, total: &AtomicU64, freed: &AtomicU64, counting: &AtomicBool) {
    if root.is_file() {
        total.fetch_add(1, Ordering::Relaxed);
        let len = std::fs::metadata(root).map(|m| m.len()).unwrap_or(0);
        if remove_file(root) {
            freed.fetch_add(len, Ordering::Relaxed);
        }
        done.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // 1. Walk once: files with sizes, folders deepest-last.
    counting.store(true, Ordering::Relaxed);
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    let mut dirs: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                dirs.push(p.clone());
                stack.push(p);
            } else {
                let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((p, len));
                total.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    counting.store(false, Ordering::Relaxed);

    // 2. Files in parallel — Windows deletes are syscall-bound, threads help.
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8)
        .min(files.len().max(1));
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((p, len)) = files.get(i) else { return };
                if remove_file(p) {
                    freed.fetch_add(*len, Ordering::Relaxed);
                }
                done.fetch_add(1, Ordering::Relaxed);
            });
        }
    });

    // 3. Empty folders, children before parents.
    for dir in dirs.iter().rev() {
        let _ = std::fs::remove_dir(dir);
    }
    if root.exists() {
        let _ = std::fs::remove_dir_all(root);
    }
}

/// Deletes one file; read-only files (git packs, some mods) are unlocked first.
fn remove_file(p: &Path) -> bool {
    if std::fs::remove_file(p).is_ok() {
        return true;
    }
    if let Ok(meta) = std::fs::metadata(p) {
        let mut perm = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(p, perm);
    }
    std::fs::remove_file(p).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_tree_counts_and_clears_readonly() {
        let root = std::env::temp_dir().join(format!("sad-remove-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for i in 0..120 {
            let d = root.join(format!("d{}", i % 9)).join("deep");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(format!("f{i}.txt")), vec![b'x'; 100]).unwrap();
        }
        let ro = root.join("ro.txt");
        std::fs::write(&ro, b"1234").unwrap();
        let mut perm = std::fs::metadata(&ro).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&ro, perm).unwrap();

        let (done, total, freed, counting) =
            (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicBool::new(false));
        remove_tree(&root, &done, &total, &freed, &counting);
        assert!(!root.exists());
        assert_eq!(total.load(Ordering::Relaxed), 121);
        assert_eq!(done.load(Ordering::Relaxed), 121);
        assert_eq!(freed.load(Ordering::Relaxed), 120 * 100 + 4);
    }
}
