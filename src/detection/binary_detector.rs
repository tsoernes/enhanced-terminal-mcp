use serde::Serialize;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct BinaryReport {
    pub name: String,
    pub category: String,
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShellInfo {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
}

// Base candidate groups for binary detection
const BASE_CANDIDATE_GROUPS: &[(&str, &[&str])] = &[
    (
        "package_managers",
        &[
            "npm", "pip", "cargo", "dnf", "apt", "snap", "flatpak", "brew",
        ],
    ),
    (
        "rust_tools",
        &["cargo", "rustc", "rustfmt", "clippy-driver"],
    ),
    (
        "python_tools",
        &[
            "python", "python3", "pip", "pytest", "black", "ruff", "mypy",
        ],
    ),
    (
        "build_systems",
        &["make", "cmake", "ninja", "gradle", "maven", "mvn"],
    ),
    ("c_cpp_tools", &["gcc", "g++", "clang", "gdb", "lldb"]),
    (
        "java_jvm_tools",
        &[
            "java",
            "javac",
            "javadoc",
            "jar",
            "jarsigner",
            "jconsole",
            "jdeps",
            "jlink",
            "jshell",
            "kotlin",
            "kotlinc",
            "scala",
            "scalac",
            "groovy",
            "groovyc",
        ],
    ),
    ("maven_tools", &["mvn", "mvnw", "mvnd"]),
    ("node_js_tools", &["node", "deno", "bun", "npm", "yarn"]),
    ("go_tools", &["go", "gofmt"]),
    ("editors_dev", &["vim", "nvim", "emacs", "code", "zed"]),
    (
        "search_productivity",
        &["rg", "fd", "fzf", "jq", "bat", "tree", "exa"],
    ),
    ("system_perf", &["htop", "ps", "top", "df", "du"]),
    ("containers", &["docker", "podman", "kubectl", "helm"]),
    ("networking", &["curl", "wget", "dig", "traceroute"]),
    ("security", &["openssl", "gpg", "ssh-keygen"]),
    ("databases", &["sqlite3", "psql", "mysql", "redis-cli"]),
    ("vcs", &["git", "gh"]),
];

const COMMON_SHELLS: &[(&str, &str)] = &[
    ("/bin/bash", "bash"),
    ("/usr/bin/bash", "bash"),
    ("/bin/zsh", "zsh"),
    ("/usr/bin/zsh", "zsh"),
    ("/usr/local/bin/zsh", "zsh"),
    ("/bin/fish", "fish"),
    ("/usr/bin/fish", "fish"),
    ("/usr/local/bin/fish", "fish"),
    ("/bin/sh", "sh"),
    ("/usr/bin/sh", "sh"),
    ("/bin/dash", "dash"),
    ("/bin/ksh", "ksh"),
    ("/bin/tcsh", "tcsh"),
    ("/bin/csh", "csh"),
];

pub fn detect_binaries(
    filter_categories: Option<Vec<String>>,
    max_concurrency: usize,
    version_timeout_ms: u64,
    include_missing: bool,
) -> Vec<BinaryReport> {
    let filter_set: Option<BTreeSet<String>> = filter_categories
        .as_ref()
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect());

    let mut tasks: Vec<(String, String)> = Vec::new();

    for (category, binaries) in BASE_CANDIDATE_GROUPS {
        if let Some(ref filter) = filter_set {
            if !filter.contains(&category.to_lowercase()) {
                continue;
            }
        }

        for binary in *binaries {
            tasks.push((category.to_string(), binary.to_string()));
        }
    }

    let max_conc = max_concurrency.max(1);
    let shared_results: Arc<Mutex<Vec<BinaryReport>>> = Arc::new(Mutex::new(Vec::new()));

    // Process in chunks
    for chunk in tasks.chunks(max_conc) {
        let mut handles = Vec::new();

        for (category, binary) in chunk.iter().cloned() {
            let results = Arc::clone(&shared_results);
            let handle = thread::spawn(move || {
                let paths = which_all(&binary);

                if paths.is_empty() {
                    if let Ok(mut vec) = results.lock() {
                        vec.push(BinaryReport {
                            name: binary,
                            category,
                            found: false,
                            path: None,
                            version: None,
                            error: None,
                        });
                    }
                    return;
                }

                let path_field = if paths.len() > 1 {
                    Some(paths.join(";"))
                } else {
                    Some(paths[0].clone())
                };

                let version_result = detect_version(&paths[0], version_timeout_ms);

                if let Ok(mut vec) = results.lock() {
                    match version_result {
                        Ok(v) => {
                            vec.push(BinaryReport {
                                name: binary,
                                category,
                                found: true,
                                path: path_field,
                                version: Some(v),
                                error: None,
                            });
                        }
                        Err(e) => {
                            vec.push(BinaryReport {
                                name: binary,
                                category,
                                found: true,
                                path: path_field,
                                version: None,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }
    }

    let mut reports = match Arc::try_unwrap(shared_results) {
        Ok(mutex) => mutex.into_inner().unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    reports.sort_by(|a, b| {
        (a.category.as_str(), a.name.as_str()).cmp(&(b.category.as_str(), b.name.as_str()))
    });

    if !include_missing {
        reports.retain(|r| r.found);
    }

    reports
}

pub fn detect_shells() -> Vec<ShellInfo> {
    let mut shells = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    for (path, name) in COMMON_SHELLS {
        if Path::new(path).exists() && !seen_names.contains(*name) {
            seen_names.insert(name.to_string());

            let version = detect_version(path, 1500).ok();

            shells.push(ShellInfo {
                name: name.to_string(),
                path: path.to_string(),
                version,
            });
        }
    }

    // Check $SHELL environment variable
    if let Ok(user_shell) = env::var("SHELL") {
        if !shells.iter().any(|s| s.path == user_shell) {
            let name = Path::new(&user_shell)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let version = detect_version(&user_shell, 1500).ok();

            shells.push(ShellInfo {
                name,
                path: user_shell,
                version,
            });
        }
    }

    shells
}

fn which_all(name: &str) -> Vec<String> {
    let mut matches = Vec::new();
    let path_var = match env::var_os("PATH") {
        Some(p) => p,
        None => return matches,
    };

    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() && is_executable(&candidate) {
            if let Some(s) = candidate.to_str() {
                matches.push(s.to_string());
            }
        }
    }
    matches
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = p.metadata() {
        let mode = meta.permissions().mode();
        mode & 0o111 != 0
    } else {
        false
    }
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn detect_version(path: &str, timeout_ms: u64) -> anyhow::Result<String> {
    let attempts: &[&[&str]] = &[&["--version"], &["version"], &["-V"]];
    let mut last_err: Option<anyhow::Error> = None;

    for args in attempts {
        match probe_version(path, args, timeout_ms) {
            Ok(line) => return Ok(line),
            Err(e) => {
                last_err = Some(e);
                if last_err
                    .as_ref()
                    .map(|er| er.to_string().contains("timeout"))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no version retrieved")))
}

fn probe_version(path: &str, args: &[&str], timeout_ms: u64) -> anyhow::Result<String> {
    let (tx, rx) = mpsc::channel();
    let path_string = path.to_string();
    let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    thread::spawn(move || {
        let output = Command::new(&path_string).args(&args_vec).output();
        let result = match output {
            Ok(out) => {
                let text = if !out.stdout.is_empty() {
                    String::from_utf8_lossy(&out.stdout).to_string()
                } else {
                    String::from_utf8_lossy(&out.stderr).to_string()
                };
                let first_line = text.lines().next().unwrap_or("").trim();
                if first_line.is_empty() {
                    Err(anyhow::anyhow!("empty version output"))
                } else {
                    Ok(first_line.to_string())
                }
            }
            Err(e) => Err(anyhow::anyhow!("spawn failed: {}", e)),
        };
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!(
            "version probe timeout after {}ms",
            timeout_ms
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(anyhow::anyhow!("version probe worker disconnected"))
        }
    }
}
