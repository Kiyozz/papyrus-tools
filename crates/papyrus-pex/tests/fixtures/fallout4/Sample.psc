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

Float Property Ratio
  Float Function Get()
    Return Stored * 2.5
  EndFunction
  Function Set(Float value)
    Stored = -value
  EndFunction
EndProperty

Float Stored = 0.25

Function Numbers(Int a, Float b)
  Int total = a + 1 - 2 * 3 / 4 % 5
  Float mixed = b + 1.5 - 2.5 * 3.5 / 4.5
  total = -total
  If a < 1 || a <= 2 || a > 3 || a >= 4
    GotoState("Busy")
  ElseIf b as Bool
    total = 0
  EndIf
  While total < 10
    total += 1
  EndWhile
EndFunction

String Function Texts(String prefix) Global
  Return prefix + " and\ta\nline"
EndFunction

Point[] Function Arrays(Int size)
  Point[] points = new Point[size]
  Point one = new Point
  one.X = points.Length
  points[0] = one
  points.Add(one, 2)
  points.Insert(one, 1)
  points.Remove(0, 1)
  points.RemoveLast()
  Int found = points.FindStruct("X", 1) + points.RFindStruct("X", 2)
  found += points.Find(one) + points.RFind(one)
  points.Clear()
  Return points
EndFunction

Function Calls()
  Sample other = Self
  other.Count = other.Count + 1
  Parent.Stop()
  Numbers(1, 2.0)
  Debug.Trace(Texts("start"))
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
