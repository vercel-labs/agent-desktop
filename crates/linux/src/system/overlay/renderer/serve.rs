//! The renderer's loop. It accepts one request per connection, plays it on
//! the cursor layer, draws the layer and answers each request once what its
//! sender waits for is on the screen: a travel once the cursor has arrived,
//! a conceal once every cursor window is unmapped. It sleeps until the next
//! frame while anything moves and until the next request otherwise.
use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, Rect};
use serde_json::Value;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;

use super::layer::CursorLayer;
use super::painter::Painter;
use super::request::{parse_locate, parse_request};
use super::screens::{Screens, X11Desk};
use super::session_watch::{SessionWatch, classify, read_manifest};
use super::stage::Stage;
use super::travel::Ticket;
use super::wire;
use crate::system::overlay::cursor_overlay::error;

const FRAME: Duration = Duration::from_millis(16);
const WATCH_EVERY: Duration = Duration::from_millis(1500);
/// How long the renderer stays with no cursor to draw before it exits.
pub(crate) const IDLE_EXIT: Duration = Duration::from_secs(30);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const LONGEST_SLEEP: Duration = Duration::from_secs(1);

/// The connections waiting on an answer.
#[derive(Default)]
struct Pending {
    streams: HashMap<Ticket, UnixStream>,
    next: Ticket,
    ready: Vec<Ticket>,
}

/// When the renderer last checked its sessions, and since when it has had
/// nothing to draw.
struct Clock {
    watched_at: Instant,
    idle_since: Option<Instant>,
    watch: SessionWatch,
}

pub(crate) struct Server {
    stage: Stage,
    listener: UnixListener,
    layer: CursorLayer,
    painter: Painter,
    screens: Screens,
    pending: Pending,
    clock: Clock,
}

fn same_user(stream: &UnixStream) -> bool {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let read = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut credentials).cast(),
            &raw mut length,
        )
    };
    read == 0 && credentials.uid == rustix::process::getuid().as_raw()
}

fn readable(listener: &UnixListener, stage: &Stage, timeout: Duration) -> io::Result<bool> {
    let mut descriptors = [
        libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: stage.connection.stream().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    let ready = unsafe {
        libc::poll(
            descriptors.as_mut_ptr(),
            descriptors.len() as libc::nfds_t,
            millis,
        )
    };
    match ready {
        -1 if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted => Ok(false),
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(descriptors[0].revents & libc::POLLIN != 0),
    }
}

impl Server {
    pub(crate) fn new(stage: Stage, listener: UnixListener, painter: Painter) -> Self {
        let now = Instant::now();
        Self {
            stage,
            listener,
            layer: CursorLayer::default(),
            painter,
            screens: Screens::default(),
            pending: Pending::default(),
            clock: Clock {
                watched_at: now,
                idle_since: Some(now),
                watch: SessionWatch::default(),
            },
        }
    }

    fn root_size(&self) -> Rect {
        let screen = self
            .stage
            .connection
            .setup()
            .roots
            .iter()
            .find(|screen| screen.root == self.stage.root);
        screen.map_or(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            |screen| Rect {
                x: 0.0,
                y: 0.0,
                width: f64::from(screen.width_in_pixels),
                height: f64::from(screen.height_in_pixels),
            },
        )
    }

    /// Runs until the X connection fails or nothing has been drawn for
    /// [`IDLE_EXIT`].
    pub(crate) fn run(&mut self) -> Result<(), AdapterError> {
        loop {
            let now = Instant::now();
            let mut answered = std::mem::take(&mut self.pending.ready);
            answered.extend(self.layer.tick(now));
            answered.extend(self.watch_sessions(now));
            self.screens.refresh(&self.stage, now)?;
            let desk = X11Desk {
                stage: &self.stage,
                screens: &self.screens,
                root_size: self.root_size(),
            };
            self.painter.paint(&self.stage, &self.layer, &desk, now)?;
            if !answered.is_empty() {
                self.stage
                    .connection
                    .get_input_focus()
                    .map_err(error)?
                    .reply()
                    .map_err(error)?;
                self.answer(&answered);
            }
            while self
                .stage
                .connection
                .poll_for_event()
                .map_err(error)?
                .is_some()
            {}
            if self.layer.is_empty() {
                let idle_since = *self.clock.idle_since.get_or_insert(now);
                if now.saturating_duration_since(idle_since) >= IDLE_EXIT
                    && self.pending.streams.is_empty()
                {
                    return Ok(());
                }
            } else {
                self.clock.idle_since = None;
            }
            let timeout = self
                .sleep_until(now)
                .saturating_duration_since(Instant::now());
            if readable(&self.listener, &self.stage, timeout).map_err(error)? {
                self.accept_all();
            }
        }
    }

    fn sleep_until(&self, now: Instant) -> Instant {
        let mut until = now + LONGEST_SLEEP;
        if let Some(change) = self.layer.next_change(now, FRAME) {
            until = until.min(change);
        }
        if !self.layer.is_empty() {
            until = until.min(self.clock.watched_at + WATCH_EVERY);
        }
        until
    }

    fn watch_sessions(&mut self, now: Instant) -> Vec<Ticket> {
        if now.saturating_duration_since(self.clock.watched_at) < WATCH_EVERY {
            return Vec::new();
        }
        self.clock.watched_at = now;
        let mut answered = Vec::new();
        for session in self.layer.sessions() {
            if self
                .clock
                .watch
                .observe(&session, classify(read_manifest(&session)))
            {
                answered.extend(self.layer.end_session(&session, now));
            }
        }
        self.clock.watch.keep_only(&self.layer.sessions());
        answered
    }

    fn answer(&mut self, tickets: &[Ticket]) {
        for ticket in tickets {
            if let Some(stream) = self.pending.streams.remove(ticket) {
                let _ = wire::write_line(&stream, &wire::success(Value::Null));
            }
        }
    }

    fn accept_all(&mut self) {
        while let Ok((stream, _)) = self.listener.accept() {
            if same_user(&stream) {
                let _ = self.accept(stream);
            }
        }
    }

    fn accept(&mut self, stream: UnixStream) -> io::Result<()> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(CONNECTION_TIMEOUT))?;
        stream.set_write_timeout(Some(CONNECTION_TIMEOUT))?;
        let message = wire::read_line(&stream)?;
        let body = &message["body"];
        let now = Instant::now();
        let root_size = self.root_size();
        let desk = X11Desk {
            stage: &self.stage,
            screens: &self.screens,
            root_size,
        };
        match message["method"].as_str() {
            Some("locate") => {
                let reply = match parse_locate(body) {
                    Ok(request) => {
                        wire::success(serde_json::to_value(self.layer.locate(&request, &desk))?)
                    }
                    Err(message) => wire::refusal(&message),
                };
                wire::write_line(&stream, &reply)
            }
            Some("update") => match parse_request(body) {
                Ok(request) => {
                    let ticket = self.pending.next;
                    self.pending.next = ticket.wrapping_add(1);
                    self.pending.streams.insert(ticket, stream);
                    let answered = self.layer.update(request, ticket, now, &desk);
                    self.pending.ready.extend(answered);
                    Ok(())
                }
                Err(message) => wire::write_line(&stream, &wire::refusal(&message)),
            },
            _ => wire::write_line(&stream, &wire::refusal("method must be locate or update")),
        }
    }
}
