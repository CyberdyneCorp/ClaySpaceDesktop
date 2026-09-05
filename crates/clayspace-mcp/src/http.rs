//! The HTTP/1.1 subset MCP's Streamable HTTP transport actually uses.
//!
//! Request line, headers, `Content-Length` and chunked bodies, keep-alive,
//! and nothing else. No compression, no trailers, no upgrades, no
//! multiplexing. A few hundred lines with the conformance cases beside them,
//! rather than a runtime and a framework to carry a fraction of what they
//! cover — and the [`crate::session::Session`] seam means that if that trade
//! ever goes bad, this module is the only thing replaced.
//!
//! Everything here reads from a `BufRead` and writes to a `Write`, so the
//! whole of it is tested against byte slices with no socket in sight.

use std::io::{BufRead, Write};

/// The most we will read of a request's head, and of its body.
///
/// A door on loopback still faces whatever else runs as this user, and a
/// server that will allocate whatever it is told to is a server that can be
/// made to exhaust a sculptor's memory from a shell prompt.
pub const MAX_HEAD_BYTES: usize = 64 * 1024;
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_HEADERS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
    Options,
    Other,
}

impl Method {
    fn parse(word: &str) -> Self {
        match word {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "DELETE" => Self::Delete,
            "OPTIONS" => Self::Options,
            _ => Self::Other,
        }
    }
}

/// Header fields, compared without regard to case as HTTP requires.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    fn push(&mut self, name: String, value: String) {
        self.0.push((name, value));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    /// The request target, path only — a query string is kept but unused.
    pub target: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    /// The path with any query string cut off.
    pub fn path(&self) -> &str {
        match self.target.find('?') {
            Some(at) => &self.target[..at],
            None => &self.target,
        }
    }

    /// Whether the connection stays open after this exchange.
    ///
    /// HTTP/1.1 keeps it open unless told otherwise; HTTP/1.0 is the other way
    /// round, and this server answers 1.1 only, so the one case to honour is
    /// an explicit close.
    pub fn keep_alive(&self) -> bool {
        !self
            .headers
            .get("connection")
            .map(|value| value.eq_ignore_ascii_case("close"))
            .unwrap_or(false)
    }

    /// Whether the client will accept an event stream in reply.
    pub fn accepts_event_stream(&self) -> bool {
        self.headers
            .get("accept")
            .map(|value| value.contains("text/event-stream") || value.contains("*/*"))
            .unwrap_or(false)
    }
}

/// A request that could not be read, and the status to answer it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadRequest {
    pub status: u16,
    pub why: String,
}

impl BadRequest {
    fn new(status: u16, why: impl Into<String>) -> Self {
        Self {
            status,
            why: why.into(),
        }
    }
}

/// Reads one request.
///
/// `Ok(None)` is a connection the peer closed cleanly between requests, which
/// is the ordinary end of a keep-alive conversation and not a fault.
pub fn read_request(reader: &mut impl BufRead) -> Result<Option<Request>, BadRequest> {
    let mut head = Vec::new();
    let start = match read_line(reader, &mut head)? {
        Some(line) => line,
        None => return Ok(None),
    };

    let mut words = start.split(' ');
    let method = words
        .next()
        .ok_or_else(|| BadRequest::new(400, "the request line has no method"))?;
    let target = words
        .next()
        .ok_or_else(|| BadRequest::new(400, "the request line has no target"))?;
    let version = words.next().unwrap_or("");
    if !version.starts_with("HTTP/1.") {
        return Err(BadRequest::new(
            505,
            format!("this server speaks HTTP/1.1 and not {version}"),
        ));
    }

    let method = Method::parse(method);
    let target = target.to_string();

    let mut headers = Headers::default();
    loop {
        let line = read_line(reader, &mut head)?
            .ok_or_else(|| BadRequest::new(400, "the headers end without a blank line"))?;
        if line.is_empty() {
            break;
        }
        if headers.0.len() >= MAX_HEADERS {
            return Err(BadRequest::new(431, "too many header fields"));
        }
        let at = line
            .find(':')
            .ok_or_else(|| BadRequest::new(400, "a header field has no colon"))?;
        let name = line[..at].trim().to_string();
        let value = line[at + 1..].trim().to_string();
        if name.is_empty() {
            return Err(BadRequest::new(400, "a header field has no name"));
        }
        headers.push(name, value);
    }

    let body = read_body(reader, &headers)?;

    Ok(Some(Request {
        method,
        target,
        headers,
        body,
    }))
}

