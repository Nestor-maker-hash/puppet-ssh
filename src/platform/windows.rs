use std::env;

use crate::command;
use crate::error::PuppetError;
use crate::retry::retry;

pub fn run() {
    println!("Windows installer selected.");
    println!();

    if !is_admin() {
        println!("Administrator privileges: NO");
        println!();
        println!("Puppet-SSH must be run as Administrator.");
        println!("Please restart it with administrator privileges.");
        return;
    }

    println!("Administrator privileges: YES");
    println!();

    match check_network() {
        Ok(()) => {
            println!("Network connectivity: OK");
        }

        Err(error) => {
            println!("Network connectivity check failed.");
            println!("Reason: {}", error);
            return;
        }
    }

    println!();

    match validate_public_key() {
        Ok(()) => {
            println!("SSH public key: VALID");
        }

        Err(error) => {
            println!("SSH public key validation failed.");
            println!("Reason: {}", error);
            return;
        }
    }

    println!();

    if !openssh_server_installed() {
        println!("OpenSSH Server: NOT INSTALLED");
        println!();

        match retry("Install OpenSSH Server", 3, install_openssh_server) {
            Ok(()) => {
                println!("OpenSSH Server installation successful.");
            }

            Err(error) => {
                println!("OpenSSH Server installation failed.");
                println!("Reason: {}", error);
                return;
            }
        }
    } else {
        println!("OpenSSH Server: INSTALLED");
    }

    println!();

    match configure_sshd_service() {
        Ok(()) => {
            println!("SSHD service: READY");
        }

        Err(error) => {
            println!("SSHD service configuration failed.");
            println!("Reason: {}", error);
            return;
        }
    }

    println!();

    match configure_firewall() {
        Ok(()) => {
            println!("Windows Firewall: READY");
        }

        Err(error) => {
            println!("Windows Firewall configuration failed.");
            println!("Reason: {}", error);
            return;
        }
    }

    println!();

    match configure_puppet_user() {
        Ok(()) => {
            println!("Puppet-SSH account: READY");

            if let Err(error) = show_connection_info() {
                println!("Connection information failed.");
                println!("Reason: {}", error);
            }
        }

        Err(error) => {
            println!("Puppet-SSH account configuration failed.");
            println!("Reason: {}", error);
        }
    }
}

fn show_connection_info() -> Result<(), PuppetError> {
    println!();
    println!("================================");
    println!("       PUPPET-SSH READY");
    println!("================================");
    println!();

    let hostname = command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "$env:COMPUTERNAME",
        ],
    )?;

    let ip = command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' -and $_.PrefixOrigin -ne 'WellKnown' } | Select-Object -First 1 -ExpandProperty IPAddress)",
        ],
    )?;

    let hostname = hostname.stdout.trim();
    let ip = ip.stdout.trim();

    println!("Computer name : {}", hostname);
    println!("Local IPv4    : {}", ip);
    println!("SSH port      : 22");
    println!();

    println!("Connect from your phone:");
    println!("ssh {}@{}", PUPPET_USER, hostname);
    println!();

    if !ip.is_empty() {
        println!("IP fallback:");
        println!("ssh {}@{}", PUPPET_USER, ip);
        println!();
    }

    Ok(())
}


fn check_network() -> Result<(), PuppetError> {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "if (Test-NetConnection -ComputerName 'www.microsoft.com' -Port 443 -InformationLevel Quiet) { exit 0 } else { exit 1 }",
        ],
    )?;

    Ok(())
}

fn is_admin() -> bool {
    command::run("net", &["session"]).is_ok()
}

fn openssh_server_installed() -> bool {
    let result = command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0).State",
        ],
    );

    match result {
        Ok(output) => output
            .stdout
            .trim()
            .eq_ignore_ascii_case("Installed"),

        Err(_) => false,
    }
}

fn install_openssh_server() -> Result<(), PuppetError> {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0",
        ],
    )?;

    if !openssh_server_installed() {
        return Err(PuppetError::TemporaryFailure(
            "Windows did not report OpenSSH Server as installed".to_string(),
        ));
    }

    Ok(())
}

fn configure_sshd_service() -> Result<(), PuppetError> {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Set-Service -Name sshd -StartupType Automatic",
        ],
    )?;

    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Start-Service sshd",
        ],
    )?;

    verify_sshd_service()
}

