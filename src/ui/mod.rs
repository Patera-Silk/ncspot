use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

use cursive::backend::Backend;
use cursive::event::Event;
use cursive::theme;
use cursive::{Cursive, CursiveRunner, Vec2};
use ncspot::BIN_NAME;

use crate::application::SHUTDOWN_REQUESTED;

pub mod album;
pub mod artist;
pub mod browse;
pub mod contextmenu;
pub mod help;
pub mod layout;
pub mod library;
pub mod listview;
pub mod modal;
pub mod pagination;
pub mod playlist;
pub mod playlists;
pub mod queue;
pub mod search;
pub mod search_results;
pub mod show;
pub mod statusbar;
pub mod tabbedview;

#[cfg(feature = "cover")]
pub mod cover;

struct PanicSafeBackend {
    inner: RefCell<Option<Box<dyn Backend>>>,
    failed: Cell<bool>,
    last_size: Cell<Vec2>,
    name: String,
}

impl PanicSafeBackend {
    fn new(inner: Box<dyn Backend>) -> Self {
        Self {
            name: inner.name().to_string(),
            inner: RefCell::new(Some(inner)),
            failed: Cell::new(false),
            last_size: Cell::new((80, 24).into()),
        }
    }

    fn run<R>(&self, f: impl FnOnce(&dyn Backend) -> R) -> Option<R> {
        if self.failed.get() {
            return None;
        }

        let inner = self.inner.borrow();
        let inner = inner.as_deref()?;
        match catch_unwind(AssertUnwindSafe(|| f(inner))) {
            Ok(value) => Some(value),
            Err(_) => {
                self.failed.set(true);
                None
            }
        }
    }

    fn run_mut<R>(&self, f: impl FnOnce(&mut dyn Backend) -> R) -> Option<R> {
        if self.failed.get() {
            return None;
        }

        let mut inner = self.inner.borrow_mut();
        let inner = inner.as_deref_mut()?;
        match catch_unwind(AssertUnwindSafe(|| f(inner))) {
            Ok(value) => Some(value),
            Err(_) => {
                self.failed.set(true);
                None
            }
        }
    }
}

impl Backend for PanicSafeBackend {
    fn poll_event(&mut self) -> Option<Event> {
        if SHUTDOWN_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }

        if self.failed.get() {
            return Some(Event::Exit);
        }

        let event = self.run_mut(|inner| inner.poll_event()).flatten();
        if self.failed.get() {
            Some(Event::Exit)
        } else {
            event
        }
    }

    fn set_title(&mut self, title: String) {
        self.run_mut(|inner| inner.set_title(title));
    }

    fn refresh(&mut self) {
        self.run_mut(|inner| inner.refresh());
    }

    fn has_colors(&self) -> bool {
        self.run(|inner| inner.has_colors()).unwrap_or(false)
    }

    fn screen_size(&self) -> Vec2 {
        let size = self
            .run(|inner| inner.screen_size())
            .unwrap_or_else(|| self.last_size.get());
        let size = Vec2::new(size.x.max(1), size.y.max(6));
        self.last_size.set(size);
        size
    }

    fn move_to(&self, pos: Vec2) {
        self.run(|inner| inner.move_to(pos));
    }

    fn print(&self, text: &str) {
        self.run(|inner| inner.print(text));
    }

    fn clear(&self, color: theme::Color) {
        self.run(|inner| inner.clear(color));
    }

    fn set_color(&self, colors: theme::ColorPair) -> theme::ColorPair {
        self.run(|inner| inner.set_color(colors)).unwrap_or(colors)
    }

    fn set_effect(&self, effect: theme::Effect) {
        self.run(|inner| inner.set_effect(effect));
    }

    fn is_persistent(&self) -> bool {
        self.run(|inner| inner.is_persistent()).unwrap_or(false)
    }

    fn unset_effect(&self, effect: theme::Effect) {
        self.run(|inner| inner.unset_effect(effect));
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for PanicSafeBackend {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.get_mut().take() {
            if self.failed.get() {
                std::mem::forget(inner);
            } else {
                let _ = catch_unwind(AssertUnwindSafe(|| drop(inner)));
            }
        }
    }
}

/// Create a CursiveRunner which implements the drawing logic and event loop.
pub fn create_cursive() -> Result<CursiveRunner<Cursive>, Box<dyn std::error::Error>> {
    let backend = Box::new(PanicSafeBackend::new(cursive::backends::try_default()?));
    let mut cursive_runner = CursiveRunner::new(cursive::Cursive::new(), backend);

    cursive_runner.set_window_title(BIN_NAME);

    Ok(cursive_runner)
}
