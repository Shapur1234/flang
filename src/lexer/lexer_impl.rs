use crate::{
    error::{CompilationError, LexerError, ReadError},
    iter::{FusedOnError, Positioned, Ungetable, UngetableAdapter},
    lexer::token::{Ident, Keyword, Literal, Op, PositionedToken, Punct, Token},
};

/// Creates a lexer iterator over input characters
///
/// Returns positioned tokens or compilation errors
pub fn lex(
    input: impl Iterator<Item = Result<char, ReadError>>,
) -> impl Iterator<Item = Result<PositionedToken, CompilationError>> {
    Lexer::new(input.positioned()).fused_on_error()
}

trait InputIterator = Iterator<Item = Result<(char, usize, usize), ReadError>>;

struct Lexer<I: InputIterator> {
    input: UngetableAdapter<I>,
    buffer: String,
}

impl<I: InputIterator> Lexer<I> {
    fn new(input: I) -> Lexer<I> {
        Lexer {
            input: input.ungetable(),
            buffer: String::new(),
        }
    }

    fn skip_whitespaces_and_comments(&mut self) {
        loop {
            match self.input.peek() {
                Some(Ok((char, ..))) if char.is_whitespace() => {
                    self.input.next();
                }
                Some(Ok(('/', ..))) => {
                    let first_slash = self.input.next().unwrap();
                    if !matches!(self.input.peek(), Some(Ok(('/', ..)))) {
                        self.input.unget(first_slash);
                        return;
                    }

                    self.input.next();
                    while self
                        .input
                        .next_if(|res| matches!(res, Ok((char, ..)) if *char != '\n'))
                        .is_some()
                    {}
                }
                _ => return,
            }
        }
    }

    fn read_identifier_or_keyword(
        &mut self,
        first_char: char,
        line: usize,
        column: usize,
    ) -> Option<Result<PositionedToken, CompilationError>> {
        if !first_char.is_alphabetic() && first_char != '_' {
            return None;
        }

        self.buffer.clear();
        self.buffer.push(first_char);

        while let Some(res) = self.input.next() {
            match res {
                Ok((char, ..)) if char.is_alphanumeric() || char == '_' => {
                    self.buffer.push(char);
                }
                Ok(other) => {
                    self.input.unget(Ok(other));
                    break;
                }
                Err(e) => return Some(Err(CompilationError::Read(e))),
            }
        }

        let token_type = match self.buffer.as_str() {
            "TRUE" => Ok(Token::Literal(Literal::Bool(true))),
            "FALSE" => Ok(Token::Literal(Literal::Bool(false))),

            "Bool" => Ok(Token::Keyword(Keyword::Bool)),
            "Int" => Ok(Token::Keyword(Keyword::Int)),
            "Real" => Ok(Token::Keyword(Keyword::Real)),

            "and" => Ok(Token::Op(Op::And)),
            "array" => Ok(Token::Keyword(Keyword::Array)),
            "break" => Ok(Token::Keyword(Keyword::Break)),
            "const" => Ok(Token::Keyword(Keyword::Const)),
            "continue" => Ok(Token::Keyword(Keyword::Continue)),
            "do" => Ok(Token::Keyword(Keyword::Do)),
            "downto" => Ok(Token::Keyword(Keyword::Downto)),
            "else" => Ok(Token::Keyword(Keyword::Else)),
            "equiv" => Ok(Token::Op(Op::Equiv)),
            "exit" => Ok(Token::Keyword(Keyword::Exit)),
            "fn" => Ok(Token::Keyword(Keyword::Fn)),
            "for" => Ok(Token::Keyword(Keyword::For)),
            "if" => Ok(Token::Keyword(Keyword::If)),
            "imply" => Ok(Token::Op(Op::Imply)),
            "not" => Ok(Token::Op(Op::Not)),
            "of" => Ok(Token::Keyword(Keyword::Of)),
            "or" => Ok(Token::Op(Op::Or)),
            "proc" => Ok(Token::Keyword(Keyword::Proc)),
            "program" => Ok(Token::Keyword(Keyword::Program)),
            "then" => Ok(Token::Keyword(Keyword::Then)),
            "to" => Ok(Token::Keyword(Keyword::To)),
            "var" => Ok(Token::Keyword(Keyword::Var)),
            "while" => Ok(Token::Keyword(Keyword::While)),
            "xor" => Ok(Token::Op(Op::Xor)),

            ident => {
                if Ident::is_valid_identifier(ident) {
                    Ok(Token::Ident(Ident(ident.to_string().into())))
                } else {
                    Err(LexerError::InvalidIdentifier {
                        ident: ident.to_string(),
                    })
                }
            }
        };

        Some(
            token_type
                .map(|token_type| PositionedToken {
                    payload: token_type,
                    line,
                    column,
                })
                .map_err(|err| CompilationError::Lexer { err, line, column }),
        )
    }

