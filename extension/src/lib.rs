use zed_extension_api::{self as zed, Result};

const RELEASE_REPO: &str = "mohamedehn/zed-github-prs";

struct GithubPrReview {
    cached_binary: Option<String>,
}

impl GithubPrReview {
    /// Returns the path of the `zpr-mcp` binary, downloading the latest release asset once.
    fn binary_path(&mut self) -> Result<String> {
        if let Some(path) = &self.cached_binary {
            if std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
                return Ok(path.clone());
            }
        }

        let (os, arch) = zed::current_platform();
        let os_name = match os {
            zed::Os::Mac => "macos",
            zed::Os::Linux => "linux",
            zed::Os::Windows => "windows",
        };
        let arch_name = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::X86 => return Err("32-bit x86 is not supported".into()),
        };
        let extension = if os == zed::Os::Windows { ".exe" } else { "" };
        let asset_name = format!("zpr-mcp-{os_name}-{arch_name}{extension}");

        let release = zed::latest_github_release(
            RELEASE_REPO,
            zed::GithubReleaseOptions { require_assets: true, pre_release: false },
        )?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("release {} has no asset {asset_name}", release.version))?;

        let path = format!("zpr-mcp-{}/{asset_name}", release.version);
        if !std::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            zed::download_file(&asset.download_url, &path, zed::DownloadedFileType::Uncompressed)
                .map_err(|e| format!("failed to download {asset_name}: {e}"))?;
            zed::make_file_executable(&path)?;
        }

        self.cached_binary = Some(path.clone());
        Ok(path)
    }
}

impl zed::Extension for GithubPrReview {
    fn new() -> Self {
        Self { cached_binary: None }
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &zed::ContextServerId,
        _project: &zed::Project,
    ) -> Result<zed::Command> {
        Ok(zed::Command { command: self.binary_path()?, args: vec![], env: vec![] })
    }
}

zed::register_extension!(GithubPrReview);
