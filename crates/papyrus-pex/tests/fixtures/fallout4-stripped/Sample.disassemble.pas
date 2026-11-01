.info
	.source "C:\\Mods\\Mod\\WorkSpace\\Github\\papyrus-linter\\crates\\papyrus-pex\\tests\\fixtures\\fallout4\\Sample.psc"
	.modifyTime 0 ;Debug info: No
	.compileTime 1793500983 ;Sun Nov 01 03:43:03 2026 Local
	.user "me"
	.computer "BUILD-MACHINE-0"
.endInfo
.userFlagsRef
	.flag default 2	; 0x00000002
	.flag conditional 1	; 0x00000001
	.flag hidden 0	; 0x00000000
	.flag collapsedonbase 4	; 0x00000004
	.flag mandatory 5	; 0x00000005
	.flag collapsedonref 3	; 0x00000003
.endUserFlagsRef
.objectTable
	.object Sample Quest
		.userFlags 1	; Flags: 0x00000001
		.docString "A script going through what a .pex can hold."
		.autoState Idle
		.structTable
			.struct Point
				.variable X Int
					.userFlags 0	; Flags: 0x00000000
					.initialValue 1
					.docString "Across."
				.endVariable
				.variable Y Float
					.userFlags 0	; Flags: 0x00000000
					.initialValue None
					.docString ""
				.endVariable
			.endStruct
		.endStructTable
		.variableTable
			.variable Stored Float
				.userFlags 0	; Flags: 0x00000000
				.initialValue 0.250000
			.endVariable
			.variable ::Name_var String const
				.userFlags 0	; Flags: 0x00000000
				.initialValue "a \"quoted\" name"
			.endVariable
			.variable ::Count_var Int
				.userFlags 0	; Flags: 0x00000000
				.initialValue 3
			.endVariable
		.endVariableTable
		.propertyTable
			.property Ratio Float 
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.function get
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return Float
					.paramTable
					.endParamTable
					.localTable
						.local ::temp0 Float
					.endLocalTable
					.code
						FMultiply ::temp0 Stored 2.500000                        ;@line ??
						Return ::temp0                                           ;@line ??
					.endCode
				.endFunction
				.function set
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
						.param value Float
					.endParamTable
					.localTable
						.local ::temp1 Float
					.endLocalTable
					.code
						FNegate ::temp1 value                                    ;@line ??
						Assign Stored ::temp1                                    ;@line ??
					.endCode
				.endFunction
			.endProperty
			.property Count Int auto
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.autoVar ::Count_var
			.endProperty
			.property Name String auto
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.autoVar ::Name_var
			.endProperty
		.endPropertyTable
		.propertyGroupTable
		.endPropertyGroupTable
		.stateTable
			.state 
				.function Texts static
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return String
					.paramTable
						.param prefix String
					.endParamTable
					.localTable
						.local ::temp20 String
					.endLocalTable
					.code
						StrCat ::temp20 prefix " and\ta\nline"                   ;@line ??
						Return ::temp20                                          ;@line ??
					.endCode
				.endFunction
				.function Calls
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
						.local ::temp30 Int
						.local ::temp31 Int
						.local ::temp32 Int
						.local ::nonevar None
						.local ::temp33 String
						.local ::temp34 Bool
						.local other Sample
						.local same Bool
					.endLocalTable
					.code
						Assign other self                                        ;@line ??
						PropGet Count other ::temp31                             ;@line ??
						IAdd ::temp32 ::temp31 1                                 ;@line ??
						Assign ::temp30 ::temp32                                 ;@line ??
						PropSet Count other ::temp30                             ;@line ??
						CallParent Stop ::nonevar                                ;@line ??
						CallMethod Numbers self ::nonevar 1 2.000000             ;@line ??
						CallStatic Sample Texts ::temp33 "start"                 ;@line ??
						CallStatic debug Trace ::nonevar ::temp33 0              ;@line ??
						Is ::temp34 other Quest                                  ;@line ??
						Assign same ::temp34                                     ;@line ??
					.endCode
				.endFunction
				.function Arrays
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return sample#point[]
					.paramTable
						.param size Int
					.endParamTable
					.localTable
						.local ::temp21 sample#point[]
						.local ::temp22 sample#point
						.local ::temp23 Int
						.local ::temp24 Int
						.local ::nonevar None
						.local ::temp25 Int
						.local ::temp26 Int
						.local ::temp27 Int
						.local ::temp28 Int
						.local ::temp29 Int
						.local points sample#point[]
						.local one sample#point
						.local found Int
					.endLocalTable
					.code
						ArrayCreate ::temp21 size                                ;@line ??
						Assign points ::temp21                                   ;@line ??
						StructCreate ::temp22                                    ;@line ??
						Assign one ::temp22                                      ;@line ??
						ArrayLength ::temp24 points                              ;@line ??
						Assign ::temp23 ::temp24                                 ;@line ??
						StructSet one X ::temp23                                 ;@line ??
						Assign ::temp22 one                                      ;@line ??
						ArraySetElement points 0 ::temp22                        ;@line ??
						ArrayAddElements points one 2                            ;@line ??
						ArrayInsertElement points one 1                          ;@line ??
						ArrayRemoveElements points 0 1                           ;@line ??
						ArrayRemoveLastElement points                            ;@line ??
						ArrayFindStruct points ::temp23 "X" 1 0                  ;@line ??
						ArrayRFindStruct points ::temp24 "X" 2 -1                ;@line ??
						IAdd ::temp25 ::temp23 ::temp24                          ;@line ??
						Assign found ::temp25                                    ;@line ??
						ArrayFindElement points ::temp26 one 0                   ;@line ??
						ArrayRFindElement points ::temp27 one -1                 ;@line ??
						IAdd ::temp28 ::temp26 ::temp27                          ;@line ??
						IAdd ::temp29 found ::temp28                             ;@line ??
						Assign found ::temp29                                    ;@line ??
						ArrayClearElements points                                ;@line ??
						Return points                                            ;@line ??
					.endCode
				.endFunction
				.function Numbers
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
						.param a Int
						.param b Float
					.endParamTable
					.localTable
						.local ::temp2 Int
						.local ::temp3 Int
						.local ::temp4 Int
						.local ::temp5 Int
						.local ::temp6 Int
						.local ::temp7 Float
						.local ::temp8 Float
						.local ::temp9 Float
						.local ::temp10 Float
						.local ::temp11 Int
						.local ::temp12 Bool
						.local ::temp13 Bool
						.local ::temp14 Bool
						.local ::temp15 Bool
						.local ::temp16 Bool
						.local ::temp17 Bool
						.local ::temp18 Bool
						.local ::temp19 Bool
						.local total Int
						.local mixed Float
						.local ::nonevar None
					.endLocalTable
					.code
						IAdd ::temp2 a 1                                         ;@line ??
						IMultiply ::temp3 2 3                                    ;@line ??
						IDivide ::temp4 ::temp3 4                                ;@line ??
						IMod ::temp5 ::temp4 5                                   ;@line ??
						ISubtract ::temp6 ::temp2 ::temp5                        ;@line ??
						Assign total ::temp6                                     ;@line ??
						FAdd ::temp7 b 1.500000                                  ;@line ??
						FMultiply ::temp8 2.500000 3.500000                      ;@line ??
						FDivide ::temp9 ::temp8 4.500000                         ;@line ??
						FSubtract ::temp10 ::temp7 ::temp9                       ;@line ??
						Assign mixed ::temp10                                    ;@line ??
						INegate ::temp11 total                                   ;@line ??
						Assign total ::temp11                                    ;@line ??
						CompareLT ::temp12 a 1                                   ;@line ??
						Cast ::temp14 ::temp12                                   ;@line ??
						JumpT ::temp14 _label0                                   ;@line ??
						CompareLTE ::temp13 a 2                                  ;@line ??
						Cast ::temp14 ::temp13                                   ;@line ??
					_label0:
						Cast ::temp16 ::temp14                                   ;@line ??
						JumpT ::temp16 _label1                                   ;@line ??
						CompareGT ::temp15 a 3                                   ;@line ??
						Cast ::temp16 ::temp15                                   ;@line ??
					_label1:
						Cast ::temp18 ::temp16                                   ;@line ??
						JumpT ::temp18 _label2                                   ;@line ??
						CompareGTE ::temp17 a 4                                  ;@line ??
						Cast ::temp18 ::temp17                                   ;@line ??
					_label2:
						JumpF ::temp18 _label3                                   ;@line ??
						CallMethod GotoState self ::nonevar "Busy"               ;@line ??
						Jump _label4                                             ;@line ??
					_label3:
						Cast ::temp19 b                                          ;@line ??
						JumpF ::temp19 _label4                                   ;@line ??
						Assign total 0                                           ;@line ??
						Jump _label4                                             ;@line ??
					_label4:
						CompareLT ::temp12 total 10                              ;@line ??
						JumpF ::temp12 _label5                                   ;@line ??
						IAdd ::temp2 total 1                                     ;@line ??
						Assign total ::temp2                                     ;@line ??
						Jump _label4                                             ;@line ??
					_label5:
					.endCode
				.endFunction
			.endState
			.state Busy
				.function OnBeginState
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
						.param previous String
					.endParamTable
					.localTable
						.local ::nonevar None
					.endLocalTable
					.code
						CallMethod Stop self ::nonevar                           ;@line ??
					.endCode
				.endFunction
			.endState
			.state Idle
				.function Calls
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
					.endLocalTable
					.code
					.endCode
				.endFunction
			.endState
		.endStateTable
	.endObject
.endObjectTable