    fn read_operator_or_punct(
        &mut self,
        first_char: char,
        line: usize,
        column: usize,
    ) -> Option<Result<PositionedToken, CompilationError>> {
        let next_char = match self.input.peek() {
            Some(Ok((char, ..))) => Some(*char),
            _ => None,
        };

        let (token_type, consume_next) = match (first_char, next_char) {
            (':', Some('=')) => (Token::Op(Op::Assign), true),
            ('!', Some('=')) => (Token::Op(Op::Neq), true),
            ('<', Some('=')) => (Token::Op(Op::Leq), true),
            ('>', Some('=')) => (Token::Op(Op::Geq), true),
            ('-', Some('>')) => (Token::Punct(Punct::Arrow), true),
            ('.', Some('.')) => (Token::Punct(Punct::DotDot), true),

            ('=', _) => (Token::Op(Op::Eq), false),
            ('<', _) => (Token::Op(Op::Lt), false),
            ('>', _) => (Token::Op(Op::Gt), false),
            ('+', _) => (Token::Op(Op::Plus), false),
            ('-', _) => (Token::Op(Op::Minus), false),
            ('*', _) => (Token::Op(Op::Times), false),
            ('/', _) => (Token::Op(Op::Div), false),
            ('%', _) => (Token::Op(Op::Mod), false),

            ('(', _) => (Token::Punct(Punct::LBracket), false),
            (')', _) => (Token::Punct(Punct::RBracket), false),
            ('{', _) => (Token::Punct(Punct::LCurlyBracket), false),
            ('}', _) => (Token::Punct(Punct::RCurlyBracket), false),
            ('[', _) => (Token::Punct(Punct::LSquareBracket), false),
            (']', _) => (Token::Punct(Punct::RSquareBracket), false),
            (':', _) => (Token::Punct(Punct::Colon), false),
            (';', _) => (Token::Punct(Punct::Semicolon), false),
            (',', _) => (Token::Punct(Punct::Comma), false),
            ('.', _) => (Token::Punct(Punct::Dot), false),

            _ => return None,
        };

        if consume_next {
            self.input.next();
        }

        Some(Ok(PositionedToken {
            payload: token_type,
            line,
            column,
        }))
    }

    fn check_trailing_not_number(&mut self, line: usize, column: usize) -> Result<(), CompilationError> {
        if let Some(Ok((char, ..))) = self.input.peek()
            && (char.is_alphanumeric() || *char == '_' || *char == '#')
        {
            let char = *char;
            self.input.next();

            return Err(CompilationError::Lexer {
                err: LexerError::UnexpectedChar { char },
                line,
                column,
            });
        }

        Ok(())
    }

