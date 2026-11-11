//! Bulk actions in the background (roadmap 87). A bulk dialog of
//! `[ui] background_batch_from` items or more closes on submit and hands its
//! [`OsAction::Batch`] here: the items are sent in order, [`CHUNK`] at a time,
//! each chunk an ordinary `run_action(OsAction::Batch(chunk))`, so every server
//! check (`check_ids`, `admin_only`, plugins, identity safety, `[writes] mode`)
//! runs exactly as for a dialog. A card in [`JobsTray`] counts the items done.
//! There is no job state on the server: closing the tab just stops sending.
//! Horizon has nothing like it (one request per row, then a reload).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::run_action;
use crate::components::dialog::RequestList;
use crate::i18n::{K, job_count, job_expired, job_summary, use_i18n};
use crate::model::{ActionOutcome, AppError, ErrorKind, OsAction, RequestPreview, WriteMode};

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
    pub id: u64,
    pub title: String,
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
}

impl Jobs {
    pub fn provide() -> Self {
        let j = Jobs {
            list: RwSignal::new(vec![]),
            next: StoredValue::new(0),
        };
        provide_context(j);
        j
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut Job)) {
        self.list.try_update(|l| l.iter_mut().find(|j| j.id == id).map(f));
    }

    fn dismiss(&self, id: u64) {
        self.list.try_update(|l| l.retain(|j| j.id != id));
    }

    /// Starts sending `items` and shows a card for them. `version` is bumped after
    /// each chunk sent for real, so the table behind refreshes as rows go.
    pub fn start(&self, title: String, items: Vec<(String, OsAction)>, version: RwSignal<u64>) {
        let id = self.next.get_value();
        self.next.set_value(id + 1);
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
}

/// The cards of every job, fixed at the bottom-right corner (rendered once in
/// the layout). A screen reader hears the progress (`aria-live="polite"`).
#[component]
pub fn JobsTray() -> impl IntoView {
    let jobs = use_context::<Jobs>().expect("Jobs::provide in the layout");
    move || {
        let list = jobs.list.get();
        (!list.is_empty()).then(|| view! {
            <div class="jobs-tray">{list.into_iter().map(|j| view! { <JobCard job=j jobs=jobs/> }).collect_view()}</div>
        })
    }
}

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
