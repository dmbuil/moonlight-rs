//! Pure geometry for the *Network Topology* panel: Horizon's column layout
//! (networks as vertical bars, devices as boxes wired to them), a deterministic
//! force layout for the graph tab (also used by the stack *Topology* tab), and
//! `viewBox` pan/zoom. No DOM, so it is unit-tested and gives the same picture
//! on the server and in the browser.

use std::collections::HashMap;

use crate::model::{TopoKind, Topology};

/// Node and spacing sizes for *Small* / *Normal*.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub box_w: f64,
    pub box_h: f64,
    pub row_gap: f64,
    pub bar_w: f64,
    pub pad: f64,
}

impl Size {
    /// Box and gap sizes of the normal view.
    pub const NORMAL: Size = Size {
        box_w: 150.0,
        box_h: 44.0,
        row_gap: 36.0,
        bar_w: 30.0,
        pad: 18.0,
    };
    /// Box and gap sizes of the *Small* view (labels cut to fit).
    pub const SMALL: Size = Size {
        box_w: 44.0,
        box_h: 30.0,
        row_gap: 18.0,
        bar_w: 18.0,
        pad: 12.0,
    };

    /// The sizes for the chosen view.
    pub fn of(small: bool) -> Size {
        if small { Size::SMALL } else { Size::NORMAL }
    }
    /// Distance between two bars: room for a box and its wire on both sides.
    pub fn col_w(&self) -> f64 {
        self.bar_w + 2.0 * self.pad + self.box_w
    }
}

const MARGIN: f64 = 16.0;
const MIN_BAR_H: f64 = 180.0;

/// A network's vertical bar in the *Topology* tab, at `x`, from `y`, `h` high.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub h: f64,
}

/// A device box (router or server) in the *Topology* tab, top-left at `x`, `y`.
#[derive(Clone, Debug, PartialEq)]
pub struct Boxed {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// A horizontal wire from a device box to a network bar, at height `y`.
/// `x2` is the end that touches the bar. One per device and network, see [`pairs`].
#[derive(Clone, Debug, PartialEq)]
pub struct Wire {
    pub device_id: String,
    pub network_id: String,
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
    /// Indices in `Topology::links`: several when the device has more than one port there.
    pub links: Vec<usize>,
}

/// The *Topology* tab's layout: canvas size, bars, boxes and wires.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Columns {
    pub width: f64,
    pub height: f64,
    pub bars: Vec<Bar>,
    pub boxes: Vec<Boxed>,
    pub wires: Vec<Wire>,
}

/// Links grouped by device and network, in `Topology::links` order, as indices
/// into it. A device can have several ports on one network (a router gets a port
/// per IPv4 subnet it is attached to); drawing one line each stacks the lines and
/// their address labels on top of each other, so both tabs draw one per pair.
pub fn pairs(t: &Topology) -> Vec<(&str, &str, Vec<usize>)> {
    let mut out: Vec<(&str, &str, Vec<usize>)> = vec![];
    let mut at: HashMap<(&str, &str), usize> = HashMap::new();
    for (i, l) in t.links.iter().enumerate() {
        let key = (l.device_id.as_str(), l.network_id.as_str());
        match at.get(&key) {
            Some(&k) => out[k].2.push(i),
            None => {
                at.insert(key, out.len());
                out.push((key.0, key.1, vec![i]));
            }
        }
    }
    out
}

