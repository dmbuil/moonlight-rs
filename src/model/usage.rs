//! Usage summaries (Overview, Admin › Overview).

use super::*;

/// One server's usage in the period (Nova `os-simple-tenant-usage`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerUsage {
    pub id: String,
    pub name: String,
    pub vcpus: i64,
    pub memory_mb: i64,
    pub local_gb: i64,
    /// Hours the server existed inside the period.
    pub hours: f64,
    /// Seconds since it started (or its lifetime, when terminated).
    pub uptime: i64,
    pub state: String,
}

impl ServerUsage {
    /// Still exists (Nova reports `terminated` for deleted ones in the period).
    pub fn active(&self) -> bool {
        !self.state.eq_ignore_ascii_case("terminated")
    }
}

/// Overview *Usage Summary* for a period (`start`/`end` are inclusive dates).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub start: String,
    pub end: String,
    pub servers: Vec<ServerUsage>,
}

impl UsageSummary {
    /// The table rows: like Horizon, only servers that still exist (the
    /// hour totals still count the ended ones).
    pub fn listed(&self) -> Vec<ServerUsage> {
        self.servers.iter().filter(|s| s.active()).cloned().collect()
    }
    /// Servers that still exist (Horizon's *Active Instances*).
    pub fn active_instances(&self) -> usize {
        self.servers.iter().filter(|s| s.active()).count()
    }
    /// RAM of the servers that still exist (Horizon's *Active RAM*).
    pub fn active_ram_mb(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.memory_mb).sum()
    }
    /// Summed per server (Nova's own totals cover only one page when paginated).
    pub fn vcpu_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.vcpus as f64 * s.hours).sum()
    }
    /// Disk GB-hours over the period, ended servers included.
    pub fn gb_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.local_gb as f64 * s.hours).sum()
    }
    /// RAM MB-hours over the period, ended servers included.
    pub fn ram_mb_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.memory_mb as f64 * s.hours).sum()
    }

    /// CSV like Horizon's *Download CSV Summary*.
    pub fn csv(&self) -> String {
        let n = |v: f64| fixed(v, 2);
        fn field(v: &str) -> String {
            if v.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        }
        let mut out = format!("Usage Report For Period:,{},{}\n", self.start, self.end);
        out += &format!("Active Instances:,{}\n", self.active_instances());
        out += &format!("This Period's VCPU-Hours:,{}\n", n(self.vcpu_hours()));
        out += &format!("This Period's GB-Hours:,{}\n", n(self.gb_hours()));
        out += &format!("This Period's RAM-Hours:,{}\n\n", n(self.ram_mb_hours()));
        out += "Instance Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours),Age (Seconds),State\n";
        for s in self.servers.iter().filter(|s| s.active()) {
            out += &format!(
                "{},{},{},{},{},{},{}\n",
                field(&s.name),
                s.vcpus,
                s.memory_mb,
                s.local_gb,
                n(s.hours),
                s.uptime,
                field(&s.state)
            );
        }
        out
    }
}

/// One project's line on Admin › Overview: its servers in the
/// period, summed like Horizon's `NovaUsage` (sizes of the servers that still
/// exist, hours of all of them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectUsage {
    pub project_id: String,
    /// The project's name, else its ID (a deleted project keeps its usage).
    pub project: String,
    pub servers: Vec<ServerUsage>,
    /// Keystone named other projects but not this one: Horizon's "(Deleted)"
    /// after the ID. `false` when no names could be read.
    #[serde(default)]
    pub deleted: bool,
}

impl ProjectUsage {
    /// The project's servers as a period-less [`UsageSummary`], to reuse its totals.
    fn summary(&self) -> UsageSummary {
        UsageSummary {
            start: String::new(),
            end: String::new(),
            servers: self.servers.clone(),
        }
    }
    /// VCPUs of the servers that still exist.
    pub fn vcpus(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.vcpus).sum()
    }
    /// Root disk GB of the servers that still exist.
    pub fn local_gb(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.local_gb).sum()
    }
    /// RAM MB of the servers that still exist.
    pub fn memory_mb(&self) -> i64 {
        self.summary().active_ram_mb()
    }
    /// VCPU-hours over the period, ended servers included.
    pub fn vcpu_hours(&self) -> f64 {
        self.summary().vcpu_hours()
    }
    /// Disk GB-hours over the period, ended servers included.
    pub fn gb_hours(&self) -> f64 {
        self.summary().gb_hours()
    }
    /// RAM MB-hours over the period, ended servers included.
    pub fn ram_mb_hours(&self) -> f64 {
        self.summary().ram_mb_hours()
    }
}

/// Admin › Overview: every project's usage over `[start, end]` (dates).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalUsage {
    pub start: String,
    pub end: String,
    /// Projects with servers in the period, by name.
    pub projects: Vec<ProjectUsage>,
}

impl GlobalUsage {
    /// Every server of every project, for the totals above the table.
    pub fn summary(&self) -> UsageSummary {
        UsageSummary {
            start: self.start.clone(),
            end: self.end.clone(),
            servers: self.projects.iter().flat_map(|p| p.servers.clone()).collect(),
        }
    }

