use std::sync::{Arc, RwLock};

use crate::window::Window;

pub trait WindowExtRedox {
    fn orbclient_window(&self) -> Arc<RwLock<orbclient::Window>>;
}

impl WindowExtRedox for Window {
    fn orbclient_window(&self) -> Arc<RwLock<orbclient::Window>> {
        self.window.orbclient_window()
    }
}
