ScriptName Sample extends Quest Hidden
{A script going through what a .pex can hold.}

Int Property Count = 3 Auto
String Property Label = "a \"quoted\" name" Auto Hidden
Float Property Ratio
  Float Function Get()
    Return Stored * 2.5
  EndFunction
  Function Set(Float value)
    Stored = -value
  EndFunction
EndProperty

Float Stored = 0.25
Bool Flag = True

Function Numbers(Int a, Float b)
  Int total = a + 1 - 2 * 3 / 4 % 5
  Float mixed = b + 1.5 - 2.5 * 3.5 / 4.5
  total = -total
  Flag = !Flag && a == 1 || a != 2
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

Int[] Function Arrays(Int size)
  Int[] values = new Int[4]
  values[0] = values.Length
  Int found = values.Find(1) + values.RFind(2)
  Return values
EndFunction

Function Calls()
  Sample other = Self
  other.Count = other.Count + 1
  Parent.Stop()
  Numbers(1, 2.0)
  Debug.Trace(Texts("start"))
EndFunction

State Busy
  Event OnBeginState()
    Stop()
  EndEvent
EndState

Auto State Idle
  Function Calls()
  EndFunction
EndState
