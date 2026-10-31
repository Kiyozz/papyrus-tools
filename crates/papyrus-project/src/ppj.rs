//! `.ppj` files, the Creation Kit's project format and Pyro's extension of
//! it, read as a configuration.
//!
//! As in both tools, a path is relative to the file, a `Script` is named from
//! the imports, and a `Folder` is looked up there too, as Pyro does. Pyro's
//! `Variables` are expanded, along with the environment's. Packages, archives
//! and build events are Pyro's job: they are reported, and never run.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use papyrus_dialect::GameId;

use crate::config::{BuildConfig, Config, ConfigError, normalize};
use crate::discover::{is_fragment_dir, is_fragment_name, is_script};

/// A `Folder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// Where it is.
    pub path: PathBuf,
    /// Its subfolders count, unless `NoRecurse` says otherwise.
    pub recurse: bool,
}

/// The scripts a `.ppj` lists: a project's, in place of those of its folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// `Folders`.
    pub folders: Vec<Folder>,
    /// `Scripts`.
    pub scripts: Vec<PathBuf>,
}

impl Sources {
    /// The listed scripts, sorted. A folder's fragments are left out unless
    /// asked for; a script named one by one stays.
    pub fn list(&self, include_fragments: bool) -> io::Result<Vec<PathBuf>> {
        let mut out = self.scripts.clone();
        for folder in &self.folders {
            walk(&folder.path, folder.recurse, include_fragments, &mut out)?;
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

fn walk(dir: &Path, recurse: bool, fragments: bool, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if recurse && (fragments || !is_fragment_dir(&path)) {
                walk(&path, recurse, fragments, out)?;
            }
        } else if is_script(&path) && (fragments || !is_fragment_name(&path)) {
            out.push(path);
        }
    }
    Ok(())
}

/// True for a `.ppj`.
pub fn is_ppj(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ppj"))
}

/// The `.ppj` files of `dir`, sorted.
pub fn in_folder(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| is_ppj(path) && path.is_file())
        .collect();
    out.sort();
    out
}

