.info
	.source "Sample.psc"
	.modifyTime 1793500966 ;Sun Nov 01 03:42:46 2026 Local
	.compileTime 1793500982 ;Sun Nov 01 03:43:02 2026 Local
	.user "me"
	.computer "BUILD-MACHINE-0"
.endInfo
.userFlagsRef
	.flag hidden 0	; 0x00000000
	.flag conditional 1	; 0x00000001
.endUserFlagsRef
.objectTable
	.object Sample Quest
		.userFlags 1	; Flags: 0x00000001
		.docString "A script going through what a .pex can hold."
		.autoState Idle
		.variableTable
			.variable Flag Bool
				.userFlags 0	; Flags: 0x00000000
				.initialValue True
			.endVariable
			.variable ::Count_var Int
				.userFlags 0	; Flags: 0x00000000
				.initialValue 3
			.endVariable
			.variable Stored Float
				.userFlags 0	; Flags: 0x00000000
				.initialValue 0.250000
			.endVariable
			.variable ::Label_var String
				.userFlags 0	; Flags: 0x00000000
				.initialValue "a \"quoted\" name"
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
						FMultiply ::temp0 Stored 2.500000                        ;@line 8
						Return ::temp0                                           ;@line 8
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
						FNegate ::temp1 value                                    ;@line 11
						Assign Stored ::temp1                                    ;@line 11
					.endCode
				.endFunction
			.endProperty
			.property Label String auto
				.userFlags 1	; Flags: 0x00000001
				.docString ""
				.autoVar ::Label_var
			.endProperty
			.property Count Int auto
				.userFlags 0	; Flags: 0x00000000
				.docString ""
				.autoVar ::Count_var
			.endProperty
		.endPropertyTable
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
						.local ::temp8 String
					.endLocalTable
					.code
						StrCat ::temp8 prefix " and\ta\nline"                    ;@line 34
						Return ::temp8                                           ;@line 34
					.endCode
				.endFunction
				.function Calls
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
						.local ::temp12 Int
						.local ::temp13 Int
						.local ::NoneVar None
						.local ::temp14 String
						.local other Sample
					.endLocalTable
					.code
						Assign other self                                        ;@line 45
						PropGet Count other ::temp13                             ;@line 46
						IAdd ::temp13 ::temp13 1                                 ;@line 46
						Assign ::temp12 ::temp13                                 ;@line 46
						PropSet Count other ::temp12                             ;@line 46
						CallParent Stop ::NoneVar                                ;@line 47
						CallMethod Numbers self ::NoneVar 1 2.000000             ;@line 48
						CallStatic Sample Texts ::temp14 "start"                 ;@line 49
						CallStatic debug Trace ::NoneVar ::temp14 0              ;@line 49
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
						.local ::temp4 Float
						.local ::temp5 Float
						.local ::temp6 Bool
						.local ::temp7 Bool
						.local total Int
						.local mixed Float
						.local ::NoneVar None
					.endLocalTable
					.code
						IAdd ::temp2 a 1                                         ;@line 19
						IMultiply ::temp3 2 3                                    ;@line 19
						IDivide ::temp3 ::temp3 4                                ;@line 19
						IMod ::temp3 ::temp3 5                                   ;@line 19
						ISubtract ::temp2 ::temp2 ::temp3                        ;@line 19
						Assign total ::temp2                                     ;@line 19
						FAdd ::temp4 b 1.500000                                  ;@line 20
						FMultiply ::temp5 2.500000 3.500000                      ;@line 20
						FDivide ::temp5 ::temp5 4.500000                         ;@line 20
						FSubtract ::temp4 ::temp4 ::temp5                        ;@line 20
						Assign mixed ::temp4                                     ;@line 20
						INegate ::temp3 total                                    ;@line 21
						Assign total ::temp3                                     ;@line 21
						Not ::temp6 Flag                                         ;@line 22
						Cast ::temp6 ::temp6                                     ;@line 22
						JumpF ::temp6 _label0                                    ;@line 22
						CompareEQ ::temp7 a 1                                    ;@line 22
						Cast ::temp6 ::temp7                                     ;@line 22
					_label0:
						Cast ::temp6 ::temp6                                     ;@line 22
						JumpT ::temp6 _label1                                    ;@line 22
						CompareEQ ::temp7 a 2                                    ;@line 22
						Not ::temp7 ::temp7                                      ;@line 22
						Cast ::temp6 ::temp7                                     ;@line 22
					_label1:
						Assign Flag ::temp6                                      ;@line 22
						CompareLT ::temp7 a 1                                    ;@line 23
						Cast ::temp7 ::temp7                                     ;@line 23
						JumpT ::temp7 _label2                                    ;@line 23
						CompareLTE ::temp6 a 2                                   ;@line 23
						Cast ::temp7 ::temp6                                     ;@line 23
					_label2:
						Cast ::temp7 ::temp7                                     ;@line 23
						JumpT ::temp7 _label3                                    ;@line 23
						CompareGT ::temp6 a 3                                    ;@line 23
						Cast ::temp7 ::temp6                                     ;@line 23
					_label3:
						Cast ::temp7 ::temp7                                     ;@line 23
						JumpT ::temp7 _label4                                    ;@line 23
						CompareGTE ::temp6 a 4                                   ;@line 23
						Cast ::temp7 ::temp6                                     ;@line 23
					_label4:
						JumpF ::temp7 _label5                                    ;@line 23
						CallMethod GotoState self ::NoneVar "Busy"               ;@line 24
						Jump _label6                                             ;@line 24
					_label5:
						Cast ::temp6 b                                           ;@line 25
						JumpF ::temp6 _label6                                    ;@line 25
						Assign total 0                                           ;@line 26
						Jump _label6                                             ;@line 26
					_label6:
						CompareLT ::temp6 total 10                               ;@line 28
						JumpF ::temp6 _label7                                    ;@line 28
						IAdd ::temp2 total 1                                     ;@line 29
						Assign total ::temp2                                     ;@line 29
						Jump _label6                                             ;@line 29
					_label7:
					.endCode
				.endFunction
				.function GetState
					.userFlags 0	; Flags: 0x00000000
					.docString "Function that returns the current state"
					.return String
					.paramTable
					.endParamTable
					.localTable
					.endLocalTable
					.code
						Return ::State                                           ;@line ??
					.endCode
				.endFunction
				.function GotoState
					.userFlags 0	; Flags: 0x00000000
					.docString "Function that switches this object to the specified state"
					.return None
					.paramTable
						.param newState String
					.endParamTable
					.localTable
						.local ::NoneVar None
					.endLocalTable
					.code
						CallMethod onEndState self ::NoneVar                     ;@line ??
						Assign ::State newState                                  ;@line ??
						CallMethod onBeginState self ::NoneVar                   ;@line ??
					.endCode
				.endFunction
				.function Arrays
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return Int[]
					.paramTable
						.param size Int
					.endParamTable
					.localTable
						.local ::temp9 Int[]
						.local ::temp10 Int
						.local ::temp11 Int
						.local values Int[]
						.local found Int
					.endLocalTable
					.code
						ArrayCreate ::temp9 4                                    ;@line 38
						Assign values ::temp9                                    ;@line 38
						ArrayLength ::temp11 values                              ;@line 39
						Assign ::temp10 ::temp11                                 ;@line 39
						ArraySetElement values 0 ::temp10                        ;@line 39
						ArrayFindElement values ::temp10 1 0                     ;@line 40
						ArrayRFindElement values ::temp11 2 -1                   ;@line 40
						IAdd ::temp10 ::temp10 ::temp11                          ;@line 40
						Assign found ::temp10                                    ;@line 40
						Return values                                            ;@line 41
					.endCode
				.endFunction
			.endState
			.state Busy
				.function onBeginState
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
						.local ::NoneVar None
					.endLocalTable
					.code
						CallMethod Stop self ::NoneVar                           ;@line 54
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
