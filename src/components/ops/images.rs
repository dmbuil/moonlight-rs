//! Images, key pairs and server groups.

use super::*;

/// An image's menu: launch, edit, delete.
pub fn image_items(lang: Lang, im: &Image) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if im.status.eq_ignore_ascii_case("active") {
        let (i, src) = (
            im.id.clone(),
            if im.is_snapshot { "instance_snapshot" } else { "image" },
        );
        items.push(MenuItem::dialog(t(lang, K::LaunchFromRow), move || {
            launch_from(lang, src, &i)
        }));
    }
    items.push(create_volume_item(
        lang,
        VolumeSource::Image,
        &im.id,
        im.size.map(|b| b.div_ceil(1 << 30).max(1) as u32),
    ));
    if im.owned_by_project {
        items.extend(image_owner_items(lang, im, needs::PUBLICIZE_IMAGE.met(&caps())));
    }
    items
}

/// What an image's owner can do: *Edit Image*, *Update Metadata* and *Delete
/// Image*. `publicize`: *Public* is offered in the edit dialog.
fn image_owner_items(lang: Lang, im: &Image, publicize: bool) -> Vec<MenuItem> {
    let im2 = im.clone();
    let formats = crate::components::dialog::prefs().images.disk_formats;
    let mut items = vec![MenuItem::dialog(t(lang, K::EditImage), move || {
        edit_image_dialog(lang, &im2, publicize, &formats)
    })];
    items.push(image_metadata_item(lang, im));
    // Glance refuses deleting a protected image; Horizon hides the item.
    if !im.protected {
        let (i, n) = (im.id.clone(), im.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteImage), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteImage),
                n.clone(),
                OsAction::ImageDelete { id: i.clone() },
            )
        }));
    }
    items
}

/// An image's *Update Metadata*.
fn image_metadata_item(lang: Lang, im: &Image) -> MenuItem {
    // `os_glance_*` properties belong to Glance's import machinery, and `[images]
    // reserved_properties` are the operator's: neither is editable.
    let reserved = crate::components::dialog::prefs().images.reserved_properties;
    let editable: Vec<_> = im
        .properties
        .iter()
        .filter(|(k, _)| !is_reserved_image_key(k, &reserved))
        .cloned()
        .collect();
    let i = im.id.clone();
    metadata_item(lang, "OS::Glance::Image", &editable, move |new, old| {
        let (set, remove) = metadata_changes(old, &new);
        OsAction::ImageSetProperties {
            id: i.clone(),
            set,
            remove,
        }
    })
}

/// Admin › Images' row menu, Horizon's admin table: *Edit Image*, *Update
/// Metadata* and *Delete Image* on every project's image (Glance lets an admin
/// change any, and publicize: `publicize_image` is admin-only by default).
pub fn admin_image_items(lang: Lang, im: &Image) -> Vec<MenuItem> {
    image_owner_items(lang, im, true)
}

/// Custom properties *Edit Image* shows as fields (Horizon's *Image Detail*
/// step); each field is named after its property.
const IMAGE_FIELD_PROPS: [&str; 4] = ["description", "architecture", "kernel_id", "ramdisk_id"];

