//! The listener, and a thread per connection.
//!
//! Thread-per-connection rather than a runtime: an agent client or two is the
//! whole load, every request ends up waiting on the interface thread anyway,
//! and the alternative was `tokio`, `hyper` and their subtrees in a workspace
//! that has no async at all.
//!
//! Nothing here touches a ViewModel. A connection thread parses, checks who is
//! asking, and hands the work to [`crate::queue::JobQueue`]; the interface
//! thread does the rest.

use std::collections::HashSet;
use std::io::{BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::access::{self, Access};
use crate::http::{self, Method, Request, Response};
use crate::jsonrpc::{self, Malformed};
use crate::protocol::{Protocol, ToolSurface, PROTOCOL_VERSION};

/// The path the whole protocol is served at.
pub const ENDPOINT: &str = "/mcp";

/// How long a connection may sit idle before it is closed.
const IDLE: Duration = Duration::from_secs(300);

/// How long a write may block before the connection is given up on.
///
/// A client that stops reading its response must cost that client its
/// connection and nothing else — in particular not a thread that never
/// returns.
const WRITE_BOUND: Duration = Duration::from_secs(30);

/// How many connections at once. A door on loopback still faces whatever else
/// runs as this user.
const MAX_CONNECTIONS: usize = 32;

/// Between comments on an idle event stream.
const STREAM_HEARTBEAT: Duration = Duration::from_secs(15);

/// A listening server.
pub struct Server {
    listener: TcpListener,
    access: Access,
    surface: Arc<dyn ToolSurface>,
    state: Arc<State>,
}

#[derive(Default)]
struct State {
    running: AtomicBool,
    connections: AtomicUsize,
    sessions: Mutex<HashSet<String>>,
}

/// What the application holds after the server is running.
#[derive(Clone)]
pub struct ServerHandle {
    access: Access,
    state: Arc<State>,
}

impl ServerHandle {
    pub fn access(&self) -> &Access {
        &self.access
    }

    pub fn url(&self) -> String {
        self.access.url()
    }

    pub fn port(&self) -> u16 {
        self.access.port
    }

    /// How many clients are connected right now, for the status area.
    pub fn connections(&self) -> usize {
        self.state.connections.load(Ordering::Relaxed)
    }

    pub fn is_listening(&self) -> bool {
        self.state.running.load(Ordering::SeqCst)
    }

    /// Stops the server and unblocks the accept loop.
    ///
    /// A blocking `accept` does not notice a flag, so this connects to the
    /// listener once to wake it. The connection is closed immediately and is
    /// refused like any other unauthenticated one if the timing goes the other
    /// way.
    pub fn stop(&self) {
        if !self.state.running.swap(false, Ordering::SeqCst) {
            return;
        }
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.access.port);
        let _ = TcpStream::connect_timeout(&address, Duration::from_millis(250));
    }
}

/// Why a server could not start.
#[derive(Debug)]
pub enum BindError {
    /// The address asked for is not on loopback.
    NotLoopback(SocketAddr),
    /// Every port tried was taken.
    NoPort {
        from: u16,
        tried: u16,
    },
    /// The operating system's entropy pool could not be read, so no secret
    /// could be made. A door with a guessable secret is worse than a door that
    /// did not open.
    NoSecret(std::io::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoopback(address) => write!(
                f,
                "{address} is not on loopback; this server is reachable from this \
                 machine and no other, and will not be told otherwise"
            ),
            Self::NoPort { from, tried } => write!(
                f,
                "no port was free between {from} and {}",
                from.saturating_add(*tried)
            ),
            Self::NoSecret(e) => write!(f, "no secret could be generated: {e}"),
            Self::Io(e) => write!(f, "the door could not be opened: {e}"),
        }
    }
}

impl std::error::Error for BindError {}

impl Server {
    /// Takes the preferred port, or the next free one after it.
    pub fn bind(surface: Arc<dyn ToolSurface>) -> Result<Self, BindError> {
        Self::bind_from(surface, access::PREFERRED_PORT, access::PORT_ATTEMPTS)
    }

    pub fn bind_from(
        surface: Arc<dyn ToolSurface>,
        first: u16,
        attempts: u16,
    ) -> Result<Self, BindError> {
        let mut last = None;
        for offset in 0..attempts.max(1) {
            let port = match first.checked_add(offset) {
                Some(port) => port,
                None => break,
            };
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            match TcpListener::bind(address) {
                Ok(listener) => return Self::from_listener(surface, listener),
                Err(e) => last = Some(e),
            }
        }
        match last {
            Some(e) if e.kind() != std::io::ErrorKind::AddrInUse => Err(BindError::Io(e)),
            _ => Err(BindError::NoPort {
                from: first,
                tried: attempts,
            }),
        }
    }

