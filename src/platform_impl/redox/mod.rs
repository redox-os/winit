#![cfg(target_os = "redox")]

use std::{
    collections::VecDeque,
    marker::PhantomData,
    os::unix::io::AsRawFd,
    sync::{Arc, RwLock},
    time::Instant,
};
use orbclient::{EventOption, Renderer};
use raw_window_handle::{
    OrbitalDisplayHandle, OrbitalWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    event::{self, VirtualKeyCode},
    event_loop::{self, ControlFlow},
    monitor,
    platform::redox::WindowExtRedox,
    window::{self, CursorGrabMode, ResizeDirection},
};

fn convert_scancode(scancode: u8) -> Option<VirtualKeyCode> {
    match scancode {
        orbclient::K_A => Some(VirtualKeyCode::A),
        orbclient::K_B => Some(VirtualKeyCode::B),
        orbclient::K_C => Some(VirtualKeyCode::C),
        orbclient::K_D => Some(VirtualKeyCode::D),
        orbclient::K_E => Some(VirtualKeyCode::E),
        orbclient::K_F => Some(VirtualKeyCode::F),
        orbclient::K_G => Some(VirtualKeyCode::G),
        orbclient::K_H => Some(VirtualKeyCode::H),
        orbclient::K_I => Some(VirtualKeyCode::I),
        orbclient::K_J => Some(VirtualKeyCode::J),
        orbclient::K_K => Some(VirtualKeyCode::K),
        orbclient::K_L => Some(VirtualKeyCode::L),
        orbclient::K_M => Some(VirtualKeyCode::M),
        orbclient::K_N => Some(VirtualKeyCode::N),
        orbclient::K_O => Some(VirtualKeyCode::O),
        orbclient::K_P => Some(VirtualKeyCode::P),
        orbclient::K_Q => Some(VirtualKeyCode::Q),
        orbclient::K_R => Some(VirtualKeyCode::R),
        orbclient::K_S => Some(VirtualKeyCode::S),
        orbclient::K_T => Some(VirtualKeyCode::T),
        orbclient::K_U => Some(VirtualKeyCode::U),
        orbclient::K_V => Some(VirtualKeyCode::V),
        orbclient::K_W => Some(VirtualKeyCode::W),
        orbclient::K_X => Some(VirtualKeyCode::X),
        orbclient::K_Y => Some(VirtualKeyCode::Y),
        orbclient::K_Z => Some(VirtualKeyCode::Z),
        orbclient::K_0 => Some(VirtualKeyCode::Key0),
        orbclient::K_1 => Some(VirtualKeyCode::Key1),
        orbclient::K_2 => Some(VirtualKeyCode::Key2),
        orbclient::K_3 => Some(VirtualKeyCode::Key3),
        orbclient::K_4 => Some(VirtualKeyCode::Key4),
        orbclient::K_5 => Some(VirtualKeyCode::Key5),
        orbclient::K_6 => Some(VirtualKeyCode::Key6),
        orbclient::K_7 => Some(VirtualKeyCode::Key7),
        orbclient::K_8 => Some(VirtualKeyCode::Key8),
        orbclient::K_9 => Some(VirtualKeyCode::Key9),

        orbclient::K_TICK => Some(VirtualKeyCode::Grave),
        orbclient::K_MINUS => Some(VirtualKeyCode::Minus),
        orbclient::K_EQUALS => Some(VirtualKeyCode::Equals),
        orbclient::K_BACKSLASH => Some(VirtualKeyCode::Backslash),
        orbclient::K_BRACE_OPEN => Some(VirtualKeyCode::LBracket),
        orbclient::K_BRACE_CLOSE => Some(VirtualKeyCode::RBracket),
        orbclient::K_SEMICOLON => Some(VirtualKeyCode::Semicolon),
        orbclient::K_QUOTE => Some(VirtualKeyCode::Apostrophe),
        orbclient::K_COMMA => Some(VirtualKeyCode::Comma),
        orbclient::K_PERIOD => Some(VirtualKeyCode::Period),
        orbclient::K_SLASH => Some(VirtualKeyCode::Slash),
        orbclient::K_BKSP => Some(VirtualKeyCode::Back),
        orbclient::K_SPACE => Some(VirtualKeyCode::Space),
        orbclient::K_TAB => Some(VirtualKeyCode::Tab),
        //orbclient::K_CAPS => Some(VirtualKeyCode::CAPS),
        orbclient::K_LEFT_SHIFT => Some(VirtualKeyCode::LShift),
        orbclient::K_RIGHT_SHIFT => Some(VirtualKeyCode::RShift),
        orbclient::K_CTRL => Some(VirtualKeyCode::LControl),
        orbclient::K_ALT => Some(VirtualKeyCode::LAlt),
        orbclient::K_ENTER => Some(VirtualKeyCode::Return),
        orbclient::K_ESC => Some(VirtualKeyCode::Escape),
        orbclient::K_F1 => Some(VirtualKeyCode::F1),
        orbclient::K_F2 => Some(VirtualKeyCode::F2),
        orbclient::K_F3 => Some(VirtualKeyCode::F3),
        orbclient::K_F4 => Some(VirtualKeyCode::F4),
        orbclient::K_F5 => Some(VirtualKeyCode::F5),
        orbclient::K_F6 => Some(VirtualKeyCode::F6),
        orbclient::K_F7 => Some(VirtualKeyCode::F7),
        orbclient::K_F8 => Some(VirtualKeyCode::F8),
        orbclient::K_F9 => Some(VirtualKeyCode::F9),
        orbclient::K_F10 => Some(VirtualKeyCode::F10),
        orbclient::K_HOME => Some(VirtualKeyCode::Home),
        orbclient::K_UP => Some(VirtualKeyCode::Up),
        orbclient::K_PGUP => Some(VirtualKeyCode::PageUp),
        orbclient::K_LEFT => Some(VirtualKeyCode::Left),
        orbclient::K_RIGHT => Some(VirtualKeyCode::Right),
        orbclient::K_END => Some(VirtualKeyCode::End),
        orbclient::K_DOWN => Some(VirtualKeyCode::Down),
        orbclient::K_PGDN => Some(VirtualKeyCode::PageDown),
        orbclient::K_DEL => Some(VirtualKeyCode::Delete),
        orbclient::K_F11 => Some(VirtualKeyCode::F11),
        orbclient::K_F12 => Some(VirtualKeyCode::F12),

        _ => None
    }
}

