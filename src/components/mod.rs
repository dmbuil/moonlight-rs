//! Shared UI pieces used by the pages: layout and navigation, `DataTable`, the dialog engine,
//! the per-resource actions (`ops`), widgets, and the pure models behind the Heat and topology
//! drawings. Compiled for both `ssr` and `hydrate`; writes leave only as `OsAction`s
//! (AGENTS.md §1.2).

pub mod assignment_rows;
pub mod datetime;
pub mod dialog;
pub mod heat_icons;
pub mod hot;
pub mod hot_icons;
pub mod jobs;
pub mod layout;
pub mod member_rows;
pub mod nav;
pub mod ops;
pub mod pager;
pub mod pick;
pub mod select;
pub mod stack_graph;
pub mod table;
pub mod topo_canvas;
pub mod topo_layout;
pub mod upload;
pub mod widgets;