    /// CSV like Horizon's Admin *Download CSV Summary* (`GlobalUsageCsvRenderer`):
    /// the totals, then one line per project; *Usage (Hours)* is VCPU-hours.
    pub fn csv(&self) -> String {
        let n = |v: f64| fixed(v, 2);
        let field = |v: &str| {
            if v.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        };
        let all = self.summary();
        let mut out = format!("Usage Report For Period:,{},{}\n", self.start, self.end);
        out += &format!("Active Instances:,{}\n", all.active_instances());
        out += &format!("This Period's VCPU-Hours:,{}\n", n(all.vcpu_hours()));
        out += &format!("This Period's GB-Hours:,{}\n", n(all.gb_hours()));
        out += &format!("This Period's RAM-Hours:,{}\n\n", n(all.ram_mb_hours()));
        out += "Project Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours)\n";
        for p in &self.projects {
            let name = if p.deleted {
                format!("{} (Deleted)", p.project)
            } else {
                p.project.clone()
            };
            out += &format!(
                "{},{},{},{},{}\n",
                field(&name),
                p.vcpus(),
                p.memory_mb(),
                p.local_gb(),
                n(p.vcpu_hours())
            );
        }
        out
    }
}

/// `v` with `places` decimals, never `-0.00`: summing an empty `f64` iterator
/// gives `-0.0`, and a tiny negative rounds to `-0.00` (*Usage
/// Summary* with no instances in the period). Real negatives keep their sign.
pub fn fixed(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => s,
    }
}

/// Validates the usage period. Empty values default to the current month up to
/// `today`; the range is inclusive and at most a year.
pub fn usage_range(
    start: &str,
    end: &str,
    today: chrono::NaiveDate,
) -> Result<(chrono::NaiveDate, chrono::NaiveDate), String> {
    use chrono::{Datelike, NaiveDate};
    let parse = |v: &str, default: NaiveDate| -> Result<NaiveDate, String> {
        if v.trim().is_empty() {
            Ok(default)
        } else {
            NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").map_err(|_| format!("invalid date: {v}"))
        }
    };
    let first = today.with_day(1).unwrap_or(today);
    let (from, to) = (parse(start, first)?, parse(end, today)?);
    if from > to {
        return Err("the start date is after the end date".into());
    }
    if (to - from).num_days() > 366 {
        return Err("the period can be at most one year".into());
    }
    Ok((from, to))
}

