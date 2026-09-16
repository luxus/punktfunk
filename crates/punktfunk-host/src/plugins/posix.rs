//! The plugin runner off Windows: a `punktfunk-scripting` wrapper found by rung, driven as a
//! systemd USER unit on Linux. Other Unix hosts resolve the runner but cannot run it.

use super::*;

#[cfg(target_os = "linux")]
const UNIT: &str = "punktfunk-scripting";

/// Wrapper name every non-Windows package installs (`/usr/bin`, `~/.local/bin`, `$out/bin`).
const RUNNER_BIN: &str = "punktfunk-scripting";

/// No elevation bar off Windows: the runner is the operator's own user unit.
pub(super) fn require_elevation(_what: &str) -> Result<()> {
    Ok(())
}

pub(super) fn usage_note() {}

pub(super) fn runner_command() -> Result<(std::path::PathBuf, Vec<String>)> {
    let exe = std::env::current_exe().ok();
    let path_var = std::env::var("PATH").ok();
    let home = std::env::var("HOME").ok();
    resolve_runner_in(
        std::env::var("PUNKTFUNK_SCRIPTING").ok().as_deref(),
        exe.as_deref().and_then(std::path::Path::parent),
        path_var.as_deref(),
        home.as_deref().map(std::path::Path::new),
        &|p| p.is_file(),
    )
    .ok_or_else(|| anyhow::anyhow!("{RUNNER_MISSING}"))
}

/// Shared with [`runtime_status`] so CLI and console say the same thing.
pub(super) const RUNNER_MISSING: &str =
    "the plugin runner isn't installed — install it first (Debian/Ubuntu: `sudo apt install \
     punktfunk-scripting`; SteamOS: re-run scripts/steamdeck/install.sh; NixOS: enable \
     `services.punktfunk.scripting`). If it is installed somewhere else, point PUNKTFUNK_SCRIPTING \
     at the punktfunk-scripting executable.";

/// Rungs: `PUNKTFUNK_SCRIPTING` → beside the host → `PATH` → `/usr` → `~/.local`.
/// Injected so tests do not mutate process env (races `getenv` in parallel).
///
/// `PATH` is the only rung a Nix install can land on: `punktfunk-scripting` is its
/// own derivation, neither beside the host nor under `/usr`.
fn resolve_runner_in(
    env: Option<&str>,
    exe_dir: Option<&std::path::Path>,
    path_var: Option<&str>,
    home: Option<&std::path::Path>,
    exists: &dyn Fn(&std::path::Path) -> bool,
) -> Option<(std::path::PathBuf, Vec<String>)> {
    use std::path::{Path, PathBuf};

    // Two-file layout (private bun + runner bundle). A rung only when both exist.
    let pair = |bun: PathBuf, runner: PathBuf| -> Option<(PathBuf, Vec<String>)> {
        (exists(&bun) && exists(&runner))
            .then(|| (bun, vec![runner.to_string_lossy().into_owned()]))
    };

    // Operator override: not existence-checked, so a typo fails naming that path
    // instead of silently using some other installed runner.
    if let Some(v) = env.map(str::trim).filter(|v| !v.is_empty()) {
        return Some((PathBuf::from(v), Vec::new()));
    }
    if let Some(p) = exe_dir.map(|d| d.join(RUNNER_BIN)).filter(|p| exists(p)) {
        return Some((p, Vec::new()));
    }
    if let Some(p) = path_var
        .into_iter()
        .flat_map(|v| v.split(':'))
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(RUNNER_BIN))
        .find(|p| exists(p))
    {
        return Some((p, Vec::new()));
    }
    // Packaged `/usr` after `PATH`: a systemd unit PATH may omit `/usr/bin`.
    let wrapper = Path::new("/usr/bin").join(RUNNER_BIN);
    if exists(&wrapper) {
        return Some((wrapper, Vec::new()));
    }
    if let Some(cmd) = pair(
        Path::new("/usr/lib").join(RUNNER_BIN).join("bun"),
        Path::new("/usr/share")
            .join(RUNNER_BIN)
            .join("runner-cli.js"),
    ) {
        return Some(cmd);
    }
    // Immutable `/usr` (SteamOS): the same payload, user-scoped under `~/.local`.
    let home = home?;
    let wrapper = home.join(".local/bin").join(RUNNER_BIN);
    if exists(&wrapper) {
        return Some((wrapper, Vec::new()));
    }
    pair(
        home.join(".local/lib").join(RUNNER_BIN).join("bun"),
        home.join(".local/share")
            .join(RUNNER_BIN)
            .join("runner-cli.js"),
    )
}

