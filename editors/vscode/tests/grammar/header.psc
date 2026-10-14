; SYNTAX TEST "source.papyrus" "ScriptName and Import"
ScriptName MyMod:Quests:Main extends Quest Conditional
; <---------- keyword.other.scriptname.papyrus
;          ^^^^^^^^^^^^^^^^^ entity.name.type.class.papyrus
;                            ^^^^^^^ storage.modifier.extends.papyrus
;                                    ^^^^^ entity.other.inherited-class.papyrus
;                                          ^^^^^^^^^^^ storage.modifier.papyrus
Import Utility
; <------ keyword.other.import.papyrus
;      ^^^^^^^ entity.name.type.papyrus
import MyMod:Shared
; <------ keyword.other.import.papyrus
;      ^^^^^^^^^^^^ entity.name.type.papyrus
scriptname lower EXTENDS ObjectReference Hidden
; <---------- keyword.other.scriptname.papyrus
;          ^^^^^ entity.name.type.class.papyrus
;                ^^^^^^^ storage.modifier.extends.papyrus
;                        ^^^^^^^^^^^^^^^ entity.other.inherited-class.papyrus
;                                        ^^^^^^ storage.modifier.papyrus
