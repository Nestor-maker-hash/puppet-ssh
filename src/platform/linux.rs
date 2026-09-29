use crate::command;

pub fn run() {
    println!("Linux installer selected.");
    println!();

    match detect_distribution() {
        Some(distro) => println!("Linux distribution: {}", distro),
        None => println!("Linux distribution: UNKNOWN"),
    }

    match detect_package_manager() {
        Some(manager) => println!("Package manager: {}", manager),
        None => println!("Package manager: UNKNOWN"),
    }

    println!();

    if !is_root() {
        println!("Root privileges: NO");
        println!();
        println!("Puppet-SSH must be run as root.");
        println!("Please restart it with root privileges.");
        return;
    }

    println!("Root privileges: YES");
    println!();

    if openssh_server_installed() {
        println!("OpenSSH Server: INSTALLED");
    } else {
        println!("OpenSSH Server: NOT INSTALLED");
        println!("Installing OpenSSH Server...");

        match install_openssh_server() {
            Ok(()) => println!("OpenSSH Server: INSTALLED"),
            Err(error) => {
                println!("OpenSSH installation failed.");
                println!("Reason: {}", error);
                return;
            }
        }
    }

    println!();
    println!("Starting SSH service...");

    match enable_and_start_sshd() {
        Ok(()) => println!("SSH service: RUNNING"),
        Err(error) => {
            println!("SSH service setup failed.");
            println!("Reason: {}", error);
            return;
        }
    }

    println!();
    println!("Configuring SSH user...");

    if let Err(error) = create_ssh_user() {
        println!("SSH user setup failed.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("Installing SSH public key...");

    if let Err(error) = install_authorized_key() {
        println!("SSH key installation failed.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("Configuring SSH authentication...");

    if let Err(error) = configure_ssh_authentication() {
        println!("SSH authentication configuration failed.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("Restarting SSH service...");

    if let Err(error) = restart_sshd() {
        println!("SSH service restart failed.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("Configuring firewall...");

    if let Err(error) = configure_firewall() {
        println!("Firewall configuration failed.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("Verifying SSH setup...");

    if let Err(error) = verify_ssh_setup() {
        println!("SSH verification failed.");
        println!("Reason: {}", error);
        return;
    }

    if let Err(error) = show_connection_info() {
        println!("Could not determine connection information.");
        println!("Reason: {}", error);
        return;
    }

    println!();
    println!("================================");
    println!("     PUPPET-SSH READY");
    println!("================================");
}

fn is_root() -> bool {
    command::run("id", &["-u"])
        .map(|output| output.stdout.trim() == "0")
        .unwrap_or(false)
}

fn detect_package_manager() -> Option<&'static str> {
    for manager in ["apt-get", "dnf", "pacman", "zypper"] {
        if command::run("sh", &["-c", &format!("command -v {}", manager)]).is_ok() {
            return Some(manager);
        }
    }

    None
}

fn openssh_server_installed() -> bool {
    command::run(
        "sh",
        &["-c", "command -v sshd"],
    )
    .is_ok()
}

fn create_ssh_user() -> Result<(), crate::error::PuppetError> {
    let user_exists = command::run("id", &["puppet-ssh"]).is_ok();

    if user_exists {
        println!("SSH user: puppet-ssh already exists");
    } else {
        command::run(
            "useradd",
            &[
                "--create-home",
                "--shell",
                "/bin/bash",
                "puppet-ssh",
            ],
        )?;

        println!("SSH user: puppet-ssh created");
    }

    if command::run("getent", &["group", "sudo"]).is_ok() {
        command::run("usermod", &["-aG", "sudo", "puppet-ssh"])?;
    } else if command::run("getent", &["group", "wheel"]).is_ok() {
        command::run("usermod", &["-aG", "wheel", "puppet-ssh"])?;
    } else {
        return Err(crate::error::PuppetError::Unsupported(
            "could not find sudo or wheel administrator group".to_string(),
        ));
    }

    println!("Administrator access: configured");

    Ok(())
}

fn show_connection_info() -> Result<(), crate::error::PuppetError> {
    let hostname = command::run("hostname", &[])?
        .stdout
        .trim()
        .to_string();

    let ip_output = command::run(
        "sh",
        &[
            "-c",
            "hostname -I | awk '{print $1}'",
        ],
    )?;

    let ip = ip_output.stdout.trim();

    println!();
    println!("Computer hostname : {}", hostname);
    println!("Local IP address  : {}", ip);
    println!();
    println!("Connect from your phone:");
    println!("  ssh puppet-ssh@{}", hostname);

    if !ip.is_empty() {
        println!("  ssh puppet-ssh@{}", ip);
    }

    Ok(())
}

fn verify_ssh_setup() -> Result<(), crate::error::PuppetError> {
    command::run("id", &["puppet-ssh"])?;

    let home = command::run(
        "sh",
        &[
            "-c",
            "getent passwd puppet-ssh | cut -d: -f6",
        ],
    )?
    .stdout
    .trim()
    .to_string();

    if home.is_empty() {
        return Err(crate::error::PuppetError::ConfigurationError(
            "could not determine puppet-ssh home directory".to_string(),
        ));
    }

    let authorized_keys = format!("{}/.ssh/authorized_keys", home);

    if !std::path::Path::new(&authorized_keys).is_file() {
        return Err(crate::error::PuppetError::ConfigurationError(
            format!("authorized_keys not found at {}", authorized_keys),
        ));
    }

    let ssh_dir = format!("{}/.ssh", home);

    let ssh_dir_owner = command::run(
        "stat",
        &["-c", "%U:%G", &ssh_dir],
    )?
    .stdout
    .trim()
    .to_string();

    if ssh_dir_owner != "puppet-ssh:puppet-ssh" {
        return Err(crate::error::PuppetError::ConfigurationError(
            format!("incorrect .ssh ownership: {}", ssh_dir_owner),
        ));
    }

    let ssh_dir_mode = command::run(
        "stat",
        &["-c", "%a", &ssh_dir],
    )?
    .stdout
    .trim()
    .to_string();

    if ssh_dir_mode != "700" {
        return Err(crate::error::PuppetError::ConfigurationError(
            format!("incorrect .ssh permissions: {}", ssh_dir_mode),
        ));
    }

    let authorized_keys_owner = command::run(
        "stat",
        &["-c", "%U:%G", &authorized_keys],
    )?
    .stdout
    .trim()
    .to_string();

    if authorized_keys_owner != "puppet-ssh:puppet-ssh" {
        return Err(crate::error::PuppetError::ConfigurationError(
            format!(
                "incorrect authorized_keys ownership: {}",
                authorized_keys_owner
            ),
        ));
    }

    let authorized_keys_mode = command::run(
        "stat",
        &["-c", "%a", &authorized_keys],
    )?
    .stdout
    .trim()
    .to_string();

    if authorized_keys_mode != "600" {
        return Err(crate::error::PuppetError::ConfigurationError(
            format!(
                "incorrect authorized_keys permissions: {}",
                authorized_keys_mode
            ),
        ));
    }

    command::run(
        "sh",
        &[
            "-c",
            "sshd -T | grep -qi '^pubkeyauthentication yes$'",
        ],
    )?;

    command::run(
        "sh",
        &[
            "-c",
            "sshd -T | grep -qi '^allowusers puppet-ssh$'",
        ],
    )?;

    println!("SSH user: VERIFIED");
    println!("Authorized key: VERIFIED");
    println!("Public-key authentication: VERIFIED");
    println!("SSH user restriction: VERIFIED");

    Ok(())
}

fn restart_sshd() -> Result<(), crate::error::PuppetError> {
    if command::run("sh", &["-c", "command -v systemctl"]).is_ok() {
        if command::run("systemctl", &["restart", "sshd"]).is_ok() {
            println!("SSH service: RESTARTED");
            return Ok(());
        }

        command::run("systemctl", &["restart", "ssh"])?;
        println!("SSH service: RESTARTED");
        return Ok(());
    }

    if command::run("sh", &["-c", "command -v rc-service"]).is_ok() {
        command::run("rc-service", &["sshd", "restart"])?;
        println!("SSH service: RESTARTED");
        return Ok(());
    }

    Err(crate::error::PuppetError::Unsupported(
        "could not restart the SSH service".to_string(),
    ))
}

fn configure_firewall() -> Result<(), crate::error::PuppetError> {
    if command::run("sh", &["-c", "command -v ufw"]).is_ok() {
        let status = command::run("ufw", &["status"])?;

        if status.stdout.to_lowercase().contains("active") {
            command::run("ufw", &["allow", "22/tcp"])?;
            println!("Firewall: TCP 22 allowed through UFW");
        } else {
            println!("Firewall: UFW installed but inactive");
        }

        return Ok(());
    }

    if command::run("sh", &["-c", "command -v firewall-cmd"]).is_ok() {
        let state = command::run("firewall-cmd", &["--state"]);

        if state.is_ok() {
            command::run(
                "firewall-cmd",
                &["--permanent", "--add-service=ssh"],
            )?;

            command::run("firewall-cmd", &["--reload"])?;

            println!("Firewall: SSH allowed through firewalld");
        } else {
            println!("Firewall: firewalld installed but inactive");
        }

        return Ok(());
    }

    println!("Firewall: no supported active firewall detected");

    Ok(())
}

fn configure_ssh_authentication() -> Result<(), crate::error::PuppetError> {
    let config_dir = "/etc/ssh/sshd_config.d";
    let config_file = format!("{}/99-puppet-ssh.conf", config_dir);

    std::fs::create_dir_all(config_dir).map_err(|error| {
        crate::error::PuppetError::ConfigurationError(format!(
            "could not create {}: {}",
            config_dir, error
        ))
    })?;

    let config = concat!(
        "PubkeyAuthentication yes\n",
        "AllowUsers puppet-ssh\n",
    );

    std::fs::write(&config_file, config).map_err(|error| {
        crate::error::PuppetError::ConfigurationError(format!(
            "could not write {}: {}",
            config_file, error
        ))
    })?;

    println!("SSH public-key authentication: ENABLED");
    println!("SSH user restriction: puppet-ssh");

    command::run("sshd", &["-t"])?;

    println!("SSH configuration: VALID");

    Ok(())
}

fn install_authorized_key() -> Result<(), crate::error::PuppetError> {
    let home = command::run(
        "sh",
        &[
            "-c",
            "getent passwd puppet-ssh | cut -d: -f6",
        ],
    )?
    .stdout
    .trim()
    .to_string();

    if home.is_empty() {
        return Err(crate::error::PuppetError::ConfigurationError(
            "could not determine puppet-ssh home directory".to_string(),
        ));
    }

    let ssh_dir = format!("{}/.ssh", home);
    let authorized_keys = format!("{}/authorized_keys", ssh_dir);

    std::fs::create_dir_all(&ssh_dir).map_err(|error| {
        crate::error::PuppetError::ConfigurationError(format!(
            "could not create {}: {}",
            ssh_dir, error
        ))
    })?;

    command::run(
        "chown",
        &["puppet-ssh:puppet-ssh", &ssh_dir],
    )?;

    command::run("chmod", &["700", &ssh_dir])?;

    let executable = std::env::current_exe().map_err(|error| {
        crate::error::PuppetError::ConfigurationError(format!(
            "could not determine executable location: {}",
            error
        ))
    })?;

    let key_path = executable
        .parent()
        .ok_or_else(|| {
            crate::error::PuppetError::ConfigurationError(
                "could not determine executable directory".to_string(),
            )
        })?
        .join("authorized_key.pub");

    let public_key = std::fs::read_to_string(&key_path)
        .map_err(|error| {
            crate::error::PuppetError::ConfigurationError(format!(
                "could not read {}: {}",
                key_path.display(),
                error
            ))
        })?;

    let public_key = public_key.trim();

    if !public_key.starts_with("ssh-ed25519 ") {
        return Err(crate::error::PuppetError::ConfigurationError(
            "authorized_key.pub must contain an Ed25519 public key".to_string(),
        ));
    }

    let existing = std::fs::read_to_string(&authorized_keys)
        .unwrap_or_default();

    if !existing.lines().any(|line| line.trim() == public_key) {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&authorized_keys)
            .map_err(|error| {
                crate::error::PuppetError::ConfigurationError(format!(
                    "could not open authorized_keys: {}",
                    error
                ))
            })?;

        writeln!(file, "{}", public_key).map_err(|error| {
            crate::error::PuppetError::ConfigurationError(format!(
                "could not write authorized key: {}",
                error
            ))
        })?;
    }

    command::run(
        "chown",
        &["puppet-ssh:puppet-ssh", &authorized_keys],
    )?;

    command::run("chmod", &["600", &authorized_keys])?;

    println!("SSH public key: INSTALLED");

    Ok(())
}

fn enable_and_start_sshd() -> Result<(), crate::error::PuppetError> {
    if command::run("sh", &["-c", "command -v systemctl"]).is_ok() {
        command::run("systemctl", &["enable", "--now", "sshd"])
            .or_else(|_| command::run("systemctl", &["enable", "--now", "ssh"]))?;
        return Ok(());
    }

    if command::run("sh", &["-c", "command -v rc-service"]).is_ok() {
        command::run("rc-service", &["sshd", "start"])?;
        return Ok(());
    }

    Err(crate::error::PuppetError::Unsupported(
        "no supported Linux service manager was found".to_string(),
    ))
}

fn install_openssh_server() -> Result<(), crate::error::PuppetError> {
    let manager = detect_package_manager().ok_or_else(|| {
        crate::error::PuppetError::Unsupported(
            "no supported Linux package manager was found".to_string(),
        )
    })?;

    match manager {
        "apt-get" => {
            command::run(
                "apt-get",
                &["update"],
            )?;

            command::run(
                "apt-get",
                &["install", "-y", "openssh-server"],
            )?;
        }

        "dnf" => {
            command::run(
                "dnf",
                &["install", "-y", "openssh-server"],
            )?;
        }

        "pacman" => {
            command::run(
                "pacman",
                &["-Sy", "--noconfirm", "openssh"],
            )?;
        }

        "zypper" => {
            command::run(
                "zypper",
                &["--non-interactive", "install", "openssh"],
            )?;
        }

        _ => {
            return Err(crate::error::PuppetError::Unsupported(
                format!("unsupported package manager: {}", manager),
            ));
        }
    }

    Ok(())
}

fn detect_distribution() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            return Some(value.trim_matches('"').to_string());
        }
    }

    None
}