/// One CRLF- or LF-terminated line, counted against the head's budget.
fn read_line(
    reader: &mut impl BufRead,
    budget: &mut Vec<u8>,
) -> Result<Option<String>, BadRequest> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) => {
                if line.is_empty() && budget.is_empty() {
                    return Ok(None);
                }
                return Err(BadRequest::new(400, "the connection ended mid-request"));
            }
            Ok(_) => {}
            Err(e) => return Err(BadRequest::new(400, format!("the connection failed: {e}"))),
        }
        budget.push(byte[0]);
        if budget.len() > MAX_HEAD_BYTES {
            return Err(BadRequest::new(431, "the request head is too large"));
        }
        if byte[0] == b'\n' {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return String::from_utf8(line)
                .map(Some)
                .map_err(|_| BadRequest::new(400, "the request head is not UTF-8"));
        }
        line.push(byte[0]);
    }
}

fn read_body(reader: &mut impl BufRead, headers: &Headers) -> Result<Vec<u8>, BadRequest> {
    let chunked = headers
        .get("transfer-encoding")
        .map(|value| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);

    if chunked {
        return read_chunked(reader);
    }

    let length = match headers.get("content-length") {
        Some(value) => value
            .trim()
            .parse::<usize>()
            .map_err(|_| BadRequest::new(400, "Content-Length is not a number"))?,
        None => return Ok(Vec::new()),
    };
    if length > MAX_BODY_BYTES {
        return Err(BadRequest::new(413, "the request body is too large"));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|e| BadRequest::new(400, format!("the body ended early: {e}")))?;
    Ok(body)
}

fn read_chunked(reader: &mut impl BufRead) -> Result<Vec<u8>, BadRequest> {
    let mut body = Vec::new();
    let mut head = Vec::new();
    loop {
        head.clear();
        let line = read_line(reader, &mut head)?
            .ok_or_else(|| BadRequest::new(400, "the chunked body ended early"))?;
        // A chunk size may carry extensions after a semicolon; they are not
        // ours to interpret and are dropped.
        let size_text = line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| BadRequest::new(400, "a chunk size is not hexadecimal"))?;
        if body.len() + size > MAX_BODY_BYTES {
            return Err(BadRequest::new(413, "the request body is too large"));
        }
        if size == 0 {
            // The trailer section, which this server does not read, ends with
            // a blank line.
            loop {
                head.clear();
                match read_line(reader, &mut head)? {
                    Some(line) if line.is_empty() => break,
                    Some(_) => continue,
                    None => break,
                }
            }
            return Ok(body);
        }
        let mut chunk = vec![0u8; size];
        reader
            .read_exact(&mut chunk)
            .map_err(|e| BadRequest::new(400, format!("a chunk ended early: {e}")))?;
        body.extend_from_slice(&chunk);
        let mut trailing = [0u8; 2];
        reader
            .read_exact(&mut trailing)
            .map_err(|e| BadRequest::new(400, format!("a chunk has no terminator: {e}")))?;
    }
}

/// One answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: "application/json",
            headers: Vec::new(),
            body,
        }
    }

    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    /// An answer with no content, which is what a notification and a session
    /// termination both get.
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// The reason a status has, for the status line.
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Unknown",
    }
}

pub fn write_response(
    writer: &mut impl Write,
    response: &Response,
    keep_alive: bool,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: {}\r\n",
        response.status,
        reason(response.status),
        response.content_type,
        response.body.len(),
        if keep_alive { "keep-alive" } else { "close" },
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    writer.write_all(head.as_bytes())?;
    writer.write_all(&response.body)?;
    writer.flush()
}

/// The head of a `text/event-stream`, after which events are written one at a
/// time and the connection stays open.
pub fn write_event_stream_head(
    writer: &mut impl Write,
    extra: &[(String, String)],
) -> std::io::Result<()> {
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n",
    );
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    writer.write_all(head.as_bytes())?;
    writer.flush()
}