/// Horizon's *Topology* tab. Each device sits right of the leftmost network it
/// is on and is wired to the others; devices on no network get a last column.
/// `collapsed` packs devices whose spans do not overlap into the same row.
pub fn columns(t: &Topology, s: Size, collapsed: bool) -> Columns {
    let net_ix: HashMap<&str, usize> = t.networks.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let orphan_col = t.networks.len();
    // (device index, first column, last column)
    let mut spans: Vec<(usize, usize, usize)> = t
        .devices
        .iter()
        .enumerate()
        .map(|(di, d)| {
            let cols: Vec<usize> = t
                .links_of(&d.id)
                .filter_map(|l| net_ix.get(l.network_id.as_str()).copied())
                .collect();
            let lo = cols.iter().min().copied().unwrap_or(orphan_col);
            let hi = cols.iter().max().copied().unwrap_or(orphan_col);
            (di, lo, hi)
        })
        .collect();
    // Routers first within a column, as they bridge networks.
    spans.sort_by_key(|&(di, lo, _)| (lo, t.devices[di].kind != TopoKind::Router));

    // Half-open column intervals: a box always takes the gap after `lo`.
    let mut rows: Vec<Vec<(usize, usize)>> = vec![];
    let mut row_of = vec![0usize; t.devices.len()];
    for &(di, lo, hi) in &spans {
        let iv = (lo, hi.max(lo + 1));
        let free = |r: &Vec<(usize, usize)>| r.iter().all(|&(a, b)| iv.1 <= a || b <= iv.0);
        let r = if collapsed { rows.iter().position(free) } else { None }.unwrap_or_else(|| {
            rows.push(vec![]);
            rows.len() - 1
        });
        rows[r].push(iv);
        row_of[di] = r;
    }

    let row_h = s.box_h + s.row_gap;
    let bar_h = (rows.len() as f64 * row_h + s.row_gap).max(MIN_BAR_H);
    let bar_x = |c: usize| MARGIN + c as f64 * s.col_w();
    let bars: Vec<Bar> = t
        .networks
        .iter()
        .enumerate()
        .map(|(i, n)| Bar {
            id: n.id.clone(),
            x: bar_x(i),
            y: MARGIN,
            h: bar_h,
        })
        .collect();

    let mut by_device: HashMap<&str, Vec<(&str, Vec<usize>)>> = HashMap::new();
    for (dev, net, links) in pairs(t) {
        by_device.entry(dev).or_default().push((net, links));
    }
    let mut boxes = Vec::with_capacity(t.devices.len());
    let mut wires = vec![];
    for &(di, lo, _) in &spans {
        let d = &t.devices[di];
        let x = bar_x(lo) + s.bar_w + s.pad;
        let y = MARGIN + s.row_gap + row_of[di] as f64 * row_h;
        let mid = y + s.box_h / 2.0;
        for (net, links) in by_device.remove(d.id.as_str()).unwrap_or_default() {
            let Some(&c) = net_ix.get(net) else { continue };
            let (x1, x2) = if c == lo {
                (x, bar_x(c) + s.bar_w)
            } else {
                (x + s.box_w, bar_x(c))
            };
            wires.push(Wire {
                device_id: d.id.clone(),
                network_id: net.to_string(),
                x1,
                x2,
                y: mid,
                links,
            });
        }
        boxes.push(Boxed { id: d.id.clone(), x, y });
    }
    let has_orphans = spans.iter().any(|&(_, lo, _)| lo == orphan_col);
    let cols = t.networks.len() + usize::from(has_orphans);
    Columns {
        width: 2.0 * MARGIN + cols.max(1) as f64 * s.col_w(),
        height: 2.0 * MARGIN + bar_h,
        bars,
        boxes,
        wires,
    }
}

// --- Graph tab --------------------------------------------------------------

/// The *Graph* tab's layout: canvas size and each node's centre.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub width: f64,
    pub height: f64,
    /// Centre of every node, keyed by network or device ID.
    pub pos: HashMap<String, (f64, f64)>,
}

/// Same input, same picture: positions start on a circle (no RNG).
pub const ITERATIONS: usize = 300;

/// Fruchterman–Reingold force layout of networks and devices joined by links.
/// `room` is the space per node, so bigger nodes spread further.
pub fn force(t: &Topology, room: f64) -> Graph {
    let ids: Vec<&str> = t
        .networks
        .iter()
        .map(|n| n.id.as_str())
        .chain(t.devices.iter().map(|d| d.id.as_str()))
        .collect();
    force_edges(
        &ids,
        t.links.iter().map(|l| (l.device_id.as_str(), l.network_id.as_str())),
        room,
    )
}

