use std::{
    cmp::Ordering,
    collections::BTreeSet,
    convert::AsRef,
    fmt::{Debug, Display},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use log::warn;
use regex::Regex;
use toml::{Table, Value};
use walkdir::{DirEntry, WalkDir};

pub const RESET: &str = "\x1B[0m";
pub const BLACK: &str = "\x1B[0;30m"; // Black
pub const RED: &str = "\x1B[0;31m"; // Red
pub const GREEN: &str = "\x1B[0;32m"; // Green
pub const YELLOW: &str = "\x1B[0;33m"; // Yellow
pub const BLUE: &str = "\x1B[0;34m"; // Blue
pub const PURPLE: &str = "\x1B[0;35m"; // Purple
pub const CYAN: &str = "\x1B[0;36m"; // Cyan
pub const WHITE: &str = "\x1B[0;37m"; // White

/// Move cusor up a line, erase it, and go to beginning
pub const ERASE: &str = "\x1b[1A\x1b[2K";

fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::parse();

    let projs = Projects::<PathBuf>::new(&args.directory, &args)?;
    projs.list();
    Ok(())
}

/// A project is uniquely identified by the path to its Cargo.toml. Note that
/// the path stored in self.0 includes the `Cargo.toml` at the end.
pub struct Project<P: AsRef<Path>> {
    path: P,
    name: Name,
    size: Option<u64>,
}

/// Name of a project. `Explicit` corresponds to a name in the package.name field
/// of a Cargo.toml, which the `Inferred` name is the name of the parent directory
/// of the Cargo.toml. This is used when no package.name field exists
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
enum Name {
    Explicit(String),
    Inferred(String),
}

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Name::Explicit(name) => f.write_str(name),
            Name::Inferred(name) => f.write_fmt(format_args!("[{name}]")),
        }
    }
}

impl<P> Project<P>
where
    P: AsRef<Path>,
{
    pub fn new(path: P) -> Result<Self> {
        // Make sure it's a valid Cargo.toml
        match path.as_ref().file_name() {
            Some(name) => {
                if name.to_str() != Some("Cargo.toml") {
                    bail!("{:?} is not a Cargo.toml file", path.as_ref())
                }
            }
            None => bail!("{:?} is not a Cargo.toml file", path.as_ref()),
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", &path.as_ref()))?;

        let toml = contents
            .parse::<Table>()
            .with_context(|| format!("Failed to parse {:?}", path.as_ref()))?;

        let name = toml
            .get("package")
            .map(Value::as_table)
            .flatten()
            .map(|pack| pack.get("name"))
            .flatten()
            .map(Value::as_str)
            .flatten()
            .map(|name| Name::Explicit(name.to_string()))
            .unwrap_or(Name::Inferred(
                path.as_ref()
                    .parent()
                    .ok_or(anyhow!(
                        "Failed to find name field or parent directory for {:?}",
                        path.as_ref()
                    ))?
                    .to_str()
                    .ok_or(anyhow!(
                        "Failed to convert path {:?} to string",
                        path.as_ref()
                    ))?
                    .to_string(),
            ));

        // Get the size
        let mut initial = Project {
            path,
            name,
            size: None,
        };
        initial.size = Some(initial.dirsize()?);

        return Ok(initial);
    }

    pub fn dirsize(&self) -> Result<u64> {
        // Get path to target/ dir
        let mut target = self.path.as_ref().parent().unwrap().to_owned();
        target.push("target/");

        match target.try_exists() {
            Ok(true) => (),
            Ok(false) => return Ok(0),
            Err(e) => {
                return Result::Err(anyhow::Error::from(e))
                    .context("failed to access target directory")
            }
        }

        Ok(WalkDir::new(target)
            .into_iter()
            .filter_map(|e| e.ok())
            .fold(0, |acc, item| acc + item.metadata().unwrap().len()))
    }
}

impl<P> Debug for Project<P>
where
    P: AsRef<Path>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("path", &self.path.as_ref())
            .field("name", &self.name as &dyn Debug)
            .field("size", &self.size as &dyn Debug)
            .finish()
    }
}

impl<P> PartialOrd for Project<P>
where
    P: AsRef<Path>,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match Some(self.size.cmp(&other.size)) {
            Some(ord) => match ord {
                Ordering::Less => Some(ord),
                Ordering::Equal => Some(self.path.as_ref().cmp(other.path.as_ref())),
                Ordering::Greater => Some(ord),
            },
            None => None,
        }
    }
}

impl<P> Ord for Project<P>
where
    P: AsRef<Path>,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.partial_cmp(other) {
            Some(order) => order,
            None => unreachable!("partial_cmp should always succed"),
        }
    }
}

impl<P> PartialEq for Project<P>
where
    P: AsRef<Path>,
{
    fn eq(&self, other: &Self) -> bool {
        self.path.as_ref() == other.path.as_ref()
    }
}