/// Horizon's *Edit Image*: name, description, requirements (kernel and ramdisk
/// for AMI-style images, architecture, minimum disk and RAM), format while
/// `queued`, visibility (`public` only with `publicize`, or when it already is)
/// and protection. Submits only the changes.
pub fn edit_image_dialog(lang: Lang, im: &Image, publicize: bool, formats: &[String]) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let ami = matches!(im.disk_format.as_deref(), Some("ami" | "aki" | "ari"));
    let queued = im.status.eq_ignore_ascii_case("queued");
    let prop = |k: &str| im.property(k).unwrap_or("").to_string();
    let mut vis: Vec<&str> = vec!["private", "shared", "community"];
    if publicize || im.visibility == "public" {
        vis.push("public");
    }
    let mut fields = vec![
        Field::text("name", tr(K::Name)).value(im.name.clone()).required(),
        Field::area("description", tr(K::Description)).value(prop("description")),
    ];
    if ami {
        fields.push(Field::text("kernel_id", tr(K::KernelId)).value(prop("kernel_id")));
        fields.push(Field::text("ramdisk_id", tr(K::RamdiskId)).value(prop("ramdisk_id")));
    }
    fields.push(
        Field::text("architecture", tr(K::Architecture))
            .value(prop("architecture"))
            .help(tr(K::ArchitectureHelp)),
    );
    fields.push(Field::number("min_disk", tr(K::MinDiskGb), 0).value(im.min_disk_gb.unwrap_or(0).to_string()));
    fields.push(Field::number("min_ram", tr(K::MinRamMb), 0).value(im.min_ram_mb.unwrap_or(0).to_string()));
    if queued {
        let opts: Vec<&str> = formats.iter().map(String::as_str).collect();
        fields.push(
            Field::select("disk_format", tr(K::DiskFormat), options_raw(&opts))
                .value(im.disk_format.clone().unwrap_or_default())
                .required(),
        );
    }
    fields.push(
        Field::select("visibility", tr(K::Visibility), options_raw(&vis))
            .value(im.visibility.clone())
            .required(),
    );
    fields.push(Field::check("protected", tr(K::Protected), im.protected));
    let old = im.clone();
    Dialog::form(tr(K::EditImage), tr(K::Save), fields, move |v| {
        let mut c = ImageChanges::default();
        let name = req(v, "name", K::Name, lang)?;
        c.name = (name != old.name).then_some(name);
        let visibility = req(v, "visibility", K::Visibility, lang)?;
        c.visibility = (visibility != old.visibility).then_some(visibility);
        c.protected = (v.flag("protected") != old.protected).then(|| v.flag("protected"));
        let min_disk = num_u32(v, "min_disk", K::MinDiskGb, lang)?.unwrap_or(0);
        c.min_disk_gb = (i64::from(min_disk) != old.min_disk_gb.unwrap_or(0)).then_some(min_disk);
        let min_ram = num_u32(v, "min_ram", K::MinRamMb, lang)?.unwrap_or(0);
        c.min_ram_mb = (i64::from(min_ram) != old.min_ram_mb.unwrap_or(0)).then_some(min_ram);
        if let Some(f) = v
            .opt("disk_format")
            .filter(|f| Some(f.as_str()) != old.disk_format.as_deref())
        {
            c.disk_format = Some(f);
        }
        for key in IMAGE_FIELD_PROPS {
            // Kernel and ramdisk are only shown (and submitted) for AMI-style images.
            if !v.0.contains_key(key) {
                continue;
            }
            let (new, cur) = (v.get(key), old.property(key));
            match (new.is_empty(), cur) {
                (true, Some(_)) => c.remove.push(key.to_string()),
                (false, c0) if c0 != Some(new.as_str()) => c.set.push((key.to_string(), new)),
                _ => {}
            }
        }
        if c == ImageChanges::default() {
            return Err(t(lang, K::NothingChanged).to_string());
        }
        Ok(OsAction::ImageUpdate {
            id: old.id.clone(),
            changes: c,
        })
    })
}

/// Horizon's *Create Image* (roadmap 45): details, source, requirements (kernel and ramdisk
/// for AMI-style formats), sharing and metadata. The *URL* source needs
/// `[images] allow_location` (Horizon's `IMAGES_ALLOW_LOCATION`) and Glance's `web-download`;
/// *File* needs `[images] upload` to be `proxy` or `direct`.
pub fn create_image_dialog(lang: Lang) -> Dialog {
    let img = crate::components::dialog::prefs().images;
    let c = caps();
    create_image_form(
        lang,
        &img,
        needs::PUBLICIZE_IMAGE.met(&c),
        needs::IMAGE_URL_IMPORT.met(&c),
        false,
    )
}