    #[allow(clippy::from_str_radix_10, clippy::is_digit_ascii_radix, clippy::too_many_lines)]
    fn read_number(
        &mut self,
        first_char: char,
        line: usize,
        column: usize,
    ) -> Option<Result<PositionedToken, CompilationError>> {
        let first_digit = if first_char.is_ascii_digit() {
            first_char
        } else {
            return None;
        };

        self.buffer.clear();
        self.buffer.push(first_digit);

        while let Some(Ok((next_char, ..))) = self.input.peek() {
            if next_char.is_digit(10) {
                self.buffer.push(*next_char);
                self.input.next();
            } else {
                break;
            }
        }

        let base = if let Some(Ok(('#', ..))) = self.input.peek() {
            self.input.next();

            let base = match u32::from_str_radix(&self.buffer, 10) {
                Ok(num) => {
                    if matches!(num, 2 | 8 | 10 | 16) {
                        num
                    } else {
                        return Some(Err(CompilationError::Lexer {
                            err: LexerError::InvalidBase { base: num },
                            line,
                            column,
                        }));
                    }
                }
                Err(err) => {
                    return Some(Err(CompilationError::Lexer {
                        err: LexerError::InvalidBaseParse {
                            base: self.buffer.clone(),
                            err,
                        },
                        line,
                        column,
                    }));
                }
            };
            self.buffer.clear();

            while let Some(Ok((next_char, ..))) = self.input.peek() {
                if next_char.is_digit(base) {
                    self.buffer.push(*next_char);
                    self.input.next();
                } else if *next_char == '.' {
                    let dot = self.input.next().unwrap();
                    if let Some(Ok(('.', ..))) = self.input.peek() {
                        self.input.unget(dot);
                        break;
                    }

                    self.buffer.push('.');
                } else {
                    break;
                }
            }

            base
        } else if let Some(Ok(('.', ..))) = self.input.peek() {
            let dot = self.input.next().unwrap();
            if let Some(Ok(('.', ..))) = self.input.peek() {
                self.input.unget(dot);
                10
            } else {
                self.buffer.push('.');
                while let Some(Ok((next_char, ..))) = self.input.peek() {
                    if next_char.is_digit(10) {
                        self.buffer.push(*next_char);
                        self.input.next();
                    } else {
                        break;
                    }
                }
                10
            }
        } else {
            10
        };

        if let Err(err) = self.check_trailing_not_number(line, column) {
            return Some(Err(err));
        }

        let dot_count = self.buffer.matches('.').count();
        if self.buffer.is_empty() || self.buffer == "." || dot_count > 1 {
            return Some(Err(CompilationError::Lexer {
                err: LexerError::UnexpectedChar {
                    char: if dot_count > 1 { '.' } else { '#' },
                },
                line,
                column,
            }));
        }

        if let Some((whole_str, decimal_str)) = self.buffer.split_once('.') {
            let whole_int = if whole_str.is_empty() {
                0
            } else {
                match i64::from_str_radix(whole_str, base) {
                    Ok(num) => num,
                    Err(err) => {
                        return Some(Err(CompilationError::Lexer {
                            err: LexerError::RealWholePartParse {
                                whole_part: whole_str.to_string(),
                                base,
                                err,
                            },
                            line,
                            column,
                        }));
                    }
                }
            };

            let decimal_int = if decimal_str.is_empty() {
                0
            } else {
                match i64::from_str_radix(decimal_str, base) {
                    Ok(num) => num,
                    Err(err) => {
                        return Some(Err(CompilationError::Lexer {
                            err: LexerError::RealDecimalPartParse {
                                decimal_part: decimal_str.to_string(),
                                base,
                                err,
                            },
                            line,
                            column,
                        }));
                    }
                }
            };

            #[allow(clippy::cast_precision_loss)]
            let value = {
                let whole_float = whole_int as f64;
                let decimal_float = decimal_int as f64;
                let decimal_len_i32 = i32::try_from(decimal_str.len()).unwrap();
                let base_f64 = f64::from(base);

                whole_float + decimal_float * base_f64.powi(-decimal_len_i32)
            };

            Some(Ok(PositionedToken {
                payload: Token::Literal(Literal::Real(i64::from_ne_bytes(value.to_ne_bytes()))),
                line,
                column,
            }))
        } else {
            let value = match i64::from_str_radix(&self.buffer, base) {
                Ok(num) => num,
                Err(err) => {
                    return Some(Err(CompilationError::Lexer {
                        err: LexerError::IntParse {
                            literal: self.buffer.clone(),
                            base,
                            err,
                        },
                        line,
                        column,
                    }));
                }
            };

            Some(Ok(PositionedToken {
                payload: Token::Literal(Literal::Int(value)),
                line,
                column,
            }))
        }
    }
}

