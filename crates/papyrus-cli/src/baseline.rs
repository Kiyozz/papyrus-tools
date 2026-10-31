//! `papyrus-baseline.json`: the problems a project already had, so that
//! adopting the linter on an older mod only reports the new ones.
//!
//! As in PHPStan, a problem is known by its file, its lint and its message,
//! never by its line: editing around it does not bring it back. Identical
//! problems are counted, so one more in a file is reported.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::report::Diagnostic;

/// The file, next to `papyrus.toml`.
pub(crate) const FILE_NAME: &str = "papyrus-baseline.json";

/// What `papyrus explain baseline` prints.
pub(crate) const HELP: &str = "\
A baseline lets you adopt papyrus on a mod that already has many problems.

`papyrus check --write-baseline` records today's problems in
papyrus-baseline.json, next to papyrus.toml. From then on, `papyrus check` only
reports new ones: the build stays green, and the mod gets no worse. Commit the
file so that everyone shares it.

A problem is known by its file, its lint and its message, never by its line,
and each is counted: editing around it keeps it known, one more in the file is
reported. Syntax errors are never recorded. The editor ignores the baseline and
shows everything.

To pay the debt down:
  papyrus check --no-baseline         shows everything again
  papyrus explain PAP015              says what a lint means, and what to do
  papyrus check --fix                 fixes what can be fixed safely
  papyrus check --write-baseline      records what remains

`papyrus check` says when problems of the baseline are gone. Delete the file to
stop using one.";

const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    file: String,
    code: String,
    message: String,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    problems: Vec<Entry>,
}

/// A file, a lint's code, a message.
type Key = (String, String, String);

/// The baseline of one project.
#[derive(Debug)]
pub(crate) struct Baseline {
    root: PathBuf,
    path: PathBuf,
    /// The known problems not met yet.
    known: BTreeMap<Key, usize>,
    /// What `--write-baseline` records.
    recording: Option<BTreeMap<Key, usize>>,
    /// The files gone through.
    seen: BTreeSet<String>,
    /// The problems hidden so far.
    pub(crate) hidden: usize,
}

impl Baseline {
    /// The baseline of the project at `root`, empty without a file.
    pub(crate) fn load(root: &Path, record: bool) -> Result<Baseline, String> {
        let root = std::path::absolute(root).map_err(|e| e.to_string())?;
        let path = root.join(FILE_NAME);
        let mut known = BTreeMap::new();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let stored: Stored = serde_json::from_str(&text).map_err(|e| {
                    format!(
                        "{}: {e} — fix it, or delete it to start over",
                        path.display()
                    )
                })?;
                if stored.version != VERSION {
                    return Err(format!(
                        "{}: version {} of the baseline, this papyrus reads {VERSION}",
                        path.display(),
                        stored.version
                    ));
                }
                for entry in stored.problems {
                    *known
                        .entry((entry.file, entry.code, entry.message))
                        .or_default() += entry.count;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
        Ok(Baseline {
            root,
            path,
            known,
            recording: record.then(BTreeMap::new),
            seen: BTreeSet::new(),
            hidden: 0,
        })
    }

    fn relative(&self, path: &Path) -> String {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        let relative = path.strip_prefix(&self.root).unwrap_or(&path);
        relative.to_string_lossy().replace('\\', "/")
    }

    /// The problems of a file left to report: the known ones are hidden, or
    /// recorded by `--write-baseline`. Syntax errors always go through.
    pub(crate) fn pass(&mut self, path: &Path, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let file = self.relative(path);
        self.seen.insert(file.clone());
        let mut kept = Vec::new();
        for diagnostic in diagnostics {
            let Some(code) = lint_code(&diagnostic.code) else {
                kept.push(diagnostic);
                continue;
            };
            let key = (file.clone(), code, diagnostic.message.clone());
            if let Some(recording) = &mut self.recording {
                *recording.entry(key).or_default() += 1;
                continue;
            }
            match self.known.get_mut(&key) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    self.hidden += 1;
                }
                _ => kept.push(diagnostic),
            }
        }
        kept
    }

    /// The known problems of the files gone through that were not met.
    pub(crate) fn gone(&self) -> usize {
        self.known
            .iter()
            .filter(|((file, _, _), _)| self.seen.contains(file))
            .map(|(_, count)| count)
            .sum()
    }

    /// Writes what was recorded: the files not gone through keep their
    /// entries, so checking one file does not forget the others. Gives the
    /// number of problems by lint.
    pub(crate) fn save(self) -> Result<BTreeMap<String, usize>, String> {
        let mut problems = self.recording.unwrap_or_default();
        for (key, count) in self.known {
            let (file, _, _) = &key;
            if !self.seen.contains(file) && self.root.join(file).is_file() {
                problems.insert(key, count);
            }
        }
        let mut by_lint = BTreeMap::new();
        for ((_, code, _), count) in &problems {
            *by_lint.entry(code.clone()).or_default() += count;
        }
        if problems.is_empty() {
            return match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(by_lint),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(by_lint),
                Err(error) => Err(format!("{}: {error}", self.path.display())),
            };
        }
        let stored = Stored {
            version: VERSION,
            problems: problems
                .into_iter()
                .map(|((file, code, message), count)| Entry {
                    file,
                    code,
                    message,
                    count,
                })
                .collect(),
        };
        let mut text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
        text.push('\n');
        std::fs::write(&self.path, text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        Ok(by_lint)
    }
}

/// The lint's code in a diagnostic's, `PAP015 locals/unused`; `None` for
/// what is not a lint.
fn lint_code(code: &str) -> Option<String> {
    let code = code.split(' ').next()?;
    code.starts_with("PAP").then(|| code.to_owned())
}
