# The F programming language

Compiler for the F programming lanuge written in Rust.

## Build

- Requires Rust nightly to build.
- For `--action=build-amd64` to work, `nasm`, `ld` and `strip` must be in `$PATH`

### Nix

```bash
nix run . -- --help           # Run the with depencencies set up by nix
nix develop                   # Enter dev shell
nix flake check               # Run unit and integration tests, clippy (linter), check formatting and audit dependencies (advisory-db)
```

### Cargo

```bash
cargo build --release         # Build `flang` to `./target/release/flang`
cargo run --release -- --help # Run `flang`
cargo test                    # Runs unit and integration tests
```

## Usage

```
flang [OPTIONS]

Options:
  -a, --action <ACTION>  What to do [default: build-amd64] [possible values: tokens, derivation, ast, opt-ast, stack, asm-amd64, build-amd64]
  -i, --input <INPUT>    Path to input, defaults to stdin
  -o, --output <OUTPUT>  Path to output, defaults to stdout
  -h, --help             Print help
  -V, --version          Print version
```

Compile and run:

```bash
flang -i program.fl -o program && ./program
```

See [`./samples/`](samples) for sample programs

## Specification

- Source code must be encoded in UTF-8
- Statements, with the exception of the last statement in a block, must end with `;`
- Blocks are defined by curly braces `{}`, ended with `;`

### Comments

- All characters following `//` until end of line are comments.

### Program declaration

- All source files must start with `program <IDENTIFIER>;`
- All source files must end with a `.`

### Identifier

- Starts with an alphabetic character or `_`, optionally followed by alphabetic or numeric characters optionally separated or ended by single `_`.

### Literals

| Type                  | Format                                     | Example         |
| --------------------- | ------------------------------------------ | --------------- |
| Int                   | Base 10 string of `0-9`s                   | `42`            |
| Int (arbitrary base)  | `base#value` where base is 2, 8, 10, or 16 | `16#2a`, `8#52` |
| Real                  | Base 10 string with `.`                    | `3.14`          |
| Real (arbitrary base) | `base#value` with one `.`                  | `16#3.1`        |
| Bool                  | `TRUE`, `FALSE`                            | `TRUE`          |

### Types

| Type                    | Description                                                             |
| ----------------------- | ----------------------------------------------------------------------- |
| `Int`                   | 64-bit signed integer                                                   |
| `Real`                  | 64-bit floating point (IEEE 754)                                        |
| `Bool`                  | Boolean                                                                 |
| `array [from..to] of T` | Array of range `from` to `to` (inclusive), `from`/`to` are Int literals |

### Declarations

#### Variables

- Declared with `var` at top level as global variables or under functions
- Always zeroed after initialization
- Reassignment: `name := <EXPR>`
- `var name1, name2, ... : Type;`

#### Constants

- Declared with `const` at top level
- Immutable, must be initialized with a literal
- `const name1, name2, ... = <LITERAL>;`

#### Functions

- `fn <NAME>(arg1: Type1, ...): Type; <VARIABLE DECLARATIONS>; { <BODY> };`
- Return value: `<NAME> := <EXPR>;`
- Early return: `exit`

#### Procedures

- `proc <NAME>(arg1: Type1, ...); <VARIABLE DECLARATIONS>; { <BODY> };`
- Early return: `exit`

### Control flow

- Conditions do not require parentheses
- Body may wrapped in curly braces `{}`
- Supports `break` and `continue` within loops

#### If

```
if <EXPR> then
  <BLOCK OR STATEMENT>
else
  <BLOCK OR STATEMENT>
```

#### While

```
while <EXPR> do
  <BLOCK OR STATEMENT>
```

#### For

```
for i := <EXPR> to|downto <EXPR> do
  <BLOCK OR STATEMENT>
```

### Operators

| Category   | Operators                                   | Applicable types                  |
| ---------- | ------------------------------------------- | --------------------------------- |
| Comparison | `=`, `!=`, `<`, `>`, `<=`, `>=`             | `Bool`, `Int`, `Real`             |
| Arithmetic | `+`, `-`, `*`, `/`, `%`                     | `Int`, `Real`                     |
| Logic      | `not`, `and`, `or`, `xor`, `imply`, `equiv` | `Bool`, `Int` (bitwise operation) |
| Indexing   | `[<EXPR>]`                                  | arrays                            |

### Standard library

| Function       | Signature      | Description            |
| -------------- | -------------- | ---------------------- |
| `readln`       | `(): Int`      | Read Int from stdin    |
| `readln_real`  | `(): Real`     | Read Real from stdin   |
| `writeln`      | `(Int)`        | Write Int to stdout    |
| `writeln_real` | `(Real)`       | Write Real to stdout   |
| `exit_ok`      | `()`           | Terminate with success |
| `exit_fail`    | `()`           | Terminate with failure |
| `bool_to_int`  | `(Bool): Int`  | Type cast              |
| `bool_to_real` | `(Bool): Real` | Type cast              |
| `int_to_bool`  | `(Int): Bool`  | Type cast              |
| `int_to_real`  | `(Int): Real`  | Type cast              |
| `real_to_bool` | `(Real): Bool` | Type cast              |
| `real_to_int`  | `(Real): Int`  | Type cast              |