fn element_state(pressed: bool) -> event::ElementState {
    if pressed {
        event::ElementState::Pressed
    } else {
        event::ElementState::Released
    }
}

#[derive(Default)]
struct EventState {
    lshift: bool,
    rshift: bool,
    lctrl: bool,
    rctrl: bool,
    lalt: bool,
    ralt: bool,
    llogo: bool,
    rlogo: bool,
    left: bool,
    middle: bool,
    right: bool,
}

impl EventState {
    fn key(&mut self, vk: VirtualKeyCode, pressed: bool) {
        match vk {
            VirtualKeyCode::LShift => self.lshift = pressed,
            VirtualKeyCode::RShift => self.rshift = pressed,
            VirtualKeyCode::LControl => self.lctrl = pressed,
            VirtualKeyCode::RControl => self.rctrl = pressed,
            VirtualKeyCode::LAlt => self.lalt = pressed,
            VirtualKeyCode::RAlt => self.ralt = pressed,
            VirtualKeyCode::LWin => self.llogo = pressed,
            VirtualKeyCode::RWin => self.rlogo = pressed,
            _ => ()
        }
    }

    fn mouse(&mut self, left: bool, middle: bool, right: bool) -> Option<(event::MouseButton, event::ElementState)> {
        if self.left != left {
            self.left = left;
            return Some((event::MouseButton::Left, element_state(self.left)));
        }

        if self.middle != middle {
            self.middle = middle;
            return Some((event::MouseButton::Middle, element_state(self.middle)));
        }

        if self.right != right {
            self.right = right;
            return Some((event::MouseButton::Right, element_state(self.right)));
        }

        None
    }

    fn modifiers(&self) -> event::ModifiersState {
        let mut modifiers = event::ModifiersState::empty();
        if self.lshift || self.rshift {
            modifiers |= event::ModifiersState::SHIFT;
        }
        if self.lctrl || self.rctrl {
            modifiers |= event::ModifiersState::CTRL;
        }
        if self.lalt || self.ralt {
            modifiers |= event::ModifiersState::ALT;
        }
        if self.llogo || self.rlogo {
            modifiers |= event::ModifiersState::LOGO
        }
        modifiers
    }
}

