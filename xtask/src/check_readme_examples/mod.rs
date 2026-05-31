use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod parser;

use parser::{BINARY_NAME, INIT_SUBCOMMAND, ParsedCommand, collect_init_commands};

pub(crate) const README_FILENAME: &str = "README.md";
pub(crate) const TEMP_DIR_PREFIX: &str = "gitless-sync-readme-";

#[derive(Debug)]
pub(crate) enum Error {
    ReadmeRead(io::Error),
    QuickStartMissing,
    NoInitCommand,
    BuildFailed {
        stderr: String,
    },
    BinaryMissing(PathBuf),
    CommandFailed {
        command: String,
        stderr: String,
        status: Option<i32>,
    },
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadmeRead(e) => write!(f, "failed to read README.md: {e}"),
            Self::QuickStartMissing => {
                write!(f, "no `## Quick Start` section with ```sh block found")
            }
            Self::NoInitCommand => write!(
                f,
                "Quick Start contained no `{BINARY_NAME} {INIT_SUBCOMMAND}` line to execute"
            ),
            Self::BuildFailed { stderr } => {
                write!(f, "`cargo build --release` failed: {stderr}")
            }
            Self::BinaryMissing(path) => write!(
                f,
                "release binary not found at {} (build did not produce expected artifact)",
                path.display()
            ),
            Self::CommandFailed {
                command,
                stderr,
                status,
            } => write!(f, "command `{command}` exited with {status:?}: {stderr}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

pub(crate) fn binary_path(root: &Path) -> PathBuf {
    root.join("target")
        .join("release")
        .join(format!("{BINARY_NAME}{}", std::env::consts::EXE_SUFFIX))
}

pub(crate) fn cargo_build_release() -> Result<(), Error> {
    let args: [&OsStr; 4] = [
        OsStr::new("build"),
        OsStr::new("--release"),
        OsStr::new("--package"),
        OsStr::new(BINARY_NAME),
    ];
    let output = Command::new("cargo")
        .args(args)
        .output()
        .map_err(Error::Io)?;
    let Output { status, stderr, .. } = output;
    if !status.success() {
        return Err(Error::BuildFailed {
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    Ok(())
}

pub(crate) fn make_temp_dir() -> Result<PathBuf, Error> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut dir = std::env::temp_dir();
    dir.push(format!("{TEMP_DIR_PREFIX}{pid}-{nanos}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

// On Linux, exec'ing a freshly written/built executable can transiently fail
// with ETXTBSY ("Text file busy") when another thread's concurrent `fork()`
// (e.g. a parallel test or build spawning a process) still holds a write file
// descriptor to it. `std::io::ErrorKind::ExecutableFileBusy` surfaces this.
// Retry with a short linear backoff so the rare race resolves instead of
// failing the run; the happy path returns on the first attempt with no sleep.
const BUSY_MAX_RETRIES: u32 = 12;
const BUSY_BACKOFF_MS: u64 = 50;

fn run_capturing_with_busy_retry(binary: &Path, args: &[&str]) -> Result<Output, Error> {
    let mut attempt: u32 = 0;
    loop {
        match Command::new(binary).args(args).output() {
            Ok(output) => return Ok(output),
            Err(e)
                if e.kind() == io::ErrorKind::ExecutableFileBusy
                    && attempt < BUSY_MAX_RETRIES =>
            {
                attempt += 1;
                thread::sleep(Duration::from_millis(BUSY_BACKOFF_MS * u64::from(attempt)));
            }
            Err(e) => return Err(Error::Io(e)),
        }
    }
}

pub(crate) fn execute_init(
    binary: &Path,
    cmd: &ParsedCommand,
    redirect_dir: &Path,
) -> Result<(), Error> {
    let cli_args: Vec<&str> = cmd.args.iter().skip(1).map(String::as_str).collect();
    let Output {
        status,
        stdout,
        stderr,
    } = run_capturing_with_busy_retry(binary, &cli_args)?;
    if !status.success() {
        return Err(Error::CommandFailed {
            command: format!("{} {}", binary.display(), cli_args.join(" ")),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            status: status.code(),
        });
    }
    if let Some(target) = cmd.redirect_to.as_deref() {
        let target_path = redirect_dir.join(target);
        let mut file = File::create(&target_path)?;
        file.write_all(&stdout)?;
        file.sync_all()?;
    }
    Ok(())
}

pub(crate) fn run() -> Result<u8, Error> {
    let root = workspace_root();
    let readme_path = root.join(README_FILENAME);
    let readme = fs::read_to_string(&readme_path).map_err(Error::ReadmeRead)?;
    if parser::extract_quick_start_sh_blocks(&readme).is_empty() {
        return Err(Error::QuickStartMissing);
    }
    let init_commands = collect_init_commands(&readme);
    if init_commands.is_empty() {
        return Err(Error::NoInitCommand);
    }

    println!("check-readme-examples: building gitless-sync (release)...");
    cargo_build_release()?;

    let binary = binary_path(&root);
    if !binary.exists() {
        return Err(Error::BinaryMissing(binary));
    }
    let redirect_dir = make_temp_dir()?;

    for cmd in &init_commands {
        let arg_tail = cmd
            .args
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "check-readme-examples: running `{} {arg_tail}` (redirect: {:?})",
            binary.display(),
            cmd.redirect_to,
        );
        execute_init(&binary, cmd, &redirect_dir)?;
    }
    println!(
        "check-readme-examples: {} init line(s) OK (tempdir: {}).",
        init_commands.len(),
        redirect_dir.display()
    );
    Ok(0)
}

#[cfg(test)]
mod tests;
