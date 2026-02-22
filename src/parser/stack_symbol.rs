use crate::{
    lexer::token::Token,
    parser::{action::Action, non_terminal::NonTerminal},
};

#[derive(Debug, Clone)]
pub enum StackSymbol {
    Terminal(Token),
    NonTerminal(NonTerminal),
    Action(Action),
}
