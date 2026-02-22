use std::{borrow::Cow, mem, str::Lines};

use crate::{asm::backend::InstructionsIterator, stack::Instruction};

static PREAMBULE: &str = include_str!("./preambule/amd64.asm");

/// Emits amd64 NASM assembly
#[derive(Debug, Clone)]
pub enum Amd64Generator<S: InstructionsIterator> {
    Preambule {
        preambule_lines: Lines<'static>,
        instructions: S,
    },
    Instruction {
        instructions: S,
        current_section: Section,
        line_buffer: Vec<Cow<'static, str>>,
    },
    Done,
}

/// Assembly section being generated
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    Unknown,
    Text,
    Bss,
}

impl<S: InstructionsIterator> Amd64Generator<S> {
    pub fn new(iter: S) -> Self {
        Amd64Generator::Preambule {
            preambule_lines: PREAMBULE.lines(),
            instructions: iter,
        }
    }
}

macro_rules! cow {
    (b $str:expr) => {
        std::borrow::Cow::Borrowed($str)
    };
    (o $format_str:literal $(, $arg:expr) *) => {
        std::borrow::Cow::Owned(format!($format_str, $($arg),*))
    };
}

impl<S: InstructionsIterator> Iterator for Amd64Generator<S> {
    type Item = Cow<'static, str>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self {
                Amd64Generator::Preambule { preambule_lines, .. } => {
                    if let Some(next_preambule_line) = preambule_lines.next() {
                        return Some(cow!(b next_preambule_line));
                    }

                    match mem::replace(self, Amd64Generator::Done) {
                        Amd64Generator::Preambule { instructions, .. } => {
                            *self = Amd64Generator::Instruction {
                                instructions,
                                current_section: Section::Unknown,
                                line_buffer: Vec::new(),
                            };
                        }
                        _ => unreachable!(),
                    }
                }
                Amd64Generator::Instruction {
                    instructions,
                    current_section,
                    line_buffer: stack,
                } => {
                    if let Some(top) = stack.pop() {
                        return Some(top);
                    }

                    let Some(instruction) = instructions.next() else {
                        *self = Amd64Generator::Done;
                        return None;
                    };

                    let should_be_bss = matches!(instruction, Instruction::VarDecl { .. });
                    let should_be_text = !matches!(instruction, Instruction::VarDecl { .. });

                    emit_instructions(instruction, stack);

                    if should_be_bss && *current_section != Section::Bss {
                        stack.push(cow!(b "section .bss"));
                        stack.push(cow!(b ""));
                        *current_section = Section::Bss;
                    }
                    if should_be_text && *current_section != Section::Text {
                        stack.push(cow!(b "section .text"));
                        stack.push(cow!(b ""));
                        *current_section = Section::Text;
                    }
                }
                Amd64Generator::Done => return None,
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn emit_instructions(instruction: Instruction, stack: &mut Vec<Cow<'static, str>>) {
    match instruction {
        Instruction::VarDecl { name, size } => stack.push(cow!(o "        f_{}: resb {}", name.0, size)),
        Instruction::FuncDecl(name) => stack.push(cow!(o "f_{name}:")),
        Instruction::EntrypointDecl => {
            stack.push(cow!(b "_start:"));
        }

        Instruction::PushInt(val) | Instruction::PushReal(val) => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(o "        mov rax, {val}"));
        }

