use std::sync::{Arc, RwLock};

pub trait WindowExtRedox {
    #[cfg(target_os = "redox")]
    fn orbclient_window(&self) -> Arc<RwLock<orbclient::Window>>;
}