/// Admin › Images' *Create Image* (roadmap 89): Project's, plus the owning
/// project (empty: the session's).
pub fn admin_create_image_dialog(lang: Lang) -> Dialog {
    let img = crate::components::dialog::prefs().images;
    let c = caps();
    create_image_form(
        lang,
        &img,
        needs::PUBLICIZE_IMAGE.met(&c),
        needs::IMAGE_URL_IMPORT.met(&c),
        true,
    )
}

/// [`create_image_dialog`] for given settings: `publicize` (the admin role) offers `public`
/// visibility, `url_import` is whether Glance has `web-download`, `admin` adds the owner.
fn create_image_form(
    lang: Lang,
    img: &crate::prefs::ImagePrefs,
    publicize: bool,
    url_import: bool,
    admin: bool,
) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let formats: Vec<&str> = img.disk_formats.iter().map(String::as_str).collect();
    let mut vis: Vec<&str> = vec!["private", "shared", "community"];
    if publicize {
        vis.push("public");
    }
    let default_vis = if vis.contains(&img.default_visibility.as_str()) {
        img.default_visibility.clone()
    } else {
        "private".into()
    };
    let mut sources = vec![("none".to_string(), tr(K::ImageSourceNone))];
    // *File* (roadmap 45): `[images] upload`, whatever the cloud, since the upload is plain Glance.
    if img.upload != ImageUpload::Off {
        sources.insert(0, ("file".to_string(), tr(K::ImageSourceFile)));
    }
    // Glance rejects `web-download` when the method is off (`GET /v2/info/import`, roadmap 45).
    if img.allow_location && url_import {
        sources.insert(0, ("url".to_string(), tr(K::ImageSourceUrl)));
    }
    let first_source = sources[0].0.clone();
    let mut fields = vec![
        Field::text("name", tr(K::ImageName)).required(),
        Field::area("description", tr(K::Description)).rows(2),
        // The source and its URL or file share a line (only one of the two shows).
        Field::select("source", tr(K::ImageSource), Source::Static(sources))
            .value(first_source)
            .required()
            .help(tr(K::ImageSourceNoneHelp))
            .half(),
        Field::text("url", tr(K::Url))
            .required()
            .help(tr(K::ImageUrlHelp))
            .when("source", "url")
            .half(),
        Field::file("file", tr(K::ImageSourceFile))
            .required()
            .help(tr(K::ImageFileHelp))
            .when("source", "file")
            .half(),
        // Format and architecture, then minimum disk and RAM, share a line each; the AMI-only
        // kernel and ramdisk (full rows) sit between the two pairs.
        Field::select("disk_format", tr(K::DiskFormat), options_raw(&formats))
            .value(formats.first().copied().unwrap_or("qcow2"))
            .required()
            .half(),
        // The hint is a tooltip: a help line would make this row taller than its neighbours.
        Field::text("architecture", tr(K::Architecture))
            .tip(tr(K::ArchitectureHelp))
            .half(),
        Field::text("kernel_id", tr(K::KernelId)).when("disk_format", "ami"),
        Field::text("ramdisk_id", tr(K::RamdiskId)).when("disk_format", "ami"),
        Field::number("min_disk", tr(K::MinDiskGb), 0).value("0").half(),
        Field::number("min_ram", tr(K::MinRamMb), 0).value("0").half(),
        Field::select("visibility", tr(K::Visibility), options_raw(&vis))
            .value(default_vis)
            .required()
            .half(),
        Field::check("protected", tr(K::Protected), false).half(),
        Field::key_values("metadata", tr(K::Metadata), &[])
            .help(tr(K::MetadataHelp))
            .catalog("OS::Glance::Image")
            .catalog_skip(&IMAGE_FIELD_PROPS),
    ];
    if admin {
        fields.insert(
            2,
            Field::select("owner", tr(K::Project), Source::Set(ChoiceSet::AllProjects))
                .help(tr(K::ImageOwnerHelp))
                .empty_label(tr(K::SelectProject)),
        );
    }
    let (max_bytes, offered) = (
        img.max_upload_bytes(),
        formats.iter().map(|f| f.to_string()).collect::<Vec<_>>(),
    );
    let details: Vec<&'static str> = fields.iter().map(|f| f.name).filter(|n| *n != "metadata").collect();
    let mut dialog = Dialog::form(tr(K::CreateImage), tr(K::CreateImage), fields, move |v| {
        let file = if v.get("source") == "file" {
            let picked =
                Picked::decode(&v.get("file")).ok_or_else(|| invalid(lang, t(lang, K::ImageSourceFile).to_string()))?;
            if picked.size > max_bytes {
                return Err(too_big(lang, max_bytes));
            }
            Some(picked.size)
        } else {
            None
        };
        let mut properties = validate_metadata(&v.pairs("metadata")).map_err(|e| invalid(lang, e))?;
        // Horizon keeps these as custom properties; kernel and ramdisk only for AMI.
        for key in IMAGE_FIELD_PROPS {
            let value = v.opt(key).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            if let Some(value) = value {
                if properties.iter().any(|(k, _)| k == key) {
                    return Err(invalid(lang, format!("{key}: repeated key")));
                }
                properties.push((key.to_string(), value));
            }
        }
        Ok(OsAction::ImageCreate(ImageSpec {
            name: req(v, "name", K::ImageName, lang)?,
            url: if v.get("source") == "url" {
                Some(req(v, "url", K::Url, lang)?)
            } else {
                None
            },
            disk_format: req(v, "disk_format", K::DiskFormat, lang)?,
            container_format: None,
            visibility: req(v, "visibility", K::Visibility, lang)?,
            protected: v.flag("protected"),
            min_disk_gb: num_u32(v, "min_disk", K::MinDiskGb, lang)?.filter(|n| *n > 0),
            min_ram_mb: num_u32(v, "min_ram", K::MinRamMb, lang)?.filter(|n| *n > 0),
            properties,
            upload_bytes: file,
            owner: v.opt("owner"),
        }))
    });
    // Horizon's wizard: the details, then the metadata (item 25 adds the catalog to that step).
    dialog = dialog.wizard(vec![
        Step::new(tr(K::ImageDetails), &details),
        Step::new(tr(K::Metadata), &["metadata"]),
    ]);
    // Picking a file preselects its disk format (Horizon does), and a file over the limit is
    // refused on the field, before anything is created.
    dialog.on_change = Some(Arc::new(move |name, vals, _| {
        if name == "file"
            && let Some(f) = Picked::decode(vals.get("file").map_or("", String::as_str))
                .and_then(|p| guess_format(&p.name, &offered))
        {
            vals.insert("disk_format".into(), f);
        }
    }));
    dialog.check = Some(Arc::new(move |vals, _| {
        let size = Picked::decode(vals.get("file").map_or("", String::as_str)).map(|p| p.size);
        if vals.get("source").is_some_and(|s| s == "file") && size.is_some_and(|n| n > max_bytes) {
            vec![("file", too_big(lang, max_bytes))]
        } else {
            vec![]
        }
    }));
    dialog
}