fn configure_key_authentication() -> Result<(), PuppetError> {
    let config_path =
        std::path::PathBuf::from(r"C:\ProgramData\ssh\sshd_config");

    let backup_path =
        std::path::PathBuf::from(r"C:\ProgramData\ssh\sshd_config.puppet-ssh.bak");

    println!("Configuring SSH public-key authentication...");
    println!("SSH config: {}", config_path.display());

    if !config_path.exists() {
        return Err(PuppetError::ConfigurationError(
            "sshd_config was not found".to_string(),
        ));
    }

    if !backup_path.exists() {
        std::fs::copy(&config_path, &backup_path)
            .map_err(|error| {
                PuppetError::ConfigurationError(format!(
                    "could not back up sshd_config: {}",
                    error
                ))
            })?;

        println!("SSH config backup: CREATED");
    } else {
        println!("SSH config backup: EXISTS");
    }

    let contents = std::fs::read_to_string(&config_path)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not read sshd_config: {}",
                error
            ))
        })?;

    let mut lines = Vec::new();
    let mut pubkey_found = false;

    for line in contents.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("PubkeyAuthentication")
            && !trimmed.starts_with("#")
        {
            if !pubkey_found {
                lines.push("PubkeyAuthentication yes".to_string());
                pubkey_found = true;
            }
            continue;
        }

        lines.push(line.to_string());
    }

    if !pubkey_found {
        lines.push("PubkeyAuthentication yes".to_string());
    }

    let updated = format!("{}\n", lines.join("\n"));

    std::fs::write(&config_path, updated)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not write sshd_config: {}",
                error
            ))
        })?;

    println!("Public-key authentication: ENABLED");

    Ok(())
}


fn configure_ssh_user_restriction() -> Result<(), PuppetError> {
    let config_path =
        std::path::PathBuf::from(r"C:\ProgramData\ssh\sshd_config");

    let contents = std::fs::read_to_string(&config_path)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not read sshd_config: {}",
                error
            ))
        })?;

    let mut lines = Vec::new();
    let mut allow_users_found = false;

    for line in contents.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("AllowUsers")
            && !trimmed.starts_with("#")
        {
            if !allow_users_found {
                lines.push(format!("AllowUsers {}", PUPPET_USER));
                allow_users_found = true;
            }
            continue;
        }

        lines.push(line.to_string());
    }

    if !allow_users_found {
        lines.push(format!("AllowUsers {}", PUPPET_USER));
    }

    let updated = format!("{}\n", lines.join("\n"));

    std::fs::write(&config_path, updated)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not write sshd_config: {}",
                error
            ))
        })?;

    println!("SSH user restriction: {} only", PUPPET_USER);

    Ok(())
}

fn verify_ssh_authentication_config() -> Result<(), PuppetError> {
    let config_path =
        std::path::PathBuf::from(r"C:\ProgramData\ssh\sshd_config");

    let contents = std::fs::read_to_string(&config_path)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not read sshd_config: {}",
                error
            ))
        })?;

    let pubkey_enabled = contents.lines().any(|line| {
        line.trim().eq_ignore_ascii_case("PubkeyAuthentication yes")
    });

    let user_restricted = contents.lines().any(|line| {
        line.trim().eq_ignore_ascii_case(
            &format!("AllowUsers {}", PUPPET_USER)
        )
    });

    if !pubkey_enabled {
        return Err(PuppetError::ConfigurationError(
            "PubkeyAuthentication yes was not found".to_string(),
        ));
    }

    if !user_restricted {
        return Err(PuppetError::ConfigurationError(
            "AllowUsers restriction for puppet-ssh was not found"
                .to_string(),
        ));
    }

    println!("SSH public-key authentication: VERIFIED");
    println!("SSH user restriction: VERIFIED");

    Ok(())
}

fn restart_sshd() -> Result<(), PuppetError> {
    println!("Restarting OpenSSH service...");

    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Restart-Service -Name sshd",
        ],
    )?;

    verify_sshd_service()?;

    println!("OpenSSH service: RUNNING");

    Ok(())
}

fn validate_sshd_config() -> Result<(), PuppetError> {
    println!("Validating OpenSSH configuration...");

    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "& \"$env:WINDIR\\System32\\OpenSSH\\sshd.exe\" -t",
        ],
    )?;

    println!("OpenSSH configuration: VALID");

    Ok(())
}

fn verify_sshd_service() -> Result<(), PuppetError> {
    let output = command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-Service -Name sshd).Status",
        ],
    )?;

    if output.stdout.trim().eq_ignore_ascii_case("Running") {
        Ok(())
    } else {
        Err(PuppetError::TemporaryFailure(
            format!(
                "sshd service is not running; reported state: {}",
                output.stdout
            ),
        ))
    }
}


fn configure_firewall() -> Result<(), PuppetError> {
    if firewall_rule_exists() {
        println!("OpenSSH firewall rule: EXISTS");
        return Ok(());
    }

    println!("OpenSSH firewall rule: NOT FOUND");
    println!("Creating firewall rule for TCP port 22...");

    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "New-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22",
        ],
    )?;

    if !firewall_rule_exists() {
        return Err(PuppetError::TemporaryFailure(
            "OpenSSH firewall rule was not found after creation".to_string(),
        ));
    }

    Ok(())
}

fn firewall_rule_exists() -> bool {
    match command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue).Enabled",
        ],
    ) {
        Ok(output) => output
            .stdout
            .trim()
            .eq_ignore_ascii_case("True"),

        Err(_) => false,
    }
}


const PUPPET_USER: &str = "puppet-ssh";

