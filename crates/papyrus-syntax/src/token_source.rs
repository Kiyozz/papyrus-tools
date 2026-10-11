//! Applying the dialect to the token stream.
//!
//! The first of the two points where the target intervenes — the second
//! being the compatibility diagnostics the parser emits on constructs that
//! do not start with a keyword.
//!
//! The principle: a word is only reserved if the target is recent enough.
//! `Var` is a type in Fallout 4, a perfectly valid variable name in Skyrim.
//! Reserving it systematically would produce thousands of false errors on
//! existing code.

use papyrus_dialect::{Dialect, GameId};

use crate::lexer::{self, Token};
use crate::{SyntaxKind, keywords};

/// Splits `source` into tokens and applies the dialect.
///
/// This is the normal lexing entry point.
pub fn lex_with_dialect(source: &str, dialect: Dialect) -> Vec<Token> {
    let mut tokens = lexer::lex(source);
    promote(&mut tokens, source, dialect);
    tokens
}

/// Promotes the identifiers reserved by the target to keywords.
///
/// Identifiers whose word is only reserved by a newer game stay
/// [`SyntaxKind::IDENT`]: that is the plan's "demotion", seen from a lexer
/// that never promoted.
pub fn promote(tokens: &mut [Token], source: &str, dialect: Dialect) {
    for token in tokens.iter_mut().filter(|t| t.kind == SyntaxKind::IDENT) {
        if let Some((kind, since)) = keywords::lookup(token.text(source)) {
            if dialect.game.accepts(since) {
                token.kind = kind;
            }
        }
    }
}

/// If `text` is a word reserved by a game newer than the target, says which.
///
/// Explains why a word behaves as an identifier here, and warns that a
/// script will not port: a variable named `Var`, accepted by Skyrim, breaks
/// compilation from Fallout 4 on.
pub fn reserved_later(text: &str, dialect: Dialect) -> Option<(SyntaxKind, GameId)> {
    keywords::lookup(text).filter(|(_, since)| !dialect.game.accepts(*since))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str, game: GameId) -> Vec<SyntaxKind> {
        lex_with_dialect(source, Dialect::new(game))
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn is_est_un_mot_cle_a_partir_de_fallout4() {
        // Checked with the compiler: Skyrim SE rejects `x is Actor`.
        assert_eq!(kinds("is", GameId::Skyrim), [SyntaxKind::IDENT]);
        assert_eq!(kinds("is", GameId::Fallout4), [SyntaxKind::IS_KW]);
        assert_eq!(kinds("is", GameId::Fallout76), [SyntaxKind::IS_KW]);
        assert_eq!(kinds("is", GameId::Starfield), [SyntaxKind::IS_KW]);
    }

    #[test]
    fn as_est_un_mot_cle_des_skyrim() {
        for game in [GameId::Skyrim, GameId::Fallout4, GameId::Starfield] {
            assert_eq!(kinds("as", game), [SyntaxKind::AS_KW], "cible {game}");
        }
    }

    #[test]
    fn les_verrous_sont_propres_a_starfield() {
        for word in ["guard", "lockguard", "trylockguard", "endtrylockguard"] {
            assert_eq!(
                kinds(word, GameId::Fallout4),
                [SyntaxKind::IDENT],
                "`{word}`"
            );
            assert_ne!(
                kinds(word, GameId::Starfield),
                [SyntaxKind::IDENT],
                "`{word}`"
            );
        }
    }

    #[test]
    fn un_script_skyrim_peut_nommer_une_variable_var() {
        // The case that makes demotion mandatory: this code is valid in
        // Skyrim, and refusing it would doom the tool on the existing corpus.
        let source = "int Var = 1";
        assert_eq!(
            kinds(source, GameId::Skyrim),
            [
                SyntaxKind::INT_KW,
                SyntaxKind::WHITESPACE,
                SyntaxKind::IDENT, // `Var`
                SyntaxKind::WHITESPACE,
                SyntaxKind::EQ,
                SyntaxKind::WHITESPACE,
                SyntaxKind::INT_NUMBER,
            ]
        );
        // The same source targeting Fallout 4: `Var` is a type there.
        assert_eq!(kinds(source, GameId::Fallout4)[2], SyntaxKind::VAR_KW);
    }

    #[test]
    fn la_promotion_ignore_la_casse() {
        for spelling in ["endfunction", "EndFunction", "ENDFUNCTION", "eNdFuNcTiOn"] {
            assert_eq!(
                kinds(spelling, GameId::Skyrim),
                [SyntaxKind::ENDFUNCTION_KW]
            );
        }
    }

    #[test]
    fn seuls_les_identifiants_sont_promus() {
        // `if` in a string or a comment stays what it is.
        assert_eq!(kinds("\"if\"", GameId::Skyrim), [SyntaxKind::STRING]);
        assert_eq!(kinds("; if", GameId::Skyrim), [SyntaxKind::LINE_COMMENT]);
        assert_eq!(kinds("{if}", GameId::Skyrim), [SyntaxKind::DOC_COMMENT]);
    }

    #[test]
    fn reserved_later_explique_la_retrogradation() {
        let skyrim = Dialect::new(GameId::Skyrim);
        assert_eq!(
            reserved_later("Var", skyrim),
            Some((SyntaxKind::VAR_KW, GameId::Fallout4))
        );
        assert_eq!(
            reserved_later("LockGuard", skyrim),
            Some((SyntaxKind::LOCKGUARD_KW, GameId::Starfield))
        );

        // Already reserved here: not a demotion.
        assert_eq!(reserved_later("if", skyrim), None);
        // Never reserved anywhere.
        assert_eq!(reserved_later("myActor", skyrim), None);

        // Nothing is demoted on the most recent target.
        let starfield = Dialect::new(GameId::Starfield);
        assert_eq!(reserved_later("LockGuard", starfield), None);
    }
}
