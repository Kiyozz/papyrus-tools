//! Le vocabulaire de l'arbre syntaxique.
//!
//! `SyntaxKind` est le contrat entre le lexer, le parser, le formateur et le
//! serveur LSP. Il est `#[repr(u16)]` parce que `rowan` stocke les nœuds sous
//! cette forme.
//!
//! Les *tokens* viennent d'abord, les *nœuds* ensuite : la frontière entre les
//! deux est le discriminant de [`SyntaxKind::SOURCE_FILE`], ce qui rend
//! [`SyntaxKind::is_node`] gratuite.

use papyrus_dialect::GameId;

/// Déclare l'énumération et la table qui permet de la reconstruire.
///
/// `rowan` range les kinds sous forme de `u16` ; il faut donc savoir revenir de
/// `u16` vers `SyntaxKind`. `rust-analyzer` le fait par `transmute` ; ce crate
/// interdit le code `unsafe`, d'où cette table générée. Écrire la liste deux
/// fois à la main serait la garantie qu'elles finiraient par diverger.
macro_rules! syntax_kinds {
    ($( $(#[$attr:meta])* $name:ident ),* $(,)?) => {
        /// Un token ou un nœud de l'arbre Papyrus.
        ///
        /// Les noms suivent la convention de `rust-analyzer` : majuscules pour
        /// les éléments de l'arbre, suffixe `_KW` pour les mots-clés.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        #[allow(non_camel_case_types)]
        pub enum SyntaxKind {
            $( $(#[$attr])* $name, )*
        }

        impl SyntaxKind {
            /// Tous les kinds, dans l'ordre de leur discriminant.
            pub(crate) const ALL: &'static [SyntaxKind] = &[ $( SyntaxKind::$name, )* ];
        }
    };
}

syntax_kinds! {
    // ══ Tokens ════════════════════════════════════════════════════════════

    // ── Trivia ────────────────────────────────────────────────────────────
    //
    // Tout ce que le parser ignore mais que l'arbre conserve, sans quoi ni le
    // formateur ni le LSP ne peuvent fonctionner.
    /// Espaces et tabulations. Jamais un retour à la ligne.
    WHITESPACE,
    /// `; commentaire jusqu'à la fin de la ligne`, retour à la ligne **exclu**.
    LINE_COMMENT,
    /// `;/ commentaire bloc /;`, éventuellement sur plusieurs lignes.
    BLOCK_COMMENT,
    /// `{ documentation }` attachée à une déclaration.
    DOC_COMMENT,
    /// `\` suivi du retour à la ligne : la déclaration continue en dessous.
    ///
    /// C'est de la trivia, mais le formateur ne doit jamais la supprimer — elle
    /// porte la structure de la ligne logique.
    LINE_CONTINUATION,

    // ── Fin de ligne ──────────────────────────────────────────────────────
    /// Retour à la ligne.
    ///
    /// **N'est pas de la trivia.** Papyrus est orienté ligne : la fin de ligne
    /// termine les déclarations comme le point-virgule ailleurs. C'est la
    /// contrainte qui structure toute la grammaire.
    NEWLINE,

    // ── Littéraux et identifiants ─────────────────────────────────────────
    /// Entier décimal.
    INT_NUMBER,
    /// Entier hexadécimal, `0xDEADBEEF`.
    HEX_NUMBER,
    /// Flottant. Exige des chiffres des deux côtés du point : ni `.5`, ni `1.`,
    /// ni notation exponentielle.
    FLOAT_NUMBER,
    /// Chaîne entre guillemets.
    STRING,
    /// Identifiant, ou mot-clé d'un jeu plus récent que la cible.
    IDENT,

    // ── Ponctuation ───────────────────────────────────────────────────────
    /// `+`
    PLUS,
    /// `-`
    MINUS,
    /// `*`
    STAR,
    /// `/`
    SLASH,
    /// `%`
    PERCENT,
    /// `+=`
    PLUS_EQ,
    /// `-=`
    MINUS_EQ,
    /// `*=`
    STAR_EQ,
    /// `/=`
    SLASH_EQ,
    /// `%=`
    PERCENT_EQ,
    /// `=`
    EQ,
    /// `==`
    EQ2,
    /// `!=`
    NEQ,
    /// `<`
    LT,
    /// `>`
    GT,
    /// `<=`
    LTEQ,
    /// `>=`
    GTEQ,
    /// `&&`
    AMP2,
    /// `||`
    PIPE2,
    /// `!`
    BANG,
    /// `.`
    DOT,
    /// `,`
    COMMA,
    /// `(`
    L_PAREN,
    /// `)`
    R_PAREN,
    /// `[`
    L_BRACK,
    /// `]`
    R_BRACK,
    /// `:` — séparateur de namespace, Fallout 4 et au-delà.
    COLON,

    // ── Mots-clés présents dès Skyrim ─────────────────────────────────────
    /// `ScriptName`
    SCRIPTNAME_KW,
    /// `Extends`
    EXTENDS_KW,
    /// `Import`
    IMPORT_KW,
    /// `Function`
    FUNCTION_KW,
    /// `EndFunction`
    ENDFUNCTION_KW,
    /// `Event`
    EVENT_KW,
    /// `EndEvent`
    ENDEVENT_KW,
    /// `Property`
    PROPERTY_KW,
    /// `EndProperty`
    ENDPROPERTY_KW,
    /// `State`
    STATE_KW,
    /// `EndState`
    ENDSTATE_KW,
    /// `If`
    IF_KW,
    /// `ElseIf`
    ELSEIF_KW,
    /// `Else`
    ELSE_KW,
    /// `EndIf`
    ENDIF_KW,
    /// `While`
    WHILE_KW,
    /// `EndWhile`
    ENDWHILE_KW,
    /// `Return`
    RETURN_KW,
    /// `Auto`
    AUTO_KW,
    /// `AutoReadOnly`
    AUTOREADONLY_KW,
    /// `Hidden`
    HIDDEN_KW,
    /// `Conditional`
    CONDITIONAL_KW,
    /// `Native`
    NATIVE_KW,
    /// `Global`
    GLOBAL_KW,
    /// `New`
    NEW_KW,
    /// `As` — le cast. Présent depuis Skyrim, contrairement à `Is`.
    AS_KW,
    /// `Self`
    SELF_KW,
    /// `Parent`
    PARENT_KW,
    /// `None`
    NONE_KW,
    /// `Length`
    LENGTH_KW,
    /// `True`
    TRUE_KW,
    /// `False`
    FALSE_KW,
    /// `Int`
    INT_KW,
    /// `Float`
    FLOAT_KW,
    /// `Bool`
    BOOL_KW,
    /// `String`
    STRING_KW,

    // ── Mots-clés ajoutés par Fallout 4 ───────────────────────────────────
    /// `Struct`
    STRUCT_KW,
    /// `EndStruct`
    ENDSTRUCT_KW,
    /// `Group`
    GROUP_KW,
    /// `EndGroup`
    ENDGROUP_KW,
    /// `Var`
    VAR_KW,
    /// `Is` — test de type. Absent de Skyrim.
    IS_KW,
    /// `Const`
    CONST_KW,
    /// `DebugOnly`
    DEBUGONLY_KW,
    /// `BetaOnly`
    BETAONLY_KW,
    /// `Mandatory`
    MANDATORY_KW,
    /// `Collapsed`
    COLLAPSED_KW,
    /// `CollapsedOnRef`
    COLLAPSEDONREF_KW,
    /// `CollapsedOnBase`
    COLLAPSEDONBASE_KW,
    /// `Default`
    DEFAULT_KW,
    /// `CustomEvent`
    CUSTOMEVENT_KW,
    /// `ScriptEventName`
    SCRIPTEVENTNAME_KW,
    /// `CustomEventName`
    CUSTOMEVENTNAME_KW,
    /// `StructVarName`
    STRUCTVARNAME_KW,

    // ── Mots-clés ajoutés par Starfield ───────────────────────────────────
    /// `Guard` — déclaration de verrou.
    GUARD_KW,
    /// `LockGuard`
    LOCKGUARD_KW,
    /// `EndLockGuard`
    ENDLOCKGUARD_KW,
    /// `TryLockGuard`
    TRYLOCKGUARD_KW,
    /// `ElseTryLockGuard`
    ELSETRYLOCKGUARD_KW,
    /// `EndTryLockGuard`
    ENDTRYLOCKGUARD_KW,
    /// `RequiresGuard`
    REQUIRESGUARD_KW,
    /// `ProtectsFunctionLogic`
    PROTECTSFUNCTIONLOGIC_KW,
    /// `Private`
    PRIVATE_KW,
    /// `Protected`
    PROTECTED_KW,
    /// `SelfOnly`
    SELFONLY_KW,

    // ── Bornes ────────────────────────────────────────────────────────────
    /// Suite d'octets que le lexer n'a pas su reconnaître.
    ///
    /// Toujours produite plutôt qu'ignorée : sans elle, l'arbre ne se
    /// reconstruirait pas à l'identique.
    ERROR,
    /// Fin de fichier. Jamais produite par le lexer, ajoutée par le parser.
    EOF,

    // ══ Nœuds ═════════════════════════════════════════════════════════════
    //
    // `SOURCE_FILE` ouvre la section : tout kind à partir d'ici est un nœud.

    /// La racine : un fichier `.psc` entier.
    SOURCE_FILE,
    /// `ScriptName Foo extends Bar Native Hidden`
    SCRIPT_HEADER,
    /// `extends Foo:Bar`
    EXTENDS_CLAUSE,
    /// `import Foo`
    IMPORT,

    /// Suite de flags. Toujours présente, éventuellement vide, pour que le
    /// formateur ait un endroit où insérer.
    FLAG_LIST,
    /// Un flag, avec ses arguments s'il en prend : `Const`, `RequiresGuard(g)`.
    FLAG,

    /// Nom en position de déclaration.
    NAME,
    /// Nom en position d'usage, `self` et `parent` compris.
    NAME_REF,
    /// `Foo:Bar[]`, `int`, `var`
    TYPE_REF,

    /// `(int aiCount = 3, Actor akRef)`
    PARAM_LIST,
    /// `int aiCount = 3`
    PARAM,

    /// Variable de script ou locale : `int x = 1`.
    VARIABLE_DECL,
    /// `int Property Foo = 1 Auto`, forme courte ou longue.
    PROPERTY_DECL,
    /// `Function Foo() … EndFunction`
    FUNCTION_DECL,
    /// `Event OnInit() … EndEvent`, y compris la forme distante
    /// `Event ObjectReference.OnActivate(…)` de Fallout 4.
    EVENT_DECL,
    /// `Auto State Foo … EndState`
    STATE_DECL,
    /// `Struct Foo … EndStruct`
    STRUCT_DECL,
    /// Un champ de structure : `int Threshold`.
    STRUCT_FIELD,
    /// `Group Foo … EndGroup`
    GROUP_DECL,
    /// `CustomEvent Foo`
    CUSTOM_EVENT_DECL,
    /// `Guard foo ProtectsFunctionLogic`
    GUARD_DECL,

    /// Corps d'une fonction, d'un événement ou d'une branche.
    STMT_LIST,
    /// Une expression employée comme instruction — presque toujours un appel.
    EXPR_STMT,
    /// `x = 1`, `x += 1`
    ASSIGN_STMT,
    /// `Return` ou `Return expr`
    RETURN_STMT,
    /// `If … ElseIf … Else … EndIf`
    IF_STMT,
    /// `ElseIf cond`
    ELSEIF_CLAUSE,
    /// `Else`
    ELSE_CLAUSE,
    /// `While … EndWhile`
    WHILE_STMT,
    /// `LockGuard g … EndLockGuard`
    LOCK_GUARD_STMT,
    /// `TryLockGuard g … EndTryLockGuard`
    TRY_LOCK_GUARD_STMT,
    /// `ElseTryLockGuard`
    ELSE_TRY_LOCK_GUARD_CLAUSE,
    /// Les verrous pris par un `LockGuard`.
    GUARD_LIST,

    /// Nombre, chaîne, `true`, `false`, `none`.
    LITERAL,
    /// `( expr )`
    PAREN_EXPR,
    /// `-x`, `!x`
    UNARY_EXPR,
    /// `a + b`, `a && b`, `a == b`
    BIN_EXPR,
    /// `x as Actor`, `x is Actor`
    CAST_EXPR,
    /// `arr[i]`
    INDEX_EXPR,
    /// `Foo(a, b)`
    CALL_EXPR,
    /// `a.b`
    FIELD_EXPR,
    /// `new float[4]`, `new Foo:Bar`
    NEW_EXPR,
    /// `(a, abForce = true)`
    ARG_LIST,
    /// Un argument, nommé ou positionnel.
    ARG,

    /// Portion que le parser n'a pas su interpréter.
    ///
    /// Les tokens sautés y sont rangés plutôt que perdus : l'arbre reste
    /// reconstructible, et le formateur sait quelle zone ne pas toucher.
    ERROR_TREE,

    /// Nœud ouvert puis abandonné. N'apparaît jamais dans l'arbre final.
    TOMBSTONE,
}

impl SyntaxKind {
    /// Reconstruit un kind depuis le `u16` que stocke `rowan`.
    pub fn from_raw(raw: u16) -> Option<SyntaxKind> {
        Self::ALL.get(raw as usize).copied()
    }

    /// Vrai pour un nœud interne, faux pour un token.
    pub const fn is_node(self) -> bool {
        (self as u16) >= (SyntaxKind::SOURCE_FILE as u16)
    }

    /// Vrai pour les tokens que le parser saute.
    ///
    /// [`SyntaxKind::NEWLINE`] n'en fait pas partie : la fin de ligne porte du
    /// sens en Papyrus.
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::WHITESPACE
                | SyntaxKind::LINE_COMMENT
                | SyntaxKind::BLOCK_COMMENT
                | SyntaxKind::DOC_COMMENT
                | SyntaxKind::LINE_CONTINUATION
        )
    }

    /// Vrai pour les commentaires, sous toutes leurs formes.
    pub const fn is_comment(self) -> bool {
        matches!(
            self,
            SyntaxKind::LINE_COMMENT | SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT
        )
    }

    /// Jeu à partir duquel ce token est un mot-clé, ou `None` s'il n'en est pas un.
    pub fn keyword_since(self) -> Option<GameId> {
        crate::keywords::since(self)
    }

    /// Vrai si ce token est un mot-clé dans au moins un jeu.
    pub fn is_keyword(self) -> bool {
        self.keyword_since().is_some()
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> u16 {
        kind as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_est_l_inverse_du_discriminant() {
        for (index, kind) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(u16::from(*kind) as usize, index, "{kind:?} mal placé");
            assert_eq!(SyntaxKind::from_raw(index as u16), Some(*kind));
        }
        assert_eq!(SyntaxKind::from_raw(SyntaxKind::ALL.len() as u16), None);
    }

    #[test]
    fn tokens_et_noeuds_ne_se_melangent_pas() {
        // La frontière est un simple `>=` sur le discriminant ; encore
        // faut-il qu'aucun token ne se soit glissé après elle.
        assert!(!SyntaxKind::EOF.is_node());
        assert!(!SyntaxKind::IDENT.is_node());
        assert!(SyntaxKind::SOURCE_FILE.is_node());
        assert!(SyntaxKind::ERROR_TREE.is_node());

        for kind in SyntaxKind::ALL {
            if kind.is_node() {
                assert!(!kind.is_trivia(), "{kind:?} est un nœud et de la trivia");
                assert!(!kind.is_keyword(), "{kind:?} est un nœud et un mot-clé");
            }
        }
    }
}