    /// A server on an address a caller chose, which must be loopback.
    ///
    /// The refusal is the point: a configuration that asks for a reachable
    /// address is an error rather than an honoured request.
    pub fn bind_at(surface: Arc<dyn ToolSurface>, address: SocketAddr) -> Result<Self, BindError> {
        if !address.ip().is_loopback() {
            return Err(BindError::NotLoopback(address));
        }
        let listener = TcpListener::bind(address).map_err(BindError::Io)?;
        Self::from_listener(surface, listener)
    }

    fn from_listener(
        surface: Arc<dyn ToolSurface>,
        listener: TcpListener,
    ) -> Result<Self, BindError> {
        let port = listener.local_addr().map_err(BindError::Io)?.port();
        let secret = access::generate_secret().map_err(BindError::NoSecret)?;
        Ok(Self {
            listener,
            access: Access {
                port,
                secret,
                pid: std::process::id(),
            },
            surface,
            state: Arc::new(State::default()),
        })
    }

    pub fn access(&self) -> &Access {
        &self.access
    }

    /// Starts accepting, on a thread of its own, and returns the handle the
    /// application holds.
    pub fn serve(self) -> ServerHandle {
        self.state.running.store(true, Ordering::SeqCst);
        let handle = ServerHandle {
            access: self.access.clone(),
            state: Arc::clone(&self.state),
        };

        std::thread::Builder::new()
            .name("clayspace-mcp".into())
            .spawn(move || self.accept_loop())
            .expect("the listener thread starts");

        handle
    }