/// [`force`] for any graph: `ids` are the nodes, `links` pairs of their IDs
/// (direction ignored; links to unknown IDs are dropped). The stack *Topology* tab uses it.
pub fn force_edges<'a>(ids: &[&str], links: impl IntoIterator<Item = (&'a str, &'a str)>, room: f64) -> Graph {
    let n = ids.len();
    if n == 0 {
        return Graph::default();
    }
    let ix: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut edges: Vec<(usize, usize)> = links
        .into_iter()
        .filter_map(|(a, b)| Some((*ix.get(a)?, *ix.get(b)?)))
        .collect();
    edges.sort_unstable();
    edges.dedup();

    let side = room * (n as f64).sqrt();
    let k = (side * side / n as f64).sqrt();
    let c = side / 2.0;
    let mut p: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = i as f64 * std::f64::consts::TAU / n as f64;
            (c + c * 0.8 * a.cos(), c + c * 0.8 * a.sin())
        })
        .collect();
    let mut temp = side / 10.0;
    let cool = temp / ITERATIONS as f64;
    for _ in 0..ITERATIONS {
        let mut disp = vec![(0.0f64, 0.0f64); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy, d) = delta(p[i], p[j], i, j);
                let f = k * k / d;
                disp[i].0 += dx / d * f;
                disp[i].1 += dy / d * f;
                disp[j].0 -= dx / d * f;
                disp[j].1 -= dy / d * f;
            }
        }
        for &(i, j) in &edges {
            let (dx, dy, d) = delta(p[i], p[j], i, j);
            let f = d * d / k;
            disp[i].0 -= dx / d * f;
            disp[i].1 -= dy / d * f;
            disp[j].0 += dx / d * f;
            disp[j].1 += dy / d * f;
        }
        // A weak spring from every node to the centre (d²/4k, like an edge) keeps
        // unlinked nodes within a few node widths instead of drifting away.
        for (q, dv) in p.iter_mut().zip(disp) {
            let (gx, gy) = (c - q.0, c - q.1);
            let pull = (gx * gx + gy * gy).sqrt() / (4.0 * k);
            let (dx, dy) = (dv.0 + gx * pull, dv.1 + gy * pull);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let step = len.min(temp);
            // No clamping to the square: it pins nodes on its edges, on top of each
            // other. The pull to the centre keeps them together; the result is fitted below.
            q.0 += dx / len * step;
            q.1 += dy / len * step;
        }
        temp -= cool;
    }
    // Fit to the nodes with a margin of one node's room.
    let (minx, maxx) = p
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (miny, maxy) = p
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.1), b.max(q.1)));
    let m = room / 2.0;
    Graph {
        width: maxx - minx + 2.0 * m,
        height: maxy - miny + 2.0 * m,
        pos: ids
            .iter()
            .zip(p)
            .map(|(id, q)| (id.to_string(), (q.0 - minx + m, q.1 - miny + m)))
            .collect(),
    }
}

/// Vector from `b` to `a` and its length; coincident nodes are pushed apart
/// along a direction derived from their indices.
fn delta(a: (f64, f64), b: (f64, f64), i: usize, j: usize) -> (f64, f64, f64) {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d > 0.01 {
        return (dx, dy, d);
    }
    let a = (i * 31 + j * 17) as f64;
    (a.cos() * 0.01, a.sin() * 0.01, 0.01)
}

// --- Pan and zoom -----------------------------------------------------------

/// SVG `viewBox`: x, y, width, height.
pub type ViewBox = (f64, f64, f64, f64);

/// Zooms by `factor` (> 1 zooms in) keeping the point at `(fx, fy)` — its
/// position as a fraction of the drawing — still under the pointer.
pub fn zoom(v: ViewBox, factor: f64, fx: f64, fy: f64, full: (f64, f64)) -> ViewBox {
    // An empty view has no aspect ratio to keep (h would be 0 / 0).
    if v.2 <= 0.0 || v.3 <= 0.0 {
        return v;
    }
    // Between 1/10 and 4× the whole drawing.
    let w = (v.2 / factor).clamp(full.0 / 10.0, full.0 * 4.0);
    let h = w * v.3 / v.2;
    (v.0 + (v.2 - w) * fx, v.1 + (v.3 - h) * fy, w, h)
}

