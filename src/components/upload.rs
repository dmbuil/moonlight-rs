//! Browser side of *Create Image* › *File* (roadmap 45): the picked file, its disk format
//! guessed from the name, and the `PUT` with progress and cancel.
//!
//! The dialog's values are strings, so the picked [`web_sys::File`] lives in a thread-local
//! here and the field's value only says `{name, size}`. The bytes are never read in the page:
//! the browser streams the `File` itself. Nothing here talks to OpenStack except through the
//! URL the caller gives (Moonlight's proxy route, or Glance with a ticket).

use leptos::prelude::{Callable, Callback};
use serde::{Deserialize, Serialize};

/// What the *File* field's value holds: the picked file's name and size in bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Picked {
    pub name: String,
    pub size: u64,
}

impl Picked {
    /// The field value for this file.
    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The file a field value describes; `None` when nothing is picked.
    pub fn decode(value: &str) -> Option<Picked> {
        serde_json::from_str(value).ok()
    }
}

/// The disk format a file name suggests (Horizon preselects it too), among `offered`.
pub fn guess_format(name: &str, offered: &[String]) -> Option<String> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let format = match ext.as_str() {
        "qcow2" | "qcow" => "qcow2",
        "img" | "raw" => "raw",
        "iso" => "iso",
        "vmdk" => "vmdk",
        "vdi" => "vdi",
        "vhd" | "vhdx" => "vhd",
        "aki" | "ari" | "ami" => ext.as_str(),
        _ => return None,
    };
    offered.iter().any(|f| f == format).then(|| format.to_string())
}

#[cfg(feature = "hydrate")]
thread_local! {
    /// The file the user picked in the open dialog.
    static PICKED: std::cell::RefCell<Option<web_sys::File>> = const { std::cell::RefCell::new(None) };
}

/// Remembers the picked file (the page's one dialog is open at a time).
#[cfg(feature = "hydrate")]
pub fn remember(file: Option<web_sys::File>) {
    PICKED.with(|p| *p.borrow_mut() = file);
}

/// An upload in progress; [`Upload::abort`] cancels it.
#[derive(Clone, Default)]
pub struct Upload {
    xhr: Option<web_sys::XmlHttpRequest>,
}

impl Upload {
    /// Stops the transfer; the callback then gets an error saying it was cancelled.
    pub fn abort(&self) {
        if let Some(x) = &self.xhr {
            let _ = x.abort();
        }
    }
}

/// How an upload ends: `Ok`, or the reason (a status and Glance's or Moonlight's text).
pub type Outcome = Result<(), String>;

/// Sends the remembered file to `url` with `PUT` (Glance wants `application/octet-stream`),
/// adding `X-Auth-Token` when `token` is set (direct mode). `progress` gets `(sent, total)`
/// bytes; `done` runs once. Returns the handle to cancel it.
#[cfg(feature = "hydrate")]
pub fn start(url: &str, token: Option<&str>, progress: Callback<(f64, f64)>, done: Callback<Outcome>) -> Upload {
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{ProgressEvent, XmlHttpRequest};

    let fail = |msg: &str| {
        done.run(Err(msg.to_string()));
        Upload::default()
    };
    let Some(file) = PICKED.with(|p| p.borrow().clone()) else { return fail("no file picked") };
    let Ok(xhr) = XmlHttpRequest::new() else { return fail("this browser can't upload files") };
    if xhr.open_with_async("PUT", url, true).is_err() {
        return fail("invalid upload address");
    }
    let _ = xhr.set_request_header("Content-Type", "application/octet-stream");
    if let Some(t) = token {
        let _ = xhr.set_request_header("X-Auth-Token", t);
    }

    let on_progress = Closure::<dyn FnMut(ProgressEvent)>::new(move |e: ProgressEvent| {
        if e.length_computable() {
            progress.run((e.loaded(), e.total()));
        }
    });
    if let Ok(up) = xhr.upload() {
        up.set_onprogress(Some(on_progress.as_ref().unchecked_ref()));
    }
    let x = xhr.clone();
    let on_load = Closure::<dyn FnMut()>::new(move || {
        let status = x.status().unwrap_or(0);
        if (200..300).contains(&status) {
            done.run(Ok(()));
        } else {
            let text = x.response_text().ok().flatten().unwrap_or_default();
            done.run(Err(format!("{status} {}", text.chars().take(300).collect::<String>()).trim().to_string()));
        }
    });
    xhr.set_onload(Some(on_load.as_ref().unchecked_ref()));
    // A refused CORS preflight (direct mode) and a dropped connection both look like this.
    let on_error = Closure::<dyn FnMut()>::new(move || done.run(Err("the connection failed".into())));
    xhr.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    let on_abort = Closure::<dyn FnMut()>::new(move || done.run(Err("cancelled".into())));
    xhr.set_onabort(Some(on_abort.as_ref().unchecked_ref()));
    // The request owns the handlers until it ends; one upload per dialog, so a few bytes leak.
    on_progress.forget();
    on_load.forget();
    on_error.forget();
    on_abort.forget();

    match xhr.send_with_opt_blob(Some(&file)) {
        Ok(()) => Upload { xhr: Some(xhr) },
        Err(_) => fail("the upload could not start"),
    }
}

/// Server render: nothing uploads.
#[cfg(not(feature = "hydrate"))]
pub fn start(_url: &str, _token: Option<&str>, _progress: Callback<(f64, f64)>, done: Callback<Outcome>) -> Upload {
    done.run(Err("uploads run in the browser".into()));
    Upload::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formats() -> Vec<String> {
        ["raw", "qcow2", "iso", "vhd"].map(String::from).to_vec()
    }

    #[test]
    fn the_extension_picks_the_disk_format_when_offered() {
        let g = |n| guess_format(n, &formats());
        assert_eq!(g("debian-12.QCOW2").as_deref(), Some("qcow2"));
        assert_eq!(g("disk.img").as_deref(), Some("raw"));
        assert_eq!(g("win.vhdx").as_deref(), Some("vhd"));
        assert_eq!(g("notes.txt"), None);
        assert_eq!(g("noextension"), None);
        assert_eq!(g("disk.vmdk"), None, "vmdk is not in the offered formats");
    }

    #[test]
    fn the_field_value_roundtrips() {
        let p = Picked { name: "a.iso".into(), size: 7 };
        assert_eq!(Picked::decode(&p.encode()), Some(p));
        assert_eq!(Picked::decode(""), None, "nothing picked");
    }
}
