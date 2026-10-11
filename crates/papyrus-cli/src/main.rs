//! `papyrus` : vérifier et formater des scripts Papyrus.
//!
//! ```sh
//! papyrus check --game skyrim Source/Scripts
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
use papyrus_project::{DiscoverOptions, decode, discover, encode, encoding};

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
    /// Signale les erreurs de syntaxe et, avec --game, les textes que le
    /// compilateur corromprait.
    Check(Common),
    /// Formate les scripts, sur place.
    Fmt(FmtArgs),
}

#[derive(Debug, Args)]
struct Common {
    /// Fichiers ou dossiers à traiter. Le dossier courant par défaut.
    paths: Vec<PathBuf>,

    /// Jeu ciblé : skyrim, fallout4, fallout76, starfield (ou sse, fo4, sf…).
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
struct FmtArgs {
    #[command(flatten)]
    common: Common,

    /// N'écrit rien ; liste les fichiers à reformater et échoue s'il y en a.
    #[arg(long)]
    check: bool,

    /// Indentation : un nombre d'espaces, ou `tab`.
    #[arg(long, default_value = "2")]
    indent: IndentArg,
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

impl Common {
    fn dialect(&self) -> Dialect {
        self.game.map(Dialect::new).unwrap_or_default()
    }

    /// Les scripts à traiter, ou `None` après avoir signalé l'erreur.
    fn scripts(&self) -> Option<Vec<PathBuf>> {
        let roots = if self.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            self.paths.clone()
        };
        let options = DiscoverOptions {
            include_fragments: self.include_fragments,
        };
        match discover(&roots, options) {
            Ok(scripts) => Some(scripts),
            Err(error) => {
                eprintln!("erreur : {error}");
                None
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut reporter = Reporter::new(std::io::stderr().is_terminal());
    let clean = match cli.command {
        Command::Check(args) => check(&args, &mut reporter),
        Command::Fmt(args) => fmt(&args, &mut reporter),
    };
    match clean {
        Some(true) => ExitCode::SUCCESS,
        Some(false) => ExitCode::FAILURE,
        None => ExitCode::from(2),
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

fn check(args: &Common, reporter: &mut Reporter) -> Option<bool> {
    let scripts = args.scripts()?;
    let dialect = args.dialect();

    for path in &scripts {
        let Some(bytes) = read(path, reporter) else {
            continue;
        };
        let decoded = decode(&bytes);
        let parse = papyrus_syntax::parse(&decoded.text, dialect);

        let mut diagnostics: Vec<Diagnostic> = parse
            .errors()
            .iter()
            .map(|e| Diagnostic {
                severity: Severity::Error,
                code: "syntax",
                message: e.message().to_owned(),
                range: e.range(),
            })
            .collect();
        if let Some(game) = args.game {
            if let Some(issue) = encoding::check(&decoded, game) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: encoding::ENCODING_LOSSY,
                    message: issue.message,
                    range: issue.range,
                });
            }
        }
        reporter.emit(path, &decoded.text, &diagnostics);
    }

    reporter.summary(scripts.len(), "vérifié(s)");
    Some(reporter.is_clean())
}

fn fmt(args: &FmtArgs, reporter: &mut Reporter) -> Option<bool> {
    let scripts = args.common.scripts()?;
    let dialect = args.common.dialect();
    let options = FormatOptions {
        indent: args.indent.0,
    };

    let mut changed = 0;
    for path in &scripts {
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

        if args.check {
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

    let verb = if args.check {
        "à reformater"
    } else {
        "reformaté(s)"
    };
    eprintln!("{changed} fichier(s) {verb} sur {}", scripts.len());
    Some(reporter.is_clean() && !(args.check && changed > 0))
}

/// Écrit à côté puis renomme : une coupure en plein milieu ne laisse jamais
/// un script à moitié écrit.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".papyrus-fmt.tmp");
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
        assert_eq!(args.game, Some(GameId::Skyrim));
    }
}
