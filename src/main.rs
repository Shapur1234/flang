#![allow(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]
#![feature(const_cmp, const_trait_impl, trait_alias)]

mod asm;
mod error;
mod iter;
mod lexer;
mod parser;
mod semantic;
mod stack;

use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Error, Read, Write, stdin, stdout},
    path::PathBuf,
    process::Command,
};

use clap::{Parser, ValueEnum};
use tempfile::tempdir;

use crate::{
    asm::{TargetArch, generate},
    iter::Utf8Reader,
    lexer::lex,
    parser::{derivation::Derivation, parse},
    semantic::{check, optimise},
    stack::{Instruction, translate},
};

#[derive(Debug, Clone, ValueEnum)]
enum CompilerAction {
    Tokens,
    Derivation,
    Ast,
    OptAst,
    Stack,
    AsmAmd64,
    BuildAmd64,
}

#[derive(Debug, Clone, Parser)]
#[command(version, about = "Flang compiler", long_about = None)]
struct Args {
    #[arg(short, long, value_enum, default_value_t = CompilerAction::BuildAmd64, help = "What to do")]
    action: CompilerAction,

    #[arg(short, long, help = "Path to input, defaults to stdin")]
    input: Option<PathBuf>,

    #[arg(short, long, help = "Path to output, defaults to stdout")]
    output: Option<PathBuf>,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    handle_input(&args)
}

fn handle_input(args: &Args) -> Result<(), String> {
    if let Some(path) = &args.input {
        let input = File::open(path).map_err(|err| format!("Failed to open input - {err}"))?;
        handle_output(args, BufReader::new(input))
    } else {
        let input = stdin().lock();
        handle_output(args, input)
    }
}

fn handle_output(args: &Args, input: impl Read) -> Result<(), String> {
    if let Some(path) = &args.output {
        let output = File::create(path).map_err(|err| format!("Failed to open output - {err}"))?;
        let result = execute(args, input, BufWriter::new(output));

        // Make output binary executable
        #[cfg(target_family = "unix")]
        if matches!(args.action, CompilerAction::BuildAmd64) {
            'set_executable: {
                use std::{fs::Permissions, os::unix::fs::PermissionsExt};

                let Ok(output_file) = File::open(path) else {
                    break 'set_executable;
                };

                let Ok(metadata) = output_file.metadata() else {
                    break 'set_executable;
                };

                let old_permissions = metadata.permissions();
                let new_permissions = Permissions::from_mode(old_permissions.mode() | 0b001_000_000);

                let _ = output_file.set_permissions(new_permissions);
            }
        }

        result
    } else {
        let output = stdout().lock();
        execute(args, input, output)
    }
}

fn execute(args: &Args, input: impl Read, output: impl Write) -> Result<(), String> {
    let reader = Utf8Reader::new(input);

    match args.action {
        CompilerAction::Tokens => tokens(output, reader)?,
        CompilerAction::Derivation => derivation(output, reader)?,
        CompilerAction::Ast => ast(output, reader)?,
        CompilerAction::OptAst => opt_ast(output, reader)?,
        CompilerAction::Stack => stack(output, reader)?,
        CompilerAction::AsmAmd64 => asm_amd64(output, reader)?,
        CompilerAction::BuildAmd64 => build_amd64(output, reader)?,
    }

    Ok(())
}

fn format_write_error(error: &Error) -> String {
    format!("Failed to write to output - {error}")
}

fn tokens(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    for token_res in lex(reader) {
        let token = token_res.map_err(|err| err.to_string())?;

        writeln!(output, "{:>4}:{:<4} {:?}", token.line, token.column, token.payload)
            .map_err(|err| format_write_error(&err))?;
    }

    Ok(())
}

fn derivation(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;

    Derivation::Program(program)
        .pretty_print(&mut output)
        .map_err(|err| format_write_error(&err))?;

    Ok(())
}

fn ast(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;
    let ast = check(program).map_err(|err| err.to_string())?;

    ast.pretty_print(&mut output).map_err(|err| format_write_error(&err))?;

    Ok(())
}

fn opt_ast(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;
    let ast = check(program).map_err(|err| err.to_string())?;
    let optimised_ast = optimise(ast);

    optimised_ast
        .pretty_print(&mut output)
        .map_err(|err| format_write_error(&err))?;

    Ok(())
}

fn stack(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;
    let ast = optimise(check(program).map_err(|err| err.to_string())?);

    let mut first = true;
    for instruction in translate(ast) {
        match instruction {
            Instruction::EntrypointDecl | Instruction::Label(_) if !first => {
                writeln!(output).map_err(|err| format_write_error(&err))?;
            }
            _ => (),
        }
        writeln!(output, "{instruction:?}").map_err(|err| format_write_error(&err))?;

        first = false;
    }

    Ok(())
}

fn asm_amd64(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;
    let ast = optimise(check(program).map_err(|err| err.to_string())?);
    let instructions = translate(ast);

    for line in generate(&TargetArch::Amd64, instructions) {
        writeln!(output, "{line}").map_err(|err| format_write_error(&err))?;
    }

    Ok(())
}

fn build_amd64(mut output: impl Write, reader: Utf8Reader<impl Read>) -> Result<(), String> {
    let program = parse(lex(reader)).map_err(|err| err.to_string())?;
    let ast = optimise(check(program).map_err(|err| err.to_string())?);
    let instructions = translate(ast);

    let dir = tempdir().map_err(|err| format!("Failed to create temp dir - {err}"))?;
    let (asm_path, object_path, bin_path) = (
        dir.path().join("flang.asm"),
        dir.path().join("flang.o"),
        dir.path().join("flang.bin"),
    );

    {
        let asm_file = File::create(&asm_path).map_err(|err| format!("Failed to create asm file - {err}"))?;
        let mut asm_writer = BufWriter::new(asm_file);

        for line in generate(&TargetArch::Amd64, instructions) {
            writeln!(asm_writer, "{line}").map_err(|err| format!("Failed to write asm - {err}"))?;
        }
    }

    if !Command::new("nasm")
        .args([
            "-f",
            "elf64",
            asm_path.to_str().unwrap(),
            "-o",
            object_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|err| format!("nasm failed - {err}"))?
        .success()
    {
        return Err("Assembly failed".to_string());
    }

    if !Command::new("ld")
        .args([object_path.to_str().unwrap(), "-o", bin_path.to_str().unwrap()])
        .status()
        .map_err(|err| format!("ld failed - {err}"))?
        .success()
    {
        return Err("Linking failed".into());
    }

    if !Command::new("strip")
        .args(["-s", bin_path.to_str().unwrap()])
        .status()
        .map_err(|err| format!("strip failed - {err}"))?
        .success()
    {
        return Err("Stripping failed".to_string());
    }

    let bin_file = File::open(&bin_path).map_err(|err| format!("Failed to read binary - {err}"))?;
    io::copy(&mut BufReader::new(bin_file), &mut output).map_err(|err| format_write_error(&err))?;

    Ok(())
}