/// Reads the `.ppj` at `path`.
pub fn read(path: &Path) -> Result<Config, ConfigError> {
    let bytes = std::fs::read(path).map_err(|e| ConfigError {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    parse(&crate::decode(&bytes).text, path)
}

/// `Game`, Pyro's names first.
fn game(value: &str) -> Result<GameId, String> {
    match value.to_ascii_lowercase().as_str() {
        "tes5" => Ok(GameId::Skyrim),
        "sf1" => Ok(GameId::Starfield),
        other => other.parse::<GameId>().map_err(|e| e.to_string()),
    }
}

/// The game a vanilla flags file belongs to.
fn game_of_flags(name: &str) -> Option<GameId> {
    match name.to_ascii_lowercase().as_str() {
        "tesv_papyrus_flags.flg" => Some(GameId::Skyrim),
        "institute_papyrus_flags.flg" => Some(GameId::Fallout4),
        "starfield_papyrus_flags.flg" => Some(GameId::Starfield),
        _ => None,
    }
}

/// `xs:boolean`, in the case Pyro also takes.
fn boolean(value: &str) -> bool {
    value.eq_ignore_ascii_case("true") || value == "1"
}

/// What `text`, starting at `@`, `%` or `$`, stands for, and how long the
/// reference is.
fn reference(text: &str, variables: &HashMap<String, String>) -> (Option<String>, usize) {
    let sigil = text.as_bytes()[0];
    let body = &text[1..];
    if sigil == b'%' {
        return match body.find('%') {
            Some(end) => (std::env::var(&body[..end]).ok(), end + 2),
            None => (None, 1),
        };
    }
    if sigil == b'@' && body.starts_with('@') {
        return (Some("@".to_owned()), 2);
    }
    let (name, len) = match body.strip_prefix('{') {
        Some(braced) => match braced.find('}') {
            Some(end) => (&braced[..end], end + 3),
            None => return (None, 1),
        },
        None => {
            let end = body
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(body.len());
            (&body[..end], end + 1)
        }
    };
    if name.is_empty() {
        return (None, 1);
    }
    let value = if sigil == b'@' {
        variables.get(name).cloned()
    } else {
        std::env::var(name).ok()
    };
    (value, len)
}

/// `text` with its variables replaced; an unknown one stays as written.
fn expand(text: &str, variables: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(['@', '%', '$']) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let (value, len) = reference(rest, variables);
        out.push_str(value.as_deref().unwrap_or(&rest[..len]));
        rest = &rest[len..];
    }
    out.push_str(rest);
    out
}

/// A path as written, on any system.
fn as_path(text: &str) -> PathBuf {
    PathBuf::from(text.replace('\\', "/"))
}

/// The file of the `Script` called `name`, from the imports, then the `.ppj`.
fn script(name: &str, dir: &Path, imports: &[PathBuf]) -> Option<PathBuf> {
    let path = if name.contains(":\\") || name.contains(":/") {
        as_path(name)
    } else {
        // Namespaces are folders.
        as_path(&name.replace(':', "/"))
    };
    let path = if is_script(&path) {
        path
    } else {
        PathBuf::from(format!("{}.psc", path.display()))
    };
    if path.is_absolute() {
        return path.is_file().then_some(path);
    }
    imports
        .iter()
        .map(PathBuf::as_path)
        .chain([dir])
        .map(|base| normalize(&base.join(&path)))
        .find(|candidate| candidate.is_file())
}

/// Where the `Folder` at `path` is: at its path, else under an import.
fn folder(path: &Path, dir: &Path, imports: &[PathBuf]) -> Option<PathBuf> {
    [dir]
        .into_iter()
        .chain(imports.iter().map(PathBuf::as_path))
        .map(|base| normalize(&base.join(path)))
        .find(|candidate| candidate.is_dir())
}

/// Reads the contents of a `.ppj`.
pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
    let error = |message: String| ConfigError {
        path: path.to_owned(),
        message,
    };
    let document = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|e| error(e.to_string()))?;
    let root = document.root_element();
    if root.tag_name().name() != "PapyrusProject" {
        return Err(error("expected a `PapyrusProject` element".to_owned()));
    }
    let dir = path.parent().unwrap_or(Path::new(""));

    let children = |section: &'static str, item: &'static str| {
        root.children()
            .filter(move |n| n.tag_name().name() == section)
            .flat_map(|n| n.children())
            .filter(move |n| n.tag_name().name() == item)
    };

    let mut variables: HashMap<String, String> = children("Variables", "Variable")
        .filter_map(|n| Some((n.attribute("Name")?.into(), n.attribute("Value")?.into())))
        .collect();
    variables.insert("PROJECT_PATH".to_owned(), dir.display().to_string());
    // A variable may refer to another one, declared before it or not.
    for _ in 0..variables.len() {
        let known = variables.clone();
        for value in variables.values_mut() {
            *value = expand(value, &known);
        }
    }

    let attribute = |name: &str| root.attribute(name).map(|v| expand(v, &variables));
    let flag = |name: &str| attribute(name).is_some_and(|v| boolean(&v));
    let items = |section: &'static str, item: &'static str| {
        children(section, item)
            .filter_map(|n| {
                let text = expand(n.text()?.trim(), &variables);
                (!text.is_empty()).then_some((n, text))
            })
            .collect::<Vec<_>>()
    };

    let mut notes = Vec::new();
    let mut imports: Vec<PathBuf> = Vec::new();
    for (_, text) in items("Imports", "Import") {
        if text.contains("://") {
            notes.push(format!("import `{text}`: remote, skipped"));
            continue;
        }
        let import = normalize(&dir.join(as_path(&text)));
        if import.is_dir() {
            imports.push(import);
        } else {
            notes.push(format!("import `{text}`: not found, skipped"));
        }
    }

    let mut sources = Sources::default();
    for (node, text) in items("Folders", "Folder") {
        if text.contains("://") {
            notes.push(format!("folder `{text}`: remote, skipped"));
            continue;
        }
        match folder(&as_path(&text), dir, &imports) {
            Some(path) => sources.folders.push(Folder {
                path,
                recurse: !node.attribute("NoRecurse").is_some_and(boolean),
            }),
            None => notes.push(format!("folder `{text}`: not found, skipped")),
        }
    }
    for (_, text) in items("Scripts", "Script") {
        match script(&text, dir, &imports) {
            Some(path) => sources.scripts.push(path),
            None => notes.push(format!("script `{text}`: not found, skipped")),
        }
    }
    let listed = root
        .children()
        .any(|n| matches!(n.tag_name().name(), "Folders" | "Scripts"));

    // The vanilla flags are the default; another file is found like a script.
    let written_flags = attribute("Flags").filter(|f| !f.is_empty());
    let vanilla = written_flags.as_deref().and_then(game_of_flags);
    let flags = match written_flags.filter(|_| vanilla.is_none()) {
        None => None,
        Some(name) => {
            let found = imports
                .iter()
                .map(PathBuf::as_path)
                .chain([dir])
                .map(|base| normalize(&base.join(as_path(&name))))
                .find(|candidate| candidate.is_file());
            if found.is_none() {
                notes.push(format!("flags `{name}`: not found, skipped"));
            }
            found
        }
    };

    let game = attribute("Game")
        .filter(|g| !g.is_empty())
        .map(|g| game(&g).map_err(error))
        .transpose()?
        .or(vanilla);

    let mut ignored: Vec<String> = root
        .children()
        .map(|n| n.tag_name().name())
        .filter(|name| matches!(*name, "Packages" | "ZipFiles") || name.ends_with("Event"))
        .map(str::to_owned)
        .collect();
    for name in ["Anonymize", "Package", "Zip"] {
        if flag(name) {
            ignored.push(name.to_owned());
        }
    }

    Ok(Config {
        path: Some(path.to_owned()),
        game,
        flags,
        imports,
        sources: listed.then_some(sources),
        notes,
        ignored,
        build: BuildConfig {
            output: attribute("Output")
                .filter(|o| !o.is_empty())
                .map(|o| normalize(&dir.join(as_path(&o)))),
            optimize: flag("Optimize"),
            release: flag("Release"),
            final_: flag("Final"),
            compiler: None,
        },
        ..Config::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary folder holding `files`, with `.` in place of its path in
    /// their contents.
    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("papyrus-ppj-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    #[test]
    fn the_creation_kit_format() {
        let root = tree(
            "ck",
            &[
                ("mod/Source/A.psc", ""),
                ("mod/Source/MyNs/B.psc", ""),
                ("mod/Source/MyNs/Deep/C.psc", ""),
                (
                    "mod/mod.ppj",
                    r#"<?xml version="1.0"?>
                    <PapyrusProject xmlns="PapyrusProject.xsd"
                        Flags="Institute_Papyrus_Flags.flg" Output="..\out"
                        Optimize="true" Release="1" Final="false" Asm="None">
                      <Imports>
                        <Import>Source</Import>
                        <Import>C:\nowhere\at\all</Import>
                      </Imports>
                      <Folders><Folder NoRecurse="true">MyNs</Folder></Folders>
                      <Scripts><Script>A.psc</Script><Script>MyNs:Deep:C</Script></Scripts>
                    </PapyrusProject>"#,
                ),
            ],
        );
        let config = read(&root.join("mod/mod.ppj")).unwrap();
        assert_eq!(config.game, Some(GameId::Fallout4));
        assert_eq!(config.flags, None);
        assert_eq!(config.imports, [root.join("mod/Source")]);
        assert_eq!(config.build.output, Some(root.join("out")));
        assert!(config.build.optimize && config.build.release && !config.build.final_);
        assert_eq!(
            config.notes,
            ["import `C:\\nowhere\\at\\all`: not found, skipped"]
        );
        let sources = config.sources.unwrap();
        assert_eq!(
            sources.list(false).unwrap(),
            [
                root.join("mod/Source/A.psc"),
                root.join("mod/Source/MyNs/B.psc"),
                root.join("mod/Source/MyNs/Deep/C.psc"),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pyro_s_variables_game_and_extras() {
        let root = tree(
            "pyro",
            &[
                ("Source/A.psc", ""),
                ("Custom.flg", ""),
                (
                    "mod.ppj",
                    r#"<PapyrusProject xmlns="PapyrusProject.xsd" Game="sse"
                        Flags="Custom.flg" Output="@out" Anonymize="true" Package="false">
                      <Variables>
                        <Variable Name="out" Value="@base/Scripts"/>
                        <Variable Name="base" Value="@{PROJECT_PATH}/Data"/>
                      </Variables>
                      <Imports><Import>https://github.com/x/y</Import></Imports>
                      <Folders><Folder>./Source</Folder></Folders>
                      <PostBuildEvent><Command>format c:</Command></PostBuildEvent>
                      <Packages><Package/></Packages>
                    </PapyrusProject>"#,
                ),
            ],
        );
        let config = read(&root.join("mod.ppj")).unwrap();
        assert_eq!(config.game, Some(GameId::Skyrim));
        assert_eq!(config.flags, Some(root.join("Custom.flg")));
        assert_eq!(config.build.output, Some(root.join("Data/Scripts")));
        assert_eq!(
            config.notes,
            ["import `https://github.com/x/y`: remote, skipped"]
        );
        assert_eq!(config.ignored, ["PostBuildEvent", "Packages", "Anonymize"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn expansion_leaves_the_rest_alone() {
        let variables = HashMap::from([("a".to_owned(), "1".to_owned())]);
        assert_eq!(expand("@a/@{a}/@@/@b/50%", &variables), "1/1/@/@b/50%");
    }

    #[test]
    fn only_a_project_file_is_read() {
        let path = Path::new("/mod/mod.ppj");
        assert!(parse("<Project/>", path).is_err());
        assert!(parse("not xml", path).is_err());
        assert!(parse(r#"<PapyrusProject Game="doom"/>"#, path).is_err());
        assert_eq!(
            parse("\u{feff}<PapyrusProject/>", path).unwrap().sources,
            None
        );
    }
}
