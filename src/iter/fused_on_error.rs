/// Iterator that can be wrapped to stop on first error
pub trait FusedOnError: Iterator {
    /// Wraps the iterator to stop yielding after the first `Err`
    fn fused_on_error(self) -> FusedOnErrorAdapter<Self>
    where
        Self: Sized;
}

impl<O, E, I: Iterator<Item = Result<O, E>>> FusedOnError for I {
    fn fused_on_error(self) -> FusedOnErrorAdapter<Self> {
        FusedOnErrorAdapter::new(self)
    }
}

/// Adapter that stops yielding after first `Err`
pub enum FusedOnErrorAdapter<I: Iterator> {
    Active(I),
    Fused,
}

impl<O, E, I: Iterator<Item = Result<O, E>>> FusedOnErrorAdapter<I> {
    pub fn new(source: I) -> FusedOnErrorAdapter<I> {
        FusedOnErrorAdapter::Active(source)
    }
}

impl<O, E, I: Iterator<Item = Result<O, E>>> Iterator for FusedOnErrorAdapter<I> {
    type Item = Result<O, E>;

    fn next(&mut self) -> Option<Self::Item> {
        let FusedOnErrorAdapter::Active(source) = self else {
            return None;
        };

        match source.next() {
            Some(Ok(val)) => Some(Ok(val)),
            Some(Err(err)) => {
                *self = FusedOnErrorAdapter::Fused;
                Some(Err(err))
            }
            None => {
                *self = FusedOnErrorAdapter::Fused;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::*;

    #[test]
    fn test_no_err() {
        let iter: array::IntoIter<Result<usize, ()>, _> = [Ok(1), Ok(2), Ok(3)].into_iter();

        let mut fused_on_error = iter.fused_on_error();

        assert_eq!(fused_on_error.next(), Some(Ok(1)));
        assert_eq!(fused_on_error.next(), Some(Ok(2)));
        assert_eq!(fused_on_error.next(), Some(Ok(3)));
        assert_eq!(fused_on_error.next(), None);
        assert_eq!(fused_on_error.next(), None);
    }

    #[test]
    fn test_err() {
        let iter: array::IntoIter<Result<usize, ()>, _> = [Ok(1), Ok(2), Err(()), Ok(3)].into_iter();
        let mut fused_on_error = iter.fused_on_error();

        assert_eq!(fused_on_error.next(), Some(Ok(1)));
        assert_eq!(fused_on_error.next(), Some(Ok(2)));
        assert_eq!(fused_on_error.next(), Some(Err(())));
        assert_eq!(fused_on_error.next(), None);
        assert_eq!(fused_on_error.next(), None);
    }
}