impl<P> Eq for Project<P> where P: AsRef<Path> {}

#[derive(Debug)]
struct Projects<P: AsRef<Path>> {
    included: BTreeSet<Project<P>>,
    ignored: BTreeSet<Project<P>>,
}

impl<P> Projects<P>
where
    P: AsRef<Path>,
{
    // TODO: make this just take the config?
    pub fn new(path: impl AsRef<Path>, config: &Cli) -> Result<Projects<PathBuf>> {
        // TODO: do the bar
        let re = if let Some(re) = &config.exclude {
            Regex::new(&re).unwrap()
        } else {
            // this is unmatchable
            Regex::new(r"\b\B").unwrap()
        };

        let mut matches = BTreeSet::new();
        let mut unmatched = BTreeSet::new();

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| !is_hidden(e) || config.hidden)
            .filter_map(|e| match e {
                Ok(e) => Some(e),
                Err(e) => {
                    warn!("WalkDir error: {e}");
                    None
                }
            })
            .filter(|d| d.file_name().to_str() == Some("Cargo.toml"))
        {
            let project = Project::new(entry.path().to_owned()).unwrap();

            if atty::is(atty::Stream::Stdout) {
                println!(
                    "{ERASE}{} <- {}",
                    to_memory_string(project.size.unwrap_or(0)),
                    project.path.parent().unwrap().to_str().unwrap()
                );
            }

            if re.find(project.path.to_str().unwrap()).is_some() {
                matches.insert(project);
            } else {
                unmatched.insert(project);
            }

            wait(15);
        }
        if config.invert {
            Ok(Projects {
                included: matches,
                ignored: unmatched,
            })
        } else {
            Ok(Projects {
                included: unmatched,
                ignored: matches,
            })
        }
    }

    pub fn list(&self) {
        let mut stdout = io::stdout().lock();
        let (count, sum) = self
            .included
            .iter()
            .inspect(|project| {
                wait(15);
                let _ = write!(
                    stdout,
                    "{} <- {}\n",
                    to_memory_string(project.size.unwrap_or(0)),
                    project.path.as_ref().parent().unwrap().to_str().unwrap()
                );
            })
            .map(|project| project.size)
            .fold((0, 0), |(count, sum), size| {
                (count + 1, sum + size.unwrap_or(0))
            });
        println!("Summary: {count} projects, {}", to_memory_string(sum));
        println!("Ignored:");
        let (count, sum) = self
            .ignored
            .iter()
            .inspect(|project| {
                wait(15);
                let _ = write!(
                    stdout,
                    "{} <- {}\n",
                    to_memory_string(project.size.unwrap_or(0)),
                    project.path.as_ref().parent().unwrap().to_str().unwrap()
                );
            })
            .map(|project| project.size)
            .fold((0, 0), |(count, sum), size| {
                (count + 1, sum + size.unwrap_or(0))
            });
        println!("Summary: {count} projects, {}", to_memory_string(sum));
    }
}

macro_rules! color {
    ($color:ident, $($args:expr),* $(,)?) => {
        format!("{}{}{RESET}", $color, format!($($args),*))
    };
}

fn to_memory_string(bytes: u64) -> String {
    match bytes {
        1_000_000_000.. => {
            color!(RED, "{:3} GB", bytes / 1_000_000_000)
        }
        1_000_000.. => {
            color!(BLUE, "{:3} MB", bytes / 1_000_000)
        }
        1_000.. => {
            color!(GREEN, "{:3} KB", bytes / 1_000)
        }
        _ => {
            // One extra space between the letters and B because the other units
            // have G/M/B
            format!("{bytes:3}  B")
        }
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}
fn wait(millis: u64) {
    thread::sleep(Duration::from_millis(millis));
}

#[derive(Parser, Debug)]
#[command(about = "Manage rustc build artifacts")]
struct Cli {
    #[command(subcommand)]
    action: Option<Action>,

    #[arg(short, long)]
    #[arg(default_value_t = String::from(env!("HOME")))]
    directory: String,

    #[arg(short, long)]
    list: bool,

    #[arg(short, long)]
    exclude: Option<String>,

    #[arg(short = 'v', long, requires = "exclude")]
    invert: bool,

    #[arg(short = 'H', long)]
    hidden: bool,

    #[arg(short, long)]
    yes: bool,
}

#[derive(Parser, Debug, Clone)]
enum Action {
    Profile,
}

trait Vanish {
    fn list(&self, config: Cli);
    fn clean(&self, config: Cli);
}

impl<P> Vanish for BTreeSet<Project<P>>
where
    P: AsRef<Path>,
{
    fn list(&self, config: Cli) {
        todo!()
    }

    fn clean(&self, config: Cli) {
        todo!()
    }
}
