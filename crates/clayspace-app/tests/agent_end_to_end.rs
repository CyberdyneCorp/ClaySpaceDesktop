//! The door, against the real application.
//!
//! Every other test of the agent surface runs against a fake session, which is
//! what makes the tool surface cheap to test and is not the same as knowing
//! the door works. This one starts the actual binary, reads the address it
//! published, and drives it over loopback: the real ViewModels, the real C++
//! engine, a real window and a real renderer behind every answer.
//!
//! It runs the application as a **subprocess** rather than building `App`
//! here, because `App` is the composition root and lives in `main.rs`. That is
//! the right shape anyway: what a client does is start a process and connect
//! to it.
//!
//! `HOME` and `XDG_STATE_HOME` are pointed at a scratch directory for the
//! child, so the test drives its own session store and never touches the
//! recent list, the autosave or the door of whatever the person running it has
//! open.
//!
//! **Asked for rather than run by default.** It starts a real application with
//! a real window and a GPU device of its own, and doing that beside the visual
//! suite is what makes both flaky: a dozen offscreen renders and a windowed
//! device contending for one adapter fail each other in ways neither would
//! alone. So it skips unless `CLAYSPACE_AGENT_E2E=1` is set, which is what
//! `just test-agent-e2e` does. The same reasoning `window_smoke` applies to a
//! display, applied to the GPU.
//!
//! ```sh
//! just test-agent-e2e
//! # or: CLAYSPACE_AGENT_E2E=1 cargo test -p clayspace-app --test agent_end_to_end
//! ```

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// How long the application may take to open a window and publish its address.
const STARTUP: Duration = Duration::from_secs(60);

/// How long one exchange may take.
///
/// Comfortably past the application's own consent bound, because one of the
/// exchanges below deliberately hits a gate nobody is at the window to answer
/// and must come back refused rather than as a read timeout here.
const EXCHANGE: Duration = Duration::from_secs(90);

// -- the application under test ---------------------------------------------

/// A running application, with its scratch session directory.
struct Running {
    child: Child,
    root: PathBuf,
    port: u16,
    secret: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn asked_for() -> bool {
    if std::env::var_os("CLAYSPACE_AGENT_E2E").is_none() {
        eprintln!(
            "skipping: this starts a real windowed application and contends with the \
             visual suite for the GPU. Run `just test-agent-e2e`."
        );
        return false;
    }
    if cfg!(target_os = "macos") {
        return true;
    }
    let displayed =
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if !displayed {
        eprintln!("skipping: no display.");
    }
    displayed
}

fn start() -> Option<Running> {
    if !asked_for() {
        return None;
    }

    let root = std::env::temp_dir().join(format!("clayspace-agent-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a scratch home");

    let child = Command::new(env!("CARGO_BIN_EXE_clayspace-app"))
        // The child's whole idea of "the session directory" is these two.
        .env("HOME", &root)
        .env("XDG_STATE_HOME", root.join("state"))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("the application starts");

    let mut running = Running {
        child,
        root: root.clone(),
        port: 0,
        secret: String::new(),
    };

    // Where the child publishes its address depends on the platform, exactly
    // as `SessionStore::discover` decides it.
    let candidates = [
        root.join("Library/Application Support/ClaySpaceDesktop/agente.acesso"),
        root.join("state/clayspace/agente.acesso"),
    ];

    let deadline = Instant::now() + STARTUP;
    while Instant::now() < deadline {
        if let Some((port, secret)) = candidates.iter().find_map(|path| read_access(path)) {
            running.port = port;
            running.secret = secret;
            return Some(running);
        }
        if let Ok(Some(status)) = running.child.try_wait() {
            panic!("the application exited before publishing its address: {status}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("the application published no address within {STARTUP:?}");
}

fn read_access(path: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut port = None;
    let mut secret = None;
    for line in text.lines() {
        match line.split_once(' ') {
            Some(("porta", value)) => port = value.trim().parse().ok(),
            Some(("chave", value)) => secret = Some(value.trim().to_string()),
            _ => {}
        }
    }
    Some((port?, secret?))
}

// -- a client ----------------------------------------------------------------

/// One request and its answer, on a connection of its own.
fn exchange(
    running: &Running,
    body: &str,
    session: Option<&str>,
) -> (u16, Vec<(String, String)>, Value) {
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\n\
         Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\n\
         {}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        running.port,
        running.secret,
        match session {
            Some(id) => format!("Mcp-Session-Id: {id}\r\n"),
            None => String::new(),
        },
        body.len(),
    );

    let mut stream = connect(running);
    stream.write_all(request.as_bytes()).expect("write");
    stream.flush().expect("flush");

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("a status line");
    let status: u16 = line.split(' ').nth(1).unwrap_or("0").parse().unwrap_or(0);

    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("a header");
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
    reader.read_exact(&mut body).expect("a body");
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).expect("JSON")
    };
    (status, headers, value)
}

fn connect(running: &Running) -> TcpStream {
    let address = format!("127.0.0.1:{}", running.port);
    let stream = TcpStream::connect(&address).expect("connects");
    stream.set_read_timeout(Some(EXCHANGE)).unwrap();
    stream.set_write_timeout(Some(EXCHANGE)).unwrap();
    stream
}

/// Initializes, and hands back the session id the server assigned.
fn initialize(running: &Running) -> String {
    let (status, headers, answer) = exchange(
        running,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"end-to-end","version":"0"}}}"#,
        None,
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["result"]["protocolVersion"], "2025-06-18",
        "{answer}"
    );
    headers
        .iter()
        .find(|(name, _)| name == "mcp-session-id")
        .map(|(_, value)| value.clone())
        .expect("a session id")
}