/// `The file is larger than the allowed 10 GiB`.
fn too_big(lang: Lang, max_bytes: u64) -> String {
    format!("{} {} GiB", t(lang, K::ImageFileTooBig), max_bytes >> 30)
}

/// `field` with the catalog's suggestions for `catalog`, when there is a resource type.
fn with_catalog(field: Field, catalog: Option<&'static str>) -> Field {
    catalog.map_or(field.clone(), |rt| field.catalog(rt))
}

/// *Update Metadata*: the current pairs in a key/value editor. `to_action`
/// gets the edited pairs (validated, sorted) and the current ones; nothing
/// is submitted when they are equal.
pub(super) fn metadata_item(
    lang: Lang,
    catalog: &'static str,
    current: &[(String, String)],
    to_action: impl Fn(Vec<(String, String)>, &[(String, String)]) -> OsAction + Send + Sync + 'static,
) -> MenuItem {
    pairs_item(
        lang,
        K::UpdateMetadata,
        K::Metadata,
        K::MetadataHelp,
        Some(catalog),
        current,
        to_action,
    )
}

/// A key/value editor entry titled `title` (metadata, extra specs, QoS or
/// group specs): `to_action` gets the new pairs and the current ones; an
/// unchanged list is refused.
pub(super) fn pairs_item(
    lang: Lang,
    title: K,
    label: K,
    help: K,
    catalog: Option<&'static str>,
    current: &[(String, String)],
    to_action: impl Fn(Vec<(String, String)>, &[(String, String)]) -> OsAction + Send + Sync + 'static,
) -> MenuItem {
    let mut current = current.to_vec();
    current.sort();
    let current = std::sync::Arc::new(current);
    let to_action = std::sync::Arc::new(to_action);
    MenuItem::dialog(t(lang, title), move || {
        let (cur, to_action) = (current.clone(), to_action.clone());
        Dialog::form(
            t(lang, title),
            t(lang, K::Save),
            vec![with_catalog(
                Field::key_values("metadata", t(lang, label), &cur).help(t(lang, help)),
                catalog,
            )],
            move |v| {
                let new = validate_metadata(&v.pairs("metadata"))?;
                if new == *cur {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(to_action(new, &cur))
            },
        )
        // Policy can still say no (Glance answers 403): a plain "no permission" instead of the raw error.
        .forbidden(t(lang, K::MetadataForbidden))
    })
}

/// *Import Public Key*: name, key type (SSH or X509, roadmap 51) and the key,
/// checked against the type before sending.
pub fn import_keypair_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::ImportPublicKey),
        t(lang, K::ImportPublicKey),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::select(
                "key_type",
                t(lang, K::KeyType),
                Source::Static(vec![
                    ("ssh".into(), t(lang, K::SshKey).into()),
                    ("x509".into(), t(lang, K::X509Certificate).into()),
                ]),
            )
            .value("ssh")
            .required(),
            Field::area("public_key", t(lang, K::PublicKey))
                .required()
                .help(t(lang, K::PublicKeyHelp)),
        ],
        move |v| {
            let (public_key, key_type) = (req(v, "public_key", K::PublicKey, lang)?, v.get("key_type"));
            check_public_key(&key_type, &public_key).map_err(|e| invalid(lang, e))?;
            Ok(OsAction::KeyPairImport {
                name: req(v, "name", K::Name, lang)?,
                public_key,
                key_type,
            })
        },
    )
}

