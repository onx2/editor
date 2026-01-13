use anyhow::{Context, Result, anyhow, bail};
use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Usage:
///   cargo run -p xtask -- spacetime publish-generate
///
/// What it does:
/// - Temporarily edits `backend/Cargo.toml` to set `[lib].crate-type = ["cdylib"]`
/// - Runs:
///     spacetime publish -s local -p ./backend editor --delete-data
///     spacetime generate --lang rust -p ./backend -o ./client/src/module_bindings
/// - Restores `backend/Cargo.toml` back to its original contents (even on failure)
///
/// Notes:
/// - This avoids manually flipping `crate-type` in source control.
/// - This uses `toml_edit` to modify Cargo.toml as a real TOML document.
fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_usage();
        bail!("missing command");
    };

    match cmd.as_str() {
        "spacetime" => {
            let Some(sub) = args.next() else {
                print_usage();
                bail!("missing subcommand for `spacetime`");
            };
            match sub.as_str() {
                "publish-generate" => spacetime_publish_generate(),
                other => {
                    print_usage();
                    bail!("unknown `spacetime` subcommand: {other}");
                }
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            print_usage();
            bail!("unknown command: {other}");
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"xtask

Usage:
  cargo run -p xtask -- spacetime publish-generate

Commands:
  spacetime publish-generate   Temporarily set backend crate-type=cdylib, then run spacetime publish + generate
"#
    );
}

/// The project root is the directory containing this workspace's `Cargo.toml`.
/// We assume `xtask` lives at `<root>/xtask`, so root is its parent directory.
fn project_root() -> Result<PathBuf> {
    let exe = env::current_exe().context("failed to locate current executable")?;
    // Typically: <root>/target/.../xtask[.exe]
    // Go up until we find `<root>/Cargo.toml` (workspace).
    // Keep it simple: assume `<root>/xtask` exists and is a parent of the executable.
    // The robust search is: ascend and find Cargo.toml containing [workspace].
    let mut dir = exe
        .parent()
        .ok_or_else(|| anyhow!("current_exe has no parent directory"))?
        .to_path_buf();

    for _ in 0..12 {
        // Heuristic: check for `<dir>/Cargo.toml` and `<dir>/backend` and `<dir>/client`
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file() && dir.join("backend").is_dir() && dir.join("client").is_dir() {
            return Ok(dir);
        }
        let Some(parent) = dir.parent() else { break };
        dir = parent.to_path_buf();
    }

    bail!(
        "could not determine project root; expected to find Cargo.toml with backend/ and client/ siblings"
    );
}

/// RAII guard that restores a file to its original contents when dropped.
struct RestoreFile {
    path: PathBuf,
    original: String,
    restored: bool,
}

impl RestoreFile {
    fn capture(path: PathBuf) -> Result<Self> {
        let original = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(Self {
            path,
            original,
            restored: false,
        })
    }

    fn write(&self, contents: &str) -> Result<()> {
        fs::write(&self.path, contents)
            .with_context(|| format!("failed to write {}", self.path.display()))
    }

    fn restore(mut self) -> Result<()> {
        self.write(&self.original)?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for RestoreFile {
    fn drop(&mut self) {
        if self.restored {
            return;
        }
        // Best-effort restore. We can't bubble errors in Drop.
        let _ = fs::write(&self.path, &self.original);
    }
}

fn spacetime_publish_generate() -> Result<()> {
    let root = project_root()?;
    let backend_dir = root.join("backend");
    let backend_cargo_toml = backend_dir.join("Cargo.toml");

    // Capture original file so we can restore it even if publish/generate fails.
    let restore_guard = RestoreFile::capture(backend_cargo_toml.clone())?;

    // Patch the Cargo.toml `[lib].crate-type` to ["cdylib"].
    let patched = set_backend_crate_type_cdylib(&restore_guard.original)
        .context("failed to patch backend/Cargo.toml to crate-type=[\"cdylib\"]")?;
    restore_guard.write(&patched)?;

    // Run both commands (in order).
    run_spacetime_publish(&root)?;
    run_spacetime_generate(&root)?;

    // Restore original file contents.
    restore_guard.restore()?;

    Ok(())
}

fn set_backend_crate_type_cdylib(original: &str) -> Result<String> {
    let mut doc = original
        .parse::<toml_edit::DocumentMut>()
        .context("failed to parse backend/Cargo.toml as TOML")?;

    // Ensure [lib] exists.
    if !doc.as_table().contains_key("lib") {
        doc["lib"] = toml_edit::table();
    }

    // Set crate-type = ["cdylib"]
    let mut arr = toml_edit::Array::default();
    arr.push("cdylib");

    doc["lib"]["crate-type"] = toml_edit::value(arr);

    Ok(doc.to_string())
}

fn run_spacetime_publish(project_root: &Path) -> Result<()> {
    // spacetime publish -s local -p ./backend editor --delete-data
    let mut cmd = Command::new(spacetime_exe()?);
    cmd.current_dir(project_root).args([
        "publish",
        "-s",
        "local",
        "-p",
        "./backend",
        "editor",
        "--delete-data",
    ]);
    run_checked(cmd, "spacetime publish")
}

fn run_spacetime_generate(project_root: &Path) -> Result<()> {
    // spacetime generate --lang rust -p ./backend -o ./client/src/module_bindings
    let mut cmd = Command::new(spacetime_exe()?);
    cmd.current_dir(project_root).args([
        "generate",
        "--lang",
        "rust",
        "-p",
        "./backend",
        "-o",
        "./client/src/module_bindings",
    ]);
    run_checked(cmd, "spacetime generate")
}

fn spacetime_exe() -> Result<PathBuf> {
    // Just rely on PATH. Return "spacetime" as a PathBuf.
    Ok(PathBuf::from("spacetime"))
}

fn run_checked(mut cmd: Command, label: &str) -> Result<()> {
    // Forward output so the user sees the prompts and progress.
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn `{label}`"))?;

    if !status.success() {
        bail!("{label} failed with exit code: {:?}", status.code());
    }
    Ok(())
}

// Small helper to avoid platform-specific string conversions
fn _os(s: &str) -> &OsStr {
    OsStr::new(s)
}
