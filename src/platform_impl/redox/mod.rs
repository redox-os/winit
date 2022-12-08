#![cfg(target_os = "redox")]

use std::{
    collections::VecDeque,
    mem,
    slice,
    str,
    sync::{Arc, mpsc, Mutex, RwLock},
    time::Instant,
};
use orbclient::EventOption;
use raw_window_handle::{
    OrbitalDisplayHandle, OrbitalWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    event::{self, StartCause, VirtualKeyCode},
    event_loop::{self, ControlFlow},
    monitor,
    window::{self, CursorGrabMode, ResizeDirection},
};

//TODO: implement in relibc
#[no_mangle]
pub extern "C" fn tzset() {
    unimplemented!("tzset");
}

const ORBITAL_FLAG_ASYNC: char = 'a';
const ORBITAL_FLAG_FRONT: char = 'f';
const ORBITAL_FLAG_BORDERLESS: char = 'l';
const ORBITAL_FLAG_RESIZABLE: char = 'r';
const ORBITAL_FLAG_TRANSPARENT: char = 't';

struct RedoxSocket {
    fd: usize,
}

impl RedoxSocket {
    unsafe fn open(path: &str) -> syscall::Result<Self> {
        let fd = syscall::open(path, syscall::O_RDWR | syscall::O_CLOEXEC)?;
        Ok(Self { fd })
    }

    fn read(&self, buf: &mut [u8]) -> syscall::Result<()> {
        let count = syscall::read(self.fd, buf)?;
        if count == buf.len() {
            Ok(())
        } else {
            Err(syscall::Error::new(syscall::EINVAL))
        }
    }

    fn write(&self, buf: &[u8]) -> syscall::Result<()> {
        let count = syscall::write(self.fd, buf)?;
        if count == buf.len() {
            Ok(())
        } else {
            Err(syscall::Error::new(syscall::EINVAL))
        }
    }

    fn fpath<'a>(&self, buf: &'a mut [u8]) -> syscall::Result<&'a str> {
        let count = syscall::fpath(self.fd, buf)?;
        str::from_utf8(&buf[..count]).map_err(|_err| {
            syscall::Error::new(syscall::EINVAL)
        })
    }
}