/// *Create Key Pair*: an Ed25519 SSH key generated on the server; the private key
/// is shown once.
pub fn generate_keypair_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateKeyPair),
        t(lang, K::CreateKeyPair),
        vec![
            Field::text("name", t(lang, K::Name))
                .required()
                .help(t(lang, K::GeneratedKeyHelp)),
        ],
        move |v| {
            Ok(OsAction::KeyPairGenerate {
                name: req(v, "name", K::Name, lang)?,
            })
        },
    )
}

/// A key pair's menu (only *Delete Key Pair*).
pub fn keypair_items(lang: Lang, name: &str) -> Vec<MenuItem> {
    let n = name.to_string();
    vec![MenuItem::danger(t(lang, K::DeleteKeyPair), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteKeyPair),
            n.clone(),
            OsAction::KeyPairDelete { name: n.clone() },
        )
    })]
}

/// *Create Server Group*: name and policy.
pub fn create_server_group_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateServerGroup),
        t(lang, K::CreateServerGroup),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::select(
                "policy",
                t(lang, K::Policy),
                options_raw(&["anti-affinity", "affinity", "soft-anti-affinity", "soft-affinity"]),
            )
            .value("anti-affinity")
            .required(),
        ],
        move |v| {
            Ok(OsAction::ServerGroupCreate {
                name: req(v, "name", K::Name, lang)?,
                policy: req(v, "policy", K::Policy, lang)?,
            })
        },
    )
}

/// A server group's menu (only *Delete Server Group*).
pub fn server_group_items(lang: Lang, g: &ServerGroup) -> Vec<MenuItem> {
    group_items(lang, g, None)
}

