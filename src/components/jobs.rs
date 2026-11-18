//! Bulk actions in the background. A bulk dialog of
//! `[ui] background_batch_from` items or more closes on submit and hands its
//! [`OsAction::Batch`] here: the items are sent in order, [`CHUNK`] at a time,
//! each chunk an ordinary `run_action(OsAction::Batch(chunk))`, so every server
//! check (`check_ids`, `admin_only`, plugins, identity safety, `[writes] mode`)
//! runs exactly as for a dialog. A card in [`JobsTray`] counts the items done.
//! There is no job state on the server: closing the tab just stops sending.
//! Horizon has nothing like it (one request per row, then a reload).
//!
//! *Create Image* › *File* uploads run here too ([`Transfer`]): the dialog closes once the
//! image record exists, and a card shows the bytes sent, the speed and the time left, with
//! *Cancel upload*; a failed or cancelled upload offers *Delete Image* for the `queued` record.
//! While one is sending, leaving or reloading the page asks first (`beforeunload`), as the
//! browser would drop the transfer.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::run_action;
use crate::components::dialog::RequestList;
use crate::components::upload::{self, FileHandle, Upload};
use crate::i18n::{K, bytes, job_count, job_expired, job_summary, upload_rate, use_i18n};
use crate::model::{ActionOutcome, AppError, ErrorKind, OsAction, RequestPreview, WriteMode};
use crate::prefs::ImageUpload;

/// Items sent per `run_action` call: small enough for the counter to move
/// every few seconds, large enough that the per-request session rescope
/// (about 0.1 s) doesn't add up.
pub const CHUNK: usize = 5;

/// How long a card that finished without failures stays before fading out.
const FADE_AFTER: std::time::Duration = std::time::Duration::from_secs(6);

/// Whether `action` should run in the background: a batch of `from` items or
/// more, `from` > 0 (`0` keeps every batch in its dialog). "From" includes
/// `from` itself, as the setting's name says.
pub fn in_background(action: &OsAction, from: u32) -> bool {
    from > 0 && matches!(action, OsAction::Batch(items) if items.len() >= from as usize)
}

/// The batch's items in order, [`CHUNK`] per call.
pub fn chunks(items: Vec<(String, OsAction)>) -> Vec<Vec<(String, OsAction)>> {
    let mut out = Vec::new();
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        out.push(it.by_ref().take(CHUNK).collect());
    }
    out
}

/// Where a job is.
#[derive(Clone, Debug, PartialEq)]
pub enum JobState {
    /// Chunks are still being sent.
    Running,
    /// Every item was sent (each may still have failed: [`Job::failed`]).
    Finished,
    /// The token expired mid-way: the rest was not sent.
    Expired,
    /// A whole chunk was refused (writes off, forbidden, invalid): the rest was
    /// not sent, as it would be refused the same way.
    Stopped(String),
}

/// One bulk action running in the background.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// Unique in the tray, from [`Jobs`]' counter.
    pub id: u64,
    /// The dialog's title (*Delete Instances*), the card's heading.
    pub title: String,
    /// How many items were picked.
    pub total: usize,
    /// Items answered so far, failed ones included.
    pub done: usize,
    /// `(label, OpenStack's error)` of each failed item, in order.
    pub failed: Vec<(String, String)>,
    /// Some chunk answered in dry-run: nothing was sent.
    pub dry_run: bool,
    /// The requests built, when the server shows them (`[ui] show_requests`).
    pub requests: Vec<RequestPreview>,
    pub state: JobState,
}

impl Job {
    /// A job that has just started: nothing answered yet.
    pub fn new(id: u64, title: String, total: usize) -> Self {
        Job {
            id,
            title,
            total,
            done: 0,
            failed: vec![],
            dry_run: false,
            requests: vec![],
            state: JobState::Running,
        }
    }

    /// Records the answer to a chunk of `sent` items; `false` stops the job.
    pub fn record(&mut self, sent: usize, answer: Result<ActionOutcome, AppError>) -> bool {
        match answer {
            Ok(out) => {
                self.done += sent;
                self.dry_run |= out.mode == WriteMode::DryRun;
                self.failed
                    .extend(out.results.into_iter().filter_map(|r| Some((r.label, r.error?))));
                self.requests.extend(out.requests);
                if self.done >= self.total {
                    self.state = JobState::Finished;
                }
                true
            }
            Err(e) if e.kind == ErrorKind::Unauthenticated => {
                self.state = JobState::Expired;
                false
            }
            Err(e) => {
                self.state = JobState::Stopped(e.to_string());
                false
            }
        }
    }

