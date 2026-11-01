.info
	.source "Sample.psc"
	.modifyTime 1793501022 ;Sun Nov  1 03:43:42 2026 Local
	.compileTime 1793501023 ;Sun Nov  1 03:43:43 2026 Local
	.user "<SUPPRESSED>"
	.computer "<SUPPRESSED>"
.endInfo
.userFlagsRef
	.flag conditional 1	; 0x00000001
	.flag default 2	; 0x00000002
	.flag collapsedonref 3	; 0x00000003
	.flag collapsedonbase 4	; 0x00000004
	.flag hidden 0	; 0x00000000
	.flag mandatory 5	; 0x00000005
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
			.propertyGroup Settings
				.userFlags 8	; Flags: 0x00000008
				.docString ""
				.property Count
				.property Name
			.endPropertyGroup
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
						ArrayCreate ::temp13 size                               ;@line 30
						Assign points ::temp13                                  ;@line 30
						StructCreate ::temp14                                   ;@line 31
						Assign one ::temp14                                     ;@line 31
						ArrayLength ::temp16 points                             ;@line 32
						Assign ::temp15 ::temp16                                ;@line 32
						StructSet one X ::temp15                                ;@line 32
						ArrayAddElements points one 2                           ;@line 33
						ArrayInsertElement points one 1                         ;@line 34
						ArrayRemoveElements points 0 1                          ;@line 35
						ArrayRemoveLastElement points                           ;@line 36
						ArrayFindStruct points ::temp15 "X" 1 0                 ;@line 37
						ArrayRFindStruct points ::temp16 "X" 2 -1               ;@line 37
						IAdd ::temp17 ::temp15 ::temp16                         ;@line 37
						Assign found ::temp17                                   ;@line 37
						ArrayGetAllMatchingStructs points ::temp13 "X" 1 0 -1   ;@line 38
						Assign matching ::temp13                                ;@line 38
						ArrayClearElements points                               ;@line 39
						Return points                                           ;@line 40
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
						LockGuards FrontDoor SideWindow                         ;@line 44
						Assign Stored 1.000000                                  ;@line 45
						UnlockGuards FrontDoor SideWindow                       ;@line 46
						TryLockGuards ::temp18 FrontDoor                        ;@line 47
						JumpF ::temp18 _label0                                  ;@line 47
						Assign Stored 2.000000                                  ;@line 48
						UnlockGuards FrontDoor                                  ;@line 48
						Jump _label0                                            ;@line 48
					_label0:
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
						IAdd ::temp0 a 1                                        ;@line 21
						IMultiply ::temp1 2 3                                   ;@line 21
						IDivide ::temp2 ::temp1 4                               ;@line 21
						IMod ::temp3 ::temp2 5                                  ;@line 21
						ISubtract ::temp4 ::temp0 ::temp3                       ;@line 21
						Assign total ::temp4                                    ;@line 21
						CompareLT ::temp5 a 1                                   ;@line 22
						Cast ::temp7 ::temp5                                    ;@line 22
						JumpT ::temp7 _label1                                   ;@line 22
						CompareLTE ::temp6 a 2                                  ;@line 22
						Cast ::temp7 ::temp6                                    ;@line 22
					_label1:
						Cast ::temp9 ::temp7                                    ;@line 22
						JumpT ::temp9 _label2                                   ;@line 22
						CompareGT ::temp8 a 3                                   ;@line 22
						Cast ::temp9 ::temp8                                    ;@line 22
					_label2:
						Cast ::temp11 ::temp9                                   ;@line 22
						JumpT ::temp11 _label3                                  ;@line 22
						CompareGTE ::temp10 a 4                                 ;@line 22
						Cast ::temp11 ::temp10                                  ;@line 22
					_label3:
						JumpF ::temp11 _label4                                  ;@line 22
						CallMethod GotoState self ::nonevar "Busy"              ;@line 23
						Jump _label5                                            ;@line 23
					_label4:
						Cast ::temp12 b                                         ;@line 24
						JumpF ::temp12 _label5                                  ;@line 24
						Assign total 0                                          ;@line 25
						Jump _label5                                            ;@line 25
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
						.local ::temp19 Int
						.local ::temp20 Int
						.local ::temp21 Int
						.local ::nonevar None
						.local ::temp22 Bool
						.local other Sample
						.local same Bool
					.endLocalTable
					.code
						Assign other self                                       ;@line 53
						PropGet Count other ::temp20                            ;@line 54
						IAdd ::temp21 ::temp20 1                                ;@line 54
						Assign ::temp19 ::temp21                                ;@line 54
						PropSet Count other ::temp19                            ;@line 54
						CallParent Stop ::nonevar                               ;@line 55
						CallMethod Numbers self ::nonevar 1 2.000000            ;@line 56
						Is ::temp22 other Quest                                 ;@line 57
						Assign same ::temp22                                    ;@line 57
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
						CallMethod Stop self ::nonevar                          ;@line 62
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
