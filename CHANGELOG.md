# Changelog

All notable changes to Moonlight, newest first. Every commit bumps the patch number; 0.4.0 is the
first version considered fit for real use, so nothing earlier is listed. The numbers in brackets
are `ROADMAP.md` items; the version is shown at the bottom of the user menu.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.4.5] - 2026-11-15

### Added

- Contextual help in dialogs, adapted from Horizon's wizard step panels, modal descriptions and
  field help, in English and Spanish [122]:
  - *Launch Instance*: an (i) on every field but name and description: zone, count, boot source
    (ephemeral versus persistent storage), *Create New Volume*, volume size, delete on instance
    delete, flavor, networks, ports, security groups, key pair (with an `ssh-keygen` example),
    customization script, disk partition, configuration drive and server group. Clearer help
    lines on the Source, Flavor and Security Groups steps.
  - Tips in Create/Edit Image, Upload to Image, Create Volume, trunks (parent port, subports
    with the `ip link … type vlan` example, segmentation type), Create User, Change Password,
    Create Flavor, admin Create Network, subnet fields, Edit Port, host aggregates and volume
    groups.
  - An intro line on 17 one-step dialogs: Allocate IP, Create Security Group, Add Rule, Change
    Volume Type, volume transfers, backups, instance Create Snapshot, volume types, QoS specs,
    group types, volume type encryption and Import Public Key, among others.

### Changed

- Long field footnotes are now one short sentence (at most 90 characters, 105 in Spanish); the
  rest moved into the field's (i), extended for complex fields such as Key Manager secrets and
  orders, application credential access rules, metadata prefixes, the image URL, security group
  rules, host routes, listener certificates and pool members. Footnotes that warn about a
  consequence (the key pair download, *Unrestricted*, *Delete Volumes*, *Reset Status*, …) stay
  visible [122].
- A few Spanish texts that used *tú* now use *usted*, like the rest of the interface.

### Fixed

- Checkbox fields never showed their (i). It now sits beside the label, and clicking it doesn't
  tick the box.

## [0.4.4] - 2026-11-15

### Added

- Load tests: `loadtest/`, a separate crate with a fake OpenStack and Goose scenarios (sign in,
  Overview, Instances, instance detail, Volumes, Networks), and criterion benchmarks on the
  mappers (`benches/mapping.rs`). `README.md` has a Performance section with the first results
  against the fake cloud [120].

### Changed

- The *Your session has expired* dialog offers only *Sign In*; ×, Escape and the backdrop also go
  to the login page. The error page for an expired session drops *Back to safety* the same way
  [121].

## [0.4.3] - 2026-11-15

### Added

- *Create Image* from a file uploads in the background: the dialog closes once the image record
  exists and the jobs tray shows the transfer (progress, speed, time left, *Cancel upload*). A
  failed or cancelled upload keeps its reason and offers *Delete Image*; the browser asks before
  closing a tab that is still sending, and the Images list shows an *Uploading N%* pill [45].
- Admin › Volume › Storage Pools: every Cinder pool with its capacities, over-subscription, thin
  or thick provisioning and volume and snapshot counts, a Storage Summary of charts, and a detail
  page with the vendor capabilities [119].

### Changed

- *Launch Instance*: choosing a flavor raises *Volume Size* to its root disk when *Create New
  Volume* is on, never lowering it [118].

## [0.4.2] - 2026-11-14

### Added

- Admin › Networks: a Network Type column and filter; the network detail shows the type,
  physical network and segmentation ID to admins [115].

### Changed

- QoS Policy selects show the policy ID, greyed, after names that occur more than once, and
  the filter matches it too [116].
- Dialogs ease their height when a wizard's content grows or shrinks (not under reduced motion)
  [117].

## [0.4.1] - 2026-11-14

### Changed

- Every module, function, type and constant has a doc comment; rustdoc builds clean in both
  targets and CI checks it [97].

## [0.4.0] - 2026-11-14

### Added

- `[ui] quota_edit = all / admin / off` decides who sees *Edit Quotas* on the session's own
  project, enforced on the server [112].
- The login page shows a background image (light and dark, crossfading with the theme), and its
  card eases to its new height when switching between Keystone credentials and a federated
  choice [114].
- The user menu ends with the Moonlight version.

### Changed

- The firewall quotas use the Firewall panel's labels instead of the raw keys [113].

[0.4.5]: https://github.com/dmbuil/moonlight-rs/compare/9c657ca...6324242
[0.4.4]: https://github.com/dmbuil/moonlight-rs/compare/918cd5a...9c657ca
[0.4.3]: https://github.com/dmbuil/moonlight-rs/compare/fab682e...918cd5a
[0.4.2]: https://github.com/dmbuil/moonlight-rs/compare/500a163...fab682e
[0.4.1]: https://github.com/dmbuil/moonlight-rs/compare/8da1dc2...500a163
[0.4.0]: https://github.com/dmbuil/moonlight-rs/commit/8da1dc2
