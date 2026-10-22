//! Looks for a newer release on GitHub. On Windows, an installed copy can
//! download the new MSI and run it; elsewhere the release page is opened.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const REPO_URL: &str = "https://github.com/CypherNaught-0x/CTRL-Desk";
const LATEST_URL: &str = "https://api.github.com/repos/CypherNaught-0x/CTRL-Desk/releases/latest";
const USER_AGENT: &str = concat!("CTRL-Desk/", env!("CARGO_PKG_VERSION"));
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Upper bound for the installer download.
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the leading "v", e.g. "0.2.0".
    pub version: String,
    pub page: String,
    /// Download URL of the Windows installer, if the release has one.
    pub msi: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(Release),
    /// The installer is running; the app should exit so it can replace it.
    Installing,
    Failed(String),
}

pub struct Updater {
    status: Arc<Mutex<Status>>,
    ctx: egui::Context,
    last_check: Option<Instant>,
}

impl Updater {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            status: Arc::new(Mutex::new(Status::Idle)),
            ctx,
            last_check: None,
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Checks on start and then once a day, if enabled.
    pub fn tick(&mut self, enabled: bool) {
        if enabled && self.last_check.is_none_or(|t| t.elapsed() >= CHECK_EVERY) {
            self.check();
        }
    }

    pub fn check(&mut self) {
        if matches!(
            self.status(),
            Status::Checking | Status::Downloading(_) | Status::Installing
        ) {
            return;
        }
        self.last_check = Some(Instant::now());
        self.run(Status::Checking, || {
            Ok(match latest()? {
                Some(r) => Status::Available(r),
                None => Status::UpToDate,
            })
        });
    }

    /// Whether [`Self::install`] can update in place, rather than only
    /// pointing at the release page.
    pub fn can_install(release: &Release) -> bool {
        release.msi.is_some() && installed_by_msi()
    }

    pub fn install(&mut self, release: Release) {
        match release.msi.clone() {
            Some(url) if installed_by_msi() => {
                self.run(Status::Downloading(release.clone()), move || {
                    run_installer(&url)
                        .map(|()| Status::Installing)
                        .map_err(|e| format!("Couldn't install the update: {e}"))
                });
            }
            _ => self.ctx.open_url(egui::OpenUrl::new_tab(&release.page)),
        }
    }

    /// Sets `status`, then runs `job` on a background thread and stores its result.
    fn run(&self, status: Status, job: impl FnOnce() -> Result<Status, String> + Send + 'static) {
        *self.status.lock().unwrap() = status;
        let (shared, ctx) = (self.status.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            *shared.lock().unwrap() = job().unwrap_or_else(Status::Failed);
            ctx.request_repaint();
        });
    }
}

/// The latest release, if it is newer than this build.
fn latest() -> Result<Option<Release>, String> {
    let body = ureq::get(LATEST_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| format!("Couldn't check for updates: {e}"))?;
    let release = parse_release(&body).ok_or("Couldn't read the release information")?;
    Ok(is_newer(&release.version, CURRENT).then_some(release))
}

fn parse_release(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = v["tag_name"].as_str()?;
    let msi = v["assets"].as_array().and_then(|assets| {
        assets.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            name.ends_with(".msi")
                .then(|| a["browser_download_url"].as_str().map(String::from))?
        })
    });
    Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        page: v["html_url"].as_str().unwrap_or(REPO_URL).to_string(),
        msi,
    })
}

/// Compares dotted version numbers, ignoring any pre-release suffix.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split(['-', '+'])
            .next()
            .unwrap_or_default()
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parse(candidate) > parse(current)
}

/// The MSI installs into `%LOCALAPPDATA%\Programs\CTRL Desk`.
fn installed_by_msi() -> bool {
    if !cfg!(windows) {
        return false;
    }
    let (Ok(exe), Some(local)) = (std::env::current_exe(), dirs::data_local_dir()) else {
        return false;
    };
    exe.parent() == Some(local.join("Programs").join("CTRL Desk").as_path())
}

/// Downloads the MSI and starts it. It upgrades in place and starts the app
/// again when done.
fn run_installer(url: &str) -> Result<(), String> {
    let name = url.rsplit('/').next().filter(|n| n.ends_with(".msi"));
    let path = std::env::temp_dir().join(name.unwrap_or("ctrl-desk-update.msi"));
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .reader();
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    drop(file);
    std::process::Command::new("msiexec")
        .arg("/i")
        .arg(&path)
        .args(["/passive", "/norestart", "LAUNCHAPP=1"])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.1.1-beta", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn parses_release() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/CypherNaught-0x/CTRL-Desk/releases/tag/v0.2.0",
            "assets": [
                {"name": "desk-ctrl-0.2.0-x86_64-pc-windows-msvc.zip", "browser_download_url": "https://x/a.zip"},
                {"name": "desk-ctrl-0.2.0-x86_64-pc-windows-msvc.msi", "browser_download_url": "https://x/a.msi"}
            ]
        }"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert!(r.page.ends_with("/v0.2.0"));
        assert_eq!(r.msi.as_deref(), Some("https://x/a.msi"));
        assert_eq!(
            parse_release(r#"{"tag_name": "v1.0.0"}"#).unwrap().msi,
            None
        );
    }
}