    fn accept_loop(self) {
        let Self {
            listener,
            access,
            surface,
            state,
        } = self;

        for stream in listener.incoming() {
            if !state.running.load(Ordering::SeqCst) {
                break;
            }
            let stream = match stream {
                Ok(stream) => stream,
                Err(_) => continue,
            };
            if state.connections.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                let mut stream = stream;
                let _ = http::write_response(
                    &mut stream,
                    &Response::text(503, "too many connections"),
                    false,
                );
                continue;
            }

            let access = access.clone();
            let surface = Arc::clone(&surface);
            let held = Arc::clone(&state);
            state.connections.fetch_add(1, Ordering::Relaxed);
            let spawned = std::thread::Builder::new()
                .name("clayspace-mcp-conn".into())
                .spawn(move || {
                    serve_connection(stream, &access, surface.as_ref(), &held);
                    held.connections.fetch_sub(1, Ordering::Relaxed);
                });
            if spawned.is_err() {
                state.connections.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}

fn serve_connection(stream: TcpStream, access: &Access, surface: &dyn ToolSurface, state: &State) {
    let _ = stream.set_read_timeout(Some(IDLE));
    let _ = stream.set_write_timeout(Some(WRITE_BOUND));
    let _ = stream.set_nodelay(true);

    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);

    loop {
        let request = match http::read_request(&mut reader) {
            Ok(Some(request)) => request,
            // The peer closed cleanly between requests, which is the ordinary
            // end of a keep-alive conversation.
            Ok(None) => return,
            Err(bad) => {
                let _ =
                    http::write_response(&mut writer, &Response::text(bad.status, bad.why), false);
                return;
            }
        };

        let keep_alive = request.keep_alive();
        match answer(&request, access, surface, state, &mut writer) {
            // The connection has become an event stream and is no longer a
            // request-and-answer conversation.
            Served::Streamed => return,
            Served::Answered(response) => {
                if http::write_response(&mut writer, &response, keep_alive).is_err() {
                    return;
                }
            }
        }
        if !keep_alive {
            return;
        }
    }
}

enum Served {
    Answered(Response),
    Streamed,
}

fn answer(
    request: &Request,
    access: &Access,
    surface: &dyn ToolSurface,
    state: &State,
    writer: &mut TcpStream,
) -> Served {
    // Origin and Host first: a page in a browser reaching a loopback server
    // through a name that resolves to it must be refused whatever secret it
    // carries, so this cannot sit behind the authentication.
    if !access::origin_is_ours(
        request.headers.get("origin"),
        request.headers.get("host"),
        access.port,
    ) {
        return Served::Answered(Response::text(
            403,
            "this server answers only what it addressed itself",
        ));
    }

    if !access::bearer(request.headers.get("authorization"))
        .map(|offered| access::secret_matches(&access.secret, offered))
        .unwrap_or(false)
    {
        // Nothing about the session behind the door: not the document, not
        // whether one is open, not what the secret looks like.
        return Served::Answered(
            Response::text(401, "unauthorized")
                .with_header("WWW-Authenticate", "Bearer realm=\"clayspace\""),
        );
    }

    if request.path() != ENDPOINT {
        return Served::Answered(Response::text(404, "not found"));
    }

    match request.method {
        Method::Post => Served::Answered(post(request, surface, state)),
        Method::Get => {
            if !request.accepts_event_stream() {
                return Served::Answered(Response::text(
                    406,
                    "a GET to this endpoint opens an event stream; accept text/event-stream",
                ));
            }
            if !session_is_live(request, state) {
                return Served::Answered(Response::text(404, "no such session"));
            }
            stream_events(writer, state);
            Served::Streamed
        }
        Method::Delete => {
            if let Some(id) = request.headers.get("mcp-session-id") {
                state
                    .sessions
                    .lock()
                    .expect("the session set is not poisoned")
                    .remove(id);
            }
            Served::Answered(Response::empty(204))
        }
        _ => Served::Answered(
            Response::text(405, "this endpoint takes POST, GET and DELETE")
                .with_header("Allow", "POST, GET, DELETE"),
        ),
    }
}

fn post(request: &Request, surface: &dyn ToolSurface, state: &State) -> Response {
    let incoming = match jsonrpc::parse(&request.body) {
        Ok(incoming) => incoming,
        Err(Malformed(answer)) => return json_response(200, &answer),
    };

    let is_initialize = incoming.method() == "initialize";

    // Every message after `initialize` carries the session it belongs to. A
    // session the server does not know is one it terminated, which the client
    // learns as a 404 and answers by initializing again.
    if !is_initialize && !session_is_live(request, state) {
        return Response::text(404, "no such session; initialize again");
    }

    let answered = Protocol::new(surface).handle(&incoming);

    match answered {
        None => Response::empty(202),
        Some(value) => {
            let response = json_response(200, &value);
            if is_initialize {
                let id = new_session_id();
                state
                    .sessions
                    .lock()
                    .expect("the session set is not poisoned")
                    .insert(id.clone());
                response
                    .with_header("Mcp-Session-Id", &id)
                    .with_header("MCP-Protocol-Version", PROTOCOL_VERSION)
            } else {
                response
            }
        }
    }
}

fn session_is_live(request: &Request, state: &State) -> bool {
    match request.headers.get("mcp-session-id") {
        Some(id) => state
            .sessions
            .lock()
            .expect("the session set is not poisoned")
            .contains(id),
        // A client that never sends one is a client of an older revision, and
        // refusing it buys nothing: the secret is what authenticates.
        None => true,
    }
}

fn new_session_id() -> String {
    access::generate_secret().unwrap_or_else(|_| format!("{}", std::process::id()))
}

/// Holds an event stream open until the client goes or the server stops.
///
/// There is nothing to send yet — this build advertises no list changes and no
/// server-initiated requests — so what this does is keep the stream the
/// transport promises, with a comment often enough that nothing in between
/// reaps it.
fn stream_events(writer: &mut TcpStream, state: &State) {
    if http::write_event_stream_head(writer, &[]).is_err() {
        return;
    }
    while state.running.load(Ordering::SeqCst) {
        std::thread::sleep(STREAM_HEARTBEAT);
        if http::write_keepalive_comment(writer).is_err() {
            return;
        }
    }
    let _ = writer.flush();
}

fn json_response(status: u16, value: &Value) -> Response {
    Response::json(
        status,
        serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{CallResult, ToolDescriptor};
    use crate::session::Refusal;
    use std::io::{BufRead, Read, Write};

    struct NoTools;

    impl ToolSurface for NoTools {
        fn tools(&self) -> Vec<ToolDescriptor> {
            Vec::new()
        }
        fn call(&self, name: &str, _arguments: &Value) -> Result<CallResult, Refusal> {
            Err(Refusal::new(
                crate::session::RefusalCode::UnknownAction,
                format!("there is no tool named {name}"),
            ))
        }
        fn instructions(&self) -> String {
            "test".into()
        }
    }

    struct Running {
        handle: ServerHandle,
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.handle.stop();
        }
    }

    fn running() -> Running {
        // A high port so a developer's own session on the preferred one is not
        // disturbed by the suite.
        let server = Server::bind_from(Arc::new(NoTools), 39_000, 200).expect("the door opens");
        Running {
            handle: server.serve(),
        }
    }

    /// One request, one answer, on a connection of its own.
    fn exchange(handle: &ServerHandle, request: &str) -> (u16, Vec<(String, String)>, String) {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), handle.port());
        let mut stream =
            TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connects");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let status: u16 = line.split(' ').nth(1).unwrap_or("0").parse().unwrap_or(0);