fn configure_puppet_user() -> Result<(), PuppetError> {
    if local_user_exists() {
        println!("User '{}': EXISTS", PUPPET_USER);
    } else {
        println!("User '{}': NOT FOUND", PUPPET_USER);
        println!("Creating local account...");

        create_puppet_user()?;

        if !local_user_exists() {
            return Err(PuppetError::TemporaryFailure(
                "puppet-ssh account was not found after creation".to_string(),
            ));
        }

        println!("User '{}': CREATED", PUPPET_USER);
    }

    if !user_is_administrator() {
        println!("User '{}': NOT ADMINISTRATOR", PUPPET_USER);
        println!("Adding account to Administrators...");

        add_user_to_administrators()?;
    }

    if !user_is_administrator() {
        return Err(PuppetError::ConfigurationError(
            "puppet-ssh account is not a member of Administrators".to_string(),
        ));
    }

    println!("User '{}': ADMINISTRATOR", PUPPET_USER);

    install_authorized_key()?;

    configure_key_authentication()?;

    configure_ssh_user_restriction()?;

    verify_ssh_authentication_config()?;

    validate_sshd_config()?;

    restart_sshd()?;

    Ok(())
}

fn install_authorized_key() -> Result<(), PuppetError> {
    let source = installer_directory()?.join("authorized_key.pub");
    let destination =
        std::path::PathBuf::from(r"C:\ProgramData\ssh\administrators_authorized_keys");

    println!("Installing SSH public key...");
    println!("Source      : {}", source.display());
    println!("Destination : {}", destination.display());

    let key = std::fs::read_to_string(&source)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not read public key: {}",
                error
            ))
        })?;

    let key = key.trim();

    if key.is_empty() {
        return Err(PuppetError::ConfigurationError(
            "public key is empty".to_string(),
        ));
    }

    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "$path = 'C:\\ProgramData\\ssh\\administrators_authorized_keys'; New-Item -ItemType File -Path $path -Force | Out-Null",
        ],
    )?;

    std::fs::write(&destination, format!("{}\n", key))
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not write authorized keys file: {}",
                error
            ))
        })?;

    command::run(
        "icacls",
        &[
            r"C:\ProgramData\ssh\administrators_authorized_keys",
            "/inheritance:r",
            "/grant",
            "*S-1-5-32-544:F",
            "/grant",
            "SYSTEM:F",
        ],
    )?;

    verify_authorized_key(&destination, key)?;

    println!("SSH public key: INSTALLED");
    println!("SSH key ACL: READY");

    Ok(())
}

fn verify_authorized_key(
    path: &std::path::Path,
    expected_key: &str,
) -> Result<(), PuppetError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not verify authorized keys file: {}",
                error
            ))
        })?;

    if contents.lines().any(|line| line.trim() == expected_key) {
        Ok(())
    } else {
        Err(PuppetError::ConfigurationError(
            "installed public key was not found in authorized keys file"
                .to_string(),
        ))
    }
}

fn local_user_exists() -> bool {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Get-LocalUser -Name 'puppet-ssh' -ErrorAction SilentlyContinue",
        ],
    )
    .is_ok()
}

fn create_puppet_user() -> Result<(), PuppetError> {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "New-LocalUser -Name 'puppet-ssh' -NoPassword -AccountNeverExpires -UserMayNotChangePassword",
        ],
    )?;

    Ok(())
}

fn user_is_administrator() -> bool {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Get-LocalGroupMember -Group 'Administrators' -Member 'puppet-ssh' -ErrorAction SilentlyContinue",
        ],
    )
    .is_ok()
}

fn add_user_to_administrators() -> Result<(), PuppetError> {
    command::run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Add-LocalGroupMember -Group 'Administrators' -Member 'puppet-ssh'",
        ],
    )?;

    Ok(())
}


fn validate_public_key() -> Result<(), PuppetError> {
    let key_path = installer_directory()?.join("authorized_key.pub");

    if !key_path.exists() {
        return Err(PuppetError::ConfigurationError(
            format!(
                "public key file not found: {}",
                key_path.display()
            ),
        ));
    }

    let contents = std::fs::read_to_string(&key_path)
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not read public key file: {}",
                error
            ))
        })?;

    let key = contents.trim();

    if key.is_empty() {
        return Err(PuppetError::ConfigurationError(
            "public key file is empty".to_string(),
        ));
    }

    let parts: Vec<&str> = key.split_whitespace().collect();

    if parts.len() < 2 {
        return Err(PuppetError::ConfigurationError(
            "public key format is invalid".to_string(),
        ));
    }

    if parts[0] != "ssh-ed25519" {
        return Err(PuppetError::ConfigurationError(
            "expected an Ed25519 public key (ssh-ed25519)".to_string(),
        ));
    }

    if parts[1].is_empty() {
        return Err(PuppetError::ConfigurationError(
            "public key data is empty".to_string(),
        ));
    }

    println!(
        "SSH public key file: {}",
        key_path.display()
    );

    Ok(())
}

fn installer_directory() -> Result<std::path::PathBuf, PuppetError> {
    let executable = env::current_exe()
        .map_err(|error| {
            PuppetError::ConfigurationError(format!(
                "could not determine executable location: {}",
                error
            ))
        })?;

    executable
        .parent()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| {
            PuppetError::ConfigurationError(
                "could not determine installer directory".to_string(),
            )
        })
}