## Calling convention

- Stack grows down
- State when entering a function: `[param1] [param2] ... [paramn] [<stack pointer> return addr]`
- State inside function: `[param1] [param2] ... [paramn] [return addr] [<base pointer> old base pointer] [return value] [local1] [local2] ... [local_m <stack pointer>]`
- State after exiting a function: `[<stack pointer> return value]`

## Formal grammar

```
S -> program IDENTIFIER ; Decls Block .

Decls -> ConstDecl Decls
Decls -> VarDecl Decls
Decls -> ProcDecl Decls
Decls -> FnDecl Decls
Decls -> ε

ConstDecl     -> const ConstDeclBody
ConstDeclBody -> VarNames = LITERAL ; ConstDeclBody
ConstDeclBody -> ε

VarDecl     -> var VarDeclBody
VarDeclBody -> VarNames : Type ; VarDeclBody
VarDeclBody -> ε

ProcDecl  -> proc IDENTIFIER ( Args )        ; VarDecls Block ;
FnDecl    -> fn   IDENTIFIER ( Args ) : Type ; VarDecls Block ;

VarDecls -> VarDecl VarDecls
VarDecls -> ε

Type -> Int
Type -> Real
Type -> Bool
Type -> array [ LITERAL .. LITERAL ] of Type

Args     -> IDENTIFIER : Type ArgsTail
Args     -> ε
ArgsTail -> , ArgsNext
ArgsTail -> ε
ArgsNext -> IDENTIFIER : Type ArgsTail
ArgsNext -> ε

VarNames     -> IDENTIFIER VarNamesTail
VarNamesTail -> , IDENTIFIER VarNamesTail
VarNamesTail -> ε

Block -> { Statements }

Statements     -> Statement AfterStatement
Statements     -> ε
AfterStatement -> ; Statements
AfterStatement -> ε

Statement -> Block
Statement -> IfStatement
Statement -> WhileStatement
Statement -> ForStatement
Statement -> Expr ExprStatementTail
Statement -> break
Statement -> continue
Statement -> exit

ExprStatementTail -> := Expr
ExprStatementTail -> ε

IfStatement -> if Expr then Statement IfTail
IfTail      -> else Statement
IfTail      -> ε

WhileStatement -> while Expr do Statement

ForStatement -> for IDENTIFIER := Expr ForDir Expr do Statement
ForDir       -> to
ForDir       -> downto

IdentifierStatement     -> IDENTIFIER IdentifierStatementTail
IdentifierStatementTail -> := Expr
IdentifierStatementTail -> [ Expr ] := Expr
IdentifierStatementTail -> ( CalledArgs )

CalledArgs     -> Expr CalledArgsTail
CalledArgs     -> ε
CalledArgsTail -> , CalledNext
CalledArgsTail -> ε
CalledNext     -> Expr CalledArgsTail
CalledNext     -> ε

Expr -> E1 E1Tail

E1 -> E2 E2Tail
E2 -> E3 E3Tail
E3 -> E4 E4Tail
E4 -> E5 E5Tail
E5 -> E6 E6Tail
E6 -> not E6
E6 -> -   E6
E6 -> +   E6
E6 -> Atom

E1Tail -> equiv E2 E1Tail
E1Tail -> imply E2 E1Tail
E1Tail -> ε
E2Tail -> or    E3 E2Tail
E2Tail -> xor   E3 E2Tail
E2Tail -> ε
E3Tail -> and   E4 E3Tail
E3Tail -> ε
E4Tail -> =     E5
E4Tail -> !=    E5
E4Tail -> <     E5
E4Tail -> >     E5
E4Tail -> <=    E5
E4Tail -> >=    E5
E4Tail -> ε
E5Tail -> +     E5 E5Tail
E5Tail -> -     E5 E5Tail
E5Tail -> ε
E6Tail -> *     E6 E6Tail
E6Tail -> /     E6 E6Tail
E6Tail -> %     E6 E6Tail
E6Tail -> ε

Atom     -> IDENTIFIER AtomTail
Atom     -> LITERAL AtomTail
Atom     -> ( Expr ) AtomTail
AtomTail -> [ Expr ] AtomTail
AtomTail -> ( CalledArgs ) AtomTail
AtomTail -> ε
```

## Mila translation

Use [`./script/mila_flang_translate.sh`](./script/mila_flang_translate.sh) to translate between F and Mila source code.

## License

[GPL-3.0-or-later](LICENSE.md)
