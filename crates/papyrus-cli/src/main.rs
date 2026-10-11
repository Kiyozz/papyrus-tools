//! `papyrus` : vérifier et formater des scripts Papyrus.
//!
//! ```sh
//! papyrus check --game skyrim Source/Scripts
//! papyrus check --fix .
//! papyrus fmt --check .
//! ```

#![forbid(unsafe_code)]

mod report;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand};
use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatError, FormatOptions, Indent};
use papyrus_lint::{LINTS, Levels, apply_fixes, vanilla_flags};
use papyrus_project::flags::{FlagDef, parse_flags};
use papyrus_project::{
    Config, Decoded, DiscoverOptions, IndentSetting, Level, decode, discover, encode,
};
use papyrus_syntax::Parse;

use report::{Diagnostic, Reporter, Severity};

#[derive(Debug, Parser)]
#[command(
    name = "papyrus",
    version,
    about = "Vérifie et formate des scripts Papyrus"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Signale les erreurs de syntaxe et ce que relèvent les lints.
    Check(CheckArgs),
    /// Formate les scripts, sur place.
    Fmt(FmtArgs),
    /// Liste les lints, leur code et leur niveau par défaut.
    Lints,
}

#[derive(Debug, Args)]
struct Common {
    /// Fichiers ou dossiers à traiter. Le dossier courant par défaut.
    paths: Vec<PathBuf>,

    /// Jeu ciblé : skyrim, fallout4, fallout76, starfield (ou sse, fo4, sf…).
    /// L'emporte sur `game` dans papyrus.toml.
    ///
    /// Sans cible, la syntaxe la plus large est acceptée, et les vérifications
    /// d'encodage sont sautées : leur conseil dépend du jeu.
    #[arg(long, short)]
    game: Option<GameId>,

    /// Inclure les fragments générés par le Creation Kit.
    #[arg(long)]
    include_fragments: bool,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[command(flatten)]
    common: Common,

    /// Applique les correctifs sûrs, sur place.
    #[arg(long)]
    fix: bool,
}

#[derive(Debug, Args)]
struct FmtArgs {
    #[command(flatten)]
    common: Common,

    /// N'écrit rien ; liste les fichiers à reformater et échoue s'il y en a.
    #[arg(long)]
    check: bool,

    /// Indentation : un nombre d'espaces, ou `tab`. Deux espaces par défaut,
    /// ou `fmt.indent` de papyrus.toml.
    #[arg(long)]
    indent: Option<IndentArg>,
}

#[derive(Debug, Clone, Copy)]
struct IndentArg(Indent);

impl FromStr for IndentArg {
    type Err = String;

    fn from_str(s: &str) -> Result<IndentArg, String> {
        if s.eq_ignore_ascii_case("tab") || s.eq_ignore_ascii_case("tabs") {
            return Ok(IndentArg(Indent::Tabs));
        }
        match s.parse::<u8>() {
            Ok(n @ 1..=8) => Ok(IndentArg(Indent::Spaces(n))),
            _ => Err(format!(
                "`{s}` : attendu un nombre d'espaces de 1 à 8, ou `tab`"
            )),
        }
    }
}

/// Les réglages en vigueur : ligne de commande, puis papyrus.toml, puis les
/// valeurs par défaut.
#[derive(Debug)]
struct Settings {
    scripts: Vec<PathBuf>,
    game: Option<GameId>,
    indent: Indent,
    flags: Vec<FlagDef>,
    levels: Levels,
}

impl Settings {
    fn dialect(&self) -> Dialect {
        self.game.map(Dialect::new).unwrap_or_default()
    }

    fn resolve(common: &Common, indent: Option<IndentArg>) -> Result<Settings, String> {
        let roots = if common.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            common.paths.clone()
        };
        let config = Config::discover(&roots[0]).map_err(|e| e.to_string())?;
        let in_config = |message: String| match &config.path {
            Some(path) => format!("{} : {message}", path.display()),
            None => message,
        };

        let options = DiscoverOptions {
            include_fragments: common.include_fragments || config.include_fragments,
        };
        let scripts = discover(&roots, options).map_err(|e| e.to_string())?;

