//! `zpr-mcp install`: copies the binary to a standard folder as `zpr`, adds that folder to
//! the PATH and registers the MCP server in Zed's settings. Safe to run again (updates).

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const SERVER_NAME: &str = "github-pr-review";

fn home_dir() -> Result<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(variable)
        .map(PathBuf::from)
        .context("cannot find your home directory")
}

fn install_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        let base = env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        Ok(PathBuf::from(base).join("zpr"))
    } else {
        Ok(home_dir()?.join(".local/bin"))
    }
}

fn zed_settings_path() -> Result<PathBuf> {
    if cfg!(windows) {
        let base = env::var_os("APPDATA").context("APPDATA is not set")?;
        return Ok(PathBuf::from(base).join("Zed/settings.json"));
    }
    let config = match env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => home_dir()?.join(".config"),
    };
    Ok(config.join("zed/settings.json"))
}

/// Copies the running binary to `target`. An existing (possibly running) binary is renamed
/// first, because Windows refuses to overwrite an executable that is in use.
fn copy_binary(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        let old = target.with_extension("old");
        let _ = fs::remove_file(&old);
        fs::rename(target, &old).context("cannot replace the installed binary")?;
    }
    fs::copy(source, target).context("cannot copy the binary")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(target, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(target_os = "macos")]
    let _ = Command::new("xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(target)
        .output();
    Ok(())
}

/// Adds `dir` to the user's PATH. Returns true when a new terminal is needed to see it.
fn add_to_path(dir: &Path) -> Result<bool> {
    if env::var_os("ZPR_INSTALL_NO_PATH").is_some() {
        return Ok(false); // for tests and managed machines
    }
    if cfg!(windows) {
        let script = "$d=$env:ZPR_DIR; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
            if(-not $p){$p=''}; \
            if(($p -split ';') -notcontains $d){[Environment]::SetEnvironmentVariable('Path',(($p.TrimEnd(';')+';'+$d).TrimStart(';')),'User')}";
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("ZPR_DIR", dir)
            .status()
            .context("cannot run PowerShell to update the PATH")?;
        if !status.success() {
            bail!("PowerShell could not update the PATH");
        }
        return Ok(true);
    }
    let on_path = env::var_os("PATH").is_some_and(|path| env::split_paths(&path).any(|p| p == dir));
    if on_path {
        return Ok(false);
    }
    let shell = env::var("SHELL").unwrap_or_default();
    let profile = home_dir()?.join(if shell.ends_with("zsh") {
        ".zshrc"
    } else {
        ".bashrc"
    });
    let line = "export PATH=\"$HOME/.local/bin:$PATH\"";
    let current = fs::read_to_string(&profile).unwrap_or_default();
    if !current.contains(line) {
        fs::write(&profile, format!("{current}\n# Added by zpr\n{line}\n"))?;
    }
    Ok(true)
}

fn server_entry(command: &str) -> String {
    format!("\"{SERVER_NAME}\": {{ \"command\": \"{command}\", \"args\": [] }}")
}

/// Offset just after the first `{` of the file, skipping comments (Zed settings are JSONC).
fn first_object_start(text: &str) -> Result<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => return Ok(i + 1),
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b if b.is_ascii_whitespace() => i += 1,
            _ => bail!("settings.json does not start with an object"),
        }
    }
    bail!("settings.json has no object")
}

/// Adds the server to Zed's settings text, or updates its path if already present.
/// Works on the raw text so comments and formatting are preserved.
fn upsert_server(text: &str, command: &str) -> Result<String> {
    let command = command.replace('"', "\\\"");
    if text.trim().is_empty() {
        return Ok(format!(
            "{{\n  \"context_servers\": {{\n    {}\n  }}\n}}\n",
            server_entry(&command)
        ));
    }
    let manual = "edit settings.json by hand (see the README)";
    if let Some(name_at) = text.find(&format!("\"{SERVER_NAME}\"")) {
        let key = text[name_at..]
            .find("\"command\"")
            .with_context(|| format!("existing entry has no command; {manual}"))?;
        let after_key = name_at + key + "\"command\"".len();
        let open = text[after_key..].find('"').with_context(|| manual)? + after_key + 1;
        let close = text[open..].find('"').with_context(|| manual)? + open;
        return Ok(format!("{}{}{}", &text[..open], command, &text[close..]));
    }
    let existing_block = text
        .find("\"context_servers\"")
        .and_then(|at| text[at..].find('{').map(|offset| at + offset + 1));
    let (position, insertion) = match existing_block {
        Some(position) => (position, format!("\n    {},", server_entry(&command))),
        None => (
            first_object_start(text)?,
            format!(
                "\n  \"context_servers\": {{\n    {}\n  }},",
                server_entry(&command)
            ),
        ),
    };
    Ok(format!(
        "{}{}{}",
        &text[..position],
        insertion,
        &text[position..]
    ))
}

