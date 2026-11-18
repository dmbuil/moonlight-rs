//! Micro-benchmarks of the pure mappers (`server::mapping`) with large, API-shaped inputs: the
//! CPU a list page costs Moonlight once OpenStack has answered, and the JSON a server function
//! then sends. Cheap and stable enough to compare a change before and after:
//!
//! ```text
//! export CARGO_TARGET_DIR=target/bench
//! cargo bench --features ssr --bench mapping -- --save-baseline before
//! cargo bench --features ssr --bench mapping -- --baseline before
//! ```
//!
//! Its own target directory, because a bench also builds the `moonlight` binary in the `release`
//! profile: in `target/` it would replace the one `cargo leptos build --release` made.
//!
//! Payloads follow the shapes of the mapping tests' fixtures and of the load test's fake
//! OpenStack (`loadtest/`): every server has a flavor, an image or a boot volume, an address and a
//! security group, so each join in the mapper does real work.

use std::collections::HashMap;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use moonlight::server::mapping;
use serde_json::{Value, json};

/// Rows per list: a small project, a big one, and a cloud seen by an admin.
const SIZES: &[usize] = &[100, 1_000, 5_000];

/// The project that owns everything below.
const PROJECT: &str = "0f5e1d8a6c2b4e3f9a7d1c0b2e4f6a8d";

/// The `n`th UUID-shaped ID of a kind.
fn uid(kind: u16, n: usize) -> String {
    format!("{kind:04x}0000-0000-4000-8000-{n:012x}")
}

/// `GET servers/detail` rows: half boot from an image, half from a volume.
fn servers(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({
                "id": uid(0x5e5, i), "name": format!("vm-{i:05}"), "status": "ACTIVE", "tenant_id": PROJECT,
                "created": format!("2026-09-{:02}T10:22:00Z", 1 + i % 28), "updated": "2026-10-01T08:00:00Z",
                "key_name": "ops-2026", "locked": false,
                "image": if i % 2 == 0 { json!({"id": uid(0x1a6, i % 50)}) } else { json!("") },
                "flavor": {"id": format!("f{}", i % 10), "original_name": format!("m1.f{}", i % 10)},
                "OS-EXT-AZ:availability_zone": "nova", "OS-EXT-STS:power_state": 1, "OS-EXT-STS:task_state": null,
                "OS-EXT-SRV-ATTR:host": format!("compute-{:02}", i % 20),
                "addresses": {format!("net-{}", i % 5): [
                    {"addr": format!("10.{}.{}.{}", i % 5, i / 250 % 250, i % 250 + 2), "version": 4,
                     "OS-EXT-IPS:type": "fixed", "OS-EXT-IPS-MAC:mac_addr": "fa:16:3e:4a:12:21"}
                ]},
                "security_groups": [{"name": "default"}, {"name": "web"}],
                "os-extended-volumes:volumes_attached": [{"id": uid(0xb01, i)}],
                "metadata": {"role": "load"}
            })
        })
        .collect()
}

/// `GET flavors/detail`.
fn flavors() -> Vec<Value> {
    (0..10)
        .map(|i| json!({"id": format!("f{i}"), "name": format!("m1.f{i}"), "vcpus": 2, "ram": 4096, "disk": 40}))
        .collect()
}

/// `GET /v2/images` rows; a third are public and owned by another project.
fn images(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({"id": uid(0x1a6, i), "name": format!("image-{i:04}"), "status": "active",
                   "visibility": if i % 3 == 0 { "public" } else { "private" },
                   "owner": if i % 3 == 0 { "other-project" } else { PROJECT },
                   "disk_format": "qcow2", "container_format": "bare", "size": 1_073_741_824_u64,
                   "min_disk": 10, "min_ram": 0, "protected": false, "created_at": "2026-01-01T00:00:00Z"})
        })
        .collect()
}

/// `GET volumes/detail` rows, each attached to the server of the same index.
fn volumes(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({"id": uid(0xb01, i), "name": format!("vol-{i:05}"), "status": "in-use", "size": 20,
                   "volume_type": "standard", "availability_zone": "nova", "bootable": "true", "encrypted": false,
                   "multiattach": false, "created_at": "2026-09-14T10:22:00.000000",
                   "attachments": [{"server_id": uid(0x5e5, i), "device": "/dev/vda", "attachment_id": uid(0xa77, i)}],
                   "volume_image_metadata": {"image_id": uid(0x1a6, i % 50), "image_name": "rhel-9.4"}})
        })
        .collect()
}

/// `GET networks` and `GET subnets`: one subnet per network.
fn networks(n: usize) -> (Vec<Value>, Vec<Value>) {
    let nets = (0..n)
        .map(|i| {
            json!({"id": uid(0x0e7, i), "name": format!("net-{i:04}"), "tenant_id": PROJECT, "status": "ACTIVE",
                   "admin_state_up": true, "shared": i % 10 == 0, "router:external": false,
                   "subnets": [uid(0x5b7, i)], "availability_zones": ["nova"]})
        })
        .collect();
    let subnets = (0..n)
        .map(|i| {
            json!({"id": uid(0x5b7, i), "name": format!("subnet-{i:04}"), "network_id": uid(0x0e7, i),
                   "cidr": format!("10.{}.{}.0/24", i / 256 % 256, i % 256), "ip_version": 4})
        })
        .collect();
    (nets, subnets)
}

/// Project › Instances: the join with flavors, images and boot volumes, then the JSON answer.
fn bench_instances(c: &mut Criterion) {
    let mut g = c.benchmark_group("instances");
    let (fl, im) = (flavors(), images(50));
    for &n in SIZES {
        let (sv, vo) = (servers(n), volumes(n));
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("map", n), &n, |b, _| {
            b.iter(|| mapping::instances(black_box(&sv), &fl, &im, &vo))
        });
        let rows = mapping::instances(&sv, &fl, &im, &vo);
        g.bench_with_input(BenchmarkId::new("to_json", n), &n, |b, _| {
            b.iter(|| serde_json::to_vec(black_box(&rows)).expect("serialisable"))
        });
    }
    g.finish();
}

/// Project › Volumes: attachments named from the servers.
fn bench_volumes(c: &mut Criterion) {
    let mut g = c.benchmark_group("volumes");
    for &n in SIZES {
        let (vo, sv) = (volumes(n), servers(n));
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("map", n), &n, |b, _| {
            b.iter(|| mapping::volumes(black_box(&vo), &sv))
        });
    }
    g.finish();
}

/// Project › Networks: subnets joined and sorted by name.
fn bench_networks(c: &mut Criterion) {
    let mut g = c.benchmark_group("networks");
    for &n in SIZES {
        let (nets, subs) = networks(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("map", n), &n, |b, _| {
            b.iter(|| mapping::networks(black_box(&nets), &subs, PROJECT))
        });
    }
    g.finish();
}

/// Project › Images: the project's own first, owners named.
fn bench_images(c: &mut Criterion) {
    let mut g = c.benchmark_group("images");
    let names = HashMap::from([("other-project".to_string(), "Other".to_string())]);
    for &n in SIZES {
        let im = images(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("map", n), &n, |b, _| {
            b.iter(|| mapping::images(black_box(&im), PROJECT, &names, &[]))
        });
    }
    g.finish();
}

criterion_group!(benches, bench_instances, bench_volumes, bench_networks, bench_images);
criterion_main!(benches);
