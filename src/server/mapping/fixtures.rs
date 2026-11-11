//! Fixtures shared by the mapping tests: payloads shaped like the real APIs.

use super::*;
use serde_json::json;

/// (load balancer, listeners, pools, members per pool, monitors)
type Octavia = (Value, Vec<Value>, Vec<Value>, Vec<(String, Vec<Value>)>, Vec<Value>);

pub(super) fn octavia() -> Octavia {
    let lb = json!({
        "id": "lb-1", "name": "web-lb", "description": "", "project_id": "p1",
        "provisioning_status": "ACTIVE", "operating_status": "DEGRADED", "admin_state_up": true,
        "vip_address": "10.0.0.50", "vip_port_id": "port-vip", "vip_subnet_id": "sub-1",
        "vip_network_id": "net-1", "provider": "amphora", "flavor_id": "", "availability_zone": null,
        "listeners": [{"id": "ls-443"}, {"id": "ls-80"}], "pools": [{"id": "pool-1"}],
        "created_at": "2026-10-01T10:00:00", "updated_at": null
    });
    let listeners = vec![
        json!({"id": "ls-443", "name": "https", "protocol": "TERMINATED_HTTPS", "protocol_port": 443,
               "default_pool_id": "pool-1", "connection_limit": -1, "provisioning_status": "ACTIVE",
               "operating_status": "ONLINE", "admin_state_up": true}),
        json!({"id": "ls-80", "name": "", "protocol": "HTTP", "protocol_port": 80, "default_pool_id": null,
               "connection_limit": 5000, "provisioning_status": "ACTIVE", "operating_status": "ONLINE",
               "admin_state_up": false}),
    ];
    let pools = vec![json!({
        "id": "pool-1", "name": "web", "protocol": "HTTP", "lb_algorithm": "ROUND_ROBIN",
        "session_persistence": {"type": "APP_COOKIE", "cookie_name": "JSESSIONID"},
        "listeners": [{"id": "ls-443"}], "members": [{"id": "m-1"}, {"id": "m-2"}],
        "healthmonitor_id": "hm-1", "provisioning_status": "ACTIVE", "operating_status": "DEGRADED",
        "admin_state_up": true
    })];
    let members = vec![(
        "pool-1".to_string(),
        vec![
            json!({"id": "m-2", "name": "", "address": "10.0.0.12", "protocol_port": 8080, "weight": 1,
               "subnet_id": "sub-1", "backup": false, "provisioning_status": "ACTIVE",
               "operating_status": "ERROR", "admin_state_up": true,
               "monitor_address": "10.0.9.12", "monitor_port": 9000}),
            json!({"id": "m-1", "name": "web-1", "address": "10.0.0.11", "protocol_port": 8080, "weight": 10,
               "subnet_id": null, "backup": true, "provisioning_status": "ACTIVE",
               "operating_status": "ONLINE", "admin_state_up": true}),
        ],
    )];
    let monitors = vec![
        json!({"id": "hm-1", "name": "", "type": "HTTP", "delay": 5, "timeout": 3, "max_retries": 3,
               "max_retries_down": 3, "http_method": "GET", "url_path": "/healthz", "expected_codes": "200-204",
               "pools": [{"id": "pool-1"}], "provisioning_status": "ACTIVE", "operating_status": "ONLINE",
               "admin_state_up": true}),
        json!({"id": "hm-other", "name": "other-lb", "type": "TCP", "pools": [{"id": "pool-x"}]}),
    ];
    (lb, listeners, pools, members, monitors)
}

pub(super) fn server_fixture() -> Value {
    json!({
        "id": "3f2a9c1e-0000-0000-0000-000000000c91",
        "name": "web-prod-01",
        "status": "ACTIVE",
        "tenant_id": "p1",
        "created": "2026-09-14T10:22:00Z",
        "updated": "2026-10-01T08:00:00Z",
        "key_name": "ops-2026",
        "locked": false,
        "image": {"id": "img-rhel"},
        "flavor": {"id": "4"},
        "OS-EXT-AZ:availability_zone": "nova",
        "OS-EXT-STS:power_state": 1,
        "OS-EXT-STS:task_state": null,
        "OS-EXT-SRV-ATTR:host": "compute-03",
        "addresses": {
            "zeta-net": [{"addr": "10.9.0.5", "OS-EXT-IPS:type": "fixed"}],
            "ecommerce-net": [
                {"addr": "10.0.12.21", "version": 4, "OS-EXT-IPS:type": "fixed", "OS-EXT-IPS-MAC:mac_addr": "fa:16:3e:4a:12:21"},
                {"addr": "203.0.113.41", "version": 4, "OS-EXT-IPS:type": "floating"}
            ]
        },
        "security_groups": [{"name": "web"}, {"name": "default"}, {"name": "web"}],
        "os-extended-volumes:volumes_attached": [{"id": "vol-1"}, {"id": "vol-gone"}],
        "metadata": {"role": "frontend", "env": "prod"}
    })
}

pub(super) fn flavors() -> Vec<Value> {
    vec![json!({"id": "4", "name": "m1.large", "vcpus": 4, "ram": 8192, "disk": 40})]
}

pub(super) fn images_fixture() -> Vec<Value> {
    vec![json!({"id": "img-rhel", "name": "rhel-9.4"})]
}
