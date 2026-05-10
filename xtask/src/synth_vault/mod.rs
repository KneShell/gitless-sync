use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(crate) const DEFAULT_SEED: u64 = 42;
pub(crate) const DEFAULT_COUNT: usize = 1000;
pub(crate) const FIXED_MTIME_EPOCH_SECS: u64 = 1_735_689_600;
pub(crate) const MIN_FILE_BYTES: usize = 1024;
pub(crate) const MAX_FILE_BYTES: usize = 100 * 1024;

const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "labore",
    "magna",
    "aliqua",
];

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Args {
    pub(crate) out: PathBuf,
    pub(crate) count: usize,
    pub(crate) seed: u64,
}

#[derive(Debug)]
pub(crate) enum Error {
    MissingOut,
    InvalidArg(String),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOut => write!(f, "--out <path> is required"),
            Self::InvalidArg(s) => write!(f, "invalid argument: {s}"),
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

pub(crate) fn run(args: &[String]) -> Result<u8, Error> {
    let parsed = parse_args(args)?;
    generate(&parsed)?;
    println!(
        "synth-vault: generated {} files (seed={}) at {}",
        parsed.count,
        parsed.seed,
        parsed.out.display()
    );
    Ok(0)
}

pub(crate) fn parse_args(args: &[String]) -> Result<Args, Error> {
    let mut out: Option<PathBuf> = None;
    let mut count = DEFAULT_COUNT;
    let mut seed = DEFAULT_SEED;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                let val = iter
                    .next()
                    .ok_or_else(|| Error::InvalidArg("--out requires a value".into()))?;
                out = Some(PathBuf::from(val));
            }
            "--count" => {
                let val = iter
                    .next()
                    .ok_or_else(|| Error::InvalidArg("--count requires a value".into()))?;
                count = val
                    .parse()
                    .map_err(|_| Error::InvalidArg(format!("--count: {val}")))?;
            }
            "--seed" => {
                let val = iter
                    .next()
                    .ok_or_else(|| Error::InvalidArg("--seed requires a value".into()))?;
                seed = val
                    .parse()
                    .map_err(|_| Error::InvalidArg(format!("--seed: {val}")))?;
            }
            other => return Err(Error::InvalidArg(other.into())),
        }
    }
    let out = out.ok_or(Error::MissingOut)?;
    Ok(Args { out, count, seed })
}

pub(crate) fn generate(args: &Args) -> Result<(), Error> {
    fs::create_dir_all(&args.out)?;
    let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(FIXED_MTIME_EPOCH_SECS);
    let mut prng = Xorshift64::new(args.seed);
    for i in 0..args.count {
        let filename = format!("note-{i:05}.md");
        let path = args.out.join(&filename);
        let span = MAX_FILE_BYTES - MIN_FILE_BYTES + 1;
        let target_size = MIN_FILE_BYTES + bounded_usize(prng.next_u64(), span);
        let content = build_content(&filename, target_size, &mut prng);
        write_file(&path, &content, mtime)?;
    }
    Ok(())
}

pub(crate) fn bounded_usize(raw: u64, modulus: usize) -> usize {
    let m_u64 = u64::try_from(modulus).unwrap_or(u64::MAX).max(1);
    let r_u64 = raw % m_u64;
    usize::try_from(r_u64).unwrap_or(0)
}

fn write_file(path: &Path, content: &str, mtime: SystemTime) -> Result<(), Error> {
    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    file.set_modified(mtime)?;
    Ok(())
}

pub(crate) fn build_content(filename: &str, target_size: usize, prng: &mut Xorshift64) -> String {
    let mut s = String::with_capacity(target_size);
    s.push_str("# ");
    s.push_str(filename);
    s.push_str("\n\n");
    while s.len() < target_size {
        let words_per_line = 8 + bounded_usize(prng.next_u64(), 12);
        for i in 0..words_per_line {
            if i > 0 {
                s.push(' ');
            }
            let word_idx = bounded_usize(prng.next_u64(), WORDS.len());
            s.push_str(WORDS[word_idx]);
        }
        s.push('\n');
    }
    s
}

pub(crate) struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed | 0x1 }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

#[cfg(test)]
mod tests;
