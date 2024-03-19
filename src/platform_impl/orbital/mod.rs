#![cfg(target_os = "redox")]

use std::fmt::{self, Display, Formatter};
use std::str;
use std::sync::Arc;

use libredox::data::TimeSpec;
use libredox::{errno, flag, Fd};
use smol_str::SmolStr;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    keyboard::Key,
};

use libredox::error::{Error, Result};

pub(crate) use self::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle};
mod event_loop;

pub use self::window::Window;
mod window;

struct RedoxSocket {
    fd: Fd,
}

impl RedoxSocket {
    fn orbital(properties: &WindowProperties<'_>) -> Result<Self> {
        Self::open_raw(&format!("{properties}"))
    }

    // Paths should be checked to ensure they are actually sockets and not normal files. If a
    // non-socket path is used, it could cause read and write to not function as expected. For
    // example, the seek would change in a potentially unpredictable way if either read or write
    // were called at the same time by multiple threads.
    fn open_raw(path: &str) -> Result<Self> {
        let fd = Fd::open(path, flag::O_RDWR | flag::O_CLOEXEC, 0)?;
        Ok(Self { fd })
    }

    fn write(&self, buf: &[u8]) -> Result<()> {
        let count = self.fd.write(buf)?;
        if count == buf.len() {
            Ok(())
        } else {
            Err(Error::new(errno::EINVAL))
        }
    }

    fn fpath<'a>(&self, buf: &'a mut [u8]) -> Result<&'a str> {
        let count = self.fd.fpath(buf)?;
        str::from_utf8(&buf[..count]).map_err(|_err| Error::new(errno::EINVAL))
    }
}

pub struct TimeSocket(Fd);

impl TimeSocket {
    fn open() -> Result<Self> {
        Fd::open("time:4", flag::O_RDWR | flag::O_CLOEXEC, 0).map(Self)
    }

    // Read current time.
    fn current_time(&self) -> Result<TimeSpec> {
        let mut bytes = [0_u8; std::mem::size_of::<TimeSpec>()];
        self.0.read(&mut bytes)?;
        Ok(*libredox::data::timespec_from_bytes(&bytes))
    }

    // Write a timeout.
    fn timeout(&self, timespec: &TimeSpec) -> Result<()> {
        let mut bytes = [0_u8; std::mem::size_of::<TimeSpec>()];
        *libredox::data::timespec_from_mut_bytes(&mut bytes) = *timespec;
        self.0.write(&bytes)?;
        Ok(())
    }

    // Wake immediately.
    fn wake(&self) -> Result<()> {
        // Writing a default TimeSpec will always trigger a time event.
        self.timeout(&TimeSpec {
            tv_sec: 0,
            tv_nsec: 0,
        })
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowId {
    fd: u64,
}

impl WindowId {
    pub const fn dummy() -> Self {
        WindowId {
            fd: u64::max_value(),
        }
    }
}

impl From<WindowId> for u64 {
    fn from(id: WindowId) -> Self {
        id.fd
    }
}

impl From<u64> for WindowId {
    fn from(fd: u64) -> Self {
        Self { fd }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId;

impl DeviceId {
    pub const fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;

struct WindowProperties<'a> {
    flags: &'a str,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    title: &'a str,
}

impl<'a> WindowProperties<'a> {
    fn new(path: &'a str) -> Self {
        // orbital:flags/x/y/w/h/t
        let mut parts = path.splitn(6, '/');
        let flags = parts.next().unwrap_or("");
        let x = parts
            .next()
            .map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let y = parts
            .next()
            .map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let w = parts
            .next()
            .map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let h = parts
            .next()
            .map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let title = parts.next().unwrap_or("");
        Self {
            flags,
            x,
            y,
            w,
            h,
            title,
        }
    }
}

impl<'a> fmt::Display for WindowProperties<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "orbital:{}/{}/{}/{}/{}/{}",
            self.flags, self.x, self.y, self.w, self.h, self.title
        )
    }
}

#[derive(Clone, Debug)]
pub struct OsError(Arc<Error>);

impl OsError {
    fn new(error: Error) -> Self {
        Self(Arc::new(error))
    }
}

impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(fmt)
    }
}

pub(crate) use crate::cursor::NoCustomCursor as PlatformCustomCursor;
pub(crate) use crate::cursor::NoCustomCursor as PlatformCustomCursorSource;
pub(crate) use crate::icon::NoIcon as PlatformIcon;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        Some("Redox Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(0, 0) // TODO
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    pub fn scale_factor(&self) -> f64 {
        1.0 // TODO
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        // FIXME no way to get real refresh rate for now.
        None
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(VideoModeHandle {
            size,
            bit_depth: 32,
            refresh_rate_millihertz: 60000,
            monitor: self.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    monitor: MonitorHandle,
}

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {
    pub key_without_modifiers: Key,
    pub text_with_all_modifiers: Option<SmolStr>,
}
