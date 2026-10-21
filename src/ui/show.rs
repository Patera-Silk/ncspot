use std::sync::{Arc, RwLock};
use std::thread;

use cursive::Cursive;
use cursive::view::ViewWrapper;

use crate::command::Command;
use crate::commands::CommandResult;
use crate::library::Library;
use crate::model::episode::Episode;
use crate::model::show::Show;
use crate::queue::Queue;
use crate::traits::ViewExt;
use crate::ui::listview::ListView;

pub struct ShowView {
    list: ListView<Episode>,
    show: Show,
}

impl ShowView {
    pub fn new(queue: Arc<Queue>, library: Arc<Library>, show: &Show) -> Self {
        let show = show.clone();
        let items = Arc::new(RwLock::new(Vec::new()));
        let list = ListView::new(items.clone(), queue.clone(), library.clone());
        let pagination = list.get_pagination().clone();
        let spotify = queue.get_spotify();
        let show_id = show.id.clone();
        let library_for_thread = library.clone();

        // Load the first page in the background so opening a podcast does not block the UI.
        thread::spawn(move || {
            let mut results = spotify.api.show_episodes(&show_id);
            let first_page = results.items.read().unwrap().clone();
            items.write().unwrap().extend(first_page);
            results.items = items;
            results.apply_pagination(&pagination);
            library_for_thread.trigger_redraw();
        });

        Self { list, show }
    }
}

impl ViewWrapper for ShowView {
    wrap_impl!(self.list: ListView<Episode>);
}

impl ViewExt for ShowView {
    fn title(&self) -> String {
        self.show.name.clone()
    }

    fn on_command(&mut self, s: &mut Cursive, cmd: &Command) -> Result<CommandResult, String> {
        self.list.on_command(s, cmd)
    }
}