#[cfg(test)]
mod usage_tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn u(name: &str, vcpus: i64, hours: f64, state: &str) -> ServerUsage {
        ServerUsage {
            id: name.into(),
            name: name.into(),
            vcpus,
            memory_mb: 2048,
            local_gb: 10,
            hours,
            uptime: 3600,
            state: state.into(),
        }
    }

    #[test]
    fn range_defaults_to_this_month_and_validates() {
        let today = d("2026-10-24");
        assert_eq!(usage_range("", "", today), Ok((d("2026-10-01"), today)));
        assert_eq!(
            usage_range("2026-09-01", "2026-09-30", today),
            Ok((d("2026-09-01"), d("2026-09-30")))
        );
        assert!(usage_range("2026-10-05", "2026-10-01", today).is_err());
        assert!(usage_range("2024-01-01", "2026-01-01", today).is_err());
        assert!(usage_range("yesterday", "", today).is_err());
    }

    #[test]
    fn totals_count_only_active_servers_where_horizon_does() {
        let s = UsageSummary {
            start: "a".into(),
            end: "b".into(),
            servers: vec![u("x", 2, 10.0, "active"), u("y", 4, 1.5, "terminated")],
        };
        assert_eq!(s.active_instances(), 1);
        assert_eq!(s.active_ram_mb(), 2048);
        assert!((s.vcpu_hours() - 26.0).abs() < 1e-9);
        assert!((s.gb_hours() - 115.0).abs() < 1e-9);
        assert_eq!(
            s.listed().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["x"],
            "ended servers are not listed"
        );
        assert!(!s.csv().contains("\ny,"), "nor in the CSV");
    }

    #[test]
    fn csv_quotes_names() {
        let s = UsageSummary {
            start: "2026-10-01".into(),
            end: "2026-10-24".into(),
            servers: vec![u("web, \"1\"", 1, 2.0, "active")],
        };
        let csv = s.csv();
        assert!(csv.starts_with("Usage Report For Period:,2026-10-01,2026-10-24\n"));
        assert!(csv.contains("\"web, \"\"1\"\"\",1,2048,10,2.00,3600,active\n"));
    }

    #[test]
    fn providers_form_a_tree_with_gpus_under_their_host() {
        let res = |used, total| {
            Some(ProviderResource {
                used,
                reserved: 0,
                total,
                allocation_ratio: 1.0,
            })
        };
        let rp = |uuid: &str, name: &str, parent: Option<&str>, vgpus| ResourceProviderRow {
            uuid: uuid.into(),
            name: name.into(),
            vcpus: None,
            memory_mb: None,
            disk_gb: None,
            parent: parent.map(Into::into),
            depth: 0,
            host: String::new(),
            descendants: 0,
            vgpus,
            others: vec![],
            custom_traits: vec![],
        };
        let rows = provider_tree(vec![
            rp("g2", "cmp-1_pci_0000_85_00_0", Some("h1"), res(1, 4)),
            rp("h2", "cmp-0", None, None),
            rp("g1", "cmp-1_pci_0000_84_00_0", Some("h1"), res(2, 4)),
            rp("h1", "cmp-1", None, None),
            rp("old", "cmp-2", None, res(3, 8)),
        ]);
        let shape: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        assert_eq!(
            shape,
            [
                ("cmp-0", 0),
                ("cmp-1", 0),
                ("pci 0000:84:00.0", 1),
                ("pci 0000:85:00.0", 1),
                ("cmp-2", 0)
            ],
            "hosts by name, GPUs under theirs, short names"
        );
        assert_eq!(
            rows[1].vgpus.as_ref().map(|v| (v.used, v.total)),
            Some((3, 8)),
            "a host shows its GPUs' sum"
        );
        assert_eq!(
            rows.iter()
                .map(|r| (r.host.as_str(), r.descendants))
                .collect::<Vec<_>>(),
            [("h2", 0), ("h1", 2), ("h1", 0), ("h1", 0), ("old", 0)],
            "each row knows its host; a host counts what it can collapse"
        );
        let typed = |uuid: &str, parent: Option<&str>, tr: &[&str]| ResourceProviderRow {
            custom_traits: tr.iter().map(|t| t.to_string()).collect(),
            ..rp(uuid, uuid, parent, res(0, 1))
        };
        let host = provider_tree(vec![
            typed("h", None, &[]),
            typed("g1", Some("h"), &["CUSTOM_B"]),
            typed("g2", Some("h"), &["CUSTOM_A", "CUSTOM_B"]),
        ]);
        assert_eq!(
            host[0].custom_traits,
            ["CUSTOM_A", "CUSTOM_B"],
            "a host lists its devices' types once each"
        );
        assert_eq!(
            vgpu_summary(&rows),
            (6, 16, 3, 2),
            "cmp-1's two GPUs and cmp-2's own VGPU (older Nova), not cmp-1's sum twice"
        );
        assert_eq!(
            short_provider_name("cmp-1", "cmp-1_NUMA0"),
            "NUMA0",
            "other children lose only the host prefix"
        );
    }

    #[test]
    fn hypervisor_totals_sum_every_hypervisor() {
        let h = |used: i64, total: i64| HypervisorRow {
            id: "h".into(),
            hostname: "h".into(),
            kind: "QEMU".into(),
            vcpus_used: used,
            vcpus: total,
            ram_used_mb: used * 1024,
            ram_mb: total * 1024,
            disk_used_gb: used * 10,
            disk_gb: total * 10,
            instances: 0,
            up: true,
            enabled: true,
        };
        let hv = Hypervisors {
            hypervisors: vec![h(2, 8), h(3, 16)],
            hosts: Fallible {
                rows: vec![],
                error: None,
            },
            placement: false,
        };
        assert_eq!(hv.totals(), [(5, 24), (5120, 24576), (50, 240)]);
    }

    #[test]
    fn global_usage_totals_every_project_and_csv_has_a_line_each() {
        let p = |name: &str, servers| ProjectUsage {
            project_id: name.into(),
            project: name.into(),
            servers,
            deleted: false,
        };
        let g = GlobalUsage {
            start: "2026-10-01".into(),
            end: "2026-10-30".into(),
            projects: vec![
                p("a, b", vec![u("x", 2, 10.0, "active")]),
                p("c", vec![u("y", 4, 1.5, "terminated"), u("z", 1, 1.0, "active")]),
            ],
        };
        let all = g.summary();
        assert_eq!(
            (all.active_instances(), all.vcpu_hours()),
            (2, 27.0),
            "every project's servers"
        );
        let csv = g.csv();
        assert!(
            csv.starts_with("Usage Report For Period:,2026-10-01,2026-10-30\nActive Instances:,2\n"),
            "{csv}"
        );
        assert!(
            csv.contains(
                "Project Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours)\n\"a, b\",2,2048,10,20.00\nc,1,2048,10,7.00\n"
            ),
            "{csv}"
        );
        let gone = GlobalUsage {
            projects: vec![ProjectUsage {
                deleted: true,
                ..p("dc87", vec![u("x", 2, 1.0, "active")])
            }],
            ..g
        };
        assert!(
            gone.csv().ends_with("dc87 (Deleted),2,2048,10,2.00\n"),
            "Horizon's label"
        );
    }

    #[test]
    fn an_empty_period_totals_zero_not_minus_zero() {
        let s = UsageSummary {
            start: "2026-10-01".into(),
            end: "2026-10-24".into(),
            servers: vec![],
        };
        assert!(
            s.vcpu_hours().is_sign_negative(),
            "an empty f64 sum is -0.0, which printed as -0.00"
        );
        let csv = s.csv();
        assert!(
            csv.contains("This Period's VCPU-Hours:,0.00\n") && !csv.contains("-0.00"),
            "{csv}"
        );
    }
}
