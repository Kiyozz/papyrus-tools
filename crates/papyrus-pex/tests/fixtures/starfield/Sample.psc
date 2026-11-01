ScriptName Sample extends Quest Hidden
{A script going through what a .pex can hold.}

Struct Point
  Int X = 1
  {Across.}
  Float Y
EndStruct

Group Settings CollapsedOnRef
  Int Property Count = 3 Auto
  String Property Name = "a \"quoted\" name" Auto Const
EndGroup

Guard FrontDoor ProtectsFunctionLogic
Guard SideWindow ProtectsFunctionLogic

Float Stored = 0.25

Function Numbers(Int a, Float b)
  Int total = a + 1 - 2 * 3 / 4 % 5
  If a < 1 || a <= 2 || a > 3 || a >= 4
    GotoState("Busy")
  ElseIf b as Bool
    total = 0
  EndIf
EndFunction

Point[] Function Arrays(Int size)
  Point[] points = new Point[size]
  Point one = new Point
  one.X = points.Length
  points.Add(one, 2)
  points.Insert(one, 1)
  points.Remove(0, 1)
  points.RemoveLast()
  Int found = points.FindStruct("X", 1) + points.RFindStruct("X", 2)
  Point[] matching = points.GetAllMatchingStructs("X", 1)
  points.Clear()
  Return points
EndFunction

Function Guarded()
  LockGuard FrontDoor, SideWindow
    Stored = 1.0
  EndLockGuard
  TryLockGuard FrontDoor
    Stored = 2.0
  EndTryLockGuard
EndFunction

Function Calls()
  Sample other = Self
  other.Count = other.Count + 1
  Parent.Stop()
  Numbers(1, 2.0)
  Bool same = other is Quest
EndFunction

State Busy
  Event OnBeginState(String previous)
    Stop()
  EndEvent
EndState

Auto State Idle
  Function Calls()
  EndFunction
EndState
