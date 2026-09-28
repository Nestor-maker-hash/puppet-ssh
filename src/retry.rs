use std::thread;
use std::time::Duration;

use crate::error::PuppetError;

pub fn retry<F, T>(
    operation_name: &str,
    max_attempts: u32,
    mut operation: F,
) -> Result<T, PuppetError>
where
    F: FnMut() -> Result<T, PuppetError>,
{
    for attempt in 1..=max_attempts {
        println!(
            "[{}] {} (attempt {}/{})",
            if attempt == 1 { "RUN" } else { "RETRY" },
            operation_name,
            attempt,
            max_attempts
        );

        match operation() {
            Ok(value) => {
                println!("[OK] {}", operation_name);
                return Ok(value);
            }

            Err(error) => {
                println!("[ERROR] {}", error);

                if !error.retryable() {
                    println!(
                        "[STOP] Error is not retryable."
                    );

                    return Err(error);
                }

                if attempt == max_attempts {
                    println!(
                        "[FAILED] {} after {} attempts",
                        operation_name,
                        max_attempts
                    );

                    return Err(error);
                }

                let delay = match attempt {
                    1 => 2,
                    2 => 5,
                    _ => 10,
                };

                println!(
                    "[WAIT] Retrying in {} seconds...",
                    delay
                );

                thread::sleep(Duration::from_secs(delay));
            }
        }
    }

    Err(PuppetError::Unknown(
        "retry loop ended unexpectedly".to_string(),
    ))
}