fn configure_zed(settings: &Path, binary: &Path) -> Result<()> {
    let command = binary.to_string_lossy().replace('\\', "/");
    let updated = if settings.exists() {
        let text = fs::read_to_string(&settings)?;
        let backup = settings.with_extension("json.zpr-backup");
        if !backup.exists() {
            fs::write(&backup, &text)?;
        }
        upsert_server(&text, &command)?
    } else {
        fs::create_dir_all(settings.parent().context("invalid settings path")?)?;
        upsert_server("", &command)?
    };
    fs::write(settings, updated)?;
    Ok(())
}

pub fn run() -> Result<()> {
    let source = env::current_exe()?;
    let dir = install_dir()?;
    fs::create_dir_all(&dir)?;
    let target = dir.join(if cfg!(windows) { "zpr.exe" } else { "zpr" });
    if fs::canonicalize(&source)? != fs::canonicalize(&target).unwrap_or_default() {
        copy_binary(&source, &target)?;
    }
    println!("Installed:   {}", target.display());

    let new_terminal = add_to_path(&dir)?;
    println!(
        "PATH:        {}",
        if new_terminal {
            "updated (open a new terminal)"
        } else {
            "already set"
        }
    );

    let settings = zed_settings_path()?;
    match configure_zed(&settings, &target) {
        Ok(()) => println!(
            "Zed:         registered `{SERVER_NAME}` in {}",
            settings.display()
        ),
        Err(e) => println!("Zed:         not configured automatically ({e:#}); see the README"),
    }
    println!("\nDone. Open a new terminal and run `zpr` inside a GitHub project.\nIn Zed, check Agent panel > settings that `{SERVER_NAME}` is enabled.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_settings_for_an_empty_file() {
        let out = upsert_server("  \n", "C:/zpr/zpr.exe").unwrap();
        assert!(out.contains("\"context_servers\"") && out.contains("C:/zpr/zpr.exe"));
    }

    #[test]
    fn inserts_after_the_first_brace_and_skips_comments() {
        let text = "// my {settings}\n{\n  \"theme\": \"One\"\n}\n";
        let out = upsert_server(text, "/bin/zpr").unwrap();
        assert!(out.starts_with("// my {settings}\n{"));
        assert_eq!(out.matches("\"context_servers\"").count(), 1);
        assert!(out.contains("\"theme\": \"One\""));
    }

    #[test]
    fn adds_to_an_existing_context_servers_block() {
        let text = "{ \"context_servers\": { \"other\": { \"command\": \"x\" } } }";
        let out = upsert_server(text, "/bin/zpr").unwrap();
        let (block, entry, other) = (
            out.find("\"context_servers\"").unwrap(),
            out.find(SERVER_NAME).unwrap(),
            out.find("\"other\"").unwrap(),
        );
        assert!(block < entry && entry < other);
    }

    #[test]
    fn installs_and_reinstalls_in_a_temp_folder_keeping_a_backup() {
        let root = env::temp_dir().join(format!("zpr-install-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let (source, target) = (root.join("download.bin"), root.join("zpr.bin"));
        let settings = root.join("zed/settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(&settings, "// keep me\n{ \"theme\": \"One\" }\n").unwrap();

        fs::write(&source, "v1").unwrap();
        copy_binary(&source, &target).unwrap();
        configure_zed(&settings, &target).unwrap();
        fs::write(&source, "v2").unwrap();
        copy_binary(&source, &target).unwrap();
        configure_zed(&settings, &target).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "v2");
        assert_eq!(
            fs::read_to_string(target.with_extension("old")).unwrap(),
            "v1"
        );
        let text = fs::read_to_string(&settings).unwrap();
        assert!(text.starts_with("// keep me") && text.contains("\"theme\": \"One\""));
        assert_eq!(text.matches(SERVER_NAME).count(), 1);
        let backup = fs::read_to_string(settings.with_extension("json.zpr-backup")).unwrap();
        assert_eq!(backup, "// keep me\n{ \"theme\": \"One\" }\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn updates_the_path_of_an_existing_entry() {
        let text = "{ \"context_servers\": { \"github-pr-review\": { \"command\": \"C:/old.exe\", \"args\": [] } } }";
        let out = upsert_server(text, "C:/new/zpr.exe").unwrap();
        assert!(out.contains("C:/new/zpr.exe") && !out.contains("old.exe"));
        assert_eq!(out.matches(SERVER_NAME).count(), 1);
    }
}