    /// Items that went through.
    pub fn succeeded(&self) -> usize {
        self.done - self.failed.len()
    }

    /// Ended well enough to fade out on its own: all sent for real, no failure.
    pub fn fades(&self) -> bool {
        self.state == JobState::Finished && self.failed.is_empty() && !self.dry_run
    }
}

/// The running and finished jobs; provided once in the layout, next to `Dialogs`.
#[derive(Clone, Copy)]
pub struct Jobs {
    list: RwSignal<Vec<Job>>,
    next: StoredValue<u64>,
    /// Image uploads, shown above the batch jobs.
    transfers: RwSignal<Vec<Transfer>>,
}

impl Jobs {
    /// Creates the tray state and provides it as context (once, in the layout).
    pub fn provide() -> Self {
        let j = Jobs {
            list: RwSignal::new(vec![]),
            next: StoredValue::new(0),
            transfers: RwSignal::new(vec![]),
        };
        provide_context(j);
        j
    }

    /// Applies `f` to job `id` if it is still there; `try_update`, as the tray may be gone
    /// (the spawned sender outlives a navigation away, see AGENTS.md §6 on disposed signals).
    fn update(&self, id: u64, f: impl FnOnce(&mut Job)) {
        self.list.try_update(|l| l.iter_mut().find(|j| j.id == id).map(f));
    }

    /// Removes the card of job `id` (its close button, or the fade of a clean finish).
    fn dismiss(&self, id: u64) {
        self.list.try_update(|l| l.retain(|j| j.id != id));
    }

    /// A fresh ID for a card.
    fn next_id(&self) -> u64 {
        let id = self.next.get_value();
        self.next.set_value(id + 1);
        id
    }

    /// Starts sending `items` and shows a card for them. `version` is bumped after
    /// each chunk sent for real, so the table behind refreshes as rows go.
    pub fn start(&self, title: String, items: Vec<(String, OsAction)>, version: RwSignal<u64>) {
        let id = self.next_id();
        self.list.update(|l| l.push(Job::new(id, title, items.len())));
        let jobs = *self;
        spawn_local(async move {
            for chunk in chunks(items) {
                let sent = chunk.len();
                let answer = run_action(OsAction::Batch(chunk)).await;
                let real = answer.as_ref().is_ok_and(|o| o.mode == WriteMode::On);
                let mut go_on = false;
                jobs.update(id, |j| go_on = j.record(sent, answer));
                if real {
                    version.try_update(|v| *v += 1);
                }
                if !go_on {
                    break;
                }
            }
            let fades = jobs
                .list
                .try_with_untracked(|l| l.iter().any(|j| j.id == id && j.fades()))
                .unwrap_or(false);
            if fades {
                set_timeout(move || jobs.dismiss(id), FADE_AFTER);
            }
        });
    }

    /// Applies `f` to transfer `id` if its card is still there.
    fn update_transfer(&self, id: u64, f: impl FnOnce(&mut Transfer)) {
        self.transfers.try_update(|l| l.iter_mut().find(|t| t.id == id).map(f));
    }

    /// Removes the card of transfer `id`.
    fn dismiss_transfer(&self, id: u64) {
        self.transfers.try_update(|l| l.retain(|t| t.id != id));
    }

    /// Cancels transfer `id`; its card then shows the upload as not finished.
    fn cancel(&self, id: u64) {
        // Cloned out first: `abort` runs `done` at once, which edits the list.
        let mine: Vec<Upload> =
            HANDLES.with_borrow(|h| h.iter().filter(|(i, _)| *i == id).map(|(_, u)| u.clone()).collect());
        mine.iter().for_each(Upload::abort);
    }

    /// Whether some upload is still sending (leaving the page would stop it).
    pub fn sending(&self) -> bool {
        self.transfers
            .try_with_untracked(|l| l.iter().any(|t| t.state == TransferState::Sending))
            .unwrap_or(false)
    }

    /// How far the upload into image `image_id` is, `0..=100`, while it sends (the Images
    /// list's status). Tracks the transfers.
    pub fn upload_percent(&self, image_id: &str) -> Option<f64> {
        self.transfers.with(|l| {
            l.iter()
                .find(|t| t.image_id == image_id && t.state == TransferState::Sending)
                .map(Transfer::percent)
        })
    }

