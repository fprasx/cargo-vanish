// FIXME: fix all let _

use std::{
    collections::BTreeSet,
    io,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Result;
use cargo_vanish::{
    consts::{ERASE, GREEN, RESET},
    erase, is_hidden, output, print,
    project::Project,
    to_memory_string, wait,
};
use clap::Parser;
use log::warn;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::parse();

    let projs = Projects::new(&args.directory, &args)?;
    if args.list {
        projs.list(&args)
    } else {
        projs.clean(&args);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct Projects {
    included: BTreeSet<Project>,
    ignored: BTreeSet<Project>,
}

impl Projects {
    // TODO: make this just take the config?
    pub fn new(path: impl AsRef<Path>, config: &Cli) -> Result<Projects> {
        // TODO: do the bar
        let re = if let Some(re) = &config.exclude {
            Regex::new(&re).unwrap()
        } else {
            // this is unmatchable
            Regex::new(r"\b\B").unwrap()
        };

        let mut matches = BTreeSet::new();
        let mut unmatched = BTreeSet::new();

        if atty::is(atty::Stream::Stdout) && !config.json {
            // Extra newline gets eaten by first erase
            output!("Searching for projects:\n");
        }

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

            if atty::is(atty::Stream::Stdout) && !config.json {
                // Erase before so that project remains displayed until next
                // one is ready
                print(ERASE);
                output!(
                    "{}",
                    project // "{} {}",
                            // to_memory_string(project.size),
                            // project.path.parent().unwrap().to_str().unwrap()
                );
            }

            if re.find(project.path().to_str().unwrap()).is_some() {
                matches.insert(project);
            } else {
                unmatched.insert(project);
            }

            wait(15);
        }

        if atty::is(atty::Stream::Stdout) && !config.json {
            // Final erase for last item
            print(ERASE);
            // Erase "searching for projects"
            print(ERASE);
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

    pub fn list(&self, config: &Cli) {
        if config.json {
            println!("{}", serde_json::to_string_pretty(&self).unwrap());
            return;
        }
        if !self.included.is_empty() {
            self.included.list();
        }
        if !self.ignored.is_empty() && config.ignored {
            output!("Ignored:");
            self.ignored.list();
        }
    }

    pub fn clean(&self, config: &Cli) {
        self.included.list();
        if !config.yes {
            output!("Are you sure you would like to clean these projects? [y/n]:");
            let mut buf = String::new();
            loop {
                print(&format!("{GREEN}> {RESET}"));
                buf.clear();
                while let Err(e) = io::stdin().read_line(&mut buf) {
                    warn!("Error reading from stdin: {e}")
                }
                if ["n", "no"]
                    .iter()
                    .any(|response| response == &&*buf.trim().to_lowercase())
                {
                    output!("Aborting.");
                    return;
                } else if ["y", "yes"]
                    .iter()
                    .any(|response| response == &&*buf.trim().to_lowercase())
                {
                    break;
                } else {
                    output!("Unknown response. Please try again.")
                }
            }
        }
        self.included.clean();
        if config.ignored {
            output!("Ignored:");
            self.ignored.list();
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Search for projects in the given directory. Defaults to $HOME
    #[arg(short, long)]
    #[arg(default_value_t = String::from(env!("HOME")))]
    directory: String,

    /// List directories that would be cleaned instead of cleaning them
    #[arg(short, long)]
    list: bool,

    /// Exlude directories that match the provided regular expression
    #[arg(short, long)]
    exclude: Option<String>,

    /// When used with -e/--exclude, ignore directories that match regex
    #[arg(short = 'v', long, requires = "exclude")]
    invert: bool,

    /// Search hidden directories for projects
    #[arg(short = 'H', long)]
    hidden: bool,

    /// Don't ask for confirmation when cleaning directories
    #[arg(short, long)]
    yes: bool,

    /// When listing, output list as json
    #[arg(short, long, requires = "list")]
    json: bool,

    /// List projects which were ignored
    #[arg(short, long, requires = "exclude")]
    ignored: bool,
}

trait Vanish {
    fn list(&self);
    fn clean(&self);
}

impl Vanish for BTreeSet<Project> {
    fn list(&self) {
        let size = self.len();
        let total: u64 = self.iter().map(|p| p.size().unwrap_or(0)).sum();
        for project in self {
            let mut stdout = io::stdout().lock();
            wait(15);
            let _ = output!(
                stdout, "{}\n",
                project // to_memory_string(project.size),
                        // project.path.parent().unwrap().to_str().unwrap()
            );
        }
        output!(
            "Summary: {size} projects, {}",
            to_memory_string(Some(total)).trim()
        )
    }

    fn clean(&self) {
        for project in self {
            wait(15);
            output!("Cleaning: {:?}", project.path());
            match Command::new("cargo")
                .arg("clean")
                .arg("--manifest-path")
                .arg(project.path())
                .stdout(Stdio::inherit())
                .status()
            {
                Ok(_) => (),
                Err(e) => warn!("Error cleaning {:?}: {e}", project.path()),
            }
            if let Err(e) = erase() {
                warn!("Error clearning screen: {e}")
            }
        }
    }
}
