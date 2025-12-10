/// Default denylist of dangerous command patterns
pub const DEFAULT_DENYLIST: &[&str] = &[
    // Destructive file operations
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf *",
    "rm -fr /",
    "rm --no-preserve-root",
    "> /dev/sda",
    "> /dev/hda",
    "dd if=/dev/zero",
    "dd if=/dev/random",
    "mkfs",
    "mkfs.ext",
    "format c:",
    // System manipulation
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "init 0",
    "init 6",
    "systemctl poweroff",
    "systemctl reboot",
    "systemctl halt",
    // Fork bombs and resource exhaustion
    ":(){:|:&};:",
    ":(){ :|:& };:",
    "fork while fork",
    // Permission changes
    "chmod 777 /",
    "chmod -R 777 /",
    "chown -R root",
    "chown root /",
    // Package manager dangers
    "apt-get remove --purge",
    "apt remove --purge",
    "yum remove",
    "dnf remove",
    "pacman -R",
    "brew uninstall --force",
    // Kernel manipulation
    "modprobe -r",
    "rmmod",
    "insmod",
    // Network attacks
    "tcpdump -w /dev/null",
    "wget http",
    "curl http",
    // Cron/service manipulation
    "crontab -r",
    // Moving system directories
    "mv /etc",
    "mv /usr",
    "mv /var",
    "mv /bin",
    "mv /sbin",
    "mv /lib",
];

/// Check if a command matches any pattern in the denylist
pub fn is_denied(command: &str, custom_patterns: &[String]) -> bool {
    let command_lower = command.to_lowercase();

    // Check default denylist
    for pattern in DEFAULT_DENYLIST {
        if command_lower.contains(&pattern.to_lowercase()) {
            return true;
        }
    }

    // Check custom patterns
    for pattern in custom_patterns {
        if !pattern.is_empty() && command_lower.contains(&pattern.to_lowercase()) {
            return true;
        }
    }

    false
}

/// Get the matched pattern for reporting
pub fn find_matched_pattern(command: &str, custom_patterns: &[String]) -> Option<String> {
    let command_lower = command.to_lowercase();

    // Check default denylist
    for pattern in DEFAULT_DENYLIST {
        if command_lower.contains(&pattern.to_lowercase()) {
            return Some(pattern.to_string());
        }
    }

    // Check custom patterns
    for pattern in custom_patterns {
        if !pattern.is_empty() && command_lower.contains(&pattern.to_lowercase()) {
            return Some(pattern.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_commands() {
        assert!(is_denied("rm -rf /", &[]));
        assert!(is_denied("sudo rm -rf /", &[]));
        assert!(is_denied("chmod 777 /etc", &[]));
        assert!(is_denied("mkfs /dev/sda", &[]));
        assert!(is_denied("dd if=/dev/zero of=/dev/sda", &[]));
        assert!(is_denied(":(){:|:&};:", &[]));
    }

    #[test]
    fn test_safe_commands() {
        assert!(!is_denied("ls -la", &[]));
        assert!(!is_denied("cat /etc/hosts", &[]));
        assert!(!is_denied("grep pattern file.txt", &[]));
        assert!(!is_denied("rm file.txt", &[]));
        assert!(!is_denied("chmod 755 script.sh", &[]));
    }

    #[test]
    fn test_custom_patterns() {
        let custom = vec!["docker rm".to_string(), "kubectl delete".to_string()];
        assert!(is_denied("docker rm -f container", &custom));
        assert!(is_denied("kubectl delete pod", &custom));
        assert!(!is_denied("docker ps", &custom));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(is_denied("RM -RF /", &[]));
        assert!(is_denied("SHUTDOWN", &[]));
        assert!(is_denied("Chmod 777 /", &[]));
    }
}