/// One tool call, returning its structured answer.
fn call(running: &Running, session: &str, tool: &str, arguments: Value) -> Value {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments },
    })
    .to_string();
    let (status, _, answer) = exchange(running, &body, Some(session));
    assert_eq!(status, 200, "{answer}");
    assert!(answer["error"].is_null(), "{answer}");
    assert_eq!(
        answer["result"]["isError"], false,
        "{tool} refused: {}",
        answer["result"]["content"][0]["text"]
    );
    answer["result"].clone()
}

/// A PNG content block back to its pixels, so a test can say what it shows.
fn decode(base64: &str) -> Vec<u8> {
    let bytes = from_base64(base64);
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("a PNG header");
    let mut pixels = vec![0u8; reader.output_buffer_size().expect("a buffer size")];
    let info = reader.next_frame(&mut pixels).expect("PNG pixels");
    pixels.truncate(info.buffer_size());
    pixels
}

fn from_base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let Some(index) = ALPHABET.iter().position(|c| *c == byte) else {
            continue;
        };
        accumulator = (accumulator << 6) | index as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    out
}

// -- the tests ---------------------------------------------------------------

/// One test, not several: starting the application costs a window, an engine
/// and a first document, and every assertion below is about the same running
/// session. Split into named steps so a failure says which one.
#[test]
fn an_agent_drives_the_running_application() {
    let Some(running) = start() else {
        return;
    };
    let session = initialize(&running);

    // -- it answers a session nobody is touching ---------------------------
    //
    // The event loop sleeps on `ControlFlow::Wait`. Without the wake-up this
    // would hang until somebody moved the mouse, which is the failure the
    // event-loop proxy exists to prevent.
    let listed = {
        let body = r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#;
        let (status, _, answer) = exchange(&running, body, Some(&session));
        assert_eq!(status, 200);
        answer["result"]["tools"]
            .as_array()
            .expect("tools")
            .to_vec()
    };
    assert!(
        listed.iter().any(|tool| tool["name"] == "stroke"),
        "the tool list has no stroke group"
    );
    assert!(
        listed.iter().any(|tool| tool["name"] == "viewport"),
        "the tool list has no viewport"
    );

    // -- it reads the real document ----------------------------------------
    let state = call(
        &running,
        &session,
        "state",
        json!({ "sections": ["document", "scene"] }),
    );
    let document = &state["structuredContent"]["document"];
    assert!(document["name"].is_string(), "{state}");
    assert_eq!(
        document["modified"], false,
        "a fresh document is unmodified"
    );
    assert!(
        !state["structuredContent"]["scene"]["layers"]
            .as_array()
            .expect("layers")
            .is_empty(),
        "the starting document has no layers: {state}"
    );

    // -- a capture is a real frame of a real surface ------------------------
    call(&running, &session, "view", json!({ "action": "frame_all" }));
    let before = call(
        &running,
        &session,
        "viewport",
        json!({ "action": "capture", "width": 256, "height": 192, "remember": "antes" }),
    );
    assert_eq!(before["structuredContent"]["image"]["width"], 256);
    let png = before["content"]
        .as_array()
        .expect("content")
        .iter()
        .find(|block| block["type"] == "image")
        .expect("an image block");
    assert_eq!(png["mimeType"], "image/png");
    // Not merely that bytes came back: a frame of one flat colour is what a
    // capture that drew nothing looks like, and two of those are identical to
    // each other whatever happened in between.
    // Not merely that bytes came back: a frame of one flat colour is what a
    // capture that drew nothing looks like, and two of those are identical to
    // each other whatever happened in between. This is the assertion that
    // caught the renderer being asked for a format its pipelines were not
    // built for — every pass failed validation and the texture came back
    // transparent black, with the reason only in a log.
    let pixels = decode(png["data"].as_str().expect("base64"));
    let first = &pixels[..4];
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel != first),
        "the capture is one flat colour ({first:?}), so it drew no surface"
    );

    // -- a stroke reaches the surface, and the frame shows it ---------------
    //
    // The whole gesture, then the frame: a capture asked for with the change
    // is taken after the change has been re-meshed, which is the property that
    // makes "did that land where I meant" answerable in one exchange.
    call(
        &running,
        &session,
        "tool",
        json!({ "action": "select", "tool": "clay" }),
    );
    call(
        &running,
        &session,
        "brush",
        json!({ "action": "set_size", "size": 0.25 }),
    );
    call(
        &running,
        &session,
        "stroke",
        json!({ "action": "begin", "at": [0.0, 0.0, 0.6], "pressure": 1.0 }),
    );
    call(
        &running,
        &session,
        "stroke",
        json!({ "action": "continue", "at": [0.12, 0.0, 0.6], "pressure": 1.0 }),
    );
    let ended = call(
        &running,
        &session,
        "stroke",
        json!({ "action": "end", "capture": "viewport", "width": 256, "height": 192 }),
    );

    // One gesture, one history entry.
    let depth = ended["structuredContent"]["history_depth"]
        .as_u64()
        .expect("a history depth");
    assert_eq!(
        depth, 1,
        "a gesture became {depth} history entries: {ended}"
    );
    assert_eq!(
        ended["structuredContent"]["settled"]["quiet"], true,
        "the frame was taken before the surface settled: {ended}"
    );

    call(&running, &session, "wait", json!({ "bound_ms": 5000 }));
    call(
        &running,
        &session,
        "viewport",
        json!({ "action": "capture", "width": 256, "height": 192, "remember": "depois" }),
    );
    let compared = call(
        &running,
        &session,
        "viewport",
        json!({ "action": "compare", "before": "antes", "after": "depois" }),
    );
    let past = compared["structuredContent"]["past_the_floor"]
        .as_u64()
        .expect("a difference past the floor");
    assert!(
        past > 0,
        "the stroke changed no pixel past this machine's render floor: {compared}"
    );

    // -- and it undoes as one step -----------------------------------------
    let undone = call(&running, &session, "history", json!({ "action": "undo" }));
    assert_eq!(
        undone["structuredContent"]["history_depth"], 0,
        "one undo did not return the document: {undone}"
    );

    // -- what can destroy work is gated ------------------------------------
    //
    // Nobody is at the window to answer, so this must be refused after its
    // bound rather than hanging the connection — and the refusal must say what
    // would lift it.
    let body = json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": { "name": "exchange", "arguments": { "action": "run_export" } },
    })
    .to_string();
    let started = Instant::now();
    let (status, _, answer) = exchange(&running, &body, Some(&session));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["result"]["isError"], true,
        "an export ran with nobody consenting to it: {answer}"
    );
    assert_eq!(answer["result"]["structuredContent"]["gate"], "export");
    assert!(
        answer["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("exportar"),
        "the refusal does not say what would lift it: {answer}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "the gate held the connection for {:?}",
        started.elapsed()
    );

    // -- the door does not let an agent open its own gate -------------------
    let body = json!({
        "jsonrpc": "2.0", "id": 10, "method": "tools/call",
        "params": { "name": "session", "arguments": { "action": "toggle_agent_door" } },
    })
    .to_string();
    let (_, _, answer) = exchange(&running, &body, Some(&session));
    assert!(
        answer["result"]["isError"] == true || !answer["error"].is_null(),
        "an agent reached a command meant for the person at the window: {answer}"
    );
}

