//! Application du dialecte au flux de tokens.
//!
//! Premier des deux points où la cible intervient — le second étant les
//! diagnostics de compatibilité émis par le parser sur les constructions qui
//! n'ouvrent pas sur un mot-clé.
//!
//! Le principe : un mot n'est réservé que si la cible est assez récente. `Var`
//! est un type en Fallout 4, un nom de variable parfaitement valide en Skyrim.
//! Réserver systématiquement produirait des milliers de fausses erreurs sur le
//! code existant.

use papyrus_dialect::{Dialect, GameId};

use crate::lexer::{self, Token};
use crate::{SyntaxKind, keywords};

/// Découpe `source` en tokens et applique le dialecte.
///
/// C'est le point d'entrée normal du lexage.
pub fn lex_with_dialect(source: &str, dialect: Dialect) -> Vec<Token> {
    let mut tokens = lexer::lex(source);
    promote(&mut tokens, source, dialect);
    tokens
}

/// Promeut en mots-clés les identifiants réservés par la cible.
///
/// Les identifiants dont le mot n'est réservé que par un jeu plus récent
/// restent des [`SyntaxKind::IDENT`] : c'est la « rétrogradation » du plan, vue
/// depuis un lexer qui n'a jamais promu.
pub fn promote(tokens: &mut [Token], source: &str, dialect: Dialect) {
    for token in tokens.iter_mut().filter(|t| t.kind == SyntaxKind::IDENT) {
        if let Some((kind, since)) = keywords::lookup(token.text(source)) {
            if dialect.game.accepts(since) {
                token.kind = kind;
            }
        }
    }
}

/// Si `text` est un mot réservé par un jeu plus récent que la cible, dit lequel.
///
/// Permet d'expliquer pourquoi un mot se comporte ici comme un identifiant, et
/// d'avertir qu'un script portera ailleurs : un nom de variable `Var` accepté
/// par Skyrim casse à la compilation dès Fallout 4.
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
        // Vérifié au compilateur : Skyrim SE rejette `x is Actor`.
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
        // Le cas qui rend la rétrogradation obligatoire : ce code est valide en
        // Skyrim, et le refuser condamnerait l'outil sur le corpus existant.
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
        // La même source vise Fallout 4 : `Var` y est un type.
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
        // `if` dans une chaîne ou un commentaire reste ce qu'il est.
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

        // Déjà réservé ici : ce n'est pas une rétrogradation.
        assert_eq!(reserved_later("if", skyrim), None);
        // Jamais réservé nulle part.
        assert_eq!(reserved_later("myActor", skyrim), None);

        // Rien n'est rétrogradé sur la cible la plus récente.
        let starfield = Dialect::new(GameId::Starfield);
        assert_eq!(reserved_later("LockGuard", starfield), None);
    }
}