        let mut headers = Vec::new();
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let line = line.trim_end().to_string();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                }
                headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
            }
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).unwrap();
        (status, headers, String::from_utf8_lossy(&body).to_string())
    }

    fn post_body(
        handle: &ServerHandle,
        secret: &str,
        body: &str,
    ) -> (u16, Vec<(String, String)>, String) {
        exchange(
            handle,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                handle.port(),
                body.len(),
            ),
        )
    }

    #[test]
    fn the_door_is_on_loopback_and_publishes_the_port_it_took() {
        let running = running();
        assert!(running.handle.is_listening());
        assert!(running.handle.port() >= 39_000);
        assert!(running.handle.url().starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn an_address_that_is_not_loopback_is_refused_rather_than_honoured() {
        let address: SocketAddr = "0.0.0.0:39999".parse().unwrap();
        match Server::bind_at(Arc::new(NoTools), address) {
            Err(BindError::NotLoopback(refused)) => assert_eq!(refused, address),
            Err(other) => panic!("refused for the wrong reason: {other}"),
            Ok(_) => panic!("a reachable address was honoured"),
        }
    }

    #[test]
    fn a_taken_port_costs_the_next_one_and_not_the_door() {
        let first = Server::bind_from(Arc::new(NoTools), 39_500, 20).unwrap();
        let second = Server::bind_from(Arc::new(NoTools), 39_500, 20).unwrap();
        assert_ne!(first.access().port, second.access().port);
        // Two sessions at once, each with a secret of its own.
        assert_ne!(first.access().secret, second.access().secret);
    }

    #[test]
    fn a_client_with_the_secret_is_served() {
        let running = running();
        let (status, headers, body) = post_body(
            &running.handle,
            &running.handle.access().secret,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        );
        assert_eq!(status, 200);
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(
            headers.iter().any(|(name, _)| name == "mcp-session-id"),
            "{headers:?}"
        );
    }

    #[test]
    fn a_client_without_the_secret_is_refused_and_says_nothing_about_the_session() {
        let running = running();
        let (status, _, body) = post_body(
            &running.handle,
            "not the secret",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        );
        assert_eq!(status, 401);
        assert!(!body.contains("clayspace"), "{body}");
        assert!(!body.contains(&running.handle.access().secret), "{body}");

        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
                running.handle.port()
            ),
        );
        assert_eq!(status, 401);
    }

    #[test]
    fn a_web_origin_is_refused_whatever_secret_it_carries() {
        let running = running();
        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: https://example.test\r\nAuthorization: Bearer {}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 403);
    }

    #[test]
    fn a_host_that_is_not_ours_is_a_rebinding_attempt() {
        let running = running();
        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: sculpt.example.test:{}\r\nAuthorization: Bearer {}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 403);
    }

    #[test]
    fn another_path_is_not_the_endpoint() {
        let running = running();
        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "GET /admin HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 404);
    }

    #[test]
    fn a_notification_is_accepted_without_an_answer() {
        let running = running();
        let (status, _, body) = post_body(
            &running.handle,
            &running.handle.access().secret,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert_eq!(status, 202);
        assert!(body.is_empty(), "{body}");
    }

    #[test]
    fn a_session_is_ended_by_delete() {
        let running = running();
        let (_, headers, _) = post_body(
            &running.handle,
            &running.handle.access().secret,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        );
        let id = headers
            .iter()
            .find(|(name, _)| name == "mcp-session-id")
            .map(|(_, value)| value.clone())
            .expect("a session id");

        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "DELETE /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nMcp-Session-Id: {id}\r\nConnection: close\r\n\r\n",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 204);

        // The session is gone, and a message carrying it is told so.
        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nMcp-Session-Id: {id}\r\nContent-Length: 46\r\nConnection: close\r\n\r\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}}",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 404);
    }

    #[test]
    fn a_get_without_an_accept_for_events_is_not_a_stream() {
        let running = running();
        let (status, _, _) = exchange(
            &running.handle,
            &format!(
                "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 406);
    }

    #[test]
    fn an_unsupported_method_names_what_the_endpoint_takes() {
        let running = running();
        let (status, headers, _) = exchange(
            &running.handle,
            &format!(
                "PUT /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                running.handle.port(),
                running.handle.access().secret,
            ),
        );
        assert_eq!(status, 405);
        assert!(
            headers.iter().any(|(name, _)| name == "allow"),
            "{headers:?}"
        );
    }

    #[test]
    fn a_stopped_server_stops_listening() {
        let running = running();
        let port = running.handle.port();
        running.handle.stop();
        assert!(!running.handle.is_listening());

        // The listener is dropped with the accept loop, so a later connection
        // has nothing to reach. Given up on after a bounded wait either way.
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let mut refused = false;
        for _ in 0..50 {
            match TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
                Ok(_) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => {
                    refused = true;
                    break;
                }
            }
        }
        assert!(refused, "the port is still accepting after stop");
    }
}