/// Zoom factor for one `wheel` event: proportional to its distance, so a mouse
/// notch (~100 px) zooms about 1.2× while a trackpad, which sends dozens of
/// small events per gesture, zooms smoothly instead of jumping to the limit.
/// `line_mode` is `deltaMode == 1` (Firefox mouse wheels); `pinch` is a trackpad
/// pinch, which browsers report as a wheel event with `ctrlKey` and tiny deltas.
pub fn wheel_factor(delta_y: f64, line_mode: bool, pinch: bool) -> f64 {
    let px = if line_mode { delta_y * 33.0 } else { delta_y };
    let k = if pinch { 0.01 } else { 0.002 };
    (-px * k).exp().clamp(0.5, 2.0)
}

/// Moves the view by a drag of `(dx, dy)` screen pixels over an element `px` wide.
pub fn pan(v: ViewBox, dx: f64, dy: f64, px: f64) -> ViewBox {
    let scale = if px > 0.0 { v.2 / px } else { 1.0 };
    (v.0 - dx * scale, v.1 - dy * scale, v.2, v.3)
}

/// How a `viewBox` is drawn in an element `w`×`h` with `xMidYMin meet`:
/// (pixels per unit, left offset in pixels).
pub fn fit(v: ViewBox, w: f64, h: f64) -> (f64, f64) {
    if v.2 <= 0.0 || v.3 <= 0.0 {
        return (1.0, 0.0);
    }
    let scale = (w / v.2).min(h / v.3);
    (scale, (w - v.2 * scale) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TopoDevice, TopoLink, TopoNetwork};

    fn net(id: &str, external: bool) -> TopoNetwork {
        TopoNetwork {
            id: id.into(),
            name: id.into(),
            external,
            shared: false,
            owned: !external,
            status: "ACTIVE".into(),
            cidrs: vec![],
        }
    }
    fn dev(id: &str, kind: TopoKind) -> TopoDevice {
        TopoDevice {
            id: id.into(),
            name: id.into(),
            kind,
            status: "ACTIVE".into(),
        }
    }
    fn link(d: &str, n: &str) -> TopoLink {
        TopoLink {
            device_id: d.into(),
            network_id: n.into(),
            port_id: None,
            ips: vec![],
            floating: vec![],
            gateway: false,
        }
    }
    /// ext ─ r1 ─ a ; vm1 on a and b ; vm2 on b ; lone on nothing
    fn sample() -> Topology {
        Topology {
            networks: vec![net("ext", true), net("a", false), net("b", false)],
            devices: vec![
                dev("r1", TopoKind::Router),
                dev("lone", TopoKind::Server),
                dev("vm1", TopoKind::Server),
                dev("vm2", TopoKind::Server),
            ],
            links: vec![
                link("r1", "ext"),
                link("r1", "a"),
                link("vm1", "a"),
                link("vm1", "b"),
                link("vm2", "b"),
            ],
            max_networks: 0,
        }
    }

    #[test]
    fn columns_put_each_device_after_its_leftmost_network() {
        let s = Size::NORMAL;
        let c = columns(&sample(), s, false);
        let bx = |id: &str| c.boxes.iter().find(|b| b.id == id).unwrap().clone();
        let bar = |i: usize| c.bars[i].x;
        assert_eq!(bx("r1").x, bar(0) + s.bar_w + s.pad);
        assert_eq!(bx("vm1").x, bar(1) + s.bar_w + s.pad);
        assert_eq!(bx("vm2").x, bar(2) + s.bar_w + s.pad);
        assert!(
            bx("lone").x > bar(2) + s.bar_w,
            "unconnected devices go in an extra column"
        );
        // One device per row; bars as tall as the rows.
        let mut ys: Vec<_> = c.boxes.iter().map(|b| b.y).collect();
        ys.dedup();
        assert_eq!(ys.len(), 4);
        assert!(c.boxes.iter().all(|b| b.y + s.box_h <= c.bars[0].y + c.bars[0].h));
        assert!(c.width >= bar(2) + s.col_w(), "room for the extra column");
    }

    #[test]
    fn wires_reach_the_near_side_of_each_bar() {
        let s = Size::NORMAL;
        let c = columns(&sample(), s, false);
        let vm1: Vec<_> = c.wires.iter().filter(|w| w.device_id == "vm1").collect();
        assert_eq!(vm1.len(), 2);
        let b = c.boxes.iter().find(|b| b.id == "vm1").unwrap();
        let to_a = vm1.iter().find(|w| w.network_id == "a").unwrap();
        let to_b = vm1.iter().find(|w| w.network_id == "b").unwrap();
        assert_eq!(
            (to_a.x1, to_a.x2),
            (b.x, c.bars[1].x + s.bar_w),
            "left: box edge to the bar's right side"
        );
        assert_eq!(
            (to_b.x1, to_b.x2),
            (b.x + s.box_w, c.bars[2].x),
            "right: box edge to the bar's left side"
        );
        assert_eq!(to_a.y, b.y + s.box_h / 2.0);
    }

    #[test]
    fn two_ports_on_one_network_make_one_wire_with_both_links() {
        // A router with an interface per IPv4 subnet of the same network.
        let mut t = sample();
        t.links.push(link("r1", "a"));
        let c = columns(&t, Size::NORMAL, false);
        let r1: Vec<_> = c.wires.iter().filter(|w| w.device_id == "r1").collect();
        assert_eq!(r1.len(), 2, "one wire per network, not per port");
        let to_a = r1.iter().find(|w| w.network_id == "a").unwrap();
        assert_eq!(to_a.links, vec![1, 5]);
        assert_eq!(pairs(&t).len(), 5);
    }

    #[test]
    fn wheel_zoom_is_proportional_to_the_distance() {
        let notch = wheel_factor(-100.0, false, false);
        assert!((1.15..1.3).contains(&notch), "a mouse notch zooms in ~1.2x: {notch}");
        assert!(
            (wheel_factor(100.0, false, false) * notch - 1.0).abs() < 1e-9,
            "out undoes in"
        );
        assert!(
            (wheel_factor(-3.0, true, false) - notch).abs() < 0.05,
            "3 lines ~ 100 px"
        );
        // 30 trackpad events of 4 px: a gentle zoom, not the 1.2^30 = 237x of a fixed step.
        let swipe = wheel_factor(-4.0, false, false).powi(30);
        assert!(swipe < 1.5, "trackpad swipe zoomed {swipe}x");
        assert_eq!(wheel_factor(-10_000.0, false, false), 2.0, "one event is capped");
    }

    #[test]
    fn collapse_packs_devices_that_do_not_overlap() {
        let c = columns(&sample(), Size::NORMAL, true);
        let y = |id: &str| c.boxes.iter().find(|b| b.id == id).unwrap().y;
        // r1 spans ext..a, vm1 a..b and vm2 b..b+1: they chain without overlapping.
        assert_eq!(y("r1"), y("vm1"));
        assert_eq!(y("vm1"), y("vm2"));
        assert!(c.height < columns(&sample(), Size::NORMAL, false).height);
    }

    #[test]
    fn force_layout_is_deterministic_bounded_and_keeps_links_short() {
        let t = sample();
        let g = force(&t, 120.0);
        assert_eq!(g, force(&t, 120.0));
        assert_eq!(g.pos.len(), 7);
        for &(x, y) in g.pos.values() {
            assert!(x.is_finite() && y.is_finite());
            assert!((0.0..=g.width).contains(&x) && (0.0..=g.height).contains(&y));
        }
        let d = |a: &str, b: &str| {
            let (p, q) = (g.pos[a], g.pos[b]);
            ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
        };
        assert!(
            d("vm2", "b") < d("vm2", "ext"),
            "a linked pair ends closer than an unlinked one"
        );
        assert_eq!(force(&Topology::default(), 120.0), Graph::default());
    }

    #[test]
    fn a_project_with_over_a_hundred_instances_lays_out_quickly_and_in_bounds() {
        // The review list of roadmap 20 asks for 100+ instances: 12 networks, 130 servers
        // spread over them (two networks each for some), 2 routers.
        let mut t = Topology::default();
        for i in 0..12 {
            t.networks.push(net(&format!("n{i}"), i == 0));
        }
        for i in 0..2 {
            t.devices.push(dev(&format!("r{i}"), TopoKind::Router));
            t.links.push(link(&format!("r{i}"), "n0"));
            t.links.push(link(&format!("r{i}"), &format!("n{}", i + 1)));
        }
        for i in 0..130 {
            t.devices.push(dev(&format!("vm{i}"), TopoKind::Server));
            t.links.push(link(&format!("vm{i}"), &format!("n{}", 1 + i % 11)));
            if i % 5 == 0 {
                t.links.push(link(&format!("vm{i}"), &format!("n{}", 1 + (i + 3) % 11)));
            }
        }
        let started = std::time::Instant::now();
        let g = force(&t, 120.0);
        assert!(
            started.elapsed().as_secs() < 5,
            "force layout of 144 nodes took {:?}",
            started.elapsed()
        );
        assert_eq!(g.pos.len(), 144);
        assert!(g.pos.values().all(|&(x, y)| x.is_finite()
            && y.is_finite()
            && (0.0..=g.width).contains(&x)
            && (0.0..=g.height).contains(&y)));
        for collapsed in [false, true] {
            let c = columns(&t, Size::NORMAL, collapsed);
            assert_eq!(c.boxes.len(), 132, "every device gets a box (collapsed: {collapsed})");
            assert!(c.height.is_finite() && c.height > 0.0);
        }
        let packed = columns(&t, Size::NORMAL, true).height;
        assert!(
            packed < columns(&t, Size::NORMAL, false).height,
            "collapsing shortens a tall topology"
        );
    }

    #[test]
    fn force_layout_keeps_nodes_apart_in_a_larger_project() {
        // Like production: many networks, few devices, most networks unlinked.
        let mut t = sample();
        for i in 0..10 {
            t.networks.push(net(&format!("n{i}"), false));
        }
        let g = force(&t, 180.0);
        let p: Vec<_> = g.pos.values().copied().collect();
        let min = p
            .iter()
            .enumerate()
            .flat_map(|(i, a)| {
                p[i + 1..]
                    .iter()
                    .map(move |b| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt())
            })
            .fold(f64::MAX, f64::min);
        assert!(min > 90.0, "closest pair {min:.0} apart");
        assert!(
            g.width < 10.0 * 180.0 && g.height < 10.0 * 180.0,
            "compact: {:.0}x{:.0}",
            g.width,
            g.height
        );
    }

    #[test]
    fn zoom_keeps_the_pointer_point_and_pan_follows_the_drag() {
        let full = (1000.0, 500.0);
        let v = zoom((0.0, 0.0, 1000.0, 500.0), 2.0, 0.25, 0.5, full);
        assert_eq!(v, (125.0, 125.0, 500.0, 250.0));
        // The point a quarter across stays a quarter across.
        assert_eq!(v.0 + v.2 * 0.25, 250.0);
        let tiny = zoom(v, 1000.0, 0.5, 0.5, full);
        assert_eq!(tiny.2, 100.0, "clamped at 10x");
        assert_eq!(
            pan((0.0, 0.0, 500.0, 250.0), 100.0, -50.0, 1000.0),
            (-50.0, 25.0, 500.0, 250.0)
        );
        // A 1000x500 drawing in a 1200x300 box: height-bound, centred horizontally.
        assert_eq!(fit((0.0, 0.0, 1000.0, 500.0), 1200.0, 300.0), (0.6, 300.0));
    }

    #[test]
    fn zoom_leaves_an_empty_view_alone() {
        // A view measured before layout (0 x 0) used to become `-100 NaN 100 NaN`.
        let empty = (0.0, 0.0, 0.0, 0.0);
        assert_eq!(zoom(empty, 1.2, 1.0, 1.0, (1000.0, 500.0)), empty);
    }
}