        let game = common.game.or(config.game).or_else(|| detect(&scripts));
        let flags = match &config.flags {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("{} : {e}", path.display()))?;
                parse_flags(&text).map_err(|e| format!("{} : {e}", path.display()))?
            }
            None => vanilla_flags(game),
        };
        let levels = Levels::from_config(&config.lints).map_err(in_config)?;
        let indent = match (indent, config.indent) {
            (Some(IndentArg(indent)), _) => indent,
            (None, Some(IndentSetting::Spaces(n))) => Indent::Spaces(n),
            (None, Some(IndentSetting::Tab)) => Indent::Tabs,
            (None, None) => Indent::default(),
        };
        Ok(Settings {
            scripts,
            game,
            indent,
            flags,
            levels,
        })
    }
}

/// Devine le jeu des scripts, quand ni `--game` ni papyrus.toml ne le
/// disent : la majorité des fichiers qui le trahissent.
///
/// Sans aucun indice, `None` : la syntaxe la plus large s'applique, et les
/// lints qui dépendent du jeu se taisent plutôt que de deviner.
fn detect(scripts: &[PathBuf]) -> Option<GameId> {
    let widest = Dialect::new(GameId::Starfield);
    let verdicts = scripts.iter().filter_map(|path| {
        let bytes = std::fs::read(path).ok()?;
        let parse = papyrus_syntax::parse(&decode(&bytes).text, widest);
        Some(papyrus_project::detect::evidence(&parse.syntax()).verdict())
    });
    match papyrus_project::detect::majority(verdicts) {
        Some((game, votes, decided)) => {
            eprintln!(
                "cible déduite : {} ({votes} fichier(s) sur {decided} qui l'indiquent) — \
                 `--game` ou `game` dans papyrus.toml pour l'imposer",
                game.display_name()
            );
            Some(game)
        }
        None => {
            eprintln!(
                "cible inconnue : syntaxe de Starfield, vérifications propres au jeu \
                 sautées — `--game` ou `game` dans papyrus.toml pour la préciser"
            );
            None
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut reporter = Reporter::new(std::io::stderr().is_terminal());
    let result = match cli.command {
        Command::Check(args) => Settings::resolve(&args.common, None)
            .map(|settings| check(&settings, args.fix, &mut reporter)),
        Command::Fmt(args) => Settings::resolve(&args.common, args.indent)
            .map(|settings| fmt(&settings, args.check, &mut reporter)),
        Command::Lints => {
            lints();
            Ok(true)
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("erreur : {error}");
            ExitCode::from(2)
        }
    }
}

fn lints() {
    for lint in LINTS {
        let level = match lint.default {
            Level::Allow => "allow",
            Level::Warn => "warn",
            Level::Error => "error",
        };
        println!(
            "{}  {:<18} {:<6} {}",
            lint.code, lint.name, level, lint.description
        );
    }
}

/// Lit un fichier, ou signale pourquoi c'est impossible.
fn read(path: &Path, reporter: &mut Reporter) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            reporter.io_error(path, &error);
            None
        }
    }
}

/// L'arbre et les diagnostics des lints d'un fichier.
fn analyze(decoded: &Decoded, settings: &Settings) -> (Parse, Vec<papyrus_lint::Diagnostic>) {
    let parse = papyrus_syntax::parse(&decoded.text, settings.dialect());
    let root = parse.syntax();
    let context = papyrus_lint::Context {
        decoded,
        root: &root,
        game: settings.game,
        flags: &settings.flags,
    };
    let lints = papyrus_lint::run(&context, &settings.levels);
    (parse, lints)
}

