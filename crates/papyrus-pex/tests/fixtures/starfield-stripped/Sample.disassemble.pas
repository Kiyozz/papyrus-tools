.info
	.source "Sample.psc"
	.modifyTime 0 ;Debug info: No
	.compileTime 1793501023 ;Sun Nov  1 03:43:43 2026 Local
	.user "<SUPPRESSED>"
	.computer "<SUPPRESSED>"
.endInfo
.userFlagsRef
	.flag conditional 1	; 0x00000001
	.flag mandatory 5	; 0x00000005
	.flag hidden 0	; 0x00000000
	.flag collapsedonbase 4	; 0x00000004
	.flag collapsedonref 3	; 0x00000003
	.flag default 2	; 0x00000002
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
			.variable Stored Float
				.userFlags 0	; Flags: 0x00000000
				.initialValue 0.250000
			.endVariable
			.variable ::Count_var Int
				.userFlags 0	; Flags: 0x00000000
				.initialValue 3
			.endVariable
		.endVariableTable
		.guardTable
			.guard FrontDoor
			.guard SideWindow
		.endGuardTable
		.propertyTable
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
				.function Arrays
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return sample#point[]
					.paramTable
						.param size Int
					.endParamTable
					.localTable
						.local ::temp13 sample#point[]
						.local ::temp14 sample#point
						.local ::temp15 Int
						.local ::temp16 Int
						.local ::nonevar None
						.local ::temp17 Int
						.local points sample#point[]
						.local one sample#point
						.local found Int
						.local matching sample#point[]
					.endLocalTable
					.code
						ArrayCreate ::temp13 size                               ;@line ??
						Assign points ::temp13                                  ;@line ??
						StructCreate ::temp14                                   ;@line ??
						Assign one ::temp14                                     ;@line ??
						ArrayLength ::temp16 points                             ;@line ??
						Assign ::temp15 ::temp16                                ;@line ??
						StructSet one X ::temp15                                ;@line ??
						ArrayAddElements points one 2                           ;@line ??
						ArrayInsertElement points one 1                         ;@line ??
						ArrayRemoveElements points 0 1                          ;@line ??
						ArrayRemoveLastElement points                           ;@line ??
						ArrayFindStruct points ::temp15 "X" 1 0                 ;@line ??
						ArrayRFindStruct points ::temp16 "X" 2 -1               ;@line ??
						IAdd ::temp17 ::temp15 ::temp16                         ;@line ??
						Assign found ::temp17                                   ;@line ??
						ArrayGetAllMatchingStructs points ::temp13 "X" 1 0 -1   ;@line ??
						Assign matching ::temp13                                ;@line ??
						ArrayClearElements points                               ;@line ??
						Return points                                           ;@line ??
					.endCode
				.endFunction
				.function Calls
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
						.local ::temp19 Int
						.local ::temp20 Int
						.local ::temp21 Int
						.local ::nonevar None
						.local ::temp22 Bool
						.local other Sample
						.local same Bool
					.endLocalTable
					.code
						Assign other self                                       ;@line ??
						PropGet Count other ::temp20                            ;@line ??
						IAdd ::temp21 ::temp20 1                                ;@line ??
						Assign ::temp19 ::temp21                                ;@line ??
						PropSet Count other ::temp19                            ;@line ??
						CallParent Stop ::nonevar                               ;@line ??
						CallMethod Numbers self ::nonevar 1 2.000000            ;@line ??
						Is ::temp22 other Quest                                 ;@line ??
						Assign same ::temp22                                    ;@line ??
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
						.local ::temp0 Int
						.local ::temp1 Int
						.local ::temp2 Int
						.local ::temp3 Int
						.local ::temp4 Int
						.local ::temp5 Bool
						.local ::temp6 Bool
						.local ::temp7 Bool
						.local ::temp8 Bool
						.local ::temp9 Bool
						.local ::temp10 Bool
						.local ::temp11 Bool
						.local ::temp12 Bool
						.local total Int
						.local ::nonevar None
					.endLocalTable
					.code
						IAdd ::temp0 a 1                                        ;@line ??
						IMultiply ::temp1 2 3                                   ;@line ??
						IDivide ::temp2 ::temp1 4                               ;@line ??
						IMod ::temp3 ::temp2 5                                  ;@line ??
						ISubtract ::temp4 ::temp0 ::temp3                       ;@line ??
						Assign total ::temp4                                    ;@line ??
						CompareLT ::temp5 a 1                                   ;@line ??
						Cast ::temp7 ::temp5                                    ;@line ??
						JumpT ::temp7 _label0                                   ;@line ??
						CompareLTE ::temp6 a 2                                  ;@line ??
						Cast ::temp7 ::temp6                                    ;@line ??
					_label0:
						Cast ::temp9 ::temp7                                    ;@line ??
						JumpT ::temp9 _label1                                   ;@line ??
						CompareGT ::temp8 a 3                                   ;@line ??
						Cast ::temp9 ::temp8                                    ;@line ??
					_label1:
						Cast ::temp11 ::temp9                                   ;@line ??
						JumpT ::temp11 _label2                                  ;@line ??
						CompareGTE ::temp10 a 4                                 ;@line ??
						Cast ::temp11 ::temp10                                  ;@line ??
					_label2:
						JumpF ::temp11 _label3                                  ;@line ??
						CallMethod GotoState self ::nonevar "Busy"              ;@line ??
						Jump _label4                                            ;@line ??
					_label3:
						Cast ::temp12 b                                         ;@line ??
						JumpF ::temp12 _label4                                  ;@line ??
						Assign total 0                                          ;@line ??
						Jump _label4                                            ;@line ??
					_label4:
					.endCode
				.endFunction
				.function Guarded
					.userFlags 0	; Flags: 0x00000000
					.docString ""
					.return None
					.paramTable
					.endParamTable
					.localTable
						.local ::temp18 Bool
					.endLocalTable
					.code
						LockGuards FrontDoor SideWindow                         ;@line ??
						Assign Stored 1.000000                                  ;@line ??
						UnlockGuards FrontDoor SideWindow                       ;@line ??
						TryLockGuards ::temp18 FrontDoor                        ;@line ??
						JumpF ::temp18 _label5                                  ;@line ??
						Assign Stored 2.000000                                  ;@line ??
						UnlockGuards FrontDoor                                  ;@line ??
						Jump _label5                                            ;@line ??
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
						CallMethod Stop self ::nonevar                          ;@line ??
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
