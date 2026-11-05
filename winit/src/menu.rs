//! Keep the native menu bar in sync with the [`Menu`] of a program.
//!
//! [`Spec::new`] turns a [`Menu`] into what the native side draws (the
//! [`Node`] tree, which only changes when the native menu must be rebuilt),
//! the per-row flags (enabled, checked; flipped in place), and the
//! messages of the live rows, keyed by a stable id.
//!
//! An id comes from the row alone, `<submenu path>/<label>` (for example
//! `File/Export/PDF`), never from an index, so an activation that arrives
//! after a rebuild cannot fire a different row. A repeated id within one
//! menu gets a `#2`, `#3`, ... suffix.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::core::menu::{Item, Menu, Predefined};

use rustc_hash::FxHashMap;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub(crate) use macos::Native;

/// The native menu bar on platforms without one: every [`Menu`] is ignored.
#[cfg(not(target_os = "macos"))]
pub(crate) struct Native<Message>(std::marker::PhantomData<Message>);

#[cfg(not(target_os = "macos"))]
impl<Message> Native<Message> {
    pub(crate) fn new() -> Self {
        Self(std::marker::PhantomData)
    }

    pub(crate) fn sync(&mut self, _menu: Option<Menu<Message>>, _proxy: &crate::Proxy<Message>) {}
}

/// A row of the native menu, without its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Action {
        id: String,
        label: String,
        shortcut: Option<String>,
        /// Whether the row is drawn as a check item.
        checkable: bool,
    },
    Submenu {
        id: String,
        label: String,
        children: Vec<Node>,
    },
    Separator,
    Predefined(Predefined),
}

/// The flags of a row, applied in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Flags {
    pub enabled: bool,
    pub checked: bool,
}

/// A [`Menu`] split into what the native side needs.
pub(crate) struct Spec<Message> {
    pub nodes: Vec<Node>,
    pub flags: Vec<(String, Flags)>,
    pub actions: FxHashMap<String, Message>,
}

impl<Message> Spec<Message> {
    pub(crate) fn new(menu: Menu<Message>) -> Self {
        let mut spec = Self {
            nodes: Vec::new(),
            flags: Vec::new(),
            actions: FxHashMap::default(),
        };
        let mut seen = FxHashMap::default();

        spec.nodes = menu
            .into_items()
            .into_iter()
            .map(|item| spec.node(item, "", &mut seen))
            .collect();

        spec
    }

    fn node(
        &mut self,
        item: Item<Message>,
        path: &str,
        seen: &mut FxHashMap<String, usize>,
    ) -> Node {
        match item {
            Item::Action(action) => {
                let id = unique_id(path, &action.label, seen);
                let enabled = action.on_activate.is_some();

                if let Some(message) = action.on_activate {
                    let _ = self.actions.insert(id.clone(), message);
                }

                self.flags.push((
                    id.clone(),
                    Flags {
                        enabled,
                        checked: action.checked.unwrap_or(false),
                    },
                ));

                Node::Action {
                    id,
                    label: action.label,
                    shortcut: action.shortcut,
                    checkable: action.checked.is_some(),
                }
            }
            Item::Submenu(submenu) => {
                let id = unique_id(path, &submenu.label, seen);

                self.flags.push((
                    id.clone(),
                    Flags {
                        enabled: submenu.enabled,
                        checked: false,
                    },
                ));

                let children = submenu
                    .items
                    .into_iter()
                    .map(|item| self.node(item, &id, seen))
                    .collect();

                Node::Submenu {
                    id,
                    label: submenu.label,
                    children,
                }
            }
            Item::Separator => Node::Separator,
            Item::Predefined(predefined) => Node::Predefined(predefined),
        }
    }
}

fn unique_id(path: &str, label: &str, seen: &mut FxHashMap<String, usize>) -> String {
    let base = if path.is_empty() {
        label.to_owned()
    } else {
        format!("{path}/{label}")
    };

    let count = seen.entry(base.clone()).or_insert(0);
    *count += 1;

    if *count == 1 {
        base
    } else {
        format!("{base}#{count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::menu::{item, item_maybe, predefined, separator, submenu};

    fn ids(nodes: &[Node]) -> Vec<String> {
        let mut out = Vec::new();
        for node in nodes {
            match node {
                Node::Action { id, .. } => out.push(id.clone()),
                Node::Submenu { id, children, .. } => {
                    out.push(id.clone());
                    out.extend(ids(children));
                }
                Node::Separator | Node::Predefined(_) => {}
            }
        }
        out
    }

    #[test]
    fn ids_follow_the_path_and_suffix_repeats() {
        let spec = Spec::new(Menu::new([
            submenu(
                "File",
                [
                    item("Save", 1),
                    submenu("Export", [item("PDF", 2)]),
                    item("Save", 3),
                ],
            ),
            submenu("Edit", [item("Save", 4)]),
        ]));

        assert_eq!(
            ids(&spec.nodes),
            [
                "File",
                "File/Save",
                "File/Export",
                "File/Export/PDF",
                "File/Save#2",
                "Edit",
                "Edit/Save"
            ]
        );
        assert_eq!(spec.actions["File/Save#2"], 3);
        assert_eq!(spec.actions["File/Export/PDF"], 2);
    }

    #[test]
    fn disabled_rows_have_no_action() {
        let spec = Spec::new(Menu::new([submenu(
            "File",
            [item_maybe::<i32>("Save", None)],
        )]));

        assert!(spec.actions.is_empty());
        assert_eq!(
            spec.flags,
            [
                (
                    "File".to_owned(),
                    Flags {
                        enabled: true,
                        checked: false
                    }
                ),
                (
                    "File/Save".to_owned(),
                    Flags {
                        enabled: false,
                        checked: false
                    }
                ),
            ]
        );
    }

    #[test]
    fn flags_do_not_change_the_nodes() {
        let build = |enabled: bool, checked: bool| {
            Spec::new(Menu::new([submenu(
                "View",
                [
                    item_maybe("Grid", enabled.then_some(1)).checked(checked),
                    separator(),
                    predefined(Predefined::Fullscreen),
                ],
            )]))
        };

        let a = build(true, true);
        let b = build(false, false);

        assert_eq!(a.nodes, b.nodes);
        assert_ne!(a.flags, b.flags);
    }

    #[test]
    fn becoming_checkable_changes_the_nodes() {
        let plain = Spec::new(Menu::new([submenu("View", [item("Grid", 1)])]));
        let check = Spec::new(Menu::new([submenu(
            "View",
            [item("Grid", 1).checked(false)],
        )]));

        assert_ne!(plain.nodes, check.nodes);
    }
}