/// A client that connects and says nothing, and one that stops reading, cost
/// that client its connection and nothing else.
#[test]
fn a_quiet_client_and_a_slow_one_do_not_stall_the_application() {
    let Some(running) = start() else {
        return;
    };
    let session = initialize(&running);

    // Connected, and silent. This must not hold a thread the next client needs.
    let quiet = connect(&running);

    // Half a request, then nothing: the server is left mid-parse.
    let mut half = connect(&running);
    half.write_all(b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1")
        .expect("write");
    half.flush().expect("flush");

    // A request whose answer is never read.
    let body = r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#;
    let mut deaf = connect(&running);
    deaf.write_all(
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\n\
             Mcp-Session-Id: {session}\r\nContent-Length: {}\r\n\r\n{body}",
            running.port,
            running.secret,
            body.len()
        )
        .as_bytes(),
    )
    .expect("write");
    deaf.flush().expect("flush");

    // With all three hanging about, an ordinary client is still served, and
    // promptly.
    let started = Instant::now();
    let state = call(
        &running,
        &session,
        "state",
        json!({ "sections": ["document"] }),
    );
    assert!(state["structuredContent"]["document"]["name"].is_string());
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "three idle connections cost a served client {:?}",
        started.elapsed()
    );

    drop(quiet);
    drop(half);
    drop(deaf);

    // And after they are gone, still served.
    call(
        &running,
        &session,
        "state",
        json!({ "sections": ["document"] }),
    );
}
