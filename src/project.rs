use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::fs;
use std::path::{Path, PathBuf};
use toml::{Table, Value};
use walkdir::WalkDir;

use crate::to_memory_string;

/// A project is uniquely identified by the path to its Cargo.toml. Note that
/// the path stored in self.0 includes the `Cargo.toml` at the end.
#[derive(Serialize)]
pub struct Project {
    path: PathBuf,
    name: Name,
    size: Option<u64>,
}

/// Name of a project. `Explicit` corresponds to a name in the package.name field
/// of a Cargo.toml, which the `Inferred` name is the name of the parent directory
/// of the Cargo.toml. This is used when no package.name field exists
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize)]
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

impl Project {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        // Make sure it's a valid Cargo.toml
        match path.file_name() {
            Some(name) => {
                if name.to_str() != Some("Cargo.toml") {
                    bail!("{:?} is not a Cargo.toml file", path)
                }
            }
            None => bail!("{:?} is not a Cargo.toml file", path),
        }

        let contents =
            fs::read_to_string(&path).with_context(|| format!("Failed to read {:?}", &path))?;

        let toml = contents
            .parse::<Table>()
            .with_context(|| format!("Failed to parse {:?}", path))?;

        let name = toml
            .get("package")
            .and_then(Value::as_table)
            .and_then(|pack| pack.get("name"))
            .and_then(Value::as_str)
            .map(|name| Name::Explicit(name.to_string()))
            .unwrap_or(Name::Inferred(
                path.parent()
                    .ok_or(anyhow!(
                        "Failed to find name field or parent directory for {:?}",
                        path
                    ))?
                    .to_str()
                    .ok_or(anyhow!("Failed to convert path {:?} to string", path))?
                    .to_string(),
            ));

        // Get the size
        let mut initial = Project {
            path: path.to_owned(),
            name,
            size: None,
        };
        initial.size = initial.dirsize()?;

        return Ok(initial);
    }

    pub fn dirsize(&self) -> Result<Option<u64>> {
        // Get path to target/ dir
        let mut target = self.path.parent().unwrap().to_owned();
        target.push("target/");

        match target.try_exists() {
            Ok(true) => (),
            Ok(false) => return Ok(None),
            Err(e) => {
                return Result::Err(anyhow::Error::from(e))
                    .context("failed to access target directory")
            }
        }

        Ok(Some(
            WalkDir::new(target)
                .into_iter()
                .filter_map(|e| e.ok())
                .fold(0, |acc, item| acc + item.metadata().unwrap().len()),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> &Option<u64> {
        &self.size
    }
}

impl Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("path", &self.path)
            .field("name", &self.name as &dyn Debug)
            .field("size", &self.size as &dyn Debug)
            .finish()
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} @ {:?}",
            to_memory_string(self.size),
            self.name,
            self.path().parent().unwrap().to_str()
        )
    }
}

impl PartialOrd for Project {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match Some(self.size.cmp(&other.size)) {
            Some(ord) => match ord {
                Ordering::Less => Some(ord),
                Ordering::Equal => Some(self.path.cmp(&other.path)),
                Ordering::Greater => Some(ord),
            },
            None => None,
        }
    }
}

impl Ord for Project {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.partial_cmp(other) {
            Some(order) => order,
            None => unreachable!("partial_cmp should always succed"),
        }
    }
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Project {}
