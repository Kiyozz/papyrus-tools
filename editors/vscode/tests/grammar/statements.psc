; SYNTAX TEST "source.papyrus" "control flow, calls, literals and operators"
  While i < 10 && !akTarget.IsDead()
; ^^^^^ keyword.control.papyrus
;         ^ keyword.operator.comparison.papyrus
;           ^^ constant.numeric.papyrus
;              ^^ keyword.operator.logical.papyrus
;                 ^ keyword.operator.logical.papyrus
;                           ^^^^^^ entity.name.function.call.papyrus
    i += 1
;     ^^ keyword.operator.assignment.papyrus
;        ^ constant.numeric.papyrus
  EndWhile
; ^^^^^^^^ keyword.control.papyrus
  If akTarget as Actor
; ^^ keyword.control.papyrus
;             ^^ keyword.operator.papyrus
;                ^^^^^ entity.name.type.papyrus
    akTarget.SetValue(0x1F, 1.5)
;            ^^^^^^^^ entity.name.function.call.papyrus
;                     ^^^^ constant.numeric.hex.papyrus
;                           ^^^ constant.numeric.papyrus
  ElseIf akTarget is Actor
; ^^^^^^ keyword.control.papyrus
;                 ^^ keyword.operator.papyrus
;                    ^^^^^ entity.name.type.papyrus
    Debug.Trace("done")
;         ^^^^^ entity.name.function.call.papyrus
;               ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;                ^^^^ string.quoted.double.papyrus
;                    ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
  Else
; ^^^^ keyword.control.papyrus
    Return
;   ^^^^^^ keyword.control.papyrus
  EndIf
; ^^^^^ keyword.control.papyrus
  Int[] counts = new Int[5]
; ^^^^^ storage.type.primitive.papyrus
;       ^^^^^^ variable.other.declaration.papyrus
;              ^ keyword.operator.assignment.papyrus
;                ^^^ keyword.operator.papyrus
;                    ^^^ storage.type.primitive.papyrus
;                        ^ constant.numeric.papyrus
  Point p = new Point
; ^^^^^ entity.name.type.papyrus
;       ^ variable.other.declaration.papyrus
;         ^ keyword.operator.assignment.papyrus
;           ^^^ keyword.operator.papyrus
;               ^^^^^ entity.name.type.papyrus
  Bool b = akTarget == None || Self != Parent
; ^^^^ storage.type.primitive.papyrus
;      ^ variable.other.declaration.papyrus
;        ^ keyword.operator.assignment.papyrus
;                   ^^ keyword.operator.comparison.papyrus
;                      ^^^^ constant.language.papyrus
;                           ^^ keyword.operator.logical.papyrus
;                              ^^^^ variable.language.papyrus
;                                   ^^ keyword.operator.comparison.papyrus
;                                      ^^^^^^ variable.language.papyrus
  Float f = -1.5 * 2 / 3 % 4
; ^^^^^ storage.type.primitive.papyrus
;       ^ variable.other.declaration.papyrus
;         ^ keyword.operator.assignment.papyrus
;           ^ keyword.operator.arithmetic.papyrus
;            ^^^ constant.numeric.papyrus
;                ^ keyword.operator.arithmetic.papyrus
;                  ^ constant.numeric.papyrus
;                    ^ keyword.operator.arithmetic.papyrus
;                      ^ constant.numeric.papyrus
;                        ^ keyword.operator.arithmetic.papyrus
;                          ^ constant.numeric.papyrus
  b = i >= 2 && i <= 3 || i > 4
;   ^ keyword.operator.assignment.papyrus
;       ^^ keyword.operator.comparison.papyrus
;          ^ constant.numeric.papyrus
;            ^^ keyword.operator.logical.papyrus
;                 ^^ keyword.operator.comparison.papyrus
;                    ^ constant.numeric.papyrus
;                      ^^ keyword.operator.logical.papyrus
;                           ^ keyword.operator.comparison.papyrus
;                             ^ constant.numeric.papyrus
  Debug.Notification("long" + \
;       ^^^^^^^^^^^^ entity.name.function.call.papyrus
;                    ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;                     ^^^^ string.quoted.double.papyrus
;                         ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
;                           ^ keyword.operator.arithmetic.papyrus
;                             ^^ punctuation.separator.continuation.papyrus
    "line")
;   ^ string.quoted.double.papyrus punctuation.definition.string.begin.papyrus
;    ^^^^ string.quoted.double.papyrus
;        ^ string.quoted.double.papyrus punctuation.definition.string.end.papyrus
  Utility.Wait(afSeconds = 1.0)
;         ^^^^ entity.name.function.call.papyrus
;                        ^ keyword.operator.assignment.papyrus
;                          ^^^ constant.numeric.papyrus
