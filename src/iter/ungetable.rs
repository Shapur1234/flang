/// Trait for iterators that support `unget` (push back an item) an unbounded number of times
pub trait Ungetable: Iterator {
    /// Wraps the iterator to support `unget` operations
    fn ungetable(self) -> UngetableAdapter<Self>
    where
        Self: Sized,
    {
        UngetableAdapter::new(self)
    }
}
impl<I: Iterator> Ungetable for I {}

/// Adapter that allows pushing items back to be yielded later
pub struct UngetableAdapter<I: Iterator> {
    source: I,
    stack: Vec<I::Item>,
}

impl<I: Iterator> UngetableAdapter<I> {
    /// Creates a new ungetable adapter
    pub fn new(source: I) -> Self {
        Self {
            source,
            stack: Vec::new(),
        }
    }

    /// Pushes an item back to be returned on next `next()`
    pub fn unget(&mut self, item: I::Item) {
        self.stack.push(item);
    }

    /// Peeks at the next item without consuming it
    pub fn peek(&mut self) -> Option<&I::Item> {
        if self.stack.is_empty()
            && let Some(item) = self.source.next()
        {
            self.stack.push(item);
        }

        self.stack.last()
    }

    /// Returns next item if it matches the predicate
    pub fn next_if(&mut self, predicate: impl FnOnce(&I::Item) -> bool) -> Option<I::Item> {
        if let Some(item) = self.peek()
            && predicate(item)
        {
            return self.next();
        }
        None
    }
}

impl<I: Iterator> Iterator for UngetableAdapter<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.stack.pop().or_else(|| self.source.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_unget() {
        let mut iter = [1, 2, 3].into_iter().ungetable();

        assert_eq!(iter.next(), Some(1));

        iter.unget(1);

        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
    }

    #[test]
    fn test_lifo_behavior() {
        let mut iter = "abc".chars().ungetable();

        let a = iter.next().unwrap();
        let b = iter.next().unwrap();

        iter.unget(a);
        iter.unget(b);

        assert_eq!(iter.next(), Some('b'));
        assert_eq!(iter.next(), Some('a'));
        assert_eq!(iter.next(), Some('c'));
    }

    #[test]
    fn test_unget_at_end() {
        let mut iter = [1].into_iter().ungetable();

        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), None);

        iter.unget(42);
        assert_eq!(iter.next(), Some(42));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_peek() {
        let mut iter = [1, 2, 3].into_iter().ungetable();

        assert_eq!(iter.peek(), Some(&1));
        assert_eq!(iter.peek(), Some(&1));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.peek(), Some(&2));
    }

    #[test]
    fn test_next_if() {
        let mut iter = [10, 20, 30].into_iter().ungetable();

        assert_eq!(iter.next_if(|&x| x > 15), None);
        assert_eq!(iter.peek(), Some(&10));

        assert_eq!(iter.next_if(|&x| x == 10), Some(10));
        assert_eq!(iter.peek(), Some(&20));
    }

    #[test]
    fn test_unget_peek_interaction() {
        let mut iter = ['a', 'b'].into_iter().ungetable();

        assert_eq!(iter.next(), Some('a'));

        iter.unget('a');
        assert_eq!(iter.peek(), Some(&'a'));

        assert_eq!(iter.next_if(|&c| c == 'a'), Some('a'));

        assert_eq!(iter.next(), Some('b'));
    }

    #[test]
    fn test_peek_end() {
        let mut iter = [1].into_iter().ungetable();
        iter.next();

        assert_eq!(iter.peek(), None);
        assert_eq!(iter.next_if(|_| true), None);
    }
}
