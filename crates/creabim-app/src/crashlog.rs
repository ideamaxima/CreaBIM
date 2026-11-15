//! Crashes kept for good: `crashes.log`, next to the activity log, is only
//! ever added to, so a crash is still there after the activity log has
//! started again. Each block has the time, the build, what went wrong and
//! the last lines of the log before it.
//!
//! A crash that kills the app outright (a segfault, the graphics driver)
//! writes nothing as it happens; each run leaves a `running-<pid>` marker
//! that a normal close takes away, so the next start finds one left behind
//! and says the session before ended without closing.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Where the crash log and the markers live: beside the activity log.
fn dir() -> Option<PathBuf> {
    crate::actlog::path()?.parent().map(Path::to_path_buf)
}

/// How a block of `crashes.log` starts: each crash's first line.
const HEAD: &str = "##### CRASH";

/// Seconds since 1970 as `YYYY-MM-DD HH:MM:SS UTC`.
fn when(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // days to a civil date (Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Add one crash to `crashes.log` in `dir`: `what` went wrong, in build
/// `build`, at `secs`, after `lines`.
fn record_in(dir: &Path, build: &str, secs: u64, what: &str, lines: &[String]) {
    let _ = std::fs::create_dir_all(dir);
    let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("crashes.log"))
    else {
        return;
    };
    let mut block = format!("\n{HEAD} {} — CreaBIM {build} #####\n{what}\n", when(secs));
    if !lines.is_empty() {
        block.push_str("--- the activity log just before ---\n");
        for l in lines {
            block.push_str(l);
            block.push('\n');
        }
    }
    let _ = f.write_all(block.as_bytes());
}

fn this_build() -> String {
    format!(
        "{} (build {})",
        env!("CARGO_PKG_VERSION"),
        env!("CREABIM_COMMIT")
    )
}

/// Keep a crash of this run: `what` and the last 40 lines of the log.
pub fn record(what: &str) {
    if cfg!(test) {
        return;
    }
    let Some(dir) = dir() else {
        return;
    };
    let (lines, _) = crate::actlog::recent();
    let from = lines.len().saturating_sub(40);
    record_in(&dir, &this_build(), now(), what, &lines[from..]);
}

/// Is process `pid` a CreaBIM still running? Where that can't be told,
/// no: one warning too many beats one too few.
fn alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .is_ok_and(|c| c.trim().to_lowercase().starts_with("creabim"))
}

/// What a marker left behind says: (pid, build, start time).
fn read_marker(p: &Path) -> Option<(u32, String, u64)> {
    let t = std::fs::read_to_string(p).ok()?;
    let mut it = t.lines();
    let pid = it.next()?.trim().parse().ok()?;
    let secs = it.next()?.trim().parse().ok()?;
    let build = it.next().unwrap_or("?").trim().to_string();
    Some((pid, build, secs))
}

/// The markers in `dir` of runs that ended without closing, taken away,
/// and a note for each run still open: (ended, still running).
fn sweep(dir: &Path) -> (Vec<(u32, String, u64)>, Vec<u32>) {
    let (mut ended, mut open) = (vec![], vec![]);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (ended, open);
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("running-") {
            continue;
        }
        match read_marker(&e.path()) {
            Some((pid, _, _)) if alive(pid) => open.push(pid),
            Some(m) => {
                ended.push(m);
                let _ = std::fs::remove_file(e.path());
            }
            None => {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    (ended, open)
}

fn marker(dir: &Path) -> PathBuf {
    dir.join(format!("running-{}", std::process::id()))
}

/// At start, once the activity log is open: report the runs before that
/// ended without closing, then leave this run's marker.
pub fn begin() {
    if cfg!(test) {
        return;
    }
    let Some(dir) = dir() else {
        return;
    };
    let (ended, open) = sweep(&dir);
    for pid in open {
        crate::actlog::line(format!("another CreaBIM is running (process {pid})"));
    }
    for (pid, build, secs) in ended {
        let what = format!(
            "The session started {} (process {pid}, CreaBIM {build}) ended without closing: \
             a crash that left no trace in the app (the graphics driver, a segfault) or the \
             computer stopping. `coredumpctl list` may show it.",
            when(secs)
        );
        crate::actlog::line(format!("CRASH (previous session): {what}"));
        record_in(&dir, &build, secs, &what, &[]);
    }
    let _ = std::fs::write(
        marker(&dir),
        format!("{}\n{}\n{}\n", std::process::id(), now(), this_build()),
    );
}

/// At a normal close: take this run's marker away.
pub fn end() {
    if cfg!(test) {
        return;
    }
    if let Some(dir) = dir() {
        let _ = std::fs::remove_file(marker(&dir));
    }
}

/// The last `n` crashes in `crashes.log`, for a problem report.
pub fn last(n: usize) -> Option<String> {
    let t = std::fs::read_to_string(dir()?.join("crashes.log")).ok()?;
    Some(last_of(&t, n))
}

fn last_of(t: &str, n: usize) -> String {
    let starts: Vec<usize> = t.match_indices(HEAD).map(|(i, _)| i).collect();
    let from = starts.len().saturating_sub(n);
    starts
        .get(from)
        .map_or(String::new(), |&i| t[i..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("creabim-crashlog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn crashes_are_added_never_replaced() {
        let d = tmp("add");
        record_in(&d, "1.1.1 (build a)", 0, "first", &["before".into()]);
        record_in(&d, "1.1.1 (build b)", 86_400, "second", &[]);
        let t = std::fs::read_to_string(d.join("crashes.log")).unwrap();
        assert!(
            t.contains("1970-01-01 00:00:00 UTC") && t.contains("first") && t.contains("before")
        );
        assert!(t.contains("1970-01-02 00:00:00 UTC") && t.contains("second"));
        assert_eq!(last_of(&t, 1).matches(HEAD).count(), 1);
        assert!(last_of(&t, 1).contains("second"));
        assert_eq!(last_of(&t, 20).matches(HEAD).count(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_marker_left_by_a_dead_run_is_a_session_that_never_closed() {
        let d = tmp("dead");
        // no process has this number
        std::fs::write(
            d.join("running-4000000000"),
            "4000000000\n100\n1.1.1 (build x)\n",
        )
        .unwrap();
        let (ended, open) = sweep(&d);
        assert_eq!(
            ended,
            vec![(4_000_000_000, "1.1.1 (build x)".to_string(), 100)]
        );
        assert!(open.is_empty());
        assert!(!d.join("running-4000000000").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_marker_of_a_run_still_open_is_left_alone() {
        let d = tmp("open");
        let me = std::process::id();
        std::fs::write(marker(&d), format!("{me}\n100\n1.1.1\n")).unwrap();
        let (ended, open) = sweep(&d);
        assert!(ended.is_empty());
        assert_eq!(open, vec![me]);
        assert!(marker(&d).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dates_read_as_a_calendar() {
        assert_eq!(when(1_794_727_771), "2026-11-15 07:29:31 UTC");
    }
}
