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

/// Runs the two-call protocol for an entry point that fills a typed array and
/// reports its length through a single in-out count.
///
/// `call` receives `(buffer, count)`. When `buffer` is null it must write the
/// required length to `count`; otherwise `count` carries the capacity in and
/// the length out, and a capacity below the length is
/// `CLAY_ERROR_BUFFER_TOO_SMALL` with the needed length written back.
///
/// That last sentence is the whole reason this exists rather than being
/// written per call site. A short buffer here is **retryable** — read the
/// count the call just wrote, grow, ask again — while
/// `CLAY_ERROR_INVALID_ARGUMENT` says the call itself was wrong and retrying
/// it is a spin. A drain loop written against neither distinction treats the
/// short buffer as a fault and drops what it was told about; one written
/// against the wrong one spins. Both failure modes are silent, so the
/// distinction lives in one place and every caller gets it.
pub(crate) fn size_query_array<T: Copy + Default>(
    operation: &'static str,
    mut call: impl FnMut(*mut T, *mut usize) -> RawResult,
) -> Result<Vec<T>> {
    let mut needed: usize = 0;
    check(call(std::ptr::null_mut(), &mut needed), operation)?;

    for _ in 0..MAX_ATTEMPTS {
        if needed == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![T::default(); needed];
        let mut count = needed;
        match check(call(buf.as_mut_ptr(), &mut count), operation) {
            Ok(()) => {
                buf.truncate(count.min(needed));
                return Ok(buf);
            }
            // The one recoverable outcome: the answer grew between the two
            // calls, and the call wrote what it actually needs. Anything else
            // — an invalid argument above all — is the caller's own bug and
            // is returned rather than retried.
            Err(e) if e.kind() == crate::error::ErrorKind::BufferTooSmall => {
                if count <= needed {
                    return Err(e);
                }
                needed = count;
            }
            Err(e) => return Err(e),
        }
    }

    Err(unstable_size(operation))
}

fn unstable_size(operation: &'static str) -> ClayError {
    // Constructed through the same path as any other failure so that callers
    // see one error type with one shape.
    match check(sys::clay_result::CLAY_ERROR_BUFFER_TOO_SMALL, operation) {
        Err(e) => e,
        Ok(()) => unreachable!("BUFFER_TOO_SMALL is not a success code"),
    }
}