pub struct EventLoop<T: 'static> {
    window_target: event_loop::EventLoopWindowTarget<T>,
    state: EventState,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Self {
        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    windows: RwLock::new(Vec::new()),
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            state: EventState::default(),
        }
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static
            + FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let exit_code = self.run_return(event_handler);
        ::std::process::exit(exit_code);
    }

    pub fn run_return<F>(&mut self, mut event_handler: F) -> i32
    where
        F: FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::default();
        let mut pollfds = Vec::new();
        loop {
            for window in self.window_target.p.windows.read().unwrap().iter() {
                let window_id = window::WindowId(WindowId {
                    raw: window.read().unwrap().as_raw_fd() as u64
                });

                for event in window.write().unwrap().events() {
                    match event.to_option() {
                        EventOption::Key(event) => {
                            if event.scancode != 0 {
                                let vk_opt = convert_scancode(event.scancode);
                                if let Some(vk) = vk_opt {
                                    self.state.key(vk, event.pressed);
                                }
                                event_handler(event::Event::WindowEvent {
                                    window_id,
                                    event: event::WindowEvent::KeyboardInput {
                                        device_id: event::DeviceId(DeviceId),
                                        input: event::KeyboardInput {
                                            scancode: event.scancode as u32,
                                            state: element_state(event.pressed),
                                            virtual_keycode: vk_opt,
                                            modifiers: self.state.modifiers(),
                                        },
                                        is_synthetic: false,
                                    },
                                }, &self.window_target, &mut control_flow);
                            }
                        },
                        EventOption::TextInput(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::ReceivedCharacter(event.character),
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Mouse(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::CursorMoved {
                                    device_id: event::DeviceId(DeviceId),
                                    position: (event.x, event.y).into(),
                                    modifiers: self.state.modifiers(),
                                },
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Button(event) => {
                            while let Some((button, state)) = self.state.mouse(event.left, event.middle, event.right) {
                                event_handler(event::Event::WindowEvent {
                                    window_id,
                                    event: event::WindowEvent::MouseInput {
                                        device_id: event::DeviceId(DeviceId),
                                        state,
                                        button,
                                        modifiers: self.state.modifiers(),
                                    },
                                }, &self.window_target, &mut control_flow);
                            }
                        },
                        EventOption::Scroll(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::MouseWheel {
                                    device_id: event::DeviceId(DeviceId),
                                    delta: event::MouseScrollDelta::LineDelta(
                                        event.x as f32, event.y as f32
                                    ),
                                    phase: event::TouchPhase::Moved,
                                    modifiers: self.state.modifiers(),
                                },
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Quit(_event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::CloseRequested
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Focus(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::Focused(event.focused)
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Move(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::Moved((event.x, event.y).into())
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Resize(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::Resized((event.width, event.height).into())
                            }, &self.window_target, &mut control_flow);
                        },
                        //TODO: Clipboard
                        EventOption::Hover(event) => {
                            if event.entered {
                                event_handler(event::Event::WindowEvent {
                                    window_id,
                                    event: event::WindowEvent::CursorEntered {
                                        device_id: event::DeviceId(DeviceId),
                                    }
                                }, &self.window_target, &mut control_flow);
                            } else {
                                event_handler(event::Event::WindowEvent {
                                    window_id,
                                    event: event::WindowEvent::CursorLeft {
                                        device_id: event::DeviceId(DeviceId),
                                    }
                                }, &self.window_target, &mut control_flow);
                            }
                        },
                        other => {
                            warn!("Unhandled: {:?}", other);
                        }
                    }
                }
            }

            event_handler(event::Event::MainEventsCleared, &self.window_target, &mut control_flow);

            //TODO: do not always request redraw
            for window in self.window_target.p.windows.read().unwrap().iter() {
                let window_id = window::WindowId(WindowId {
                    raw: window.read().unwrap().as_raw_fd() as u64
                });

                event_handler(event::Event::RedrawRequested(
                    window_id
                ), &self.window_target, &mut control_flow);
            }

            event_handler(event::Event::RedrawEventsCleared, &self.window_target, &mut control_flow);

            let poll_timeout_opt = match control_flow {
                ControlFlow::Poll => None,
                ControlFlow::Wait => Some(-1),
                ControlFlow::WaitUntil(instant) => {
                    instant.checked_duration_since(Instant::now()).map(|duration| {
                        duration.as_millis().try_into().unwrap()
                    })
                },
                //TODO: close windows?
                ControlFlow::ExitWithCode(code) => return code,
            };

            // Poll windows if needed
            if let Some(poll_timeout) = poll_timeout_opt {
                pollfds.clear();
                for window in self.window_target.p.windows.read().unwrap().iter() {
                    pollfds.push(libc::pollfd {
                        fd: window.read().unwrap().as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    });
                }
                let _nevents = unsafe {
                    libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, poll_timeout)
                };
            }
        }
    }

    pub fn window_target(&self) -> &event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            _marker: PhantomData,
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    _marker: PhantomData<T>,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, _event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        unimplemented!("EventLoopProxy::send_event");
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            _marker: PhantomData,
        }
    }
}