        Instruction::PushGlobalAddr(name) => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(o "        lea rax, [rel f_{name}]"));
        }
        Instruction::PushLocalAddr(offset) => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(o "        lea rax, [rbp {:+} ]", offset));
        }

        Instruction::PushBasePtr => stack.push(cow!(b "        push rbp")),
        Instruction::PopBasePtr => stack.push(cow!(b "        pop rbp")),
        Instruction::SetBasePtrToStackPtr => stack.push(cow!(b "        mov rbp, rsp")),

        Instruction::PushToSpecialReg1 => stack.push(cow!(b "        pop r12")),
        Instruction::PushToSpecialReg2 => stack.push(cow!(b "        pop r13")),
        Instruction::PopFromSpecialReg1 => stack.push(cow!(b "        push r12")),
        Instruction::PopFromSpecialReg2 => stack.push(cow!(b "        push r13")),

        Instruction::StAlloc(bytes) => {
            stack.push(cow!(b "        rep stosb"));
            stack.push(cow!(o "        mov rcx, {bytes}"));
            stack.push(cow!(b "        xor rax, rax"));
            stack.push(cow!(b "        mov rdi, rsp"));
            stack.push(cow!(o "        sub rsp, {bytes}"));
        }
        Instruction::StFree(bytes) => {
            stack.push(cow!(o "        add rsp, {bytes}"));
        }
        Instruction::StMove { from, bytes, by } => {
            if by > 0 {
                stack.push(cow!(b "        cld"));
                stack.push(cow!(b "        rep movsb"));
                stack.push(cow!(b "        dec rdi"));
                stack.push(cow!(b "        add rdi, rcx"));
                stack.push(cow!(b "        dec rsi"));
                stack.push(cow!(b "        add rsi, rcx"));
                stack.push(cow!(b "        std"));
                stack.push(cow!(o "        mov rcx, {bytes}"));
                stack.push(cow!(o "        lea rdi, [rsi {:+}]", by));
                stack.push(cow!(o "        add rsi, {from}"));
                stack.push(cow!(b "        mov rsi, rsp"));
            } else {
                stack.push(cow!(b "        rep movsb"));
                stack.push(cow!(b "        cld"));
                stack.push(cow!(o "        mov rcx, {bytes}"));
                stack.push(cow!(o "        lea rdi, [rsi  {by}]"));
                stack.push(cow!(o "        add rsi, {from}"));
                stack.push(cow!(b "        mov rsi, rsp"));
            }
        }

        Instruction::Read(bytes) => {
            stack.push(cow!(b "        rep movsb"));
            stack.push(cow!(b "        cld"));
            stack.push(cow!(o "        mov rcx, {bytes}"));
            stack.push(cow!(b "        mov rdi, rsp"));
            stack.push(cow!(o "        sub rsp, {bytes}"));
            stack.push(cow!(b "        pop rsi"));
        }
        Instruction::Write(bytes) => {
            stack.push(cow!(o "        add rsp, {}", bytes + 8));
            stack.push(cow!(b "        rep movsb"));
            stack.push(cow!(b "        cld"));
            stack.push(cow!(o "        mov rcx, {bytes}"));
            stack.push(cow!(b "        mov rsi, rsp"));
            stack.push(cow!(o "        mov rdi, [rsp + {bytes}]"));
        }

        Instruction::Pop(bytes) => {
            stack.push(cow!(o "        add rsp, {bytes}"));
        }

        Instruction::AddInt => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        add rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::SubInt => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        sub rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::MulInt => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        imul rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }

        Instruction::DivInt => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        idiv rcx"));
            stack.push(cow!(b "        cqo"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::ModInt => {
            stack.push(cow!(b "        push rdx"));
            stack.push(cow!(b "        idiv rcx"));
            stack.push(cow!(b "        cqo"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }

        Instruction::AddReal => emit_op_real(stack, "addsd"),
        Instruction::SubReal => emit_op_real(stack, "subsd"),
        Instruction::MulReal => emit_op_real(stack, "mulsd"),
        Instruction::DivReal => emit_op_real(stack, "divsd"),

        Instruction::EqInt => emit_cmp_int(stack, "sete"),
        Instruction::NeqInt => emit_cmp_int(stack, "setne"),
        Instruction::LtInt => emit_cmp_int(stack, "setl"),
        Instruction::GtInt => emit_cmp_int(stack, "setg"),
        Instruction::LeqInt => emit_cmp_int(stack, "setle"),
        Instruction::GeqInt => emit_cmp_int(stack, "setge"),

        Instruction::EqReal => emit_cmp_real(stack, "sete"),
        Instruction::NeqReal => emit_cmp_real(stack, "setne"),
        Instruction::LtReal => emit_cmp_real(stack, "setb"),
        Instruction::GtReal => emit_cmp_real(stack, "seta"),
        Instruction::LeqReal => emit_cmp_real(stack, "setbe"),
        Instruction::GeqReal => emit_cmp_real(stack, "setae"),

        Instruction::Not => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        xor rax, 0xffffffffffffffff"));
            stack.push(cow!(b "        pop rax"));
        }
        Instruction::And => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        and rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::Or => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        or rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::Xor => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        xor rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::Imply => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        or rax, rcx"));
            stack.push(cow!(b "        xor rax, 1"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }
        Instruction::Equiv => {
            stack.push(cow!(b "        push rax"));
            stack.push(cow!(b "        xor rax, 0xffffffffffffffff"));
            stack.push(cow!(b "        xor rax, rcx"));
            stack.push(cow!(b "        pop rax"));
            stack.push(cow!(b "        pop rcx"));
        }

        Instruction::Label(name) => stack.push(cow!(o "{name}:")),
        Instruction::Jmp(name) => stack.push(cow!(o "        jmp {name}")),
        Instruction::JmpIfFalse(name) => {
            stack.push(cow!(o "        jz {name}"));
            stack.push(cow!(b "        test rax, rax"));
            stack.push(cow!(b "        pop rax"));
        }

        Instruction::Call(name) => stack.push(cow!(o "        call f_{name}")),
        Instruction::Return => stack.push(cow!(b "        ret")),
    }
}

fn emit_cmp_int(stack: &mut Vec<Cow<'static, str>>, instruction: &str) {
    stack.push(cow!(b "        push rax"));
    stack.push(cow!(b "        movzx rax, al"));
    stack.push(cow!(o "        {instruction} al"));
    stack.push(cow!(b "        cmp rax, rcx"));
    stack.push(cow!(b "        pop rax"));
    stack.push(cow!(b "        pop rcx"));
}

fn emit_cmp_real(stack: &mut Vec<Cow<'static, str>>, instruction: &str) {
    stack.push(cow!(b "        push rax"));
    stack.push(cow!(b "        movzx rax, al"));
    stack.push(cow!(o "        {instruction} al"));
    stack.push(cow!(b "        ucomisd xmm0, xmm1"));
    stack.push(cow!(b "        movq xmm0, rax"));
    stack.push(cow!(b "        movq xmm1, rcx"));
    stack.push(cow!(b "        pop rax"));
    stack.push(cow!(b "        pop rcx"));
}

fn emit_op_real(stack: &mut Vec<Cow<'static, str>>, instruction: &str) {
    stack.push(cow!(b "        push rax"));
    stack.push(cow!(b "        movq rax, xmm0"));
    stack.push(cow!(o "        {instruction} xmm0, xmm1"));
    stack.push(cow!(b "        movq xmm0, rax"));
    stack.push(cow!(b "        movq xmm1, rcx"));
    stack.push(cow!(b "        pop rax"));
    stack.push(cow!(b "        pop rcx"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_generation_order() {
        let mut generator = Amd64Generator::Instruction {
            instructions: [Instruction::AddInt].into_iter(),
            current_section: Section::Text,
            line_buffer: Vec::new(),
        };

        assert_eq!(generator.next(), Some(cow!(b "        pop rcx")));
        assert_eq!(generator.next(), Some(cow!(b "        pop rax")));
        assert_eq!(generator.next(), Some(cow!(b "        add rax, rcx")));
        assert_eq!(generator.next(), Some(cow!(b "        push rax")));
        assert_eq!(generator.next(), None);
    }
}
