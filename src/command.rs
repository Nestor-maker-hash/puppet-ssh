use std::process::Command;

use crate::error::PuppetError;

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub fn run(program: &str, args: &[&str]) -> Result<CommandOutput, PuppetError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| {
            PuppetError::CommandFailed(format!(
                "could not start '{}': {}",
                program, error
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    let stderr = String::from_utf8_lossy(&output.stderr)
        .trim()
        .to_string();

    if output.status.success() {
        Ok(CommandOutput { stdout, stderr })
    } else {
        Err(classify_failure(
            program,
            &stdout,
            &stderr,
        ))
    }
}

fn classify_failure(
    program: &str,
    stdout: &str,
    stderr: &str,
) -> PuppetError {
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "command returned a failure status"
    };

    let lower = message.to_lowercase();

    if lower.contains("access is denied")
        || lower.contains("access denied")
        || lower.contains("permission denied")
    {
        return PuppetError::PermissionDenied(format!(
            "{}: {}",
            program, message
        ));
    }

    if lower.contains("network")
        || lower.contains("connection")
        || lower.contains("internet")
    {
        return PuppetError::NetworkFailure(format!(
            "{}: {}",
            program, message
        ));
    }

    PuppetError::CommandFailed(format!(
        "{}: {}",
        program, message
    ))
}