/// Nothing to grant off Windows: the runner is a systemd USER unit, so it already runs as the
/// operator and reads exactly what they can.
pub(super) fn grant(_dir: Option<&str>) -> Result<()> {
    println!("Nothing to grant: the plugin runner is a systemd USER unit, so it runs as you.");
    Ok(())
}

/// Lifts a mask left by [`disable`] first; a no-op when there is none.
#[cfg(target_os = "linux")]
pub(super) fn enable() -> Result<()> {
    run_systemctl(&["unmask", UNIT])?;
    run_systemctl(&["enable", "--now", UNIT])?;
    println!("Plugin runner enabled and started ({UNIT}).");
    Ok(())
}

/// The packages enable the unit in GLOBAL scope (`/etc/systemd/user`), which a user-scope
/// disable cannot undo: `is-enabled` keeps answering `enabled`. Mask is the per-user opt-out
/// there. The SteamOS install writes the unit into `~/.config/systemd/user`, where mask is
/// refused, so mask only when disable left it enabled.
#[cfg(target_os = "linux")]
pub(super) fn disable() -> Result<()> {
    run_systemctl(&["disable", "--now", UNIT])?;
    if systemctl_output(&["is-enabled", UNIT]).as_deref() == Some("enabled") {
        run_systemctl(&["mask", UNIT])?;
    }
    println!("Plugin runner stopped and disabled ({UNIT}).");
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn runtime_status() -> RuntimeStatus {
    let enabled_raw = systemctl_output(&["is-enabled", UNIT]);
    let active = systemctl_output(&["is-active", UNIT]).unwrap_or_default();
    // `is-enabled` is `not-found` when the unit file is missing; the runner payload
    // is the other half of "can we install plugins".
    let unit_known = enabled_raw.as_deref().is_some_and(|s| s != "not-found");
    let installed = unit_known || runner_command().is_ok();
    RuntimeStatus {
        installed,
        enabled: enabled_raw.as_deref() == Some("enabled"),
        running: active == "active",
        unit: UNIT,
        principal: None,
        detail: if installed {
            String::new()
        } else {
            RUNNER_MISSING.into()
        },
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn runtime_status() -> RuntimeStatus {
    RuntimeStatus {
        installed: false,
        enabled: false,
        running: false,
        unit: "punktfunk-scripting",
        principal: None,
        detail: "the plugin runner is only available on Linux and Windows hosts".into(),
    }
}

#[cfg(target_os = "linux")]
fn run_systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .context("run systemctl (is systemd available in this session?)")?;
    if !status.success() {
        bail!(
            "systemctl --user {} failed — is the punktfunk-scripting package installed?",
            args.join(" ")
        );
    }
    Ok(())
}

/// Trimmed `systemctl --user` stdout, or `None` if it could not run. Queries exit
/// non-zero for a normal "inactive"/"disabled", so the text is the answer.
#[cfg(target_os = "linux")]
fn systemctl_output(args: &[&str]) -> Option<String> {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(target_os = "linux")]
pub(super) fn restart_runtime() -> Result<()> {
    run_systemctl(&["restart", UNIT])
}

#[cfg(not(target_os = "linux"))]
pub(super) fn enable() -> Result<()> {
    bail!("the plugin runner is only available on Linux and Windows hosts")
}

#[cfg(not(target_os = "linux"))]
pub(super) fn disable() -> Result<()> {
    bail!("the plugin runner is only available on Linux and Windows hosts")
}

#[cfg(not(target_os = "linux"))]
pub(super) fn restart_runtime() -> Result<()> {
    bail!("the plugin runner is only available on Linux and Windows hosts")
}

/// Whether the IPv4 listener on `127.0.0.1:port` (or `0.0.0.0:port`) is this user.
///
/// A registration outlives its plugin by up to the lease TTL, and another local
/// user who binds the freed port would otherwise receive the UI secret and
/// answer the launch. Fail closed if `/proc/net/tcp` cannot be read.
#[cfg(target_os = "linux")]
pub(crate) fn listener_is_runner(port: u16) -> bool {
    let Ok(table) = std::fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    uids_are_ours(&loopback_listen_uids(&table, port), euid())
}

#[cfg(target_os = "linux")]
fn uids_are_ours(uids: &[u32], me: u32) -> bool {
    !uids.is_empty() && uids.iter().all(|&u| u == me)
}

#[cfg(target_os = "linux")]
fn euid() -> u32 {
    // SAFETY: geteuid reads this thread's credentials and touches no memory.
    unsafe { libc::geteuid() }
}

/// UIDs of TCP_LISTEN sockets on `port` bound to 127.0.0.1 or 0.0.0.0.
#[cfg(target_os = "linux")]
fn loopback_listen_uids(table: &str, port: u16) -> Vec<u32> {
    let want = format!("{port:04X}");
    let mut uids = Vec::new();
    for line in table.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 8 {
            continue;
        }
        let Some((addr, p)) = cols[1].split_once(':') else {
            continue;
        };
        if !p.eq_ignore_ascii_case(&want) || !cols[3].eq_ignore_ascii_case("0A") {
            continue;
        }
        if !proc_ipv4_is_loopback_or_any(addr) {
            continue;
        }
        if let Ok(uid) = cols[7].parse::<u32>() {
            uids.push(uid);
        }
    }
    uids
}

/// `/proc/net/tcp` prints IPv4 addresses as native-endian hex of the in_addr.
#[cfg(target_os = "linux")]
fn proc_ipv4_is_loopback_or_any(hex: &str) -> bool {
    let Ok(n) = u32::from_str_radix(hex, 16) else {
        return false;
    };
    let ip = std::net::Ipv4Addr::from(n.to_le_bytes());
    ip.is_loopback() || ip.is_unspecified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn present(ps: Vec<PathBuf>) -> impl Fn(&Path) -> bool {
        move |p: &Path| ps.iter().any(|q| q == p)
    }

    /// Layouts the resolver must serve. Nix lands on `PATH` and nowhere else.
    #[test]
    fn runner_resolution_table() {
        let beside = Path::new("/opt/punktfunk/bin");
        let nix = Path::new("/run/current-system/sw/bin");
        let home = Path::new("/home/deck");

        let exists = present(vec![nix.join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(
                None,
                Some(beside),
                Some(nix.to_str().unwrap()),
                None,
                &exists
            ),
            Some((nix.join(RUNNER_BIN), Vec::new()))
        );

        let exists = present(vec![beside.join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(
                Some("/nix/store/abc/bin/punktfunk-scripting"),
                Some(beside),
                Some("/usr/bin"),
                Some(home),
                &exists
            ),
            Some(("/nix/store/abc/bin/punktfunk-scripting".into(), Vec::new()))
        );
        // Override is not existence-checked: a typo fails naming that path.
        assert_eq!(
            resolve_runner_in(Some("/nope/pf"), Some(beside), None, None, &exists),
            Some(("/nope/pf".into(), Vec::new()))
        );
        // Empty/whitespace is unset, not a path.
        assert_eq!(
            resolve_runner_in(Some("  "), Some(beside), None, None, &exists),
            Some((beside.join(RUNNER_BIN), Vec::new()))
        );
        let exists = present(vec![beside.join(RUNNER_BIN), nix.join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(
                None,
                Some(beside),
                Some(nix.to_str().unwrap()),
                None,
                &exists
            ),
            Some((beside.join(RUNNER_BIN), Vec::new()))
        );
        // PATH is walked entry by entry; empty entries skipped.
        let exists = present(vec![nix.join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(
                None,
                Some(Path::new("/nowhere")),
                Some(":/nope:/run/current-system/sw/bin"),
                None,
                &exists
            ),
            Some((nix.join(RUNNER_BIN), Vec::new()))
        );

        // Packaged wrapper even when the unit PATH omits `/usr/bin`.
        let exists = present(vec![PathBuf::from("/usr/bin").join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(
                None,
                Some(Path::new("/nowhere")),
                Some("/nope"),
                None,
                &exists
            ),
            Some((PathBuf::from("/usr/bin").join(RUNNER_BIN), Vec::new()))
        );
        // Private two-file layout when the wrapper is absent.
        let bun = PathBuf::from("/usr/lib").join(RUNNER_BIN).join("bun");
        let cli = PathBuf::from("/usr/share")
            .join(RUNNER_BIN)
            .join("runner-cli.js");
        let exists = present(vec![bun.clone(), cli.clone()]);
        assert_eq!(
            resolve_runner_in(None, None, None, None, &exists),
            Some((bun, vec![cli.to_string_lossy().into_owned()]))
        );
        // Half of that layout is not a rung — do not spawn bun with no script.
        let exists = present(vec![PathBuf::from("/usr/lib").join(RUNNER_BIN).join("bun")]);
        assert_eq!(resolve_runner_in(None, None, None, None, &exists), None);

        // SteamOS payload is user-scoped; reached only via HOME.
        let exists = present(vec![home.join(".local/bin").join(RUNNER_BIN)]);
        assert_eq!(
            resolve_runner_in(None, None, None, Some(home), &exists),
            Some((home.join(".local/bin").join(RUNNER_BIN), Vec::new()))
        );
        assert_eq!(resolve_runner_in(None, None, None, None, &exists), None);
        let bun = home.join(".local/lib").join(RUNNER_BIN).join("bun");
        let cli = home
            .join(".local/share")
            .join(RUNNER_BIN)
            .join("runner-cli.js");
        let exists = present(vec![bun.clone(), cli.clone()]);
        assert_eq!(
            resolve_runner_in(None, None, None, Some(home), &exists),
            Some((bun, vec![cli.to_string_lossy().into_owned()]))
        );

        let exists = present(vec![]);
        assert_eq!(
            resolve_runner_in(None, Some(beside), Some("/nope"), Some(home), &exists),
            None
        );
    }

    /// Miss text must name every install path, including NixOS (not only `apt`).
    #[test]
    fn the_missing_runner_error_names_every_platform_it_can_be_installed_on() {
        for hint in [
            "apt install",
            "steamdeck/install.sh",
            "NixOS",
            "PUNKTFUNK_SCRIPTING",
        ] {
            assert!(RUNNER_MISSING.contains(hint), "missing hint: {hint}");
        }
    }

    /// `/proc/net/tcp` listen rows: loopback/any count, established and foreign
    /// addresses do not, and a mixed-uid port is not ours.
    #[cfg(target_os = "linux")]
    #[test]
    fn loopback_listen_uids_read_the_listen_rows() {
        let table = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1
   1: 0100007F:1F90 0100007F:C001 01 00000000:00000000 00:00000000 00000000  1000        0 2
   2: 00000000:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 3
   3: 0200A8C0:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 4
   4: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 5
";
        let uids = loopback_listen_uids(table, 0x1F90);
        assert_eq!(uids, vec![1000, 0]);
        assert!(
            !uids_are_ours(&uids, 1000),
            "a foreign uid on the same port must fail the runner check"
        );
        assert!(uids_are_ours(&[1000], 1000));
        assert!(!uids_are_ours(&[], 1000));
        assert!(proc_ipv4_is_loopback_or_any("0100007F"));
        assert!(proc_ipv4_is_loopback_or_any("00000000"));
        assert!(!proc_ipv4_is_loopback_or_any("0200A8C0"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_loopback_listen_we_own_is_the_runner() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();
        assert!(
            listener_is_runner(port),
            "this process's loopback listener must count as the runner"
        );
        drop(listener);
        assert!(
            !listener_is_runner(port),
            "a port with no listener must not count as the runner"
        );
    }
}