impl<I: Iterator<Item = Result<(char, usize, usize), ReadError>>> Iterator for Lexer<I> {
    type Item = Result<PositionedToken, CompilationError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespaces_and_comments();

        let (char, line, column) = match self.input.next()? {
            Ok(next) => next,
            Err(e) => {
                return Some(Err(CompilationError::Read(e)));
            }
        };

        if let Some(token) = self.read_operator_or_punct(char, line, column) {
            return Some(token);
        }

        if let Some(token) = self.read_number(char, line, column) {
            return Some(token);
        }

        if let Some(token) = self.read_identifier_or_keyword(char, line, column) {
            return Some(token);
        }

        Some(Err(CompilationError::Lexer {
            err: LexerError::UnexpectedChar { char },
            line,
            column,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant, clippy::cast_possible_wrap)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    fn lex_ok(s: &str) -> Vec<Token> {
        lex(s.chars().map(Ok)).map(|x| x.unwrap().payload).collect()
    }

    fn lex_err(s: &str) -> CompilationError {
        lex(s.chars().map(Ok))
            .find_map(Result::err)
            .expect("Expected lexer to return an error")
    }

    #[test]
    fn test_whitespace_and_comments() {
        assert!(lex_ok("   \n\t  ").is_empty());
        assert!(lex_ok("// hello\n").is_empty());
        assert!(lex_ok("// x y z").is_empty());
        assert_eq!(
            lex_ok("  //a\n  //b\nvar_name"),
            vec![Token::Ident(Ident(Cow::Borrowed("var_name")))]
        );
        assert_eq!(
            lex_ok("// skip\nalpha_one"),
            vec![Token::Ident(Ident(Cow::Borrowed("alpha_one")))]
        );
    }

    #[test]
    fn test_identifiers_valid() {
        let valid = ["foo_bar", "_hidden", "name_", "MyType", "Alpha123", "a_b_c"];
        for id in valid {
            assert_eq!(lex_ok(id)[0], Token::Ident(Ident(Cow::Owned(id.to_string()))));
        }
    }

    #[test]
    fn test_identifiers_invalid() {
        assert!(matches!(lex_err("bad__name"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("__bad"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("name__"), CompilationError::Lexer { .. }));
    }

    #[test]
    fn test_boolean_literals() {
        assert_eq!(lex_ok("TRUE")[0], Token::Literal(Literal::Bool(true)));
        assert_eq!(lex_ok("FALSE")[0], Token::Literal(Literal::Bool(false)));
    }

    #[test]
    fn test_keywords() {
        let keywords = [
            ("program", Keyword::Program),
            ("var", Keyword::Var),
            ("const", Keyword::Const),
            ("fn", Keyword::Fn),
            ("proc", Keyword::Proc),
            ("exit", Keyword::Exit),
            ("if", Keyword::If),
            ("then", Keyword::Then),
            ("else", Keyword::Else),
            ("while", Keyword::While),
            ("do", Keyword::Do),
            ("for", Keyword::For),
            ("to", Keyword::To),
            ("downto", Keyword::Downto),
            ("break", Keyword::Break),
            ("continue", Keyword::Continue),
            ("array", Keyword::Array),
            ("of", Keyword::Of),
        ];
        for (s, kw) in keywords {
            assert_eq!(lex_ok(s)[0], Token::Keyword(kw));
        }
    }

    #[test]
    fn test_logic_operators() {
        let ops = [
            ("not", Op::Not),
            ("and", Op::And),
            ("or", Op::Or),
            ("xor", Op::Xor),
            ("imply", Op::Imply),
            ("equiv", Op::Equiv),
        ];
        for (s, op) in ops {
            assert_eq!(lex_ok(s)[0], Token::Op(op));
        }
    }

    #[test]
    fn test_single_char_operators() {
        let ops = [
            ("=", Op::Eq),
            ("<", Op::Lt),
            (">", Op::Gt),
            ("+", Op::Plus),
            ("-", Op::Minus),
            ("*", Op::Times),
            ("/", Op::Div),
            ("%", Op::Mod),
        ];
        for (s, op) in ops {
            assert_eq!(lex_ok(s)[0], Token::Op(op));
        }
    }

    #[test]
    fn test_multi_char_operators() {
        let ops = [(":=", Op::Assign), ("!=", Op::Neq), ("<=", Op::Leq), (">=", Op::Geq)];
        for (s, op) in ops {
            assert_eq!(lex_ok(s)[0], Token::Op(op));
        }
    }

    #[test]
    fn test_punctuation() {
        let puncts = [
            ("(", Punct::LBracket),
            (")", Punct::RBracket),
            ("{", Punct::LCurlyBracket),
            ("}", Punct::RCurlyBracket),
            ("[", Punct::LSquareBracket),
            ("]", Punct::RSquareBracket),
            (":", Punct::Colon),
            (";", Punct::Semicolon),
            (",", Punct::Comma),
            (".", Punct::Dot),
            ("..", Punct::DotDot),
        ];
        for (s, p) in puncts {
            assert_eq!(lex_ok(s)[0], Token::Punct(p));
        }
    }

    #[test]
    fn test_integers() {
        assert_eq!(lex_ok("0")[0], Token::Literal(Literal::Int(0)));
        assert_eq!(lex_ok("42")[0], Token::Literal(Literal::Int(42)));
        assert_eq!(
            lex_ok("-123"),
            vec![Token::Op(Op::Minus), Token::Literal(Literal::Int(123))]
        );
    }

    #[test]
    fn test_reals() {
        assert_eq!(
            lex_ok("0.0")[0],
            Token::Literal(Literal::Real(0.0_f64.to_bits() as i64))
        );
        assert_eq!(
            lex_ok("3.14159")[0],
            Token::Literal(Literal::Real(3.14159_f64.to_bits() as i64))
        );
        assert_eq!(
            lex_ok("42.0")[0],
            Token::Literal(Literal::Real(42.0_f64.to_bits() as i64))
        );
    }

    #[test]
    fn test_based_integers() {
        assert_eq!(lex_ok("2#1010")[0], Token::Literal(Literal::Int(10)));
        assert_eq!(lex_ok("8#70")[0], Token::Literal(Literal::Int(56)));
        assert_eq!(lex_ok("16#FF")[0], Token::Literal(Literal::Int(255)));
        assert_eq!(lex_ok("10#99")[0], Token::Literal(Literal::Int(99)));
    }

    #[test]
    fn test_based_reals() {
        assert_eq!(
            lex_ok("2#11.01")[0],
            Token::Literal(Literal::Real(3.25_f64.to_bits() as i64))
        );
        assert_eq!(
            lex_ok("16#A.F")[0],
            Token::Literal(Literal::Real(10.9375_f64.to_bits() as i64))
        );
    }

    #[test]
    fn test_range_operator() {
        let tokens = lex_ok("0..20");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Literal(Literal::Int(0)));
        assert_eq!(tokens[1], Token::Punct(Punct::DotDot));
        assert_eq!(tokens[2], Token::Literal(Literal::Int(20)));
    }

    #[test]
    fn test_minus_parsing() {
        let expected = [
            Token::Ident(Ident(Cow::Borrowed("x"))),
            Token::Op(Op::Minus),
            Token::Literal(Literal::Int(1)),
        ];
        assert_eq!(lex_ok("x - 1"), expected);
        assert_eq!(lex_ok("x -1"), expected);
        assert_eq!(lex_ok("x-1"), expected);
    }

    #[test]
    fn test_lexer_error_invalid_base() {
        assert!(matches!(lex_err("5#123"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("0#0"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("16#"), CompilationError::Lexer { .. }));
    }

    #[test]
    fn test_lexer_error_invalid_number() {
        assert!(matches!(lex_err("42a"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("2#1.0.1"), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("10#."), CompilationError::Lexer { .. }));
        assert!(matches!(lex_err("16#ABC.G"), CompilationError::Lexer { .. }));
    }
}