/// One event on an open stream.
pub fn write_event(writer: &mut impl Write, id: u64, data: &str) -> std::io::Result<()> {
    let mut out = format!("id: {id}\n");
    for line in data.split('\n') {
        out.push_str("data: ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    writer.write_all(out.as_bytes())?;
    writer.flush()
}

/// A comment line, which keeps a quiet stream from being reaped by anything in
/// between and costs two bytes.
pub fn write_keepalive_comment(writer: &mut impl Write) -> std::io::Result<()> {
    writer.write_all(b": \n\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn parse(bytes: &[u8]) -> Result<Option<Request>, BadRequest> {
        read_request(&mut BufReader::new(bytes))
    }

    #[test]
    fn a_post_with_a_body() {
        let request = parse(
            b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:7457\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
        )
        .unwrap()
        .unwrap();
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path(), "/mcp");
        assert_eq!(request.body, b"{}");
        assert_eq!(
            request.headers.get("CONTENT-type"),
            Some("application/json")
        );
        assert!(request.keep_alive());
    }

    #[test]
    fn a_query_string_is_not_part_of_the_path() {
        let request = parse(b"GET /mcp?since=3 HTTP/1.1\r\n\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(request.path(), "/mcp");
        assert_eq!(request.target, "/mcp?since=3");
    }

    #[test]
    fn a_chunked_body_is_reassembled() {
        let request = parse(
            b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(request.body, b"hello world");
    }

    #[test]
    fn a_chunk_extension_is_dropped_rather_than_misread() {
        let request = parse(
            b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3;a=b\r\nabc\r\n0\r\n\r\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(request.body, b"abc");
    }

    #[test]
    fn a_closed_connection_between_requests_is_not_a_fault() {
        assert_eq!(parse(b"").unwrap(), None);
    }

    #[test]
    fn a_connection_that_ends_mid_request_is() {
        assert!(parse(b"POST /mcp HTT").is_err());
    }

    #[test]
    fn an_explicit_close_ends_the_conversation() {
        let request = parse(b"POST /mcp HTTP/1.1\r\nConnection: close\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(!request.keep_alive());
    }

    #[test]
    fn a_body_larger_than_the_budget_is_refused_rather_than_allocated() {
        let head = format!(
            "POST /mcp HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        let refusal = parse(head.as_bytes()).unwrap_err();
        assert_eq!(refusal.status, 413);
    }

    #[test]
    fn a_head_larger_than_the_budget_is_refused() {
        let mut request = String::from("POST /mcp HTTP/1.1\r\n");
        for i in 0..2000 {
            request.push_str(&format!("X-Padding-{i}: {}\r\n", "x".repeat(64)));
        }
        request.push_str("\r\n");
        let refusal = parse(request.as_bytes()).unwrap_err();
        assert!(refusal.status == 431, "{refusal:?}");
    }

    #[test]
    fn a_version_this_server_does_not_speak_is_named() {
        let refusal = parse(b"POST /mcp HTTP/2.0\r\n\r\n").unwrap_err();
        assert_eq!(refusal.status, 505);
    }

    #[test]
    fn a_response_states_its_length_and_whether_the_connection_stays() {
        let mut out = Vec::new();
        write_response(&mut out, &Response::json(200, b"{\"a\":1}".to_vec()), true).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
        assert!(text.contains("Content-Length: 7\r\n"), "{text}");
        assert!(text.contains("Connection: keep-alive\r\n"), "{text}");
        assert!(text.ends_with("\r\n\r\n{\"a\":1}"), "{text}");
    }

    #[test]
    fn an_event_carries_every_line_of_its_data() {
        let mut out = Vec::new();
        write_event(&mut out, 4, "{\"a\":1}\n{\"b\":2}").unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "id: 4\ndata: {\"a\":1}\ndata: {\"b\":2}\n\n"
        );
    }

    #[test]
    fn an_accept_header_decides_whether_a_stream_is_wanted() {
        let request = parse(b"GET /mcp HTTP/1.1\r\nAccept: text/event-stream\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(request.accepts_event_stream());
        let request = parse(b"GET /mcp HTTP/1.1\r\nAccept: application/json\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(!request.accepts_event_stream());
    }
}
