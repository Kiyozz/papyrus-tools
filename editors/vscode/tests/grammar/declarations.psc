; SYNTAX TEST "source.papyrus" "properties, variables, functions, events, states, structs, groups"
Actor Property PlayerRef Auto Const Mandatory
; <----- entity.name.type.papyrus
;     ^^^^^^^^ storage.type.property.papyrus
;              ^^^^^^^^^ variable.other.property.papyrus
;                        ^^^^ storage.modifier.papyrus
;                             ^^^^^ storage.modifier.papyrus
;                                   ^^^^^^^^^ storage.modifier.papyrus
Int Property Count = 0 Auto Hidden
; <--- storage.type.primitive.papyrus
;   ^^^^^^^^ storage.type.property.papyrus
;            ^^^^^ variable.other.property.papyrus
;                  ^ keyword.operator.assignment.papyrus
;                    ^ constant.numeric.papyrus
;                      ^^^^ storage.modifier.papyrus
;                           ^^^^^^ storage.modifier.papyrus
Float[] Property Weights Auto
; <------- storage.type.primitive.papyrus
;       ^^^^^^^^ storage.type.property.papyrus
;                ^^^^^^^ variable.other.property.papyrus
;                        ^^^^ storage.modifier.papyrus
String Property Name
; <------ storage.type.primitive.papyrus
;      ^^^^^^^^ storage.type.property.papyrus
;               ^^^^ variable.other.property.papyrus
  String Function Get()
; ^^^^^^ storage.type.primitive.papyrus
;        ^^^^^^^^ storage.type.function.papyrus
;                 ^^^ entity.name.function.papyrus
  EndFunction
; ^^^^^^^^^^^ storage.type.function.papyrus
EndProperty
; <----------- storage.type.property.papyrus
Int counter = 0
; <--- storage.type.primitive.papyrus
;   ^^^^^^^ variable.other.declaration.papyrus
;           ^ keyword.operator.assignment.papyrus
;             ^ constant.numeric.papyrus
ObjectReference[] targets
; <----------------- entity.name.type.papyrus
;                 ^^^^^^^ variable.other.declaration.papyrus
Struct Point
; <------ storage.type.struct.papyrus
;      ^^^^^ entity.name.type.struct.papyrus
  Float X = 0.0
; ^^^^^ storage.type.primitive.papyrus
;       ^ variable.other.declaration.papyrus
;         ^ keyword.operator.assignment.papyrus
;           ^^^ constant.numeric.papyrus
EndStruct
; <--------- storage.type.struct.papyrus
Group Settings CollapsedOnRef
; <----- storage.type.group.papyrus
;     ^^^^^^^^ entity.name.section.group.papyrus
;              ^^^^^^^^^^^^^^ storage.modifier.papyrus
EndGroup
; <-------- storage.type.group.papyrus
CustomEvent OnTick
; <----------- storage.type.event.papyrus
;           ^^^^^^ entity.name.function.event.papyrus
Int Function Total(Actor akTarget, Int aiCount = 1) Global Native
; <--- storage.type.primitive.papyrus
;   ^^^^^^^^ storage.type.function.papyrus
;            ^^^^^ entity.name.function.papyrus
;                  ^^^^^ entity.name.type.papyrus
;                        ^^^^^^^^ variable.other.declaration.papyrus
;                                  ^^^ storage.type.primitive.papyrus
;                                      ^^^^^^^ variable.other.declaration.papyrus
;                                              ^ keyword.operator.assignment.papyrus
;                                                ^ constant.numeric.papyrus
;                                                   ^^^^^^ storage.modifier.papyrus
;                                                          ^^^^^^ storage.modifier.papyrus
Event OnInit()
; <----- storage.type.function.papyrus
;     ^^^^^^ entity.name.function.papyrus
EndEvent
; <-------- storage.type.function.papyrus
Event Actor.OnDeath(Actor akSender, Actor akKiller)
; <----- storage.type.function.papyrus
;     ^^^^^^^^^^^^^ entity.name.function.papyrus
;                   ^^^^^ entity.name.type.papyrus
;                         ^^^^^^^^ variable.other.declaration.papyrus
;                                   ^^^^^ entity.name.type.papyrus
;                                         ^^^^^^^^ variable.other.declaration.papyrus
EndEvent
; <-------- storage.type.function.papyrus
Auto State Waiting
; <---- storage.modifier.papyrus
;    ^^^^^ storage.type.state.papyrus
;          ^^^^^^^ entity.name.section.state.papyrus
EndState
; <-------- storage.type.state.papyrus
State Done
; <----- storage.type.state.papyrus
;     ^^^^ entity.name.section.state.papyrus
endState
; <-------- storage.type.state.papyrus
function lower() native
; <-------- storage.type.function.papyrus
;        ^^^^^ entity.name.function.papyrus
;                ^^^^^^ storage.modifier.papyrus
