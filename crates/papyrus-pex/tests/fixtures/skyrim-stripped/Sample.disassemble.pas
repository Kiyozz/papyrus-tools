.info
	.source "Sample.psc"
	.modifyTime 0 ;Debug info: No
	.compileTime 1793500982 ;Sun Nov 01 03:43:02 2026 Local
	.user "me"
	.computer "BUILD-MACHINE-0"
.endInfo
.userFlagsRef
	.flag conditional 1	; 0x00000001
	.flag hidden 0	; 0x00000000
.endUserFlagsRef
.objectTable
	.object Sample Quest
		.userFlags 1	; Flags: 0x00000001
		.docString "A script going through what a .pex can hold."
		.autoState Idle
		.variableTable
			.variable Stored Float
				.userFlags 0	; Flags: 0x00000000
				.initialValue 0.250000
			.endVariable
			.variable Flag Bool
				.userFlags 0	; Flags: 0x00000000
				.initialValue True
			.endVariable
			.variable ::Count_var Int
				.userFlags 0	; Flags: 0x00000000
				.initialValue 3
			.endVariable
			.variable ::Label_var String
				.userFlags 0	; Flags: 0x00000000
				.initialValue "a \"quoted\" name"
			.endVariable
		.endVariableTable
		.propertyTable
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
						StrCat ::temp8 prefix " and\ta\nline"                    ;@line ??
						Return ::temp8                                           ;@line ??
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
						ArrayCreate ::temp9 4                                    ;@line ??
						Assign values ::temp9                                    ;@line ??
						ArrayLength ::temp11 values                              ;@line ??
						Assign ::temp10 ::temp11                                 ;@line ??
						ArraySetElement values 0 ::temp10                        ;@line ??
						ArrayFindElement values ::temp10 1 0                     ;@line ??
						ArrayRFindElement values ::temp11 2 -1                   ;@line ??
						IAdd ::temp10 ::temp10 ::temp11                          ;@line ??
						Assign found ::temp10                                    ;@line ??
						Return values                                            ;@line ??
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
						IAdd ::temp2 a 1                                         ;@line ??
						IMultiply ::temp3 2 3                                    ;@line ??
						IDivide ::temp3 ::temp3 4                                ;@line ??
						IMod ::temp3 ::temp3 5                                   ;@line ??
						ISubtract ::temp2 ::temp2 ::temp3                        ;@line ??
						Assign total ::temp2                                     ;@line ??
						FAdd ::temp4 b 1.500000                                  ;@line ??
						FMultiply ::temp5 2.500000 3.500000                      ;@line ??
						FDivide ::temp5 ::temp5 4.500000                         ;@line ??
						FSubtract ::temp4 ::temp4 ::temp5                        ;@line ??
						Assign mixed ::temp4                                     ;@line ??
						INegate ::temp3 total                                    ;@line ??
						Assign total ::temp3                                     ;@line ??
						Not ::temp6 Flag                                         ;@line ??
						Cast ::temp6 ::temp6                                     ;@line ??
						JumpF ::temp6 _label0                                    ;@line ??
						CompareEQ ::temp7 a 1                                    ;@line ??
						Cast ::temp6 ::temp7                                     ;@line ??
					_label0:
						Cast ::temp6 ::temp6                                     ;@line ??
						JumpT ::temp6 _label1                                    ;@line ??
						CompareEQ ::temp7 a 2                                    ;@line ??
						Not ::temp7 ::temp7                                      ;@line ??
						Cast ::temp6 ::temp7                                     ;@line ??
					_label1:
						Assign Flag ::temp6                                      ;@line ??
						CompareLT ::temp7 a 1                                    ;@line ??
						Cast ::temp7 ::temp7                                     ;@line ??
						JumpT ::temp7 _label2                                    ;@line ??
						CompareLTE ::temp6 a 2                                   ;@line ??
						Cast ::temp7 ::temp6                                     ;@line ??
					_label2:
						Cast ::temp7 ::temp7                                     ;@line ??
						JumpT ::temp7 _label3                                    ;@line ??
						CompareGT ::temp6 a 3                                    ;@line ??
						Cast ::temp7 ::temp6                                     ;@line ??
					_label3:
						Cast ::temp7 ::temp7                                     ;@line ??
						JumpT ::temp7 _label4                                    ;@line ??
						CompareGTE ::temp6 a 4                                   ;@line ??
						Cast ::temp7 ::temp6                                     ;@line ??
					_label4:
						JumpF ::temp7 _label5                                    ;@line ??
						CallMethod GotoState self ::NoneVar "Busy"               ;@line ??
						Jump _label6                                             ;@line ??
					_label5:
						Cast ::temp6 b                                           ;@line ??
						JumpF ::temp6 _label6                                    ;@line ??
						Assign total 0                                           ;@line ??
						Jump _label6                                             ;@line ??
					_label6:
						CompareLT ::temp6 total 10                               ;@line ??
						JumpF ::temp6 _label7                                    ;@line ??
						IAdd ::temp2 total 1                                     ;@line ??
						Assign total ::temp2                                     ;@line ??
						Jump _label6                                             ;@line ??
					_label7:
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
						Assign other self                                        ;@line ??
						PropGet Count other ::temp13                             ;@line ??
						IAdd ::temp13 ::temp13 1                                 ;@line ??
						Assign ::temp12 ::temp13                                 ;@line ??
						PropSet Count other ::temp12                             ;@line ??
						CallParent Stop ::NoneVar                                ;@line ??
						CallMethod Numbers self ::NoneVar 1 2.000000             ;@line ??
						CallStatic Sample Texts ::temp14 "start"                 ;@line ??
						CallStatic debug Trace ::NoneVar ::temp14 0              ;@line ??
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
						CallMethod Stop self ::NoneVar                           ;@line ??
					.endCode
				.endFunction
			.endState
		.endStateTable
	.endObject
.endObjectTable
