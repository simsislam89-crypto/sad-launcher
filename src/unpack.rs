//! Fast zip extraction for modpacks, maps and runtimes.
//!
//! Packs ship thousands of small config files. Writing them one by one on
//! a single thread is slow on Windows (every closed file is scanned by the
//! antivirus), so entries are spread over several threads, each with its
//! own reader of the same archive. Folders are created up front, entries
//! are streamed to disk without buffering whole files in memory, and the
//! biggest entries start first so no thread is left with a long tail.

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// One entry to write: archive index → absolute target path.
#[derive(Clone, Debug)]
pub struct Job {
    pub index: usize,
    pub size: u64,
    pub target: PathBuf,
}

fn open(archive: &Path) -> Result<zip::ZipArchive<BufReader<File>>> {
    let file = File::open(archive).with_context(|| format!("Нет архива {}", archive.display()))?;
    zip::ZipArchive::new(BufReader::with_capacity(64 * 1024, file)).context("Архив не читается")
}

/// Text of one entry (e.g. `modrinth.index.json`) without unpacking the rest.
pub fn read_text(archive: &Path, name: &str) -> Option<String> {
    let mut zip = open(archive).ok()?;
    let mut entry = zip.by_name(name).ok()?;
    let mut text = String::with_capacity(entry.size() as usize);
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Entries under any of `prefixes` (e.g. `overrides/`, `client-overrides/`)
/// mapped into `root` with the prefix cut off. A later prefix wins over an
/// earlier one for the same path, as pack formats require.
pub fn prefixed_jobs(archive: &Path, prefixes: &[String], root: &Path) -> Result<Vec<Job>> {
    let zip = open(archive)?;
    let mut by_rel: BTreeMap<PathBuf, Job> = BTreeMap::new();
    for prefix in prefixes {
        let prefix = format!("{}/", prefix.trim_end_matches('/'));
        for index in 0..zip.len() {
            let Some(name) = zip.name_for_index(index) else { continue };
            let Some(rest) = name.replace('\\', "/").strip_prefix(&prefix).map(str::to_string) else {
                continue;
            };
            if rest.is_empty() || rest.ends_with('/') {
                continue;
            }
            let Some(rel) = safe_rel(&rest) else { continue };
            by_rel.insert(
                rel.clone(),
                Job {
                    index,
                    size: 0,
                    target: root.join(rel),
                },
            );
        }
    }
    let mut jobs: Vec<Job> = by_rel.into_values().collect();
    fill_sizes(archive, &mut jobs)?;
    Ok(jobs)
}

/// Every file entry of the archive mapped into `dest`.
pub fn all_jobs(archive: &Path, dest: &Path) -> Result<Vec<Job>> {
    let mut zip = open(archive)?;
    let mut jobs = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else { continue };
        jobs.push(Job {
            index,
            size: entry.size(),
            target: dest.join(rel),
        });
    }
    Ok(jobs)
}

fn fill_sizes(archive: &Path, jobs: &mut [Job]) -> Result<()> {
    let mut zip = open(archive)?;
    for job in jobs.iter_mut() {
        job.size = zip.by_index_raw(job.index).map(|e| e.size()).unwrap_or(0);
    }
    Ok(())
}

