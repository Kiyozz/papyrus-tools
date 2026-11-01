.info
	.source "C:\\Mods\\Mod\\WorkSpace\\Github\\papyrus-linter\\crates\\papyrus-pex\\tests\\fixtures\\fallout4\\Sample.psc"
	.modifyTime 1793500966 ;Sun Nov 01 03:42:46 2026 Local
	.compileTime 1793500983 ;Sun Nov 01 03:43:03 2026 Local
	.user "me"
	.computer "BUILD-MACHINE-0"
.endInfo
.userFlagsRef
	.flag conditional 1	; 0x00000001
	.flag mandatory 5	; 0x00000005
	.flag default 2	; 0x00000002
	.flag collapsedonref 3	; 0x00000003
	.flag collapsedonbase 4	; 0x00000004
	.flag hidden 0	; 0x00000000
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
			.variable ::Name_var String const
				.userFlags 0	; Flags: 0x00000000
				.initialValue "a \"quoted\" name"
			.endVariable
			.variable ::Count_var Int
				.userFlags 0	; Flags: 0x00000000
				.initialValue 3
			.endVariable
			.variable Stored Float
				.userFlags 0	; Flags: 0x00000000
				.initialValue 0.250000
			.endVariable
		.endVariableTable
		.propertyTable
			.property Name String auto
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.autoVar ::Name_var
			.endProperty
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
						FMultiply ::temp0 Stored 2.500000                        ;@line 17
						Return ::temp0                                           ;@line 17
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
						FNegate ::temp1 value                                    ;@line 20
						Assign Stored ::temp1                                    ;@line 20
					.endCode
				.endFunction
			.endProperty
			.property Count Int auto
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.autoVar ::Count_var
			.endProperty
		.endPropertyTable
		.propertyGroupTable
			.propertyGroup Settings
				.userFlags 8	; Flags: 0x00000008
				.docString ""
				.property Count
				.property Name
			.endPropertyGroup
			.propertyGroup 
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.property Ratio
			.endPropertyGroup
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
						StrCat ::temp20 prefix " and\ta\nline"                   ;@line 41
						Return ::temp20                                          ;@line 41
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
						IAdd ::temp2 a 1                                         ;@line 27
						IMultiply ::temp3 2 3                                    ;@line 27
						IDivide ::temp4 ::temp3 4                                ;@line 27
						IMod ::temp5 ::temp4 5                                   ;@line 27
						ISubtract ::temp6 ::temp2 ::temp5                        ;@line 27
						Assign total ::temp6                                     ;@line 27
						FAdd ::temp7 b 1.500000                                  ;@line 28
						FMultiply ::temp8 2.500000 3.500000                      ;@line 28
						FDivide ::temp9 ::temp8 4.500000                         ;@line 28
						FSubtract ::temp10 ::temp7 ::temp9                       ;@line 28
						Assign mixed ::temp10                                    ;@line 28
						INegate ::temp11 total                                   ;@line 29
						Assign total ::temp11                                    ;@line 29
						CompareLT ::temp12 a 1                                   ;@line 30
						Cast ::temp14 ::temp12                                   ;@line 30
						JumpT ::temp14 _label0                                   ;@line 30
						CompareLTE ::temp13 a 2                                  ;@line 30
						Cast ::temp14 ::temp13                                   ;@line 30
					_label0:
						Cast ::temp16 ::temp14                                   ;@line 30
						JumpT ::temp16 _label1                                   ;@line 30
						CompareGT ::temp15 a 3                                   ;@line 30
						Cast ::temp16 ::temp15                                   ;@line 30
					_label1:
						Cast ::temp18 ::temp16                                   ;@line 30
						JumpT ::temp18 _label2                                   ;@line 30
						CompareGTE ::temp17 a 4                                  ;@line 30
						Cast ::temp18 ::temp17                                   ;@line 30
					_label2:
						JumpF ::temp18 _label3                                   ;@line 30
						CallMethod GotoState self ::nonevar "Busy"               ;@line 31
						Jump _label4                                             ;@line 31
					_label3:
						Cast ::temp19 b                                          ;@line 32
						JumpF ::temp19 _label4                                   ;@line 32
						Assign total 0                                           ;@line 33
						Jump _label4                                             ;@line 33
					_label4:
						CompareLT ::temp12 total 10                              ;@line 35
						JumpF ::temp12 _label5                                   ;@line 35
						IAdd ::temp2 total 1                                     ;@line 36
						Assign total ::temp2                                     ;@line 36
						Jump _label4                                             ;@line 36
					_label5:
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
						Assign other self                                        ;@line 60
						PropGet Count other ::temp31                             ;@line 61
						IAdd ::temp32 ::temp31 1                                 ;@line 61
						Assign ::temp30 ::temp32                                 ;@line 61
						PropSet Count other ::temp30                             ;@line 61
						CallParent Stop ::nonevar                                ;@line 62
						CallMethod Numbers self ::nonevar 1 2.000000             ;@line 63
						CallStatic Sample Texts ::temp33 "start"                 ;@line 64
						CallStatic debug Trace ::nonevar ::temp33 0              ;@line 64
						Is ::temp34 other Quest                                  ;@line 65
						Assign same ::temp34                                     ;@line 65
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
						ArrayCreate ::temp21 size                                ;@line 45
						Assign points ::temp21                                   ;@line 45
						StructCreate ::temp22                                    ;@line 46
						Assign one ::temp22                                      ;@line 46
						ArrayLength ::temp24 points                              ;@line 47
						Assign ::temp23 ::temp24                                 ;@line 47
						StructSet one X ::temp23                                 ;@line 47
						Assign ::temp22 one                                      ;@line 48
						ArraySetElement points 0 ::temp22                        ;@line 48
						ArrayAddElements points one 2                            ;@line 49
						ArrayInsertElement points one 1                          ;@line 50
						ArrayRemoveElements points 0 1                           ;@line 51
						ArrayRemoveLastElement points                            ;@line 52
						ArrayFindStruct points ::temp23 "X" 1 0                  ;@line 53
						ArrayRFindStruct points ::temp24 "X" 2 -1                ;@line 53
						IAdd ::temp25 ::temp23 ::temp24                          ;@line 53
						Assign found ::temp25                                    ;@line 53
						ArrayFindElement points ::temp26 one 0                   ;@line 54
						ArrayRFindElement points ::temp27 one -1                 ;@line 54
						IAdd ::temp28 ::temp26 ::temp27                          ;@line 54
						IAdd ::temp29 found ::temp28                             ;@line 54
						Assign found ::temp29                                    ;@line 54
						ArrayClearElements points                                ;@line 55
						Return points                                            ;@line 56
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
						CallMethod Stop self ::nonevar                           ;@line 70
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
