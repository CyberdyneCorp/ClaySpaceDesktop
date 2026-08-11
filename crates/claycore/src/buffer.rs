//! The engine's size-query protocol, wrapped once.
//!
//! Many entry points report the size they need when called with a null buffer,
//! then fill it when called again. Spelling that out at each of the dozens of
//! call sites invites one of them to get the retry wrong, so it lives here.

use std::ffi::c_char;

use claycore_sys as sys;

use crate::error::{check, ClayError, RawResult, Result};

/// How many times to re-ask when the required size changes between the query
/// and the fill. A document being edited on another thread can legitimately
/// grow the answer once; growing it repeatedly means something is wrong.
const MAX_ATTEMPTS: usize = 4;

/// Runs the two-call protocol for an entry point that fills a byte buffer.
///
/// `call` receives `(buffer, size)`. When `buffer` is null it must write the
/// required size (including any NUL) to `size`; otherwise it must fill up to
/// `size` bytes.
pub(crate) fn size_query_bytes(
    operation: &'static str,
    mut call: impl FnMut(*mut c_char, *mut usize) -> RawResult,
) -> Result<Vec<u8>> {
    for _ in 0..MAX_ATTEMPTS {
        let mut needed: usize = 0;
        check(call(std::ptr::null_mut(), &mut needed), operation)?;

        if needed == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; needed];
        let mut capacity = needed;
        let code = call(buf.as_mut_ptr() as *mut c_char, &mut capacity);

        // The one recoverable outcome: the answer grew between the two calls.
        if let Err(e) = check(code, operation) {
            if e.kind() == crate::error::ErrorKind::BufferTooSmall {
                continue;
            }
            return Err(e);
        }

        buf.truncate(capacity.min(needed));
        return Ok(buf);
    }

    Err(unstable_size(operation))
}

/// Same protocol, for entry points whose payload is text.
pub(crate) fn size_query_string(
    operation: &'static str,
    call: impl FnMut(*mut c_char, *mut usize) -> RawResult,
) -> Result<String> {
    let mut bytes = size_query_bytes(operation, call)?;
    // The engine writes a NUL terminator inside the reported size; Rust
    // strings do not carry one.
    if let Some(pos) = bytes.iter().position(|&b| b == 0) {
        bytes.truncate(pos);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn unstable_size(operation: &'static str) -> ClayError {
    // Constructed through the same path as any other failure so that callers
    // see one error type with one shape.
    match check(sys::clay_result::CLAY_ERROR_BUFFER_TOO_SMALL, operation) {
        Err(e) => e,
        Ok(()) => unreachable!("BUFFER_TOO_SMALL is not a success code"),
    }
}
