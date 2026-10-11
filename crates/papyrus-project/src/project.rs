//! Projects: the scripts covered by one `papyrus.toml`.
//!
//! A command line or an editor window may span several projects — a Skyrim
//! mod next to a Fallout 4 one. Each script belongs to the project of the
//! first `papyrus.toml` found walking up from it; scripts no `papyrus.toml`
//! covers belong to the root they were found under, with the defaults.
//!
//! Projects are sealed: a script only sees the scripts of its own project.
//! Following `extends` into another project, a shared library for instance,
//! is left to an explicit setting.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{Config, ConfigError, FILE_NAME};
use crate::discover::{is_fragment_dir, is_fragment_name, is_script};

/// The scripts of one project, and its configuration.
#[derive(Debug, Clone)]
pub struct Project {
    /// The folder of its `papyrus.toml`, or the root it was found under.
    pub root: PathBuf,
    /// The configuration; `config.path` is `None` without a `papyrus.toml`.
    pub config: Config,
    /// Its scripts, sorted.
    pub scripts: Vec<PathBuf>,
}

/// Why projects could not be gathered.
#[derive(Debug)]
pub enum ProjectError {
    /// A path that does not exist or cannot be read.
    Io(PathBuf, io::Error),
    /// An unreadable `papyrus.toml`.
    Config(ConfigError),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io(path, error) if error.kind() == io::ErrorKind::NotFound => {
                write!(f, "{}: not found", path.display())
            }
            ProjectError::Io(path, error) => write!(f, "{}: {error}", path.display()),
            ProjectError::Config(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProjectError {}

/// Finds the `papyrus.toml` covering a folder, remembering every folder it
/// went through.
#[derive(Debug, Default)]
pub struct ConfigLocator {
    found: HashMap<PathBuf, Option<PathBuf>>,
}

impl ConfigLocator {
    /// The `papyrus.toml` covering `path`, a file or a folder.
    pub fn locate(&mut self, path: &Path) -> Option<PathBuf> {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        let dir = if path.is_dir() {
            path.as_path()
        } else {
            path.parent()?
        };
        self.locate_dir(dir)
    }

    fn locate_dir(&mut self, dir: &Path) -> Option<PathBuf> {
        if let Some(found) = self.found.get(dir) {
            return found.clone();
        }
        let candidate = dir.join(FILE_NAME);
        let found = if candidate.is_file() {
            Some(candidate)
        } else {
            dir.parent().and_then(|parent| self.locate_dir(parent))
        };
        self.found.insert(dir.to_owned(), found.clone());
        found
    }
}

/// Gathers the projects of `roots`.
///
/// A folder is walked recursively; fragments are skipped unless
/// `include_fragments` or the project's own configuration asks for them. An
/// explicitly given file is always kept.
pub fn projects(roots: &[PathBuf], include_fragments: bool) -> Result<Vec<Project>, ProjectError> {
    let mut gather = Gather {
        locator: ConfigLocator::default(),
        configs: HashMap::new(),
        include_fragments,
        projects: BTreeMap::new(),
    };
    for root in roots {
        let root = std::path::absolute(root).unwrap_or_else(|_| root.clone());
        if root.is_dir() {
            gather.walk(&root, &root)?;
        } else if root.exists() {
            let fallback = root.parent().unwrap_or(&root).to_owned();
            gather.add(&root, &fallback)?;
        } else {
            return Err(ProjectError::Io(
                root,
                io::Error::from(io::ErrorKind::NotFound),
            ));
        }
    }
    Ok(gather
        .projects
        .into_iter()
        .map(|(root, (config, mut scripts))| {
            scripts.sort();
            scripts.dedup();
            Project {
                root,
                config,
                scripts,
            }
        })
        .collect())
}

struct Gather {
    locator: ConfigLocator,
    configs: HashMap<PathBuf, Config>,
    include_fragments: bool,
    projects: BTreeMap<PathBuf, (Config, Vec<PathBuf>)>,
}

impl Gather {
    /// The configuration covering `dir`, and the root of its project.
    fn config(&mut self, dir: &Path, fallback: &Path) -> Result<(PathBuf, Config), ProjectError> {
        let Some(path) = self.locator.locate(dir) else {
            return Ok((fallback.to_owned(), Config::default()));
        };
        if !self.configs.contains_key(&path) {
            let text =
                std::fs::read_to_string(&path).map_err(|e| ProjectError::Io(path.clone(), e))?;
            let config = Config::parse(&text, &path).map_err(ProjectError::Config)?;
            self.configs.insert(path.clone(), config);
        }
        let root = path.parent().unwrap_or(&path).to_owned();
        Ok((root, self.configs[&path].clone()))
    }

    fn add(&mut self, script: &Path, fallback: &Path) -> Result<(), ProjectError> {
        let (root, config) = self.config(script, fallback)?;
        self.projects
            .entry(root)
            .or_insert_with(|| (config, Vec::new()))
            .1
            .push(script.to_owned());
        Ok(())
    }

    fn walk(&mut self, dir: &Path, fallback: &Path) -> Result<(), ProjectError> {
        let (_, config) = self.config(dir, fallback)?;
        let fragments = self.include_fragments || config.include_fragments;
        let entries = std::fs::read_dir(dir).map_err(|e| ProjectError::Io(dir.to_owned(), e))?;
        for entry in entries {
            let path = entry
                .map_err(|e| ProjectError::Io(dir.to_owned(), e))?
                .path();
            if path.is_dir() {
                if fragments || !is_fragment_dir(&path) {
                    self.walk(&path, fallback)?;
                }
            } else if is_script(&path) && (fragments || !is_fragment_name(&path)) {
                self.add(&path, fallback)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("papyrus-projects-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    fn names(project: &Project) -> Vec<String> {
        project
            .scripts
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn each_papyrus_toml_starts_a_project() {
        let root = tree(
            "split",
            &[
                ("skyrim/papyrus.toml", "game = \"skyrim\"\n"),
                ("skyrim/Scripts/A.psc", ""),
                (
                    "fallout/papyrus.toml",
                    "game = \"fallout4\"\ninclude-fragments = true\n",
                ),
                ("fallout/B.psc", ""),
                ("fallout/Fragments/QF_Quest_01.psc", ""),
                ("loose/C.psc", ""),
                ("loose/TIF__0001.psc", ""),
            ],
        );
        let found = projects(std::slice::from_ref(&root), false).unwrap();
        let summary: Vec<(String, Vec<String>)> = found
            .iter()
            .map(|p| {
                let name = p
                    .root
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                (name, names(p))
            })
            .collect();
        assert_eq!(
            summary,
            [
                // Without a papyrus.toml: the root given, with the defaults.
                (String::new(), vec!["C.psc".to_owned()]),
                // Its own configuration asks for the fragments.
                (
                    "fallout".to_owned(),
                    vec!["B.psc".to_owned(), "QF_Quest_01.psc".to_owned()]
                ),
                ("skyrim".to_owned(), vec!["A.psc".to_owned()]),
            ]
        );
        assert_eq!(found[2].config.game, Some(papyrus_dialect::GameId::Skyrim));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_explicit_file_is_always_kept() {
        let root = tree("explicit", &[("TIF__0001.psc", "")]);
        let file = root.join("TIF__0001.psc");
        let found = projects(&[file], false).unwrap();
        assert_eq!(names(&found[0]), ["TIF__0001.psc"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_root_is_an_error() {
        let error = projects(&[PathBuf::from("does/not/exist")], false).unwrap_err();
        assert!(error.to_string().ends_with("not found"), "{error}");
    }
}
