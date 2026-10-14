; SYNTAX TEST "source.papyrus" "comments and strings"
Int x = 1 ; after code
; <--- storage.type.primitive.papyrus
;   ^ variable.other.declaration.papyrus
;     ^ keyword.operator.assignment.papyrus
;       ^ constant.numeric.papyrus
;         ^ comment.line.semicolon.papyrus punctuation.definition.comment.papyrus
;          ^^^^^^^^^^^ comment.line.semicolon.papyrus
;/ a block
; <-- comment.block.papyrus punctuation.definition.comment.papyrus
; ^^^^^^^^^ comment.block.papyrus
   Int y = 2
; <------------- comment.block.papyrus
/;
; <-- comment.block.papyrus punctuation.definition.comment.papyrus
{ Documentation
; <- comment.block.documentation.papyrus punctuation.definition.comment.papyrus
;^^^^^^^^^^^^^^^ comment.block.documentation.papyrus
  Int z = 3 }
; <------------ comment.block.documentation.papyrus
;           ^ comment.block.documentation.papyrus punctuation.definition.comment.papyrus
String s = "; not a comment"
; <------ storage.type.primitive.papyrus
;      ^ variable.other.declaration.papyrus
;        ^ keyword.operator.assignment.papyrus
;          ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;           ^^^^^^^^^^^^^^^ string.quoted.double.papyrus
;                          ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
String t = "a \"quote\" and \\n"
; <------ storage.type.primitive.papyrus
;      ^ variable.other.declaration.papyrus
;        ^ keyword.operator.assignment.papyrus
;          ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;           ^^ string.quoted.double.papyrus
;             ^^ string.quoted.double.papyrus constant.character.escape.papyrus
;               ^^^^^ string.quoted.double.papyrus
;                    ^^ string.quoted.double.papyrus constant.character.escape.papyrus
;                      ^^^^^ string.quoted.double.papyrus
;                           ^^ string.quoted.double.papyrus constant.character.escape.papyrus
;                             ^ string.quoted.double.papyrus
;                              ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
String u = "{ not documentation }"
; <------ storage.type.primitive.papyrus
;      ^ variable.other.declaration.papyrus
;        ^ keyword.operator.assignment.papyrus
;          ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;           ^^^^^^^^^^^^^^^^^^^^^ string.quoted.double.papyrus
;                                ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