/// A relative path inside the archive that stays inside the target folder.
fn safe_rel(path: &str) -> Option<PathBuf> {
    if path.starts_with('/') || path.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => continue,
            ".." => return None,
            s => out.push(s),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Writes `jobs` in parallel. `done` counts written files (for progress).
/// Returns the number of files written.
pub fn extract(archive: &Path, mut jobs: Vec<Job>, cancel: Option<&AtomicBool>, done: &AtomicU64) -> Result<u64> {
    if jobs.is_empty() {
        return Ok(0);
    }
    // Folders first, once each: threads then only create files.
    let dirs: HashSet<&Path> = jobs.iter().filter_map(|j| j.target.parent()).collect();
    for dir in dirs {
        std::fs::create_dir_all(dir).with_context(|| format!("Не создать папку {}", dir.display()))?;
    }
    jobs.sort_by(|a, b| b.size.cmp(&a.size));
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8)
        .min(jobs.len());
    let next = AtomicUsize::new(0);
    let failed: std::sync::Mutex<Option<anyhow::Error>> = std::sync::Mutex::new(None);
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut zip = match open(archive) {
                    Ok(z) => z,
                    Err(e) => {
                        stop.store(true, Ordering::Relaxed);
                        failed.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(e);
                        return;
                    }
                };
                let mut buf = vec![0u8; 256 * 1024];
                loop {
                    if stop.load(Ordering::Relaxed) || cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { return };
                    if let Err(e) = write_entry(&mut zip, job, &mut buf) {
                        stop.store(true, Ordering::Relaxed);
                        failed.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(e);
                        return;
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().unwrap_or_else(|p| p.into_inner()) {
        return Err(e);
    }
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        bail!("Отменено");
    }
    Ok(done.load(Ordering::Relaxed))
}

fn write_entry(zip: &mut zip::ZipArchive<BufReader<File>>, job: &Job, buf: &mut [u8]) -> Result<()> {
    let mut entry = zip.by_index(job.index)?;
    let file = File::create(&job.target).with_context(|| format!("Не записать {}", job.target.display()))?;
    if job.size > 0 {
        let _ = file.set_len(job.size);
    }
    let mut out = BufWriter::with_capacity(256 * 1024, file);
    loop {
        let n = entry.read(buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
    }
    out.flush()?;
    Ok(())
}

/// Whole archive into `dest` (maps, runtimes).
pub fn unzip(archive: &Path, dest: &Path) -> Result<u64> {
    std::fs::create_dir_all(dest)?;
    let jobs = all_jobs(archive, dest)?;
    extract(archive, jobs, None, &AtomicU64::new(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sad-unpack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn overrides_go_straight_into_root_and_client_wins() {
        let dir = tmp("ovr");
        let archive = dir.join("p.zip");
        make_zip(
            &archive,
            &[
                ("modrinth.index.json", b"{}"),
                ("overrides/config/a.toml", b"base"),
                ("overrides/config/b.toml", b"b"),
                ("client-overrides/config/a.toml", b"client"),
                ("overrides/../evil.txt", b"x"),
            ],
        );
        let root = dir.join("root");
        let jobs = prefixed_jobs(&archive, &["overrides".into(), "client-overrides".into()], &root).unwrap();
        assert_eq!(jobs.len(), 2);
        let n = extract(&archive, jobs, None, &AtomicU64::new(0)).unwrap();
        assert_eq!(n, 2);
        assert_eq!(std::fs::read_to_string(root.join("config/a.toml")).unwrap(), "client");
        assert_eq!(std::fs::read_to_string(root.join("config/b.toml")).unwrap(), "b");
        assert!(!dir.join("evil.txt").exists());
        assert_eq!(read_text(&archive, "modrinth.index.json").as_deref(), Some("{}"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unzip_writes_many_files_in_parallel() {
        let dir = tmp("many");
        let archive = dir.join("m.zip");
        let names: Vec<String> = (0..300).map(|i| format!("d{}/f{i}.txt", i % 7)).collect();
        let datas: Vec<Vec<u8>> = (0..300).map(|i| vec![b'a' + (i % 26) as u8; i * 37]).collect();
        let entries: Vec<(&str, &[u8])> = names.iter().map(|s| s.as_str()).zip(datas.iter().map(|d| d.as_slice())).collect();
        make_zip(&archive, &entries);
        let out = dir.join("out");
        assert_eq!(unzip(&archive, &out).unwrap(), 300);
        for (i, name) in names.iter().enumerate() {
            assert_eq!(std::fs::read(out.join(name)).unwrap(), datas[i]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_stops_extraction() {
        let dir = tmp("cancel");
        let archive = dir.join("c.zip");
        make_zip(&archive, &[("a.txt", b"1"), ("b.txt", b"2")]);
        let cancel = AtomicBool::new(true);
        let jobs = all_jobs(&archive, &dir.join("o")).unwrap();
        assert!(extract(&archive, jobs, Some(&cancel), &AtomicU64::new(0)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
