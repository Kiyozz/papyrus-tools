//! What `papyrus explain` says about each lint: why it matters, an example,
//! and what to do.

use crate::Lint;

/// The long explanation of `lint`.
pub fn explanation(lint: &Lint) -> &'static str {
    match lint.code {
        "PAP001" => {
            r#"Compilers corrupt text silently, and still say "Compilation succeeded". Skyrim's
reads Windows-1252 unless the file has a UTF-8 BOM: without one, `é` becomes
`Ã©`. Those of Fallout 4 and Starfield read strict UTF-8, and lose any
Windows-1252 byte for good. Only what reaches the .pex counts: strings and
docstrings.

    Debug.Notification("Épée trouvée")

Save the file as UTF-8 with a BOM or Windows-1252 for Skyrim, as UTF-8 for
Fallout 4 and Starfield."#
        }
        "PAP002" => {
            r#"The game's .flg file says which objects each flag may sit on. The compiler
refuses the others.

    bool done Hidden    ; `Hidden` is for scripts and properties

Remove the flag, or move it to an object that allows it. A project with its own
flags sets `flags` in papyrus.toml."#
        }
        "PAP003" => {
            r#"`; papyrus: allow(...)` names lints. A typo suppresses nothing, and the
warning you meant to silence stays.

    ; papyrus: allow(naming/local)

Write the name `papyrus lints` gives, or its code. `all` covers every lint."#
        }
        "PAP004" => {
            r#"The engine calls events by name, on the types that receive them. A misspelled
event, or one the script's type never gets, compiles and never runs.

    ScriptName MyQuest extends Quest
    Event OnActivatee(ObjectReference akActionRef)    ; a typo
    Event OnCombatStateChanged(Actor akTarget, int aeCombatState)    ; for actors

Correct the name, or extend the type that receives the event."#
        }
        "PAP005" => {
            r#"The compiler refuses a call with too many arguments, or without the ones that
have no default.

    Debug.Notification()

Give what the callee declares; signature help in the editor lists it."#
        }
        "PAP006" => {
            r#"A named argument must be a parameter of the callee, given once.

    target.MoveTo(player, afXOffset = 10.0, afXOffset = 20.0)

Correct the name, or remove the repeated argument."#
        }
        "PAP007" => {
            r#"Papyrus casts implicitly upward only: a child script to its parent, int to
float, anything to bool or string. The rest needs `as`, and some types never
cast at all. The compiler refuses the value of a call that returns nothing too.

    Actor target = akActivator            ; ObjectReference to Actor
    int count = 1.5

Cast (`akActivator as Actor`) when you know the type: the quick fix writes it.
The cast gives None when it fails, so test the result."#
        }
        "PAP008" => {
            r#"The function, variable, property or field exists neither on the script nor on
its ancestors. Most often a typo, or a member of a child type reached from its
parent.

    Game.GetPlayr()

Use the suggested name. If the member comes from a library or a script
extender, add its sources to `imports` in papyrus.toml."#
        }
        "PAP009" => {
            r#"A typed function that reaches its end returns the default value, 0, False, ""
or None. The compiler accepts it, and the caller gets a silent wrong answer.

    int Function Price()
        if rare
            return 100
        endif
    EndFunction

Add a `Return` on every path."#
        }
        "PAP010" => {
            r#"`GotoState` to an undeclared state compiles. The script then behaves as if in
the empty state, and the states you meant never run.

    GotoState("Wating")

Correct the name, or declare the state. A state only a child script declares
is not reported."#
        }
        "PAP011" => {
            r#"An event must have the parameters of the one it implements. The compiler
refuses another signature.

    Event OnActivate()    ; the engine passes akActionRef

Copy the signature; the parameters' names may differ, not their types."#
        }
        "PAP012" => {
            r#"An `AutoReadOnly` or `Const` property, or one without `Set`, cannot be
assigned.

    int Property Max = 10 AutoReadOnly
    Function Reset()
        Max = 0
    EndFunction

Make the property `Auto`, or write a `Set`."#
        }
        "PAP013" => {
            r#"A `Global` function runs without an instance: it has no `self`, and sees no
member. Conversely, a function that is not `Global` needs an object to be
called on.

    MyQuest.Start()              ; on the script, not an instance
    int Function Next() Global
        return count + 1         ; a member of the script
    EndFunction

Use a property holding the instance, or remove `Global`."#
        }
        "PAP014" => {
            r#"Nothing runs after a statement that always returns, so these lines are dead:
often a condition removed, or a `Return` left while debugging.

    return
    Debug.Trace("done")

Remove them, or the `Return`."#
        }
        "PAP015" => {
            r#"The variable is written and never read: a leftover, or a typo in the name used
later on.

    int unused = 1

Remove it. If its initial value is a call made for its effects, keep the call."#
        }
        "PAP016" => {
            r#"Nothing reads this parameter, nor any after it. Events and overrides are left
out, since their signature is imposed.

    Function Heal(Actor target, float amount)    ; `amount` never read

Remove it and update the callers, or use it."#
        }
        "PAP017" => {
            r#"Only SKSE declares this member or script. The compiler fails without SKSE's
sources, and the game without SKSE at runtime.

If the mod requires SKSE, set `skse = true` in papyrus.toml, or add its sources
to `imports`. Otherwise, do without this member."#
        }
        "PAP018" => {
            r#"The type names a script found nowhere. It is only reported once the game's
sources are in the project or its imports: before that, unknown is not wrong.

    MyQeust Property quest Auto

Correct the name, or add the folder holding the script to `imports`."#
        }
        "PAP019" => {
            r#"The compiler refuses a local declared twice in a block, named like a parameter
or a script variable, or hiding one of an enclosing block.

    Function F(int count)
        int count = 0
    EndFunction

Rename it, or drop the type to assign the existing one."#
        }
        "PAP020" => {
            r#"Papyrus ignores case, so this compiles. It only reads worse, and searches
miss it.

    Debug.notification("x")

`papyrus check --fix` writes the declared case. Properties need `--unsafe`:
plugins bind them by name."#
        }
        "PAP021" => {
            r#"The compiler refuses a `ScriptName` other than the file's name.

    ; MyQuestScript.psc
    ScriptName MyQuest

From Fallout 4, a namespace also needs its folder:

    ; Source/Stray.psc, where Source/MyMod/Stray.psc is expected
    ScriptName MyMod:Stray

When the root of the scripts is known, by `sources` in papyrus.toml or the
imports of a `.ppj`, every folder under it counts, a namespace left out too:

    ; Source/MyMod/Stray.psc, with sources = ["Source"]
    ScriptName Stray

Rename one of them, or move the file: the quick fix rewrites the `ScriptName`."#
        }
        "PAP022" => {
            r#"A script cannot be its own ancestor.

    ScriptName A extends B    ; and B extends A

Break the cycle, often by moving the shared part to a new parent."#
        }
        "PAP023" => {
            r#"The compiler refuses a function or event declared twice in a state, a variable
or property declared twice, a second `Auto` state. From Fallout 4, a property
cannot be named like a variable either.

    int count
    int count

Remove or rename one."#
        }
        "PAP024" => {
            r#"`Return value` needs a function with a return type, and a typed function needs
a value.

    Event OnInit()
        return 1
    EndEvent

Give the function a type, or remove the value. Skyrim accepts `Return None` and
a call that returns nothing, so these are not reported."#
        }
        "PAP025" => {
            r#"An override, in a child script or in a state, must keep the parameters, the
defaults and the return type of the original. A `Global` function cannot be
redefined. From Fallout 4, a function cannot be named like an event; in
Starfield, a native function cannot be overridden.

    ; Parent: int Function Price(int count = 1)
    int Function Price(int count = 2)

Copy the original signature. Defaults are compared as written: `True` and
`true` differ for the compiler, numbers by their value."#
        }
        "PAP026" => {
            r#"A function in a state must exist in the empty state or on an ancestor, and a
`Global` function cannot be in a state.

    State Waiting
        Function Tick()
        EndFunction
    EndState

Declare `Function Tick()` outside the state, empty if need be."#
        }
        "PAP027" => {
            r#"A full property only holds a `Get` and a `Set` of the right shape, and no
initial value. `AutoReadOnly` needs one.

    int Property Count = 1
        int Function Get()
            return 1
        EndFunction
    EndProperty

Remove the initial value, or make the property `Auto`."#
        }
        "PAP028" => {
            r#"Arrays hold at most 128 elements. In Skyrim, the size must also be a literal.

    int[] values = new int[size]    ; Skyrim

Use a literal from 1 to 128. Skyrim mods needing more split them in several
arrays."#
        }
        "PAP029" => {
            r#"Only a variable, a property or an array element can be assigned.

    GetCount() = 1
    items.Length = 0

Assign what the call returns to a variable, or create another array."#
        }
        "PAP030" => {
            r#"The initial value of a script variable or a property, and a parameter's
default, are stored in the .pex: they must be literals.

    float delay = 60.0 * 2.0

Write `120.0`, or assign it in `OnInit`."#
        }
        "PAP031" => {
            r#"From Fallout 4, parameters with a default must come last.

    Function F(int a = 1, int b)

Reorder them, or give `b` a default. Skyrim accepts this."#
        }
        "PAP032" => {
            r#"From Fallout 4, only a `Native` script may declare `Native` functions.

    ScriptName MyScript
    Function F() Native

Flag the script `Native`, or give the function a body. Native functions are
implemented by the game or a script extender plugin."#
        }
        "PAP033" => {
            r#"Integers are 32 bits. From Fallout 4, a decimal literal stops at 2147483647;
hexadecimal ones reach 0xFFFFFFFF.

    int big = 4294967296

Use a smaller value."#
        }
        "PAP034" => {
            r#"`RegisterForUpdate` and `RegisterForUpdateGameTime` keep firing, stored in the
save, even after the mod is removed; and updates can pile up when the game
struggles. The usual advice of the community.

    RegisterForUpdate(1.0)

Use `RegisterForSingleUpdate`, and call it again at the end of `OnUpdate`."#
        }
        "PAP035" => {
            r#"`Game.GetPlayer()` makes a call to the game on each iteration, for a value that
does not change.

    while i < count
        Game.GetPlayer().AddItem(items[i])
        i += 1
    endwhile

Keep it in a variable before the loop, or use `PlayerRef`, a property."#
        }
        "PAP036" => {
            r#"Comparing a boolean with `True` or `False` says it twice.

    if done == True
    if done == False

Write `if done` and `if !done`: the fix does."#
        }
        "PAP037" => {
            r#"Assigning a value to itself does nothing: most often a parameter and a
variable mixed up.

    count = count

Assign what you meant, or remove the line."#
        }
        "PAP038" => {
            r#"A value compared with itself always gives the same answer: most often a typo
in one of the names.

    if count == count

Compare with what you meant."#
        }
        "PAP039" => {
            r#"Nothing reads this script variable. `Conditional` ones are left out: the
game's conditions read them.

    int unused

Remove it."#
        }
        "PAP040" => {
            r#"An `Import` gives access to a script's global functions and types without its
name. This one is used by nothing in the script.

    Import Utility

Remove it. Writing `Utility.Wait(1.0)` in full needs no `Import`."#
        }
        "PAP041" => {
            r#"Whatever its arguments, this function calls itself before it can return,
directly or through others: nothing ever stops it.

    int Function Sum(int n)
        return n + Sum(n - 1)
    EndFunction

Add the case that ends it, tested before the call. Only what is certain is
reported: a function with a `GotoState`, redefined in a state or overridden by
a child is left alone, since these may end it."#
        }
        "PAP101" => {
            r#"Locals and parameters are written in lowerCamelCase. A style lint, for the
compiler ignores case.

    int Hit_Points

`papyrus check --fix` renames them, or set the lint to `allow` for a mod with
its own convention."#
        }
        "PAP102" => {
            r#"Script variables, properties and fields are written in lowerCamelCase. A style
lint.

    int Hit_Points

Plugins bind properties by name: a new name disconnects them. `--fix --unsafe`
only changes the case."#
        }
        "PAP103" => {
            r#"Functions, events, states, structs and groups are written in PascalCase. A
style lint.

    Function do_something()

Rename it, or set the lint to `allow` for a mod with its own convention."#
        }
        _ => lint.description,
    }
}

#[cfg(test)]
mod tests {
    use crate::{LINTS, explanation};

    #[test]
    fn every_lint_is_explained() {
        for lint in LINTS {
            assert_ne!(explanation(lint), lint.description, "{}", lint.code);
        }
    }
}
