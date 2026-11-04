//! Where the server log goes (`[log]`, roadmap 81): stdout (systemd's journal,
//! containers) and/or a file rotated by `tracing-appender`, as text or JSON.
//! Built once in `main.rs`, right after the settings are read, so the start-up
//! lines reach every output; a log file that can't be written stops start-up
//! like any other bad setting. The file holds usernames and client addresses,
//! never tokens (AGENTS.md §1.5), so it is created without access for others.
//! Old files are deleted here, not by `tracing-appender`: its pruning deletes the
//! current file on every start when it may keep only one (roadmap 84).

use std::path::Path;
use std::time::Duration;

use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{self, RollingFileAppender};
use tracing_subscriber::fmt::{self, MakeWriter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

use super::settings::{Log, LogFormat, Rotation};

type BoxedLayer = Box<dyn Layer<Registry> + Send + Sync>;

/// The subscriber `[log]` asks for, and the guard of the file's background
/// writer (`None` without a file): keep it alive until exit, dropping it flushes
/// the last lines. Fails with a `[log] file …` message when the file can't be
/// opened or its directory doesn't exist.
pub fn subscriber(log: &Log) -> Result<(impl Subscriber + Send + Sync + use<>, Option<WorkerGuard>), String> {
    let filter = EnvFilter::try_new(&log.filter).unwrap_or_else(|_| EnvFilter::new("moonlight=info,warn"));
    let mut layers: Vec<BoxedLayer> = Vec::new();
    let mut guard = None;
    if let Some(path) = log.file() {
        // Lines are queued to a writer thread so a slow disk never stalls a request.
        let (writer, g) = tracing_appender::non_blocking(appender(path, log)?);
        layers.push(layer(log.format, false, writer));
        guard = Some(g);
    }
    if log.file().is_none() || log.stdout {
        layers.push(layer(log.format, true, std::io::stdout));
    }
    Ok((tracing_subscriber::registry().with(layers).with(filter), guard))
}

/// One output; ANSI colours only on a terminal-bound stream, never in a file.
fn layer<W>(format: LogFormat, ansi: bool, writer: W) -> BoxedLayer
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let l = fmt::layer().with_ansi(ansi).with_writer(writer);
    match format {
        LogFormat::Text => l.boxed(),
        LogFormat::Json => l.json().boxed(),
    }
}

/// How often old log files are looked for: rotations are hourly at most.
const PRUNE_EVERY: Duration = Duration::from_secs(3600);

/// The rotating file: `moonlight.log` with `never`, else `moonlight.log.<date>`
/// (`2026-10-29`, hourly `2026-10-29-13`). A restart appends to the current
/// file unless `[log] keep = no`; files beyond `[log] max_files` are deleted at
/// start-up and then every hour by [`prune`].
fn appender(path: &Path, log: &Log) -> Result<RollingFileAppender, String> {
    let err = |e: String| format!("[log] file {}: {e}", path.display());
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| err("not a file name".into()))?;
    // `tracing-appender` would create a missing directory; a typo should stop start-up instead.
    if !dir.is_dir() {
        return Err(err(format!("directory {} does not exist", dir.display())));
    }
    restrict_umask();
    let rotation = match log.rotation {
        Rotation::Never => rolling::Rotation::NEVER,
        Rotation::Hourly => rolling::Rotation::HOURLY,
        Rotation::Daily => rolling::Rotation::DAILY,
    };
    if !log.keep_current() {
        let current = dir.join(current_name(name, log.rotation, chrono::Utc::now()));
        match std::fs::remove_file(&current) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(err(format!("can't start {} empty: {e}", current.display()))),
            _ => {}
        }
    }
    // No `max_log_files`: `tracing-appender` prunes before opening the current
    // file and keeps `max - 1`, so with 1 it deletes the file it is about to
    // append to, on every start (0.2.5, `prune_old_logs`; roadmap 84).
    let appender = rolling::Builder::new().rotation(rotation).filename_prefix(name).build(dir).map_err(|e| err(chain(&e)))?;
    let max = log.max_files() as usize;
    if max > 0 {
        let (dir, name) = (dir.to_path_buf(), name.to_string());
        prune(&dir, &name, max);
        std::thread::Builder::new()
            .name("log-prune".into())
            .spawn(move || loop {
                std::thread::sleep(PRUNE_EVERY);
                prune(&dir, &name, max);
            })
            .map_err(|e| err(format!("can't start the pruning thread: {e}")))?;
    }
    Ok(appender)
}

/// The file `tracing-appender` writes now: the prefix alone with `never`, else
/// the prefix and the UTC period (its own date formats).
fn current_name(prefix: &str, rotation: Rotation, now: chrono::DateTime<chrono::Utc>) -> String {
    match rotation {
        Rotation::Never => prefix.to_string(),
        Rotation::Daily => format!("{prefix}.{}", now.format("%Y-%m-%d")),
        Rotation::Hourly => format!("{prefix}.{}", now.format("%Y-%m-%d-%H")),
    }
}

/// Deletes the oldest log files of `prefix` in `dir` so that `max` remain, the
/// current one included. The date suffixes sort by time, so the newest name is
/// the current file and is never deleted (`max` ≥ 1). Failures are reported on
/// stderr: the log itself may be what is failing.
fn prune(dir: &Path, prefix: &str, max: usize) {
    let dotted = format!("{prefix}.");
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n == prefix || n.starts_with(&dotted))
        .collect();
    names.sort();
    let extra = names.len().saturating_sub(max);
    for n in &names[..extra] {
        if let Err(e) = std::fs::remove_file(dir.join(n)) {
            eprintln!("[log] could not delete the old log file {n}: {e}");
        }
    }
}

