//! `OsAction`: every write Moonlight can send, and what it needs.

use super::*;

/// Every mutating operation Moonlight can perform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OsAction {
    /// Project › Quotas' *Edit Quotas* (roadmap 82): only the changed keys,
    /// each one of `service.keys()`.
    QuotaUpdate {
        service: QuotaService,
        project_id: String,
        values: std::collections::BTreeMap<String, i64>,
    },
    /// Admin › Defaults' *Update Defaults* (roadmap 85): the `default` quota
    /// class of Compute or Volume (`PUT os-quota-class-sets/default`); Neutron's
    /// defaults live in its configuration, so it has none. Admins only.
    DefaultQuotaUpdate {
        service: QuotaService,
        values: std::collections::BTreeMap<String, i64>,
    },
    // Admin › Metadata Definitions (roadmap 85); `id` is the namespace
    // (`OS::Compute::Watchdog`). Every one `admin_only`.
    /// *Import Namespace*: `POST /v2/metadefs/namespaces` with `definition`
    /// (Glance's JSON: namespace, properties, objects…), its `visibility` and
    /// `protected` set from the form, as Horizon.
    MetadefNamespaceImport {
        definition: String,
        public: bool,
        protected: bool,
    },
    /// *Edit Namespace*: `PUT …/namespaces/{id}`. Glance replaces the
    /// namespace's fields, so the dialog sends them all.
    MetadefNamespaceUpdate {
        id: String,
        display_name: Option<String>,
        description: Option<String>,
        public: bool,
        protected: bool,
    },
    /// `DELETE …/namespaces/{id}` (Glance refuses a protected one).
    MetadefNamespaceDelete {
        id: String,
    },
    /// *Update Associations*: `DELETE …/{id}/resource_types/{name}` per removed
    /// resource type, then `POST …/{id}/resource_types` per added one (names
    /// such as `OS::Nova::Flavor`, in `*_type_ids` so `check_ids` vets them).
    /// `prefixes` gives an added type its `prefix` (roadmap 89); Glance can't
    /// change one in place, so a new prefix is a removal and an addition.
    MetadefNamespaceSetTypes {
        id: String,
        add_type_ids: Vec<String>,
        remove_type_ids: Vec<String>,
        #[serde(default)]
        prefixes: Vec<(String, String)>,
    },
    // Instances
    ServerLaunch(LaunchSpec),
    ServerUpdate {
        id: String,
        name: String,
    },
    ServerStart {
        id: String,
    },
    ServerStop {
        id: String,
    },
    ServerReboot {
        id: String,
        kind: RebootKind,
    },
    ServerPause {
        id: String,
    },
    ServerUnpause {
        id: String,
    },
    ServerSuspend {
        id: String,
    },
    ServerResume {
        id: String,
    },
    ServerLock {
        id: String,
    },
    ServerUnlock {
        id: String,
    },
    ServerSnapshot {
        id: String,
        name: String,
    },
    ServerDelete {
        id: String,
    },
    /// Replaces all of the server's metadata (`PUT /servers/{id}/metadata`).
    ServerSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// *Edit Security Groups* (roadmap 42): Nova adds/removes each group **by name** on every
    /// port of the server, one `addSecurityGroup` / `removeSecurityGroup` action per group.
    ServerSetSecurityGroups {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    ServerAttachVolume {
        server_id: String,
        volume_id: String,
    },
    ServerDetachVolume {
        server_id: String,
        volume_id: String,
    },
    ServerResize {
        id: String,
        flavor_id: String,
    },
    ServerConfirmResize {
        id: String,
    },
    ServerRevertResize {
        id: String,
    },
    /// Nova >= 2.94 (sent by the SDK), which also rebuilds volume-backed servers (>= 2.93).
    ServerRebuild {
        id: String,
        image_id: String,
    },
    ServerShelve {
        id: String,
    },
    /// With an availability zone, Nova >= 2.77.
    ServerUnshelve {
        id: String,
        availability_zone: Option<String>,
    },
    /// Nova returns the rescue `adminPass`: shown once, never logged.
    ServerRescue {
        id: String,
        image_id: Option<String>,
    },
    ServerUnrescue {
        id: String,
    },
    /// Exactly one of `network_id` / `port_id`; `fixed_ip` only with a network.
    ServerAttachInterface {
        id: String,
        network_id: Option<String>,
        port_id: Option<String>,
        fixed_ip: Option<String>,
    },
    ServerDetachInterface {
        id: String,
        port_id: String,
    },
    // Images
    /// *Edit Image*: only the fields that changed (roadmap 33).
    ImageUpdate {
        id: String,
        changes: ImageChanges,
    },
    ImageDelete {
        id: String,
    },
    /// *Create Image*: `POST /v2/images`, then the `web-download` import when it has a URL (roadmap 45).
    ImageCreate(ImageSpec),
    /// Custom properties, as JSON-Patch: `set` adds or replaces, `remove` deletes.
    ImageSetProperties {
        id: String,
        set: Vec<(String, String)>,
        remove: Vec<String>,
    },
    // Key pairs & server groups
    /// `key_type`: `ssh` or `x509` (`KEYPAIR_TYPES`).
    KeyPairImport {
        name: String,
        public_key: String,
        key_type: String,
    },
    /// Generated on the Moonlight server (Nova >= 2.92 no longer generates
    /// keys); only the public key is sent, the private key is returned once.
    KeyPairGenerate {
        name: String,
    },
    KeyPairDelete {
        name: String,
    },
    ServerGroupCreate {
        name: String,
        policy: String,
    },
    ServerGroupDelete {
        id: String,
    },
    // Volumes & snapshots
    VolumeCreate {
        name: String,
        description: Option<String>,
        size_gb: u32,
        volume_type: Option<String>,
        availability_zone: Option<String>,
        source: VolumeSource,
        source_id: Option<String>,
    },
    VolumeUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    VolumeExtend {
        id: String,
        new_size_gb: u32,
    },
    VolumeDelete {
        id: String,
    },
    /// Replaces all of the volume's metadata (`PUT /volumes/{id}/metadata`).
    VolumeSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// `migrate`: Cinder may move the data to another backend (`on-demand`).
    VolumeRetype {
        id: String,
        new_type: String,
        migrate: bool,
    },
    /// Creates a Glance image from the volume; `force` uploads an in-use volume.
    VolumeUploadImage {
        id: String,
        image_name: String,
        disk_format: String,
        force: bool,
        /// Glance visibility of the new image (preset from `[images] default_visibility`).
        #[serde(default)]
        visibility: Option<String>,
    },
    /// The answer carries the transfer ID and `auth_key`: shown once, never logged.
    VolumeTransferCreate {
        volume_id: String,
        name: Option<String>,
    },
    /// `auth_key` is typed by the user; `Debug` would show it, so only `kind()`
    /// may be logged (as for every action).
    VolumeTransferAccept {
        transfer_id: String,
        auth_key: String,
    },
    VolumeSnapshotCreate {
        volume_id: String,
        name: String,
        description: Option<String>,
        force: bool,
    },
    SnapshotUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    SnapshotDelete {
        id: String,
    },
    /// Replaces all of the snapshot's metadata (`PUT /snapshots/{id}/metadata`).
    SnapshotSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// `POST /groups/{id}/action` `{"delete": {"delete-volumes": …}}` (volume 3.13).
    /// Without `delete_volumes` Cinder refuses a group that still has volumes.
    VolumeGroupDelete {
        id: String,
        delete_volumes: bool,
    },
    /// *Create Group*: `POST /groups` (volume 3.13). Cinder needs the group type
    /// and at least one volume type; `None` zone lets Cinder pick.
    VolumeGroupCreate {
        name: String,
        description: Option<String>,
        group_type_id: String,
        volume_type_ids: Vec<String>,
        availability_zone: Option<String>,
    },
    /// *Clone Group* (`source_group_id`) or *Create Group* from a group snapshot
    /// (`group_snapshot_id`): `POST /groups/action` `create-from-src` (volume
    /// 3.14). Exactly one source must be set.
    VolumeGroupCreateFrom {
        name: String,
        description: Option<String>,
        source_group_id: Option<String>,
        group_snapshot_id: Option<String>,
    },
    /// *Edit Group*: `PUT /groups/{id}` with the name and description.
    VolumeGroupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    /// *Manage Volumes* / *Remove Volumes from Group*: `PUT /groups/{id}` with
    /// `add_volumes` / `remove_volumes` as comma-separated IDs (Cinder's format);
    /// an empty list is left out of the body.
    VolumeGroupSetVolumes {
        id: String,
        add_volume_ids: Vec<String>,
        remove_volume_ids: Vec<String>,
    },
    /// *Create Snapshot* of a group: `POST /group_snapshots` (volume 3.14).
    GroupSnapshotCreate {
        group_id: String,
        name: String,
        description: Option<String>,
    },
    /// `DELETE /group_snapshots/{id}` (volume 3.14).
    GroupSnapshotDelete {
        id: String,
    },
    /// *Create Backup*: `POST /backups`. `force` backs up an attached volume,
    /// `incremental` needs an earlier full backup of the volume; `snapshot_id`
    /// backs up that snapshot instead of the volume's current data.
    BackupCreate {
        volume_id: String,
        name: String,
        description: Option<String>,
        incremental: bool,
        force: bool,
        snapshot_id: Option<String>,
    },
    /// *Edit Backup*: `PUT /backups/{id}` (name and description; volume 3.9).
    BackupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    /// *Restore Backup*: `POST /backups/{id}/restore`. Without `volume_id` Cinder
    /// creates a new volume (named `name`); with it, that volume is overwritten.
    BackupRestore {
        id: String,
        volume_id: Option<String>,
        name: Option<String>,
    },
    /// `DELETE /backups/{id}`.
    BackupDelete {
        id: String,
    },
    // Networks, subnets, ports
    NetworkCreate {
        name: String,
        admin_up: bool,
        subnet: Option<SubnetSpec>,
    },
    /// Admin › Networks' *Create Network* (roadmap 85): for any project, with
    /// Neutron's `provider` attributes, *Shared* and *External*. Admins only
    /// (`admin_only`), whatever Neutron's policy says.
    AdminNetworkCreate(Box<AdminNetworkSpec>),
    NetworkUpdate {
        id: String,
        name: String,
        admin_up: bool,
    },
    /// Admin › Networks' *Edit Network*: Horizon's admin form, which adds
    /// *Shared* and *External Network* (`router:external`). Provider attributes
    /// are not editable: ML2 refuses to change them. Admins only.
    AdminNetworkUpdate {
        id: String,
        name: String,
        admin_up: bool,
        shared: bool,
        external: bool,
    },
    /// Admin › RBAC Policies' *Create RBAC Policy*: `POST /rbac-policies`.
    /// `kind` is one of [`RBAC_KINDS`] (Horizon's *Action and Object Type*);
    /// `target_project` a project ID or `*` (every project). Admins only.
    RbacPolicyCreate {
        kind: String,
        object_id: String,
        target_project: String,
    },
    /// *Edit Policy* (Horizon's *Update RBAC Policy*): `PUT /rbac-policies/{id}`
    /// with a new `target_tenant`, a project ID or `*`. Admins only.
    RbacPolicyUpdate {
        id: String,
        target_project: String,
    },
    /// `DELETE /rbac-policies/{id}`. Admins only.
    RbacPolicyDelete {
        id: String,
    },
    NetworkDelete {
        id: String,
    },
    SubnetCreate {
        network_id: String,
        spec: SubnetSpec,
    },
    SubnetUpdate {
        id: String,
        name: String,
        enable_dhcp: bool,
        dns_nameservers: Vec<String>,
    },
    SubnetDelete {
        id: String,
    },
    PortCreate {
        network_id: String,
        name: String,
        admin_up: bool,
        fixed_ip: Option<String>,
    },
    /// *Edit Port*: only the `Some` fields are sent (see [`PortSettings::changes`]).
    PortUpdate {
        id: String,
        name: Option<String>,
        admin_up: Option<bool>,
        /// `binding:vnic_type` (`normal`, `direct`, `macvtap`…); needs `binding`.
        vnic_type: Option<String>,
        /// Needs the `port-security` extension.
        port_security: Option<bool>,
        security_group_ids: Option<Vec<String>>,
    },
    /// Replaces the port's security groups (an empty list removes them all).
    PortSetSecurityGroups {
        id: String,
        security_group_ids: Vec<String>,
    },
    PortDelete {
        id: String,
    },
    // Routers
    RouterCreate {
        name: String,
        admin_up: bool,
        external_network_id: Option<String>,
        /// The gateway's QoS policy (`qos-gateway-ip`; needs the `qos` plugin).
        #[serde(default)]
        qos_policy_id: Option<String>,
    },
    /// Admin › Routers' *Create Router* (roadmap 85): for any project, with
    /// SNAT, zone hints, DVR and HA. Admins only (`admin_only`).
    AdminRouterCreate(Box<AdminRouterSpec>),
    RouterUpdate {
        id: String,
        name: String,
        admin_up: bool,
        /// `(gateway network ID, enable_snat)`, only when SNAT changes: Neutron
        /// wants the network in the same `external_gateway_info`.
        #[serde(default)]
        snat: Option<(String, bool)>,
        /// Admin *Edit Router* (roadmap 89): the new router type (`true` =
        /// distributed) and HA mode, only when they change. Neutron accepts
        /// them only while the router is down.
        #[serde(default)]
        distributed: Option<bool>,
        #[serde(default)]
        ha: Option<bool>,
    },
    RouterSetGateway {
        id: String,
        network_id: String,
    },
    RouterClearGateway {
        id: String,
    },
    RouterAddInterface {
        id: String,
        subnet_id: String,
    },
    RouterRemoveInterface {
        id: String,
        port_id: String,
    },
    /// Atomic (`extraroute-atomic`): only this route changes.
    RouterAddRoute {
        id: String,
        destination: String,
        nexthop: String,
    },
    RouterRemoveRoute {
        id: String,
        destination: String,
        nexthop: String,
    },
    /// Plain `extraroute` (no `-atomic`): replaces the router's whole route list.
    RouterSetRoutes {
        id: String,
        routes: Vec<(String, String)>,
    },
    RouterDelete {
        id: String,
    },
    // Security groups
    SecurityGroupCreate {
        name: String,
        description: Option<String>,
    },
    SecurityGroupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    SecurityGroupDelete {
        id: String,
    },
    RuleCreate(RuleSpec),
    RuleDelete {
        id: String,
    },
    // Floating IPs
    /// With `port_id`, Neutron allocates and associates in one request.
    FloatingIpAllocate {
        network_id: String,
        description: Option<String>,
        #[serde(default)]
        port_id: Option<String>,
        /// `qos-fip`; needs the `qos` plugin.
        #[serde(default)]
        qos_policy_id: Option<String>,
    },
    /// Admin › Floating IPs' *Allocate IP To Project* (roadmap 85): for any
    /// project, optionally a given address of the pool. Admins only.
    AdminFloatingIpAllocate {
        network_id: String,
        project_id: String,
        address: Option<String>,
        description: Option<String>,
    },
    // Admin › Compute (roadmap 85); every one `admin_only`.
    /// *Create Flavor*: `POST /flavors`, then `addTenantAccess` per project.
    FlavorCreate(Box<FlavorSpec>),
    /// *Edit Flavor* (`[admin] flavor_edit`, roadmap 89), Horizon's: Nova can't
    /// change a flavor, so `DELETE /flavors/{id}`, `POST /flavors` with the same
    /// ID (`spec.flavor_id`), then its `extra_specs` and `addTenantAccess` back.
    FlavorReplace {
        spec: Box<FlavorSpec>,
        extra_specs: Vec<(String, String)>,
    },
    /// `DELETE /flavors/{id}`.
    FlavorDelete {
        id: String,
    },
    /// *Update Metadata* of a flavor: `POST …/os-extra_specs` with `set`, then
    /// one `DELETE …/os-extra_specs/{key}` per key in `unset`.
    FlavorSetExtraSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// *Modify Access* of a private flavor: `addTenantAccess` /
    /// `removeTenantAccess` per project ID.
    FlavorSetAccess {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    /// *Create Host Aggregate*: `POST /os-aggregates`, then `add_host` per host.
    AggregateCreate {
        name: String,
        zone: Option<String>,
        hosts: Vec<String>,
    },
    /// *Edit Host Aggregate*: `PUT /os-aggregates/{id}`; `zone: None` clears it.
    AggregateUpdate {
        id: String,
        name: String,
        zone: Option<String>,
    },
    /// *Manage Hosts*: `add_host` / `remove_host` actions, one per host.
    AggregateSetHosts {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    /// *Update Metadata* of an aggregate: one `set_metadata`, `unset` keys as `null`.
    AggregateSetMetadata {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// `DELETE /os-aggregates/{id}` (Nova refuses one that still has hosts).
    AggregateDelete {
        id: String,
    },
    /// *Enable* / *Disable Service* of a `nova-compute`: `PUT /os-services/{id}`
    /// (compute 2.53, UUID `id`); `reason` only when disabling.
    ComputeServiceSet {
        id: String,
        enabled: bool,
        reason: Option<String>,
    },
    /// *Migrate Instance* (cold): `migrate` action; `host` needs compute 2.56.
    ServerMigrate {
        id: String,
        host: Option<String>,
    },
    /// *Live Migrate Instance*: `os-migrateLive` at compute 2.30. `host: None`
    /// lets the scheduler pick; `block_migration: None` sends `"auto"` (2.25).
    ServerLiveMigrate {
        id: String,
        host: Option<String>,
        block_migration: Option<bool>,
    },
    // Admin › Volume (roadmap 85); every one `admin_only`.
    /// *Update Volume Status*: `os-reset_status` with `status` only (Cinder's
    /// record, not the back end), one of [`VOLUME_STATUSES`].
    VolumeResetStatus {
        id: String,
        status: String,
    },
    /// *Migrate Volume*: `os-migrate_volume` to a back-end pool
    /// (`host@backend#pool`); `force_host_copy` skips the driver's own migration.
    VolumeMigrate {
        id: String,
        host: String,
        force_host_copy: bool,
    },
    /// *Manage Volume*: `POST /os-volume-manage`, a back-end volume Cinder
    /// doesn't know yet becomes one, in the session's project.
    VolumeManage(Box<ManageVolumeSpec>),
    /// *Unmanage Volume*: `os-unmanage`; Cinder forgets it, the back end keeps it.
    VolumeUnmanage {
        id: String,
    },
    /// *Update Status* of a snapshot: `os-reset_status`, one of [`SNAPSHOT_STATUSES`].
    SnapshotResetStatus {
        id: String,
        status: String,
    },
    /// *Create Volume Type*: `POST /types` (raw: the SDK's `type::create` posts
    /// to `types/{id}/action`).
    VolumeTypeCreate {
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// *Edit Volume Type*: `PUT /types/{id}`; an empty description clears it.
    VolumeTypeUpdate {
        id: String,
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// `DELETE /types/{id}` (Cinder refuses one still in use).
    VolumeTypeDelete {
        id: String,
    },
    /// *View Extra Specs*: `POST …/extra_specs` with `set`, then one `DELETE
    /// …/extra_specs/{key}` per key in `unset`.
    VolumeTypeSetExtraSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// *Manage QoS Spec Association*: disassociate `current_qos_id`, then
    /// associate `qos_id` (`None`: leave it without). Cinder makes both GETs
    /// with `vol_type_id` (they change state: sent through `Exec`, as writes).
    VolumeTypeSetQos {
        id: String,
        qos_id: Option<String>,
        current_qos_id: Option<String>,
    },
    /// *Create Encryption* (`POST …/encryption`) or, with `update`, *Update
    /// Encryption* (`PUT …/encryption/provider`, as cinderclient: Cinder
    /// ignores the last segment).
    VolumeTypeEncryptionSet {
        id: String,
        spec: EncryptionSpec,
        update: bool,
    },
    /// *Delete Encryption*: `DELETE …/encryption/provider`.
    VolumeTypeEncryptionDelete {
        id: String,
    },
    /// *Create QoS Spec*: `POST /qos-specs` with the consumer and specs.
    QosSpecCreate {
        name: String,
        consumer: String,
        specs: Vec<(String, String)>,
    },
    /// *Edit Consumer*: `PUT /qos-specs/{id}` with `consumer`.
    QosSpecSetConsumer {
        id: String,
        consumer: String,
    },
    /// *Manage Specs*: `PUT /qos-specs/{id}` with `set`, then `PUT
    /// …/delete_keys` with `unset`.
    QosSpecSetSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// `DELETE /qos-specs/{id}` (Cinder refuses one still associated).
    QosSpecDelete {
        id: String,
    },
    /// *Create Group Type*: `POST /group_types` (volume 3.11).
    GroupTypeCreate {
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// *Edit Group Type*: `PUT /group_types/{id}`.
    GroupTypeUpdate {
        id: String,
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// `DELETE /group_types/{id}`.
    GroupTypeDelete {
        id: String,
    },
    /// *View Specs* of a group type: `POST …/group_specs` with `set`, then one
    /// `DELETE …/group_specs/{key}` per key in `unset`.
    GroupTypeSetSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    FloatingIpAssociate {
        id: String,
        port_id: String,
    },
    FloatingIpDisassociate {
        id: String,
    },
    FloatingIpRelease {
        id: String,
    },
    PortForwardingCreate(PortForwardingSpec),
    PortForwardingUpdate {
        id: String,
        spec: PortForwardingSpec,
    },
    PortForwardingDelete {
        floating_ip_id: String,
        id: String,
    },
    // Load balancers
    /// `cascade` also deletes listeners, pools, members and monitors (Octavia
    /// refuses to delete a load balancer that still has them otherwise).
    LoadBalancerDelete {
        id: String,
        cascade: bool,
    },
    /// Boxed: by far the largest action.
    LoadBalancerCreate(Box<LbCreateSpec>),
    LoadBalancerUpdate {
        id: String,
        name: String,
        description: String,
        admin_up: bool,
    },
    ListenerCreate {
        loadbalancer_id: String,
        spec: LbListenerSpec,
        default_pool_id: Option<String>,
    },
    ListenerUpdate {
        id: String,
        name: String,
        description: String,
        connection_limit: Option<u32>,
        default_pool_id: Option<String>,
        admin_up: bool,
        /// The listener's protocol, to check the options (not sent).
        #[serde(default)]
        protocol: String,
        /// `None`: leave the options alone (the cloud's Octavia is too old for
        /// them, or the dialog didn't show them).
        #[serde(default)]
        options: Option<LbListenerOptions>,
        /// `TERMINATED_HTTPS` listeners: `(certificate, SNI certificates)`;
        /// `None` = unchanged.
        #[serde(default)]
        tls: Option<(String, Vec<String>)>,
    },
    ListenerDelete {
        id: String,
    },
    /// `listener_id`: also becomes that listener's default pool.
    PoolCreate {
        loadbalancer_id: String,
        listener_id: Option<String>,
        spec: LbPoolSpec,
    },
    PoolUpdate {
        id: String,
        name: String,
        description: String,
        algorithm: String,
        session_persistence: Option<String>,
        cookie_name: Option<String>,
        admin_up: bool,
        /// The pool's protocol, to check `tls_enabled` (not sent).
        #[serde(default)]
        protocol: String,
        /// `(tls_enabled, tls_ciphers)`; `None`: left alone (Octavia < 2.8).
        #[serde(default)]
        tls: Option<(bool, Option<String>)>,
    },
    PoolDelete {
        id: String,
    },
    MemberCreate {
        pool_id: String,
        spec: LbMemberSpec,
    },
    /// `None` monitor fields are sent as `null` (health-check the member itself).
    MemberUpdate {
        pool_id: String,
        id: String,
        name: String,
        weight: u16,
        backup: bool,
        admin_up: bool,
        #[serde(default)]
        monitor_address: Option<String>,
        #[serde(default)]
        monitor_port: Option<u16>,
    },
    /// Octavia's batch update (`PUT /lbaas/pools/{id}/members`): creates the new
    /// members, updates those matched by address and port, and **deletes the
    /// ones left out** (roadmap 54).
    PoolSetMembers {
        pool_id: String,
        members: Vec<LbMemberSpec>,
    },
    /// `POST /v3/users/{user_id}/application_credentials`; the secret comes back
    /// once (`ActionOutcome.secret`, with `openrc` and `clouds.yaml`).
    AppCredentialCreate(Box<AppCredentialSpec>),
    /// Identity › Projects (admins, roadmap 57): `POST /v3/projects` in
    /// `domain_id`, set by `run_action` (see [`OsAction::set_domain`]).
    ProjectCreate {
        name: String,
        description: Option<String>,
        enabled: bool,
        #[serde(default)]
        domain_id: String,
    },
    /// `PATCH /v3/projects/{id}` (Horizon's *Edit Project*, including enabled).
    ProjectUpdate {
        id: String,
        name: String,
        description: String,
        enabled: bool,
    },
    /// `DELETE /v3/projects/{id}`.
    ProjectDelete {
        id: String,
    },
    /// `PUT /v3/projects/{p}/users/{u}/roles/{r}` or `…/groups/{g}/…` (one of
    /// `user_id` / `group_id`): *Manage Members* / *Modify Groups*.
    ProjectRoleGrant {
        project_id: String,
        user_id: Option<String>,
        group_id: Option<String>,
        role_id: String,
    },
    /// The same with `DELETE`.
    ProjectRoleRevoke {
        project_id: String,
        user_id: Option<String>,
        group_id: Option<String>,
        role_id: String,
    },
    /// Identity › Users: `POST /v3/users`, then the role on the primary project.
    UserCreate(Box<UserSpec>),
    /// `PATCH /v3/users/{id}` (name, email, description, primary project).
    UserUpdate {
        id: String,
        name: String,
        email: String,
        description: String,
        project_id: Option<String>,
    },
    /// `PATCH /v3/users/{id}` with `enabled`.
    UserSetEnabled {
        id: String,
        enabled: bool,
    },
    /// `PATCH /v3/users/{id}` with `password` (an admin setting it; masked).
    UserSetPassword {
        id: String,
        password: String,
    },
    /// `DELETE /v3/users/{id}`.
    UserDelete {
        id: String,
    },
    /// Identity › Groups: `POST /v3/groups` in `domain_id` (set by `run_action`).
    GroupCreate {
        name: String,
        description: Option<String>,
        #[serde(default)]
        domain_id: String,
    },
    /// `PATCH /v3/groups/{id}`.
    GroupUpdate {
        id: String,
        name: String,
        description: String,
    },
    /// `DELETE /v3/groups/{id}`.
    GroupDelete {
        id: String,
    },
    /// `PUT /v3/groups/{g}/users/{u}`.
    GroupAddUser {
        group_id: String,
        user_id: String,
    },
    /// `DELETE /v3/groups/{g}/users/{u}`.
    GroupRemoveUser {
        group_id: String,
        user_id: String,
    },
    /// Identity › Roles (admins, roadmap 57): `POST /v3/roles`.
    RoleCreate {
        name: String,
    },
    /// `PATCH /v3/roles/{id}` (the name; Horizon's *Edit*).
    RoleUpdate {
        id: String,
        name: String,
    },
    /// `DELETE /v3/roles/{id}`: also removes every assignment of the role.
    RoleDelete {
        id: String,
    },
    /// `DELETE /v3/users/{user_id}/application_credentials/{id}`.
    AppCredentialDelete {
        #[serde(default)]
        user_id: String,
        id: String,
    },
    MemberDelete {
        pool_id: String,
        id: String,
    },
    MonitorCreate {
        pool_id: String,
        spec: LbMonitorSpec,
    },
    /// The type cannot change; every other field can.
    MonitorUpdate {
        id: String,
        spec: LbMonitorSpec,
        admin_up: bool,
    },
    MonitorDelete {
        id: String,
    },
    /// L7 policies and rules (roadmap 54).
    L7PolicyCreate {
        listener_id: String,
        spec: LbL7PolicySpec,
    },
    /// JSON: the unused redirect target is sent as `null`.
    L7PolicyUpdate {
        id: String,
        spec: LbL7PolicySpec,
    },
    L7PolicyDelete {
        id: String,
    },
    L7RuleCreate {
        policy_id: String,
        spec: LbL7RuleSpec,
    },
    /// JSON: a removed key is sent as `null`.
    L7RuleUpdate {
        policy_id: String,
        id: String,
        spec: LbL7RuleSpec,
    },
    L7RuleDelete {
        policy_id: String,
        id: String,
    },
    // Trunks
    TrunkCreate {
        name: String,
        description: Option<String>,
        port_id: String,
        admin_up: bool,
        subports: Vec<SubportSpec>,
    },
    TrunkUpdate {
        id: String,
        name: String,
        description: String,
        admin_up: bool,
    },
    TrunkDelete {
        id: String,
    },
    TrunkAddSubports {
        id: String,
        subports: Vec<SubportSpec>,
    },
    TrunkRemoveSubports {
        id: String,
        port_ids: Vec<String>,
    },
    // Firewall (FWaaS v2, roadmap 93)
    FirewallRuleCreate(FirewallRuleSpec),
    FirewallRuleUpdate {
        id: String,
        spec: FirewallRuleSpec,
    },
    FirewallRuleDelete {
        id: String,
    },
    /// `rule_ids` in evaluation order.
    FirewallPolicyCreate {
        name: String,
        description: String,
        rule_ids: Vec<String>,
        audited: bool,
    },
    FirewallPolicyUpdate {
        id: String,
        name: String,
        description: String,
        audited: bool,
    },
    FirewallPolicyDelete {
        id: String,
    },
    /// Inserts a rule before or after one already in the policy (neither: first).
    FirewallPolicyInsertRule {
        id: String,
        rule_id: String,
        before_id: Option<String>,
        after_id: Option<String>,
    },
    FirewallPolicyRemoveRule {
        id: String,
        rule_id: String,
    },
    /// `None` policies: no ingress / egress filtering.
    FirewallGroupCreate {
        name: String,
        description: String,
        ingress_policy_id: Option<String>,
        egress_policy_id: Option<String>,
        port_ids: Vec<String>,
        admin_up: bool,
    },
    /// `None` policies are cleared (`null`).
    FirewallGroupUpdate {
        id: String,
        name: String,
        description: String,
        ingress_policy_id: Option<String>,
        egress_policy_id: Option<String>,
        admin_up: bool,
    },
    /// *Add Port* / *Remove Port*: Neutron takes the whole new list.
    FirewallGroupSetPorts {
        id: String,
        port_ids: Vec<String>,
    },
    FirewallGroupDelete {
        id: String,
    },
    // Network QoS
    QosPolicyCreate {
        name: String,
        description: Option<String>,
        shared: bool,
    },
    QosPolicyUpdate {
        id: String,
        name: String,
        description: String,
        shared: bool,
    },
    QosPolicyDelete {
        id: String,
    },
    QosRuleCreate {
        policy_id: String,
        spec: QosRuleSpec,
    },
    QosRuleUpdate {
        policy_id: String,
        id: String,
        spec: QosRuleSpec,
    },
    QosRuleDelete {
        policy_id: String,
        kind: String,
        id: String,
    },
    // Key Manager (roadmap 55)
    SecretCreate(SecretSpec),
    SecretDelete {
        id: String,
    },
    /// `secrets`: (role, `secret_ref` URL) pairs; Barbican takes refs, not IDs.
    ContainerCreate {
        name: Option<String>,
        container_type: String,
        secrets: Vec<(String, String)>,
    },
    /// Deletes the container only; its secrets stay.
    ContainerDelete {
        id: String,
    },
    /// *Edit Access* (roadmap 75): `PUT v1/secrets/{id}/acl`, which replaces the
    /// whole `read` ACL. `users` are Keystone user IDs (checked like IDs).
    SecretAclSet {
        id: String,
        project_access: bool,
        users: Vec<String>,
    },
    /// *Reset Access*: `DELETE v1/secrets/{id}/acl`, back to Barbican's default
    /// (every project member, no extra users).
    SecretAclDelete {
        id: String,
    },
    /// *Edit Access* on a container: `PUT v1/containers/{id}/acl`. The secrets in
    /// it have their own ACLs (Octavia needs both).
    ContainerAclSet {
        id: String,
        project_access: bool,
        users: Vec<String>,
    },
    /// *Reset Access* on a container: `DELETE v1/containers/{id}/acl`.
    ContainerAclDelete {
        id: String,
    },
    /// *Order Key* (roadmap 58): `POST v1/orders`; Barbican generates the key
    /// or key pair asynchronously.
    OrderCreate(OrderSpec),
    /// *Delete Order*: `DELETE v1/orders/{id}`, the order record only; the
    /// generated secret or container stays.
    OrderDelete {
        id: String,
    },
    // Orchestration (Heat, roadmap 62). Heat paths take the stack's name and
    // ID (`stacks/{name}/{id}`), so both travel; `name` is checked like an ID.
    /// `POST …/actions {"check": null}`: Heat checks each resource still exists.
    StackCheck {
        name: String,
        id: String,
    },
    /// `{"suspend": null}`: suspends the resources that can be (servers are paused…).
    StackSuspend {
        name: String,
        id: String,
    },
    /// `{"resume": null}`.
    StackResume {
        name: String,
        id: String,
    },
    /// Deletes the stack and every resource it created.
    StackDelete {
        name: String,
        id: String,
    },
    /// `POST stacks`.
    StackCreate(StackSpec),
    /// *Change Stack Template*: `PATCH stacks/{name}/{id}`, so parameters left
    /// out (hidden ones the form can't show) and the environment keep their values.
    StackUpdate {
        id: String,
        spec: StackSpec,
    },
    // Alarming (Aodh, roadmap 79)
    /// `POST v2/alarms`.
    AlarmCreate(AlarmSpec),
    /// `PUT v2/alarms/{id}`: Aodh replaces the alarm, so `spec` is all of it.
    AlarmUpdate {
        id: String,
        spec: AlarmSpec,
    },
    // Bulk
    /// Runs every action in order, continuing after a failure (Horizon's bulk
    /// *Delete …*). Each item carries its display name for the results list.
    /// Not nestable; at most [`MAX_BATCH`] items.
    Batch(Vec<(String, OsAction)>),
}

impl OsAction {
    /// Variant name for logs (no IDs, names or keys).
    /// The custom image property keys an action writes or removes (checked against
    /// `[images] reserved_properties` on the server, roadmap 25).
    pub fn image_property_keys(&self) -> Vec<&str> {
        match self {
            OsAction::ImageCreate(spec) => spec.properties.iter().map(|(k, _)| k.as_str()).collect(),
            OsAction::ImageSetProperties { set, remove, .. } => set
                .iter()
                .map(|(k, _)| k.as_str())
                .chain(remove.iter().map(String::as_str))
                .collect(),
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.image_property_keys()).collect(),
            _ => vec![],
        }
    }

    /// CIDRs of the subnets an action creates (checked against `[network]` on the server).
    pub fn new_subnet_cidrs(&self) -> Vec<&str> {
        match self {
            OsAction::NetworkCreate { subnet: Some(s), .. } => vec![s.cidr.as_str()],
            OsAction::AdminNetworkCreate(spec) => spec.subnet.iter().map(|s| s.cidr.as_str()).collect(),
            OsAction::SubnetCreate { spec, .. } => vec![spec.cidr.as_str()],
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.new_subnet_cidrs()).collect(),
            _ => vec![],
        }
    }

    /// Points identity actions at the signed-in user (roadmap 57): users manage
    /// their own application credentials, whatever the browser sent.
    pub fn set_user(&mut self, me: &str) {
        match self {
            OsAction::AppCredentialCreate(spec) => spec.user_id = me.to_string(),
            OsAction::AppCredentialDelete { user_id, .. } => *user_id = me.to_string(),
            OsAction::Batch(items) => items.iter_mut().for_each(|(_, a)| a.set_user(me)),
            _ => {}
        }
    }

    /// Whether the action creates something owned by a domain (a project, user
    /// or group), so `run_action` must say which.
    pub fn needs_domain(&self) -> bool {
        match self {
            OsAction::ProjectCreate { .. } | OsAction::UserCreate(_) | OsAction::GroupCreate { .. } => true,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.needs_domain()),
            _ => false,
        }
    }

    /// Puts projects, users and groups in `domain`: the session's domain context,
    /// else the user's own domain, like Horizon. Without it Keystone would use
    /// its default domain (`Default`) for a project-scoped token, not the user's
    /// (roadmap 57: a project created from *ttcp-spain* landed in `Default`).
    pub fn set_domain(&mut self, domain: &str) {
        match self {
            OsAction::ProjectCreate { domain_id, .. } | OsAction::GroupCreate { domain_id, .. } => {
                *domain_id = domain.to_string()
            }
            OsAction::UserCreate(spec) => spec.domain_id = domain.to_string(),
            OsAction::Batch(items) => items.iter_mut().for_each(|(_, a)| a.set_domain(domain)),
            _ => {}
        }
    }

    /// Horizon's identity safety rules (roadmap 57), checked by `run_action`
    /// with the signed-in user and project: no deleting or disabling oneself or
    /// the current project, and no removing one's own roles there (stricter than
    /// Horizon, which guards `admin` only: telling roles apart needs a lookup
    /// dry-run can't make, and this way nobody locks themselves out).
    pub fn identity_safety(&self, me: &str, project: &str) -> Result<(), String> {
        match self {
            OsAction::UserDelete { id } if id == me => Err("you cannot delete your own user".into()),
            OsAction::UserSetEnabled { id, enabled: false } if id == me => {
                Err("you cannot disable your own user".into())
            }
            OsAction::ProjectDelete { id } if id == project => {
                Err("you cannot delete the project you are signed in to".into())
            }
            OsAction::ProjectUpdate { id, enabled: false, .. } if id == project => {
                Err("you cannot disable the project you are signed in to".into())
            }
            OsAction::ProjectRoleRevoke {
                project_id,
                user_id: Some(u),
                ..
            } if project_id == project && u == me => {
                Err("you cannot remove your own roles on the project you are signed in to".into())
            }
            OsAction::Batch(items) => items.iter().try_for_each(|(_, a)| a.identity_safety(me, project)),
            _ => Ok(()),
        }
    }

    /// Every ID in the action is safe to paste into a URL path (roadmap 48): the
    /// strings under a key that is `id`, ends in `_id`/`_ids`, or is `kind` (QoS
    /// rule paths), and key pair names. Found by walking the serialised action,
    /// so new variants are covered without a list to keep. Empty strings pass:
    /// the action itself rejects a missing required ID.
    pub fn check_ids(&self) -> Result<(), AppError> {
        fn walk(v: &serde_json::Value, key: &str, keypair: bool) -> Result<(), AppError> {
            use serde_json::Value;
            let path_key = key == "id"
                || key.ends_with("_id")
                || key.ends_with("_ids")
                || key == "kind"
                || (keypair && key == "name");
            match v {
                Value::String(s) if path_key && !s.is_empty() => check_id(s),
                Value::Array(items) => items.iter().try_for_each(|x| walk(x, key, keypair)),
                Value::Object(m) => m.iter().try_for_each(|(k, x)| walk(x, k, keypair)),
                _ => Ok(()),
            }
        }
        // Nova puts key pair names in the path (`os-keypairs/{name}`), Heat stack
        // names (`stacks/{name}/{id}`).
        let keypair = matches!(
            self,
            OsAction::KeyPairImport { .. }
                | OsAction::KeyPairGenerate { .. }
                | OsAction::KeyPairDelete { .. }
                | OsAction::StackCheck { .. }
                | OsAction::StackSuspend { .. }
                | OsAction::StackResume { .. }
                | OsAction::StackDelete { .. }
                | OsAction::StackCreate(_)
                | OsAction::StackUpdate { .. }
        );
        let v = serde_json::to_value(self).map_err(|e| AppError::invalid(e.to_string()))?;
        walk(&v, "", keypair)
    }

    /// The plugin an action belongs to, if any (checked on the server).
    pub fn plugins(&self) -> Vec<Plugin> {
        let tls = |l: &LbListenerSpec| l.protocol == LB_TLS_PROTOCOL;
        match self {
            // Certificates come from Barbican (roadmap 54).
            OsAction::ListenerCreate { spec, .. } if tls(spec) => vec![Plugin::LoadBalancer, Plugin::KeyManager],
            OsAction::LoadBalancerCreate(s) if s.listener.as_ref().is_some_and(tls) => {
                vec![Plugin::LoadBalancer, Plugin::KeyManager]
            }
            OsAction::ListenerUpdate { tls: Some(_), .. } => vec![Plugin::LoadBalancer, Plugin::KeyManager],
            OsAction::LoadBalancerDelete { .. }
            | OsAction::LoadBalancerCreate(_)
            | OsAction::LoadBalancerUpdate { .. }
            | OsAction::ListenerCreate { .. }
            | OsAction::ListenerUpdate { .. }
            | OsAction::ListenerDelete { .. }
            | OsAction::PoolCreate { .. }
            | OsAction::PoolUpdate { .. }
            | OsAction::PoolDelete { .. }
            | OsAction::MemberCreate { .. }
            | OsAction::MemberUpdate { .. }
            | OsAction::PoolSetMembers { .. }
            | OsAction::MemberDelete { .. }
            | OsAction::MonitorCreate { .. }
            | OsAction::MonitorUpdate { .. }
            | OsAction::MonitorDelete { .. }
            | OsAction::L7PolicyCreate { .. }
            | OsAction::L7PolicyUpdate { .. }
            | OsAction::L7PolicyDelete { .. }
            | OsAction::L7RuleCreate { .. }
            | OsAction::L7RuleUpdate { .. }
            | OsAction::L7RuleDelete { .. } => vec![Plugin::LoadBalancer],
            OsAction::QosPolicyCreate { .. }
            | OsAction::QosPolicyUpdate { .. }
            | OsAction::QosPolicyDelete { .. }
            | OsAction::QosRuleCreate { .. }
            | OsAction::QosRuleUpdate { .. }
            | OsAction::QosRuleDelete { .. } => vec![Plugin::Qos],
            OsAction::TrunkCreate { .. }
            | OsAction::TrunkUpdate { .. }
            | OsAction::TrunkDelete { .. }
            | OsAction::TrunkAddSubports { .. }
            | OsAction::TrunkRemoveSubports { .. } => vec![Plugin::Trunk],
            OsAction::FirewallRuleCreate(_)
            | OsAction::FirewallRuleUpdate { .. }
            | OsAction::FirewallRuleDelete { .. }
            | OsAction::FirewallPolicyCreate { .. }
            | OsAction::FirewallPolicyUpdate { .. }
            | OsAction::FirewallPolicyDelete { .. }
            | OsAction::FirewallPolicyInsertRule { .. }
            | OsAction::FirewallPolicyRemoveRule { .. }
            | OsAction::FirewallGroupCreate { .. }
            | OsAction::FirewallGroupUpdate { .. }
            | OsAction::FirewallGroupSetPorts { .. }
            | OsAction::FirewallGroupDelete { .. } => vec![Plugin::Firewall],
            OsAction::SecretCreate(_)
            | OsAction::SecretDelete { .. }
            | OsAction::SecretAclSet { .. }
            | OsAction::SecretAclDelete { .. }
            | OsAction::ContainerAclSet { .. }
            | OsAction::ContainerAclDelete { .. }
            | OsAction::ContainerCreate { .. }
            | OsAction::ContainerDelete { .. }
            | OsAction::OrderCreate(_)
            | OsAction::OrderDelete { .. } => {
                vec![Plugin::KeyManager]
            }
            OsAction::StackCheck { .. }
            | OsAction::StackSuspend { .. }
            | OsAction::StackResume { .. }
            | OsAction::StackDelete { .. }
            | OsAction::StackCreate(_)
            | OsAction::StackUpdate { .. } => vec![Plugin::Orchestration],
            OsAction::AlarmCreate(_) | OsAction::AlarmUpdate { .. } => vec![Plugin::Alarming],
            OsAction::BackupCreate { .. }
            | OsAction::BackupUpdate { .. }
            | OsAction::BackupRestore { .. }
            | OsAction::BackupDelete { .. } => vec![Plugin::Backup],
            OsAction::QuotaUpdate {
                service: QuotaService::LoadBalancer,
                ..
            } => vec![Plugin::LoadBalancer],
            OsAction::QuotaUpdate {
                service: QuotaService::KeyManager,
                ..
            } => vec![Plugin::KeyManager],
            OsAction::RouterCreate {
                qos_policy_id: Some(_), ..
            }
            | OsAction::FloatingIpAllocate {
                qos_policy_id: Some(_), ..
            } => vec![Plugin::Qos],
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.plugins()).collect(),
            _ => vec![],
        }
    }

    /// Whether it changes the quotas of a project other than `session_project`
    /// (Identity › *Modify Quotas*, roadmap 82): `run_action` then requires the
    /// `admin` role. The session's own project stays editable by whoever the
    /// cloud's policy allows, as Project › Quotas.
    pub fn other_projects_quotas(&self, session_project: &str) -> bool {
        match self {
            OsAction::QuotaUpdate { project_id, .. } => project_id != session_project,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.other_projects_quotas(session_project)),
            _ => false,
        }
    }

    /// Writes of the Admin dashboard (roadmap 85): `run_action` refuses them
    /// without the `admin` role on the token, before any request.
    pub fn admin_only(&self) -> bool {
        match self {
            OsAction::AdminNetworkCreate(_)
            | OsAction::AdminNetworkUpdate { .. }
            | OsAction::RbacPolicyCreate { .. }
            | OsAction::RbacPolicyUpdate { .. }
            | OsAction::RbacPolicyDelete { .. }
            | OsAction::AdminFloatingIpAllocate { .. }
            | OsAction::AdminRouterCreate(_)
            | OsAction::FlavorCreate(_)
            | OsAction::FlavorReplace { .. }
            | OsAction::FlavorDelete { .. }
            | OsAction::FlavorSetExtraSpecs { .. }
            | OsAction::FlavorSetAccess { .. }
            | OsAction::AggregateCreate { .. }
            | OsAction::AggregateUpdate { .. }
            | OsAction::AggregateSetHosts { .. }
            | OsAction::AggregateSetMetadata { .. }
            | OsAction::AggregateDelete { .. }
            | OsAction::ComputeServiceSet { .. }
            | OsAction::ServerMigrate { .. }
            | OsAction::ServerLiveMigrate { .. }
            | OsAction::VolumeResetStatus { .. }
            | OsAction::VolumeMigrate { .. }
            | OsAction::VolumeManage(_)
            | OsAction::VolumeUnmanage { .. }
            | OsAction::SnapshotResetStatus { .. }
            | OsAction::VolumeTypeCreate { .. }
            | OsAction::VolumeTypeUpdate { .. }
            | OsAction::VolumeTypeDelete { .. }
            | OsAction::VolumeTypeSetExtraSpecs { .. }
            | OsAction::VolumeTypeSetQos { .. }
            | OsAction::VolumeTypeEncryptionSet { .. }
            | OsAction::VolumeTypeEncryptionDelete { .. }
            | OsAction::QosSpecCreate { .. }
            | OsAction::QosSpecSetConsumer { .. }
            | OsAction::QosSpecSetSpecs { .. }
            | OsAction::QosSpecDelete { .. }
            | OsAction::GroupTypeCreate { .. }
            | OsAction::GroupTypeUpdate { .. }
            | OsAction::GroupTypeDelete { .. }
            | OsAction::GroupTypeSetSpecs { .. }
            | OsAction::DefaultQuotaUpdate { .. }
            | OsAction::MetadefNamespaceImport { .. }
            | OsAction::MetadefNamespaceUpdate { .. }
            | OsAction::MetadefNamespaceDelete { .. }
            | OsAction::MetadefNamespaceSetTypes { .. } => true,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.admin_only()),
            _ => false,
        }
    }

    /// The variant's name (`ServerDelete`), for logs: never the payload.
    pub fn kind(&self) -> String {
        let dbg = format!("{self:?}");
        dbg.split(['(', ' ', '{']).next().unwrap_or("").to_string()
    }

    /// The size of the file that follows this action, for *Create Image* › *File*.
    pub fn image_upload_bytes(&self) -> Option<u64> {
        match self {
            OsAction::ImageCreate(spec) => spec.upload_bytes,
            _ => None,
        }
    }
}
