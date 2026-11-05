//! The macOS menu bar (NSMenu), built with `muda`.
//!
//! Every call here must run on the main thread (`muda` panics elsewhere);
//! [`Native::sync`] is called from the event loop, which runs there.
use super::{Flags, Node, Spec};
use crate::Proxy;
use crate::core::menu::{Menu, Predefined};

use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
use rustc_hash::{FxHashMap, FxHashSet};

// A poisoned lock still holds a usable map; panicking inside an AppKit
// callback would abort the process instead.
use std::sync::{Arc, Mutex, PoisonError};

/// The messages of the live rows plus the proxy that delivers them; shared
/// with the `muda` event handler.
struct Router<Message: 'static> {
    actions: FxHashMap<String, Message>,
    proxy: Proxy<Message>,
    /// Rows chosen since the last sync; AppKit may have flipped their check
    /// mark, so their flags are written again even when unchanged.
    clicked: FxHashSet<String>,
}

/// A row the sync can flip in place.
enum Handle {
    Item(MenuItem),
    Check(CheckMenuItem),
    Submenu(Submenu),
}

/// The installed menu bar.
struct Installed {
    // Kept alive while installed; dropping it does not remove it from NSApp.
    menu: muda::Menu,
    nodes: Vec<Node>,
    handles: FxHashMap<String, Handle>,
    /// The flags last written to each row.
    applied: FxHashMap<String, Flags>,
}

pub(crate) struct Native<Message: 'static> {
    installed: Option<Installed>,
    router: Option<Arc<Mutex<Router<Message>>>>,
    /// Rows that failed to build; not retried until they change.
    failed: Option<Vec<Node>>,
}

impl<Message: Send + 'static> Native<Message> {
    pub(crate) fn new() -> Self {
        Self {
            installed: None,
            router: None,
            failed: None,
        }
    }

    /// Brings the native menu bar in line with `menu`.
    ///
    /// Same rows: only the flags that changed (or rows chosen since the last
    /// sync, whose check mark AppKit flips itself) are written. Different
    /// rows: the menu is rebuilt, and the old one stays installed if that
    /// fails. `None`: the menu is taken down.
    pub(crate) fn sync(
        &mut self,
        menu: impl FnOnce() -> Option<Menu<Message>>,
        proxy: &Proxy<Message>,
    ) {
        let Some(menu) = menu() else {
            if let Some(installed) = self.installed.take() {
                installed.menu.remove_for_nsapp();
            }
            if let Some(router) = &self.router {
                router
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .actions
                    .clear();
            }
            return;
        };

        let Spec {
            nodes,
            flags,
            actions,
        } = Spec::new(menu);

        let clicked = {
            let mut router = self
                .router(proxy)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);

            router.actions = actions;
            std::mem::take(&mut router.clicked)
        };

        let is_same = self
            .installed
            .as_ref()
            .is_some_and(|installed| installed.nodes == nodes);

        if !is_same && self.failed.as_ref() != Some(&nodes) {
            match build(nodes.clone()) {
                Ok(installed) => {
                    if let Some(old) = self.installed.take() {
                        old.menu.remove_for_nsapp();
                    }
                    installed.menu.init_for_nsapp();
                    self.installed = Some(installed);
                    self.failed = None;
                }
                Err(error) => {
                    log::error!(
                        "Failed to build the native menu bar, keeping the old one: {error}"
                    );
                    self.failed = Some(nodes);
                    return;
                }
            }
        }

        if let Some(installed) = &mut self.installed {
            apply(installed, &flags, &clicked);
        }
    }

    /// Returns the router, installing the `muda` event handler on first use.
    fn router(&mut self, proxy: &Proxy<Message>) -> &Arc<Mutex<Router<Message>>> {
        self.router.get_or_insert_with(|| {
            let router = Arc::new(Mutex::new(Router {
                actions: FxHashMap::default(),
                proxy: proxy.clone(),
                clicked: FxHashSet::default(),
            }));

            let handler = router.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                let mut router = handler.lock().unwrap_or_else(PoisonError::into_inner);

                // Taken, not cloned: a second click before the next update
                // (which refreshes the map) cannot send a stale message.
                let _ = router.clicked.insert(event.id.0.clone());

                match router.actions.remove(&event.id.0) {
                    Some(message) => router.proxy.send(message),
                    None => log::debug!("Ignored inactive menu item: {}", event.id.0),
                }
            }));

            router
        })
    }
}

