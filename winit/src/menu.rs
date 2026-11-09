//! Keep the native menu bar in sync with the [`Menu`] of a program.
//!
//! [`Spec::new`] turns a [`Menu`] into what the native side draws (the
//! [`Node`] tree, which only changes when the native menu must be rebuilt),
//! the per-row flags (enabled, checked; flipped in place), and the
//! messages of the live rows, keyed by a stable id.
//!
//! An id comes from the row alone, `<submenu path>/<label>` (for example
//! `File/Export/PDF`), never from an index, so an activation that arrives
//! after a rebuild cannot fire a different row. An id already taken within
//! one menu (a repeated label, or a label that itself contains `/` or `#`)
//! gets the first free `#2`, `#3`, ... suffix, so every row has its own id and
//! a click can never send another row's message.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::core::menu::{Item, Menu, Predefined};

use rustc_hash::{FxHashMap, FxHashSet};

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

    /// Never calls `menu`, so the program does not build a menu nobody draws.
    pub(crate) fn sync(
        &mut self,
        _menu: impl FnOnce() -> Option<Menu<Message>>,
        _proxy: &crate::Proxy<Message>,
    ) -> Option<String> {
        None
    }
}

/// What a sync did with the rows it was given.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome<T, E> {
    /// The rows are the installed ones; only flags and actions change.
    Unchanged,
    /// The rows failed before and have not changed, so they were not rebuilt.
    Skipped,
    /// The rows were built.
    Built(T),
    /// The rows were built just now and that failed.
    Failed(E),
}

/// Decides when the native menu is rebuilt, without touching AppKit.
#[derive(Debug, Default)]
pub(crate) struct Reconciler {
    /// Rows that failed to build; not retried until they change.
    failed: Option<Vec<Node>>,
}

impl Reconciler {
    /// Builds `nodes` unless they are the `installed` rows or failed before.
    pub(crate) fn reconcile<T, E>(
        &mut self,
        installed: Option<&[Node]>,
        nodes: &[Node],
        build: impl FnOnce(Vec<Node>) -> Result<T, E>,
    ) -> Outcome<T, E> {
        if installed == Some(nodes) {
            return Outcome::Unchanged;
        }

        if self.failed.as_deref() == Some(nodes) {
            return Outcome::Skipped;
        }

        match build(nodes.to_vec()) {
            Ok(built) => {
                self.failed = None;
                Outcome::Built(built)
            }
            Err(error) => {
                self.failed = Some(nodes.to_vec());
                Outcome::Failed(error)
            }
        }
    }

    /// The menu was taken down; the same rows are built again if they return.
    pub(crate) fn taken_down(&mut self) {
        self.failed = None;
    }
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
        let mut seen = FxHashSet::default();

        spec.nodes = menu
            .into_items()
            .into_iter()
            .map(|item| spec.node(item, "", &mut seen))
            .collect();

        spec
    }

    fn node(&mut self, item: Item<Message>, path: &str, seen: &mut FxHashSet<String>) -> Node {
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

fn unique_id(path: &str, label: &str, seen: &mut FxHashSet<String>) -> String {
    let base = if path.is_empty() {
        label.to_owned()
    } else {
        format!("{path}/{label}")
    };

    let mut id = base.clone();
    let mut suffix = 1;

    // Check the final id, not the base: a label such as `Save#2` or `A/B` can
    // spell an id another row already took.
    while !seen.insert(id.clone()) {
        suffix += 1;
        id = format!("{base}#{suffix}");
    }

    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::menu::{item, item_maybe, predefined, separator, submenu};

    fn rows(label: &str) -> Vec<Node> {
        Spec::new(Menu::new([submenu(label, [item("Row", 1)])])).nodes
    }

    fn fail(_: Vec<Node>) -> Result<(), &'static str> {
        Err("boom")
    }

    fn pass(_: Vec<Node>) -> Result<(), &'static str> {
        Ok(())
    }

    #[test]
    fn a_failed_first_build_is_reported_once_and_recorded() {
        let mut reconciler = Reconciler::default();
        let nodes = rows("A");

        let first = reconciler.reconcile(None, &nodes, fail);
        assert_eq!(first, Outcome::Failed("boom"));
        assert_eq!(reconciler.failed, Some(nodes));
    }

    #[test]
    fn the_same_failed_rows_are_not_rebuilt() {
        let mut reconciler = Reconciler::default();
        let nodes = rows("A");
        let _ = reconciler.reconcile(None, &nodes, fail);

        let again = reconciler.reconcile(None, &nodes, |_| -> Result<(), &str> {
            panic!("rebuilt a failed menu")
        });
        assert_eq!(again, Outcome::Skipped);
    }

    #[test]
    fn a_new_menu_builds_and_clears_the_failure() {
        let mut reconciler = Reconciler::default();
        let (a, b) = (rows("A"), rows("B"));
        let _ = reconciler.reconcile(None, &a, fail);

        let built = reconciler.reconcile(None, &b, pass);
        assert_eq!(built, Outcome::Built(()));
        assert_eq!(reconciler.failed, None);

        // Rows `b` are installed; the failed menu coming back is reported again.
        let again = reconciler.reconcile(Some(&b), &a, fail);
        assert_eq!(again, Outcome::Failed("boom"));
    }

    #[test]
    fn installed_rows_are_unchanged_and_never_rebuilt() {
        let mut reconciler = Reconciler::default();
        let nodes = rows("A");

        let outcome = reconciler.reconcile(Some(&nodes), &nodes, fail);
        assert_eq!(outcome, Outcome::Unchanged);
    }

    #[test]
    fn taking_the_menu_down_lets_a_failed_menu_be_reported_again() {
        let mut reconciler = Reconciler::default();
        let nodes = rows("A");
        let _ = reconciler.reconcile(None, &nodes, fail);

        reconciler.taken_down();

        assert_eq!(
            reconciler.reconcile(None, &nodes, fail),
            Outcome::Failed("boom")
        );
    }

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
    fn labels_that_spell_another_id_get_their_own() {
        let spec = Spec::new(Menu::new([submenu(
            "File",
            [
                item("Save", 1),
                item("Save", 2),
                item("Save#2", 3),
                item("A/B", 4),
                submenu("A", [item("B", 5)]),
            ],
        )]));

        let ids = ids(&spec.nodes);
        let unique: FxHashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate ids: {ids:?}");

        // Every live row keeps its own message.
        let mut messages: Vec<_> = spec.actions.values().copied().collect();
        messages.sort_unstable();
        assert_eq!(messages, [1, 2, 3, 4, 5]);
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