    /// Sends `file` (`size` bytes) into the image `image_id` that now exists, called `name`,
    /// and shows a card for it. `proxy` goes through Moonlight's route; `direct` asks the
    /// server for a ticket first and goes to Glance itself. `version` is bumped when the data
    /// arrived, so the Images list shows the image going `active`.
    pub fn upload(
        &self,
        image_id: String,
        name: String,
        file: FileHandle,
        size: u64,
        mode: ImageUpload,
        version: RwSignal<u64>,
    ) {
        let id = self.next_id();
        let now = now_ms();
        self.transfers
            .update(|l| l.insert(0, Transfer::new(id, image_id.clone(), name, size, now)));
        let jobs = *self;
        spawn_local(async move {
            let (url, token) = match mode {
                ImageUpload::Direct => match crate::api::get_image_upload_ticket(image_id.clone(), size).await {
                    Ok(t) => (t.url, Some(t.token)),
                    Err(e) => return jobs.update_transfer(id, |t| t.end(Err(e.to_string()), now_ms())),
                },
                _ => (crate::model::image_upload_path(&image_id), None),
            };
            let progress = Callback::new(move |(sent, total): (f64, f64)| {
                jobs.update_transfer(id, |t| t.advance(sent, total, now_ms()))
            });
            let done = Callback::new(move |r: upload::Outcome| {
                HANDLES.with_borrow_mut(|h| h.retain(|(i, _)| *i != id));
                let ok = r.is_ok();
                jobs.update_transfer(id, |t| t.end(r, now_ms()));
                if ok {
                    version.try_update(|v| *v += 1);
                    set_timeout(move || jobs.dismiss_transfer(id), FADE_AFTER);
                }
            });
            let handle = upload::start(file, &url, token.as_deref(), progress, done);
            // `start` may already have failed (and called `done`): keep only a live request.
            if jobs.sending_id(id) {
                HANDLES.with_borrow_mut(|h| h.push((id, handle)));
            }
        });
    }

    /// Whether transfer `id` is still sending.
    fn sending_id(&self, id: u64) -> bool {
        self.transfers
            .try_with_untracked(|l| l.iter().any(|t| t.id == id && t.state == TransferState::Sending))
            .unwrap_or(false)
    }

    /// Deletes the `queued` image a failed transfer left, then removes its card.
    fn delete_image(&self, id: u64, image_id: String, version: RwSignal<u64>) {
        let jobs = *self;
        jobs.update_transfer(id, |t| t.state = TransferState::Deleting);
        spawn_local(async move {
            match crate::api::run_action(OsAction::ImageDelete { id: image_id }).await {
                Ok(_) => {
                    version.try_update(|v| *v += 1);
                    jobs.dismiss_transfer(id);
                }
                Err(e) => jobs.update_transfer(id, |t| t.state = TransferState::Failed(e.to_string())),
            }
        });
    }
}