/// `tracing-appender` opens each rotated file with the process umask and no
/// mode of its own, so the umask is tightened instead: from here on files are
/// created at most `0640`. Moonlight writes no other files (tokens never touch
/// disk, AGENTS.md §1.6).
fn restrict_umask() {
    use rustix::fs::Mode;
    let no_group_write_no_other = Mode::WGRP | Mode::RWXO;
    let old = rustix::process::umask(no_group_write_no_other);
    rustix::process::umask(old | no_group_write_no_other);
}

/// An error and its causes, `InitError` alone only says "failed to create log file".
fn chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        out.push_str(&format!(": {c}"));
        cause = c.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::settings::KeepSetting;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A fresh directory under the system temp dir, removed on drop.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("moonlight-logging-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Logs one line through the subscriber for `log`, drops the guard, returns the files written.
    fn log_one(log: &Log, dir: &Path) -> Vec<(String, String, u32)> {
        let (sub, guard) = subscriber(log).unwrap();
        tracing::subscriber::with_default(sub, || tracing::info!(user = "alice", "signed in"));
        drop(guard);
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let mode = std::fs::metadata(&p).unwrap().permissions().mode();
                (p.file_name().unwrap().to_str().unwrap().to_string(), std::fs::read_to_string(&p).unwrap(), mode)
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn a_log_file_gets_the_lines_without_colours_and_unreadable_by_others() {
        let t = Tmp::new("text");
        let log = Log { file: Some(t.0.join("moonlight.log").display().to_string()), rotation: Rotation::Never, ..Log::default() };
        let files = log_one(&log, &t.0);
        let [(name, text, mode)] = files.as_slice() else { panic!("one file expected: {files:?}") };
        assert_eq!(name, "moonlight.log", "`never` writes the configured name");
        assert!(text.contains("signed in") && text.contains("user=\"alice\""), "{text}");
        assert!(!text.contains('\x1b'), "no ANSI colours in the file: {text:?}");
        assert_eq!(mode & 0o027, 0, "no group write, nothing for others (mode {mode:o})");
    }

    #[test]
    fn rotated_files_carry_the_date_and_json_is_one_object_per_line() {
        let t = Tmp::new("json");
        let log = Log { file: Some(t.0.join("m.log").display().to_string()), format: LogFormat::Json, ..Log::default() };
        let files = log_one(&log, &t.0);
        let [(name, text, _)] = files.as_slice() else { panic!("one file expected: {files:?}") };
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, &format!("m.log.{date}"), "daily rotation by default");
        let line: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!((line["fields"]["message"].as_str(), line["fields"]["user"].as_str()), (Some("signed in"), Some("alice")));
    }

    #[test]
    fn a_missing_log_directory_stops_start_up() {
        let t = Tmp::new("missing");
        let log = Log { file: Some(t.0.join("nope/moonlight.log").display().to_string()), ..Log::default() };
        let err = subscriber(&log).err().unwrap();
        assert!(err.starts_with("[log] file ") && err.contains("does not exist"), "{err}");
        assert!(!t.0.join("nope").exists(), "the directory is not created");
    }

    /// Writes `line` through a fresh appender for `log`, as one run of the server would.
    fn run(log: &Log, line: &str) {
        use std::io::Write;
        let mut a = appender(Path::new(log.file.as_deref().unwrap()), log).unwrap();
        writeln!(a, "{line}").unwrap();
        a.flush().unwrap();
    }

    fn read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).unwrap_or_default()
    }

    #[test]
    fn a_restart_keeps_the_current_file_unless_keep_is_off() {
        // Roadmap 84: with one file allowed, tracing-appender deleted the current
        // file on every start; the log then only had lines since the last restart.
        let today = current_name("m.log", Rotation::Daily, chrono::Utc::now());
        for rotation in [Rotation::Daily, Rotation::Never] {
            let t = Tmp::new(&format!("restart-{rotation:?}"));
            let name = if rotation == Rotation::Never { "m.log".to_string() } else { today.clone() };
            let log = Log { file: Some(t.0.join("m.log").display().to_string()), rotation, max_files: Some(1), ..Log::default() };
            run(&log, "before restart");
            run(&log, "after restart");
            assert_eq!(read(&t.0, &name), "before restart\nafter restart\n", "{rotation:?}, keep = yes, max_files = 1");
            let fresh = Log { keep: KeepSetting::Flag(false), ..log };
            run(&fresh, "fresh start");
            assert_eq!(read(&t.0, &name), "fresh start\n", "{rotation:?}, keep = no: starts empty");
        }
    }

    #[test]
    fn pruning_keeps_the_newest_files_and_ignores_others() {
        let t = Tmp::new("prune");
        for n in ["m.log.2026-10-27", "m.log.2026-10-28", "m.log.2026-10-29", "m.log.2026-10-30", "other.log", "m.logs"] {
            std::fs::write(t.0.join(n), "x").unwrap();
        }
        prune(&t.0, "m.log", 2);
        let mut left: Vec<_> = std::fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, ["m.log.2026-10-29", "m.log.2026-10-30", "m.logs", "other.log"], "newest two of this log; other files untouched");
        assert_eq!(current_name("m.log", Rotation::Hourly, chrono::DateTime::from_timestamp(0, 0).unwrap()), "m.log.1970-01-01-00");
    }
}
