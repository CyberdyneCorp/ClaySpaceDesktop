//! Prints what this build linked and what this machine actually offers.
//!
//! The two answers differ: a backend can be compiled in and still fail to
//! register when its driver is unavailable at runtime. Run with
//! `cargo run -p claycore --example diagnostics`.

fn main() {
    println!("engine version   : {}", claycore::version());
    println!("expected ABI     : {}", claycore::EXPECTED_ABI);

    let compiled = claycore::compiled_backends();
    println!(
        "compiled backends: {}",
        if compiled.is_empty() {
            "(cpu only)".to_string()
        } else {
            compiled
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    match claycore::backends() {
        Ok(found) => {
            println!(
                "registered       : {}",
                found
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for backend in &compiled {
                if !found.contains(backend) {
                    println!(
                        "  note: {backend} was compiled in but did not register on this machine"
                    );
                }
            }
        }
        Err(e) => println!("registered       : discovery failed: {e}"),
    }
}
