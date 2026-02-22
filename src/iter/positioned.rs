/// Iterators that can track (line, column) positions
pub trait Positioned: Iterator {
    /// Wraps the iterator to yield `(item, line, column)`
    fn positioned(self) -> PositionedAdapter<Self>
    where
        Self: Sized;
}

impl<E, I: Iterator<Item = Result<char, E>>> Positioned for I {
    fn positioned(self) -> PositionedAdapter<Self>
    where
        Self: Sized,
    {
        PositionedAdapter::new(self)
    }
}

/// Wraps an iterator to add line/column tracking
pub struct PositionedAdapter<I: Iterator> {
    inner: I,
    line: usize,
    column: usize,
}

impl<E, I: Iterator<Item = Result<char, E>>> PositionedAdapter<I> {
    /// Wraps an iterator to yield (line, column) positions
    pub fn new(inner: I) -> PositionedAdapter<I> {
        PositionedAdapter {
            inner,
            line: 1,
            column: 1,
        }
    }
}

impl<E, I: Iterator<Item = Result<char, E>>> Iterator for PositionedAdapter<I> {
    // (character, line, column)
    type Item = Result<(char, usize, usize), E>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Ok(read_char)) => {
                let (prev_line, prev_col) = (self.line, self.column);
                if read_char == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }

                Some(Ok((read_char, prev_line, prev_col)))
            }
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::*;

    #[test]
    fn test_positioned_tracking() {
        let input: array::IntoIter<Result<char, ()>, _> = [Ok('a'), Ok('\n'), Ok('b')].into_iter();
        let mut iter = input.positioned();

        assert_eq!(iter.next(), Some(Ok(('a', 1, 1))));
        assert_eq!(iter.next(), Some(Ok(('\n', 1, 2))));
        assert_eq!(iter.next(), Some(Ok(('b', 2, 1))));
        assert_eq!(iter.next(), None);
    }
}