unsafe impl<T> Send for EventLoopProxy<T> {}

impl<T> Unpin for EventLoopProxy<T> {}

pub struct EventLoopWindowTarget<T: 'static> {
    windows: RwLock<Vec<Arc<RwLock<orbclient::Window>>>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn primary_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Orbital(OrbitalDisplayHandle::empty())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowId {
    raw: u64,
}

impl WindowId {
    pub const fn dummy() -> Self {
        WindowId {
            raw: u64::max_value(),
        }
    }
}

impl From<WindowId> for u64 {
    fn from(id: WindowId) -> Self {
        id.raw
    }
}

impl From<u64> for WindowId {
    fn from(raw: u64) -> Self {
        Self {
            raw
        }
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
pub struct PlatformSpecificWindowBuilderAttributes;

pub struct Window {
    inner: Arc<RwLock<orbclient::Window>>,
}

impl Window {
    pub(crate) fn new<T: 'static>(
        el: &EventLoopWindowTarget<T>,
        attrs: window::WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, error::OsError> {
        let scale = MonitorHandle.scale_factor();

        let (x, y) = if let Some(pos) = attrs.position {
            pos.to_physical::<i32>(scale).into()
        } else {
            (-1, -1)
        };

        let (w, h) = if let Some(size) = attrs.inner_size {
            size.to_physical::<u32>(scale).into()
        } else {
            //TODO: what is a sane default?
            (800, 600)
        };

        //TODO: min/max inner_size

        let mut flags = vec![orbclient::WindowFlag::Async];

        if attrs.resizable {
            flags.push(orbclient::WindowFlag::Resizable);
        }

        //TODO: maximized, fullscreen, visible

        if attrs.transparent {
            flags.push(orbclient::WindowFlag::Transparent);
        }

        if ! attrs.decorations {
            flags.push(orbclient::WindowFlag::Borderless);
        }

        if attrs.always_on_top {
            flags.push(orbclient::WindowFlag::Front);
        }

        //TODO: window_icon

        let window = orbclient::Window::new_flags(
            x,
            y,
            w,
            h,
            &attrs.title,
            &flags
        ).ok_or(os_error!(OsError))?;

        let inner = Arc::new(RwLock::new(window));
        el.windows.write().unwrap().push(inner.clone());
        Ok(Self {
            inner
        })
    }

    pub fn id(&self) -> WindowId {
        WindowId {
            raw: self.inner.read().unwrap().as_raw_fd() as u64,
        }
    }

    pub fn primary_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    pub fn current_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn scale_factor(&self) -> f64 {
        MonitorHandle.scale_factor()
    }

    pub fn request_redraw(&self) {
        warn!("request_redraw not implemented on Redox");
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        let window = self.inner.read().unwrap();
        Ok((window.x(), window.y()).into())
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        //TODO: adjust for window decorations
        self.inner_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        //TODO: adjust for window decorations
        let (x, y) = position.to_physical::<i32>(self.scale_factor()).into();
        self.inner.write().unwrap().set_pos(x, y);
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let window = self.inner.read().unwrap();
        (window.width(), window.height()).into()
    }

    pub fn set_inner_size(&self, size: Size) {
        let (w, h) = size.to_physical::<u32>(self.scale_factor()).into();
        self.inner.write().unwrap().set_size(w, h);
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        //TODO: adjust for window decorations
        self.inner_size()
    }

    pub fn set_min_inner_size(&self, _: Option<Size>) {
        warn!("set_min_inner_size not implemented on Redox");
    }

    pub fn set_max_inner_size(&self, _: Option<Size>) {
        warn!("set_max_inner_size not implemented on Redox");
    }

    pub fn set_title(&self, title: &str) {
        self.inner.write().unwrap().set_title(title);
    }

    pub fn set_visible(&self, _visibility: bool) {
        warn!("set_visible not implemented on Redox");
    }

    pub fn is_visible(&self) -> Option<bool> {
        warn!("is_visible not implemented on Redox");
        None
    }

    pub fn set_resizable(&self, _resizeable: bool) {
        warn!("set_resizable not implemented on Redox");
    }

    pub fn is_resizable(&self) -> bool {
        warn!("is_resizable not implemented on Redox");
        false
    }

    pub fn set_minimized(&self, _minimized: bool) {
        warn!("set_minimized not implemented on Redox");
    }

    pub fn set_maximized(&self, _maximized: bool) {
        warn!("set_maximized not implemented on Redox");
    }

    pub fn is_maximized(&self) -> bool {
        warn!("is_maximized not implemented on Redox");
        false
    }

    pub fn set_fullscreen(&self, _monitor: Option<window::Fullscreen>) {
        warn!("set_fullscreen not implemented on Redox");
    }

    pub fn fullscreen(&self) -> Option<window::Fullscreen> {
        warn!("fullscreen not implemented on Redox");
        None
    }

    pub fn set_decorations(&self, _decorations: bool) {
        warn!("set_decorations not implemented on Redox");
    }

    pub fn is_decorated(&self) -> bool {
        warn!("is_decorated not implemented on Redox");
        true
    }

    pub fn set_always_on_top(&self, _always_on_top: bool) {
        warn!("set_always_on_top not implemented on Redox");
    }

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {
        warn!("set_window_icon not implemented on Redox");
    }

    pub fn set_ime_position(&self, _position: Position) {
        warn!("set_ime_position not implemented on Redox");
    }

    pub fn set_ime_allowed(&self, _allowed: bool) {
        warn!("set_ime_allowed not implemented on Redox");
    }

    pub fn focus_window(&self) {
        warn!("focus_window not implemented on Redox");
    }

    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {
        warn!("request_user_attention not implemented on Redox");
    }

    pub fn set_cursor_icon(&self, _: window::CursorIcon) {
        warn!("set_cursor_icon not implemented on Redox");
    }

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        warn!("set_cursor_position not implemented on Redox");
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        warn!("set_cursor_grab not implemented on Redox");
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_visible(&self, _: bool) {
        warn!("set_cursor_visible not implemented on Redox");
    }

    pub fn drag_window(&self) -> Result<(), error::ExternalError> {
        warn!("drag_window not implemented on Redox");
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), error::ExternalError> {
        warn!("drag_resize_window not implemented on Redox");
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        warn!("set_cursor_hittest not implemented on Redox");
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        //TODO
        RawWindowHandle::Orbital(OrbitalWindowHandle::empty())
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Orbital(OrbitalDisplayHandle::empty())
    }
}

impl WindowExtRedox for Window {
    fn orbclient_window(&self) -> Arc<RwLock<orbclient::Window>> {
        self.inner.clone()
    }
}

#[derive(Default, Clone, Debug)]
pub struct OsError;

use std::fmt::{self, Display, Formatter};
impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fmt, "Redox OS Error")
    }
}

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

    pub fn video_modes(&self) -> impl Iterator<Item = monitor::VideoMode> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(monitor::VideoMode {
            video_mode: VideoMode {
                size,
                bit_depth: 32,
                refresh_rate_millihertz: 60000,
                monitor: self.clone(),
            },
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoMode {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    monitor: MonitorHandle,
}

impl VideoMode {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: self.monitor.clone(),
        }
    }
}