fn build(nodes: Vec<Node>) -> muda::Result<Installed> {
    let menu = muda::Menu::new();
    let mut handles = FxHashMap::default();

    for node in &nodes {
        match node {
            Node::Submenu { .. } => {
                let submenu = submenu(node, &mut handles)?;
                menu.append(&submenu)?;
            }
            // NSMenu shows only submenus at the top level.
            _ => log::warn!("Ignored a top-level native menu row that is not a submenu"),
        }
    }

    Ok(Installed {
        menu,
        nodes,
        handles,
        applied: FxHashMap::default(),
    })
}

fn submenu(node: &Node, handles: &mut FxHashMap<String, Handle>) -> muda::Result<Submenu> {
    let Node::Submenu {
        id,
        label,
        children,
    } = node
    else {
        unreachable!("submenu() is only called with a submenu node");
    };

    let submenu = Submenu::with_id(id.as_str(), label, true);

    for child in children {
        match child {
            Node::Action {
                id,
                label,
                shortcut,
                checkable,
            } => {
                let accelerator = shortcut.as_deref().and_then(parse_accelerator);

                if *checkable {
                    let item = CheckMenuItem::with_id(id.as_str(), label, true, false, accelerator);
                    submenu.append(&item)?;
                    let _ = handles.insert(id.clone(), Handle::Check(item));
                } else {
                    let item = MenuItem::with_id(id.as_str(), label, true, accelerator);
                    submenu.append(&item)?;
                    let _ = handles.insert(id.clone(), Handle::Item(item));
                }
            }
            Node::Submenu { .. } => {
                let nested = self::submenu(child, handles)?;
                submenu.append(&nested)?;
            }
            Node::Separator => submenu.append(&PredefinedMenuItem::separator())?,
            Node::Predefined(predefined) => submenu.append(&self::predefined(*predefined))?,
        }
    }

    let _ = handles.insert(id.clone(), Handle::Submenu(submenu.clone()));

    Ok(submenu)
}

fn apply(installed: &mut Installed, flags: &[(String, Flags)], clicked: &FxHashSet<String>) {
    for (id, flags) in flags {
        if installed.applied.get(id) == Some(flags) && !clicked.contains(id) {
            continue;
        }

        match installed.handles.get(id) {
            Some(Handle::Item(item)) => item.set_enabled(flags.enabled),
            Some(Handle::Check(item)) => {
                item.set_enabled(flags.enabled);
                item.set_checked(flags.checked);
            }
            Some(Handle::Submenu(submenu)) => submenu.set_enabled(flags.enabled),
            None => {}
        }

        let _ = installed.applied.insert(id.clone(), *flags);
    }
}

fn parse_accelerator(shortcut: &str) -> Option<Accelerator> {
    match shortcut.parse() {
        Ok(accelerator) => Some(accelerator),
        Err(error) => {
            log::warn!("Ignored invalid menu shortcut {shortcut:?}: {error}");
            None
        }
    }
}

fn predefined(predefined: Predefined) -> PredefinedMenuItem {
    match predefined {
        Predefined::About => PredefinedMenuItem::about(None, None),
        Predefined::Services => PredefinedMenuItem::services(None),
        Predefined::Hide => PredefinedMenuItem::hide(None),
        Predefined::HideOthers => PredefinedMenuItem::hide_others(None),
        Predefined::ShowAll => PredefinedMenuItem::show_all(None),
        // Sends `terminate:`, which goes through `applicationShouldTerminate:`
        // and so reaches `system::quit_requests` when enabled.
        Predefined::Quit => PredefinedMenuItem::quit(None),
        Predefined::Minimize => PredefinedMenuItem::minimize(None),
        // On macOS `muda`'s maximize item performs Zoom.
        Predefined::Zoom => PredefinedMenuItem::maximize(None),
        Predefined::Fullscreen => PredefinedMenuItem::fullscreen(None),
        Predefined::CloseWindow => PredefinedMenuItem::close_window(None),
        Predefined::BringAllToFront => PredefinedMenuItem::bring_all_to_front(None),
    }
}