thread_local! {
    /// The request of each transfer still sending, by [`Transfer::id`], to cancel it. A
    /// thread-local, not a `StoredValue::new_local` in [`Jobs`]: the layout also renders on the
    /// server, where a local value made on one worker thread and dropped on another panics.
    /// Only the browser ever fills it.
    static HANDLES: std::cell::RefCell<Vec<(u64, Upload)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Milliseconds since the epoch (the browser's clock in the page).
fn now_ms() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64
}

/// Where an image upload is.
#[derive(Clone, Debug, PartialEq)]
pub enum TransferState {
    /// Bytes are on the way.
    Sending,
    /// The image service has the data.
    Done,
    /// It stopped (cancelled, refused, connection lost): the reason. The image stays `queued`.
    Failed(String),
    /// *Delete Image* was pressed on a failed upload and is running.
    Deleting,
}

/// One *Create Image* › *File* upload in the background.
#[derive(Clone, Debug, PartialEq)]
pub struct Transfer {
    /// Unique in the tray, from [`Jobs`]' counter (shared with batch jobs).
    pub id: u64,
    /// The image the data goes into.
    pub image_id: String,
    /// The image's name, the card's heading.
    pub name: String,
    pub sent: f64,
    pub total: f64,
    /// When it started and when the last progress came, in ms (speed and time left).
    pub started: f64,
    pub at: f64,
    pub state: TransferState,
}

impl Transfer {
    /// An upload of `total` bytes that has just started at `now` ms.
    pub fn new(id: u64, image_id: String, name: String, total: u64, now: f64) -> Self {
        Transfer {
            id,
            image_id,
            name,
            sent: 0.0,
            total: total as f64,
            started: now,
            at: now,
            state: TransferState::Sending,
        }
    }

    /// Records the browser's progress event: `sent` of `total` bytes at `now` ms.
    pub fn advance(&mut self, sent: f64, total: f64, now: f64) {
        if self.state == TransferState::Sending {
            (self.sent, self.at) = (sent, now);
            if total > 0.0 {
                self.total = total;
            }
        }
    }

    /// Records how it ended at `now` ms.
    pub fn end(&mut self, outcome: upload::Outcome, now: f64) {
        self.at = now;
        self.state = match outcome {
            Ok(()) => {
                self.sent = self.total;
                TransferState::Done
            }
            Err(e) => TransferState::Failed(e),
        };
    }

    /// Share sent, `0..=100`.
    pub fn percent(&self) -> f64 {
        if self.total > 0.0 {
            (self.sent / self.total * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    /// Average speed in bytes per second and seconds left, once a second has gone by and
    /// something was sent (before that the numbers jump around).
    pub fn rate(&self) -> Option<(f64, f64)> {
        let secs = (self.at - self.started) / 1000.0;
        (secs >= 1.0 && self.sent > 0.0).then(|| {
            let per_s = self.sent / secs;
            (per_s, ((self.total - self.sent) / per_s).max(0.0))
        })
    }
}

/// The cards of every job, fixed at the bottom-right corner (rendered once in
/// the layout). A screen reader hears the progress (`aria-live="polite"`).
#[component]
pub fn JobsTray() -> impl IntoView {
    let jobs = use_context::<Jobs>().expect("Jobs::provide in the layout");
    // Leaving the page while an upload sends would drop it: let the browser ask first.
    #[cfg(feature = "hydrate")]
    {
        let handle = window_event_listener(leptos::ev::beforeunload, move |ev| {
            if jobs.sending() {
                ev.prevent_default();
                ev.set_return_value("");
            }
        });
        on_cleanup(move || handle.remove());
    }
    let version = use_context::<crate::components::dialog::Dialogs>()
        .map(|d| d.version)
        .unwrap_or_else(|| RwSignal::new(0));
    move || {
        let (list, transfers) = (jobs.list.get(), jobs.transfers.get());
        (!list.is_empty() || !transfers.is_empty()).then(|| view! {
            <div class="jobs-tray">
                {transfers.into_iter().map(|t| view! { <TransferCard t=t jobs=jobs version=version/> }).collect_view()}
                {list.into_iter().map(|j| view! { <JobCard job=j jobs=jobs/> }).collect_view()}
            </div>
        })
    }
}

/// One upload's card: the image name, a progress bar, `58% · 1.2 GB / 2.1 GB`, the speed and
/// time left, and *Cancel upload*; then *Uploaded*, or why it stopped with *Delete Image*.
#[component]
fn TransferCard(t: Transfer, jobs: Jobs, version: RwSignal<u64>) -> impl IntoView {
    let i18n = use_i18n();
    let id = t.id;
    let pct = t.percent();
    let counts = format!("{pct:.0}%  ·  {} / {}", bytes(t.sent as u64), bytes(t.total as u64));
    let rate = t.rate();
    let (class, body) = match t.state.clone() {
        TransferState::Sending => (
            "job-card job-running",
            view! {
                <progress max="100" value=pct></progress>
                <span class="job-count">{counts}</span>
                {rate.map(|(per_s, left)| view! { <span class="job-count muted">{move || upload_rate(i18n.lang(), per_s, left)}</span> })}
                <small class="muted">{move || i18n.t(K::UploadKeepTabOpen)}</small>
                <div class="job-actions">
                    <button type="button" class="btn btn-sm" on:click=move |_| jobs.cancel(id)>{move || i18n.t(K::CancelUpload)}</button>
                </div>
            }
            .into_any(),
        ),
        TransferState::Done => (
            "job-card job-ok",
            view! { <span class="job-count">{move || i18n.t(K::Uploaded)}" · "{bytes(t.total as u64)}</span> }.into_any(),
        ),
        TransferState::Failed(why) => {
            let image_id = t.image_id.clone();
            (
                "job-card job-failed",
                view! {
                    <span class="job-stop">{move || i18n.t(K::UploadFailedTitle)}": "{why}</span>
                    <small class="muted">{move || i18n.t(K::UploadFailedText)}</small>
                    <div class="job-actions">
                        <button type="button" class="btn btn-sm btn-danger"
                            on:click=move |_| jobs.delete_image(id, image_id.clone(), version)>{move || i18n.t(K::DeleteImage)}</button>
                    </div>
                }
                .into_any(),
            )
        }
        TransferState::Deleting => (
            "job-card job-running",
            view! { <span class="job-count muted">{move || i18n.t(K::DeletingImage)}</span> }.into_any(),
        ),
    };
    let closable = !matches!(t.state, TransferState::Sending | TransferState::Deleting);
    view! {
        <section class=class role="status" aria-live="polite">
            <header>
                <span class="job-dot" aria-hidden="true"></span>
                <strong class="job-title" title=t.name.clone()>{t.name.clone()}</strong>
                {closable.then(|| view! {
                    <button class="btn-close" type="button" aria-label=move || i18n.t(K::Close) on:click=move |_| jobs.dismiss_transfer(id)>"×"</button>
                })}
            </header>
            {body}
        </section>
    }
}

/// One job's card in the tray: title, progress bar and counts while running; then the outcome,
/// the failed items with OpenStack's error, and the requests in dry-run.
#[component]
fn JobCard(job: Job, jobs: Jobs) -> impl IntoView {
    let i18n = use_i18n();
    let id = job.id;
    let running = job.state == JobState::Running;
    let (done, total, ok, n_failed) = (job.done, job.total, job.succeeded(), job.failed.len());
    let status = match job.state.clone() {
        JobState::Running => view! {
            <progress max=total value=done></progress>
            <span class="job-count">{move || job_count(i18n.lang(), done, total)}</span>
            <small class="muted">{move || i18n.t(K::JobKeepTabOpen)}</small>
        }
        .into_any(),
        JobState::Finished => {
            view! { <span class="job-count">{move || job_summary(i18n.lang(), ok, n_failed)}</span> }.into_any()
        }
        JobState::Expired => view! {
            <span class="job-stop">{move || job_expired(i18n.lang(), done, total)}</span>
            <a class="btn" href="/login" rel="external">{move || i18n.t(K::SignIn)}</a>
        }
        .into_any(),
        JobState::Stopped(e) => {
            let e = if e.contains("writes are disabled") {
                i18n.t(K::WritesOff).to_string()
            } else {
                e
            };
            view! { <span class="job-stop">{e}</span><span class="job-count">{move || job_count(i18n.lang(), done, total)}</span> }.into_any()
        }
    };
    let dry = job
        .dry_run
        .then(|| view! { <span class="job-dry">{move || i18n.t(K::DryRunTitle)}</span> });
    let failures = (!job.failed.is_empty()).then(|| view! {
        <ul class="job-failures">
            {job.failed.into_iter().map(|(label, e)| view! { <li><span class="strong">{label}</span>": "{e}</li> }).collect_view()}
        </ul>
    });
    let requests = (!job.requests.is_empty()).then(|| {
        view! {
            <details class="job-requests">
                <summary>{move || i18n.t(K::JobRequests)}</summary>
                <RequestList requests=job.requests/>
            </details>
        }
    });
    // The state is a dot before the title (blue running, green done, red failed).
    let class = if n_failed > 0 || matches!(job.state, JobState::Expired | JobState::Stopped(_)) {
        "job-card job-failed"
    } else if running {
        "job-card job-running"
    } else {
        "job-card job-ok"
    };
    view! {
        <section class=class role="status" aria-live="polite">
            <header>
                <span class="job-dot" aria-hidden="true"></span>
                <strong class="job-title">{job.title}</strong>
                {(!running).then(|| view! {
                    <button class="btn-close" type="button" aria-label=move || i18n.t(K::Close) on:click=move |_| jobs.dismiss(id)>"×"</button>
                })}
            </header>
            {status}
            {dry}
            {failures}
            {requests}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ItemResult;

    fn item(i: usize) -> (String, OsAction) {
        (format!("vm-{i}"), OsAction::ServerStart { id: format!("id-{i}") })
    }

    fn outcome(mode: WriteMode, results: &[(&str, Option<&str>)]) -> ActionOutcome {
        let results = results
            .iter()
            .map(|(l, e)| ItemResult {
                label: l.to_string(),
                error: e.map(str::to_string),
            })
            .collect();
        ActionOutcome {
            mode,
            requests: vec![],
            secret: None,
            results,
            created: None,
            cli: vec![],
        }
    }

    #[test]
    fn only_batches_over_the_threshold_run_in_the_background() {
        let batch = |n: usize| OsAction::Batch((0..n).map(item).collect());
        assert!(!in_background(&batch(9), 10));
        assert!(in_background(&batch(10), 10), "`from` includes the number itself");
        assert!(in_background(&batch(11), 10));
        assert!(!in_background(&batch(500), 0), "0 keeps every batch in the dialog");
        assert!(
            !in_background(&OsAction::ServerStart { id: "x".into() }, 1),
            "not a batch"
        );
    }

    #[test]
    fn chunks_keep_the_order_and_hold_at_most_chunk_items() {
        let parts = chunks((0..12).map(item).collect());
        assert_eq!(parts.iter().map(Vec::len).collect::<Vec<_>>(), [5, 5, 2]);
        let labels: Vec<_> = parts.into_iter().flatten().map(|(l, _)| l).collect();
        assert_eq!(labels, (0..12).map(|i| format!("vm-{i}")).collect::<Vec<_>>());
        assert!(chunks(vec![]).is_empty());
    }

    #[test]
    fn a_job_counts_items_failures_and_finishes() {
        let mut j = Job::new(0, "Delete Instances".into(), 7);
        assert!(j.record(
            5,
            Ok(outcome(
                WriteMode::On,
                &[
                    ("a", None),
                    ("b", Some("in use")),
                    ("c", None),
                    ("d", None),
                    ("e", None)
                ]
            ))
        ));
        assert_eq!((j.done, j.state.clone()), (5, JobState::Running));
        assert!(j.record(2, Ok(outcome(WriteMode::On, &[("f", None), ("g", None)]))));
        assert_eq!(
            (j.done, j.succeeded(), j.failed.clone(), j.state.clone()),
            (7, 6, vec![("b".into(), "in use".into())], JobState::Finished)
        );
        assert!(!j.fades(), "a failure keeps the card until dismissed");
    }

    #[test]
    fn a_clean_real_job_fades_and_a_dry_run_one_stays() {
        let mut j = Job::new(0, "t".into(), 1);
        j.record(1, Ok(outcome(WriteMode::On, &[("a", None)])));
        assert!(j.fades());
        let mut d = Job::new(1, "t".into(), 1);
        d.record(1, Ok(outcome(WriteMode::DryRun, &[("a", None)])));
        assert!(d.dry_run && !d.fades(), "dry-run stays: nothing was sent");
    }

    #[test]
    fn a_transfer_counts_bytes_speed_and_time_left() {
        let mut t = Transfer::new(0, "img".into(), "debian".into(), 1000, 10_000.0);
        assert_eq!((t.percent(), t.rate()), (0.0, None), "nothing sent yet");
        t.advance(100.0, 1000.0, 10_500.0);
        assert_eq!(t.rate(), None, "under a second: no speed yet");
        t.advance(250.0, 1000.0, 12_000.0);
        assert_eq!(t.percent(), 25.0);
        assert_eq!(t.rate(), Some((125.0, 6.0)), "250 B in 2 s; 750 B left");
        t.end(Ok(()), 15_000.0);
        assert_eq!((t.state.clone(), t.percent()), (TransferState::Done, 100.0));
        t.advance(10.0, 1000.0, 16_000.0);
        assert_eq!(t.sent, 1000.0, "a late progress event doesn't move a finished upload");
    }

    #[test]
    fn a_cancelled_upload_keeps_its_reason() {
        let mut t = Transfer::new(0, "img".into(), "debian".into(), 1000, 0.0);
        t.advance(400.0, 1000.0, 2_000.0);
        t.end(Err("cancelled".into()), 3_000.0);
        assert_eq!(t.state, TransferState::Failed("cancelled".into()));
        assert_eq!(t.percent(), 40.0, "the bytes sent stay as they were");
    }

    #[test]
    fn an_expired_session_or_a_refused_chunk_stops_the_job() {
        let mut j = Job::new(0, "t".into(), 12);
        j.record(5, Ok(outcome(WriteMode::On, &[("a", None); 5])));
        assert!(!j.record(5, Err(AppError::new(ErrorKind::Unauthenticated, "expired"))));
        assert_eq!(
            (j.done, j.state.clone()),
            (5, JobState::Expired),
            "the chunk that met the expired token is not counted"
        );
        let mut k = Job::new(1, "t".into(), 12);
        assert!(!k.record(5, Err(AppError::invalid("writes are disabled"))));
        assert_eq!(k.state, JobState::Stopped("writes are disabled".into()));
        assert!(!k.fades());
    }
}