/// The detail page's menu: the same, going back to the list after deleting.
pub fn server_group_detail_items(lang: Lang, g: &ServerGroup) -> Vec<MenuItem> {
    group_items(lang, g, Some("/project/server_groups"))
}

fn group_items(lang: Lang, g: &ServerGroup, back: Option<&'static str>) -> Vec<MenuItem> {
    let (i, n) = (g.id.clone(), g.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteServerGroup), move || {
        let d = Dialog::confirm(
            lang,
            t(lang, K::DeleteServerGroup),
            n.clone(),
            OsAction::ServerGroupDelete { id: i.clone() },
        );
        match back {
            Some(href) => d.then_go(href),
            None => d,
        }
    })]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;
    use std::collections::HashMap;

    fn source_values(d: &Dialog) -> Vec<String> {
        let f = d.fields.iter().find(|f| f.name == "source").unwrap();
        let Kind::Select(Source::Static(opts)) = &f.kind else {
            panic!()
        };
        opts.iter().map(|(v, _)| v.clone()).collect()
    }

    #[test]
    fn create_image_offers_file_only_when_uploads_are_enabled() {
        use crate::prefs::{ImagePrefs, ImageUpload};
        let with = |upload, url: bool| ImagePrefs {
            upload,
            allow_location: url,
            ..Default::default()
        };
        let d = |p: &ImagePrefs, web_download| create_image_form(Lang::EnUs, p, false, web_download, false);
        assert_eq!(
            source_values(&d(&with(ImageUpload::Off, false), true)),
            ["none"],
            "nothing changes until an operator enables it"
        );
        assert_eq!(
            source_values(&d(&with(ImageUpload::Proxy, false), true)),
            ["file", "none"]
        );
        assert_eq!(
            source_values(&d(&with(ImageUpload::Direct, true), true)),
            ["url", "file", "none"]
        );
        assert_eq!(
            source_values(&d(&with(ImageUpload::Proxy, true), false)),
            ["file", "none"],
            "Glance has no web-download"
        );
    }

    #[test]
    fn create_image_catalog_leaves_out_the_keys_with_their_own_fields() {
        let d = create_image_form(Lang::EnUs, &Default::default(), false, true, false);
        let meta = d.fields.iter().find(|f| f.name == "metadata").unwrap();
        assert_eq!(meta.catalog, Some("OS::Glance::Image"));
        for key in ["description", "architecture", "kernel_id", "ramdisk_id"] {
            assert!(
                meta.catalog_skip.contains(&key),
                "{key} has its own field: offering it would be a repeated key"
            );
        }
    }

    #[test]
    fn admin_create_image_picks_the_owner() {
        let names = |d: &Dialog| d.fields.iter().map(|f| f.name).take(3).collect::<Vec<_>>();
        assert_eq!(
            names(&create_image_form(Lang::EnUs, &Default::default(), false, true, false)),
            ["name", "description", "source"]
        );
        let d = create_image_form(Lang::EnUs, &Default::default(), true, true, true);
        assert_eq!(names(&d), ["name", "description", "owner"], "roadmap 89");
        assert!(d.steps[0].fields.contains(&"owner"), "on the details step");
        let base = [
            ("name", "deb"),
            ("source", "none"),
            ("disk_format", "qcow2"),
            ("visibility", "private"),
        ];
        let build = |extra: &[(&str, &str)]| {
            let all: Vec<(&str, &str)> = base.iter().chain(extra).copied().collect();
            (d.build)(&vals(&all)).unwrap()
        };
        assert!(matches!(build(&[("owner", "p9")]), OsAction::ImageCreate(s) if s.owner.as_deref() == Some("p9")));
        assert!(
            matches!(build(&[("owner", "")]), OsAction::ImageCreate(s) if s.owner.is_none()),
            "empty: the session's project"
        );
    }

    #[test]
    fn create_image_is_a_wizard_with_metadata_on_its_own_step() {
        let d = create_image_form(Lang::EnUs, &Default::default(), false, true, false);
        assert_eq!(d.steps.len(), 2);
        assert_eq!(d.steps[1].fields, ["metadata"], "metadata is the second step");
        let mut in_steps: Vec<_> = d.steps.iter().flat_map(|s| s.fields.clone()).collect();
        let mut all: Vec<_> = d.fields.iter().map(|f| f.name).collect();
        in_steps.sort();
        all.sort();
        assert_eq!(in_steps, all, "every field is on exactly one step");
        let half: Vec<_> = d.fields.iter().filter(|f| f.half).map(|f| f.name).collect();
        assert_eq!(
            half,
            [
                "source",
                "url",
                "file",
                "disk_format",
                "architecture",
                "min_disk",
                "min_ram",
                "visibility",
                "protected"
            ],
            "pairs share a line"
        );
    }

    #[test]
    fn create_image_from_a_file_carries_its_size_and_respects_the_limit() {
        use crate::prefs::{ImagePrefs, ImageUpload};
        let prefs = ImagePrefs {
            upload: ImageUpload::Proxy,
            max_upload_gb: 1,
            ..Default::default()
        };
        let d = create_image_form(Lang::EnUs, &prefs, false, true, false);
        let picked = |size: u64| {
            Picked {
                name: "debian.qcow2".into(),
                size,
            }
            .encode()
        };
        let base = |file: &str| {
            vec![
                ("name", "Debian".to_string()),
                ("source", "file".into()),
                ("file", file.to_string()),
                ("disk_format", "qcow2".into()),
                ("visibility", "private".into()),
                ("min_disk", "0".into()),
                ("min_ram", "0".into()),
            ]
        };
        let build = |file: &str| {
            let owned = base(file);
            let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
            (d.build)(&vals(&pairs))
        };
        let Ok(OsAction::ImageCreate(spec)) = build(&picked(5 << 20)) else {
            panic!("a file within the limit builds")
        };
        assert_eq!((spec.upload_bytes, spec.url), (Some(5 << 20), None));
        assert!(
            build(&picked((1 << 30) + 1)).unwrap_err().contains("1 GiB"),
            "over [images] max_upload_gb"
        );
        assert!(build("").is_err(), "no file picked");

        // The field shows the same problem before submitting, and picking a file sets its format.
        let check = d.check.as_ref().unwrap();
        let vals_of = |size: u64| {
            HashMap::from([
                ("source".to_string(), "file".to_string()),
                ("file".to_string(), picked(size)),
            ])
        };
        assert!(
            check(&vals_of((1 << 30) + 1), &Sets::new())
                .iter()
                .any(|(f, _)| *f == "file")
        );
        assert!(check(&vals_of(1), &Sets::new()).is_empty());
        let mut v = HashMap::from([(
            "file".to_string(),
            Picked {
                name: "disk.iso".into(),
                size: 1,
            }
            .encode(),
        )]);
        (d.on_change.as_ref().unwrap())("file", &mut v, &Sets::new());
        assert_eq!(v.get("disk_format").map(String::as_str), Some("iso"));
    }

    #[test]
    fn import_key_pair_sends_the_chosen_type() {
        let d = import_keypair_dialog(Lang::EnUs);
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
        let a = (d.build)(&vals(&[("name", "web"), ("key_type", "x509"), ("public_key", pem)])).unwrap();
        assert_eq!(
            a,
            OsAction::KeyPairImport {
                name: "web".into(),
                public_key: pem.into(),
                key_type: "x509".into()
            }
        );
        assert!(
            (d.build)(&vals(&[("name", "web"), ("key_type", "ssh"), ("public_key", pem)])).is_err(),
            "checked before sending"
        );
    }

    #[test]
    fn edit_image_sends_only_the_changes() {
        let im = Image {
            id: "i".into(),
            name: "deb".into(),
            is_snapshot: false,
            status: "active".into(),
            visibility: "private".into(),
            protected: false,
            disk_format: Some("qcow2".into()),
            size: None,
            owned_by_project: true,
            owner_id: None,
            owner_name: None,
            properties: vec![("description".into(), "old".into())],
            min_disk_gb: Some(5),
            min_ram_mb: None,
            import_failed: None,
        };
        let formats = vec!["raw".to_string(), "qcow2".to_string()];
        let d = edit_image_dialog(Lang::EnUs, &im, false, &formats);
        let names: Vec<_> = d.fields.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "name",
                "description",
                "architecture",
                "min_disk",
                "min_ram",
                "visibility",
                "protected"
            ],
            "no kernel/ramdisk/format"
        );
        let Kind::Select(Source::Static(vis)) = &d.fields[5].kind else {
            panic!()
        };
        assert!(!vis.iter().any(|(v, _)| v == "public"), "public needs the admin role");
        let base = [
            ("name", "deb"),
            ("description", "old"),
            ("architecture", ""),
            ("min_disk", "5"),
            ("min_ram", "0"),
            ("visibility", "private"),
            ("protected", "false"),
        ];
        let with = |changes: &[(&'static str, &'static str)]| {
            let mut all: Vec<(&str, &str)> = base.to_vec();
            for (k, v) in changes {
                all.retain(|(a, _)| a != k);
                all.push((k, v));
            }
            (d.build)(&vals(&all))
        };
        assert!(with(&[]).is_err(), "nothing changed");
        let OsAction::ImageUpdate { changes, .. } =
            with(&[("description", ""), ("architecture", "x86_64"), ("min_disk", "10")]).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            changes,
            ImageChanges {
                min_disk_gb: Some(10),
                set: vec![("architecture".into(), "x86_64".into())],
                remove: vec!["description".into()],
                ..Default::default()
            }
        );
        let ami = Image {
            disk_format: Some("ami".into()),
            status: "queued".into(),
            visibility: "public".into(),
            ..im
        };
        let d = edit_image_dialog(Lang::EnUs, &ami, false, &formats);
        for f in ["kernel_id", "ramdisk_id", "disk_format"] {
            assert!(d.fields.iter().any(|x| x.name == f), "{f}");
        }
        let Kind::Select(Source::Static(vis)) = &d.fields.iter().find(|f| f.name == "visibility").unwrap().kind else {
            panic!()
        };
        assert!(vis.iter().any(|(v, _)| v == "public"), "already public: kept");
    }

    #[test]
    fn admin_edits_any_projects_image_and_may_publicize() {
        let im = Image {
            id: "i".into(),
            name: "deb".into(),
            is_snapshot: false,
            status: "active".into(),
            visibility: "private".into(),
            protected: false,
            disk_format: Some("qcow2".into()),
            size: None,
            owned_by_project: false,
            owner_id: Some("p9".into()),
            owner_name: None,
            properties: vec![],
            min_disk_gb: None,
            min_ram_mb: None,
            import_failed: None,
        };
        assert!(
            image_items(Lang::EnUs, &im)
                .iter()
                .all(|i| i.label != "Edit Image" && i.label != "Update Metadata"),
            "Project: another project's image is read-only"
        );
        let own = image_items(
            Lang::EnUs,
            &Image {
                owned_by_project: true,
                ..im.clone()
            },
        );
        assert!(
            dialog_of(&own, "Update Metadata").forbidden.is_some(),
            "a 403 shows the no-permission text"
        );
        let items = admin_image_items(Lang::EnUs, &im);
        assert_eq!(
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(),
            ["Edit Image", "Update Metadata", "Delete Image"]
        );
        let d = dialog_of(&items, "Edit Image");
        let Kind::Select(Source::Static(vis)) = &d.fields.iter().find(|f| f.name == "visibility").unwrap().kind else {
            panic!()
        };
        assert!(vis.iter().any(|(v, _)| v == "public"), "an admin may publicize");
        let locked = admin_image_items(Lang::EnUs, &Image { protected: true, ..im });
        assert!(
            locked.iter().all(|i| i.label != "Delete Image"),
            "Glance refuses deleting a protected image (Horizon hides it)"
        );
    }
}