fn check(settings: &Settings, fix: bool, reporter: &mut Reporter) -> bool {
    let mut fixed = 0;
    for path in &settings.scripts {
        let Some(bytes) = read(path, reporter) else {
            continue;
        };
        let mut decoded = decode(&bytes);
        let (mut parse, mut lints) = analyze(&decoded, settings);

        // Jamais sur un arbre incomplet, ni sur un fichier mal décodé.
        let fixable = lints.iter().any(|d| d.fix.is_some());
        if fix && fixable && parse.ok() && !decoded.lossy {
            let result = apply_fixes(&decoded.text, &lints)
                .and_then(|text| encode(&text, decoded.encoding).map(|bytes| (text, bytes)));
            match result {
                Some((text, bytes)) => {
                    if let Err(error) = write_atomic(path, &bytes) {
                        reporter.io_error(path, &error);
                        continue;
                    }
                    fixed += 1;
                    decoded.text = text;
                    (parse, lints) = analyze(&decoded, settings);
                }
                None => reporter.skip(path, "correctif refusé — merci de signaler ce bug"),
            }
        }

        let mut diagnostics: Vec<Diagnostic> = parse
            .errors()
            .iter()
            .map(|e| Diagnostic {
                severity: Severity::Error,
                code: "syntax".to_owned(),
                message: e.message().to_owned(),
                range: e.range(),
            })
            .collect();
        diagnostics.extend(lints.into_iter().map(|d| {
            let hint = if d.fix.is_some() {
                " Corrigeable avec `papyrus check --fix`."
            } else {
                ""
            };
            Diagnostic {
                severity: if d.level == Level::Error {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                code: format!("{} {}", d.lint.code, d.lint.name),
                message: format!("{}{hint}", d.message),
                range: d.range,
            }
        }));
        reporter.emit(path, &decoded.text, &diagnostics);
    }

    if fix {
        eprintln!("{fixed} fichier(s) corrigé(s)");
    }
    reporter.summary(settings.scripts.len(), "vérifié(s)");
    reporter.is_clean()
}

fn fmt(settings: &Settings, check_only: bool, reporter: &mut Reporter) -> bool {
    let dialect = settings.dialect();
    let options = FormatOptions {
        indent: settings.indent,
    };

    let mut changed = 0;
    for path in &settings.scripts {
        let Some(bytes) = read(path, reporter) else {
            continue;
        };
        let decoded = decode(&bytes);
        if decoded.lossy {
            reporter.skip(
                path,
                "octets invalides pour son encodage ; le réécrire en perdrait",
            );
            continue;
        }

        let formatted = match papyrus_fmt::format(&decoded.text, dialect, &options) {
            Ok(formatted) => formatted,
            Err(FormatError::Syntax(errors)) => {
                reporter.skip(
                    path,
                    &format!(
                        "{} erreur(s) de syntaxe, voir `papyrus check`",
                        errors.len()
                    ),
                );
                continue;
            }
            Err(error @ FormatError::Unsafe { .. }) => {
                reporter.skip(path, &format!("{error} — merci de signaler ce bug"));
                continue;
            }
        };
        if formatted == decoded.text {
            continue;
        }
        changed += 1;

        if check_only {
            println!("{}", path.display());
            continue;
        }
        // Réencodé comme lu : le formateur ne change pas l'encodage.
        let Some(bytes) = encode(&formatted, decoded.encoding) else {
            reporter.skip(
                path,
                "le résultat n'est pas représentable dans l'encodage d'origine",
            );
            continue;
        };
        if let Err(error) = write_atomic(path, &bytes) {
            reporter.io_error(path, &error);
        }
    }

    let verb = if check_only {
        "à reformater"
    } else {
        "reformaté(s)"
    };
    eprintln!("{changed} fichier(s) {verb} sur {}", settings.scripts.len());
    reporter.is_clean() && !(check_only && changed > 0)
}

/// Écrit à côté puis renomme : une coupure en plein milieu ne laisse jamais
/// un script à moitié écrit.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".papyrus.tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_indentation_se_lit_en_espaces_ou_en_tabulations() {
        assert_eq!("4".parse::<IndentArg>().unwrap().0, Indent::Spaces(4));
        assert_eq!("tab".parse::<IndentArg>().unwrap().0, Indent::Tabs);
        assert_eq!("TABS".parse::<IndentArg>().unwrap().0, Indent::Tabs);
        assert!("0".parse::<IndentArg>().is_err());
        assert!("deux".parse::<IndentArg>().is_err());
    }

    #[test]
    fn la_ligne_de_commande_est_coherente() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn les_alias_de_jeu_sont_acceptes() {
        let cli = Cli::try_parse_from(["papyrus", "check", "--game", "sse", "x"]).unwrap();
        let Command::Check(args) = cli.command else {
            panic!("sous-commande");
        };
        assert_eq!(args.common.game, Some(GameId::Skyrim));
    }
}
