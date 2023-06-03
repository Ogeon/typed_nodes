use std::iter::Peekable;

pub(crate) trait IterExt: Iterator + Sized {
    fn with_is_last(self) -> IsLastIter<Self> {
        IsLastIter {
            inner: self.peekable(),
        }
    }
}

impl<I: Iterator> IterExt for I {}

pub(crate) struct IsLastIter<I: Iterator> {
    inner: Peekable<I>,
}

impl<I: Iterator> Iterator for IsLastIter<I> {
    type Item = (bool, I::Item);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(value) = self.inner.next() {
            let is_last = self.inner.peek().is_none();

            Some((is_last, value))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