impl Drop for RedoxSocket {
    fn drop(&mut self) {
        let _ = syscall::close(self.fd);
    }
}

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
        let (user_events_sender, user_events_receiver) = mpsc::channel();

        let event_socket = Arc::new(unsafe {
            RedoxSocket::open("event:").unwrap()
        });

        let wake_socket = Arc::new(unsafe {
            RedoxSocket::open("time:4").unwrap()
        });

        event_socket.write(&syscall::Event {
            id: wake_socket.fd,
            flags: syscall::EventFlags::EVENT_READ,
            data: wake_socket.fd,
        }).unwrap();

        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    windows: RwLock::new(Vec::new()),
                    user_events_sender,
                    user_events_receiver,
                    creates: Mutex::new(VecDeque::new()),
                    redraws: Arc::new(Mutex::new(VecDeque::new())),
                    destroys: Arc::new(Mutex::new(VecDeque::new())),
                    event_socket,
                    wake_socket,
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

    pub fn run_return<F>(&mut self, mut event_handler_inner: F) -> i32
    where
        F: FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        // Wrapper for event handler function that prevents ExitWithCode from being unset
        let mut event_handler = move |
            event: event::Event<'_, T>,
            window_target: &event_loop::EventLoopWindowTarget<T>,
            control_flow: &mut ControlFlow
        | {
            if let ControlFlow::ExitWithCode(code) = control_flow {
                event_handler_inner(event, window_target, &mut ControlFlow::ExitWithCode(*code));
            } else {
                event_handler_inner(event, window_target, control_flow);
            }
        };

        let mut control_flow = ControlFlow::default();
        let mut start_cause = StartCause::Init;

        let code = loop {
            event_handler(event::Event::NewEvents(start_cause), &self.window_target, &mut control_flow);

            if start_cause == StartCause::Init {
                event_handler(event::Event::Resumed, &self.window_target, &mut control_flow);
            }

            // Handle window creates
            while let Some(window) = self.window_target.p.creates.lock().unwrap().pop_front() {
                let window_id = WindowId {
                    fd: window.fd as u64,
                };

                let mut buf: [u8; 4096] = [0; 4096];
                let path = window.fpath(&mut buf)
                    .expect("failed to read properties");
                let properties = WindowProperties::new(path);

                // Send resize event on create to indicate first size
                event_handler(event::Event::WindowEvent {
                    window_id: window::WindowId(window_id),
                    event: event::WindowEvent::Resized((properties.w, properties.h).into()),
                }, &self.window_target, &mut control_flow);

                // Send resize event on create to indicate first position
                event_handler(event::Event::WindowEvent {
                    window_id: window::WindowId(window_id),
                    event: event::WindowEvent::Moved((properties.x, properties.y).into()),
                }, &self.window_target, &mut control_flow);
            }

            // Handle window destroys
            while let Some(destroy_id) = self.window_target.p.destroys.lock().unwrap().pop_front() {
                event_handler(event::Event::WindowEvent {
                    window_id: window::WindowId(destroy_id),
                    event: event::WindowEvent::Destroyed,
                }, &self.window_target, &mut control_flow);

                self.window_target.p.windows.write().unwrap().retain(|window| {
                    window.fd as u64 != destroy_id.fd
                });
            }

            //TODO: do window destroys here for efficiency
            let mut i = 0;
            let mut resize_opt = None;
            loop {
                let window = {
                    let windows = self.window_target.p.windows.read().unwrap();
                    match windows.get(i) {
                        Some(window) => window.clone(),
                        None => break,
                    }
                };

                let window_id = WindowId {
                    fd: window.fd as u64,
                };

                let mut event_buf = [0u8; 16 * mem::size_of::<orbclient::Event>()];
                let count = syscall::read(window.fd, &mut event_buf)
                    .expect("failed to read window events");
                let events = unsafe {
                    slice::from_raw_parts(
                        event_buf.as_ptr() as *const orbclient::Event,
                        count / mem::size_of::<orbclient::Event>()
                    )
                };

                for event in events {
                    match event.to_option() {
                        EventOption::Key(event) => {
                            if event.scancode != 0 {
                                let vk_opt = convert_scancode(event.scancode);
                                if let Some(vk) = vk_opt {
                                    self.state.key(vk, event.pressed);
                                }
                                event_handler(event::Event::WindowEvent {
                                    window_id: window::WindowId(window_id),
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
                                window_id: window::WindowId(window_id),
                                event: event::WindowEvent::ReceivedCharacter(event.character),
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Mouse(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id: window::WindowId(window_id),
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
                                    window_id: window::WindowId(window_id),
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
                                window_id: window::WindowId(window_id),
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
                                window_id: window::WindowId(window_id),
                                event: event::WindowEvent::CloseRequested
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Focus(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id: window::WindowId(window_id),
                                event: event::WindowEvent::Focused(event.focused)
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Move(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id: window::WindowId(window_id),
                                event: event::WindowEvent::Moved((event.x, event.y).into())
                            }, &self.window_target, &mut control_flow);
                        },
                        EventOption::Resize(event) => {
                            event_handler(event::Event::WindowEvent {
                                window_id: window::WindowId(window_id),
                                event: event::WindowEvent::Resized((event.width, event.height).into())
                            }, &self.window_target, &mut control_flow);

                            // Acknowledge resize after event loop
                            resize_opt = Some((event.width, event.height));
                        },
                        //TODO: Clipboard
                        EventOption::Hover(event) => {
                            if event.entered {
                                event_handler(event::Event::WindowEvent {
                                    window_id: window::WindowId(window_id),
                                    event: event::WindowEvent::CursorEntered {
                                        device_id: event::DeviceId(DeviceId),
                                    }
                                }, &self.window_target, &mut control_flow);
                            } else {
                                event_handler(event::Event::WindowEvent {
                                    window_id: window::WindowId(window_id),
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

                if count == event_buf.len() {
                    // If event buf was full, process same window again to ensure all events are drained
                    continue;
                }

                // Acknowledge the latest resize event
                if let Some((w, h)) = resize_opt.take() {
                    window.write(&format!("S,{},{}", w, h).as_bytes())
                        .expect("failed to acknowledge resize");

                    // Require redraw after resize
                    let mut redraws = self.window_target.p.redraws.lock().unwrap();
                    if !redraws.contains(&window_id) {
                        redraws.push_back(window_id);
                    }
                }

                // Move to next window
                i += 1;
            }

            while let Ok(event) = self.window_target.p.user_events_receiver.try_recv() {
                event_handler(event::Event::UserEvent(event), &self.window_target, &mut control_flow);
            }

            event_handler(event::Event::MainEventsCleared, &self.window_target, &mut control_flow);

            // To avoid deadlocks the redraws lock is not held during event processing
            while let Some(window_id) = {
                let mut redraws = self.window_target.p.redraws.lock().unwrap();
                redraws.pop_front()
            } {
                event_handler(
                    event::Event::RedrawRequested(window::WindowId(window_id)),
                    &self.window_target,
                    &mut control_flow,
                );
            }

            event_handler(event::Event::RedrawEventsCleared, &self.window_target, &mut control_flow);

            let mut requested_resume = None;
            let wait = match control_flow {
                ControlFlow::Poll => false,
                ControlFlow::Wait => true,
                ControlFlow::WaitUntil(instant) => {
                    requested_resume = Some(instant);
                    true
                },
                //TODO: close windows?
                ControlFlow::ExitWithCode(code) => break code,
            };

            if wait {
                //TODO: could we re-use wake socket?
                // Re-using wake socket caused extra wake events before because there were leftover
                // timeouts, and then new timeouts were added each time a spurious timeout expired
                let timeout_socket = unsafe {
                    RedoxSocket::open("time:4").unwrap()
                };

                self.window_target.p.event_socket.write(&syscall::Event {
                    id: timeout_socket.fd,
                    flags: syscall::EventFlags::EVENT_READ,
                    data: 0,
                }).unwrap();

                let start = Instant::now();
                if let Some(instant) = requested_resume {
                    let mut time = syscall::TimeSpec::default();
                    timeout_socket.read(&mut time).unwrap();

                    match instant.checked_duration_since(start) {
                        Some(duration) => {
                            time.tv_sec += duration.as_secs() as i64;
                            time.tv_nsec += duration.subsec_nanos() as i32;
                            while time.tv_nsec >= 1_000_000_000 {
                                time.tv_sec += 1;
                                time.tv_nsec -= 1_000_000_000;
                            }
                        },
                        None => (),
                    }

                    //TODO: can we just write the instant directly?
                    timeout_socket.write(&time).unwrap();
                }

                // Wait for event if needed
                let mut event = syscall::Event::default();
                self.window_target.p.event_socket.read(&mut event).unwrap();

                if event.id == timeout_socket.fd {
                    // If the event is from the special timeout socket, report that resume time
                    // was reached
                    match requested_resume {
                        Some(requested_resume) => {
                            start_cause = StartCause::ResumeTimeReached {
                                start,
                                requested_resume,
                            };
                        },
                        None => {
                            warn!("unexpected timeout {:?}", event);
                            start_cause = StartCause::WaitCancelled {
                                start,
                                requested_resume,
                            };
                        },
                    }
                } else {
                    start_cause = StartCause::WaitCancelled {
                        start,
                        requested_resume,
                    };
                }
            } else {
                start_cause = StartCause::Poll;
            }
        };

        event_handler(event::Event::LoopDestroyed, &self.window_target, &mut control_flow);

        code
    }

    pub fn window_target(&self) -> &event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_events_sender: self.window_target.p.user_events_sender.clone(),
            wake_socket: self.window_target.p.wake_socket.clone(),
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    user_events_sender: mpsc::Sender<T>,
    wake_socket: Arc<RedoxSocket>,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.user_events_sender
            .send(event)
            .map_err(|mpsc::SendError(x)| event_loop::EventLoopClosed(x))?;

        // Writing a default TimeSpec will always trigger a time event
        self.wake_socket.write(&syscall::TimeSpec::default()).unwrap();

        Ok(())
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            user_events_sender: self.user_events_sender.clone(),
            wake_socket: self.wake_socket.clone(),
        }
    }
}

unsafe impl<T> Send for EventLoopProxy<T> {}

impl<T> Unpin for EventLoopProxy<T> {}

pub struct EventLoopWindowTarget<T: 'static> {
    windows: RwLock<Vec<Arc<RedoxSocket>>>,
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: mpsc::Receiver<T>,
    creates: Mutex<VecDeque<Arc<RedoxSocket>>>,
    redraws: Arc<Mutex<VecDeque<WindowId>>>,
    destroys: Arc<Mutex<VecDeque<WindowId>>>,
    event_socket: Arc<RedoxSocket>,
    wake_socket: Arc<RedoxSocket>,
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
        Self {
            fd
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
        // orbital:/x/y/w/h/t
        let mut parts = path.split('/');
        let flags = parts.next().unwrap_or("");
        let x = parts.next().map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let y = parts.next().map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let w = parts.next().map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let h = parts.next().map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let title = parts.next().unwrap_or("");
        Self {
            flags,
            x,
            y,
            w,
            h,
            title
        }
    }
}

pub struct Window {
    window_socket: Arc<RedoxSocket>,
    redraws: Arc<Mutex<VecDeque<WindowId>>>,
    destroys: Arc<Mutex<VecDeque<WindowId>>>,
    wake_socket: Arc<RedoxSocket>,
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

        // Async by default
        let mut flag_str = ORBITAL_FLAG_ASYNC.to_string();

        if attrs.resizable {
            flag_str.push(ORBITAL_FLAG_RESIZABLE);
        }

        //TODO: maximized, fullscreen, visible

        if attrs.transparent {
            flag_str.push(ORBITAL_FLAG_TRANSPARENT);
        }

        if ! attrs.decorations {
            flag_str.push(ORBITAL_FLAG_BORDERLESS);
        }

        if attrs.always_on_top {
            flag_str.push(ORBITAL_FLAG_FRONT);
        }

        //TODO: window_icon

        // Open window
        let window = unsafe {
            RedoxSocket::open(&format!(
                "orbital:{}/{}/{}/{}/{}/{}",
                flag_str, x, y, w, h, attrs.title
            )).expect("failed to open window")
        };

        // Add to event socket
        el.event_socket.write(&syscall::Event {
            id: window.fd,
            flags: syscall::EventFlags::EVENT_READ,
            data: window.fd,
        }).unwrap();

        let window_socket = Arc::new(window);
        el.windows.write().unwrap().push(window_socket.clone());

        // Notify event thread that this window was created, it will send some default events
        el.creates.lock().unwrap().push_back(window_socket.clone());

        // Writing a default TimeSpec will always trigger a time event
        el.wake_socket.write(&syscall::TimeSpec::default()).unwrap();

        Ok(Self {
            window_socket,
            redraws: el.redraws.clone(),
            destroys: el.destroys.clone(),
            wake_socket: el.wake_socket.clone(),
        })
    }

    pub fn id(&self) -> WindowId {
        WindowId {
            fd: self.window_socket.fd as u64,
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
        let window_id = self.id();
        let mut redraws = self.redraws.lock().unwrap();
        if !redraws.contains(&window_id) {
            redraws.push_back(window_id);

            // Writing a default TimeSpec will always trigger a time event
            self.wake_socket.write(&syscall::TimeSpec::default()).unwrap();
        }
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        Ok((properties.x, properties.y).into())
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        //TODO: adjust for window decorations
        self.inner_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        //TODO: adjust for window decorations
        let (x, y): (i32, i32) = position.to_physical::<i32>(self.scale_factor()).into();
        self.window_socket.write(&format!("P,{},{}", x, y).as_bytes())
            .expect("failed to set position");
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        (properties.w, properties.h).into()
    }

    pub fn set_inner_size(&self, size: Size) {
        let (w, h): (u32, u32) = size.to_physical::<u32>(self.scale_factor()).into();
        self.window_socket.write(&format!("S,{},{}", w, h).as_bytes())
            .expect("failed to set size");
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
        self.window_socket.write(&format!("T,{}", title).as_bytes())
            .expect("failed to set title");
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
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        properties.flags.contains(ORBITAL_FLAG_RESIZABLE)
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
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        ! properties.flags.contains(ORBITAL_FLAG_BORDERLESS)
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
        let mut handle = OrbitalWindowHandle::empty();
        handle.window = self.window_socket.fd as *mut _;
        RawWindowHandle::Orbital(handle)
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Orbital(OrbitalDisplayHandle::empty())
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.destroys.lock().unwrap().push_back(self.id());

        // Writing a default TimeSpec will always trigger a time event
        self.wake_socket.write(&syscall::TimeSpec::default()).unwrap();
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

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
