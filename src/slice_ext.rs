// SPDX-License-Identifier: Apache-2.0 OR MIT
pub trait SplitAtInclusive<T> {
    fn split_at_inclusive(&self, mid: usize) -> (&[T], &[T]);
}
impl<T> SplitAtInclusive<T> for [T] {
    fn split_at_inclusive(&self, mid: usize) -> (&[T], &[T]) {
        self.split_at(mid + 1)
    }
}
pub fn chunks_with_remainder<T>(items: &[T], chunk_size: usize) -> Vec<Vec<&T>> {
    items.chunks(chunk_size).map(|c| c.iter().collect()).collect()
}
pub fn windowed<T>(items: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 || size > items.len() {
        return Vec::new();
    }
    items.windows(size).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_inc() {
        let v = [1, 2, 3, 4];
        let (l, r) = v.split_at_inclusive(1);
        assert_eq!(l, &[1, 2]);
        assert_eq!(r, &[3, 4]);
    }
    #[test]
    fn chunks_rem() {
        let v = [1, 2, 3, 4, 5];
        let c = chunks_with_remainder(&v, 2);
        assert_eq!(c.len(), 3);
    }
    #[test]
    fn windowed_basic() {
        let v = [1, 2, 3, 4];
        let w = windowed(&v, 2);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0], &[1, 2]);
    }
    #[test]
    fn windowed_too_large() {
        let v = [1, 2];
        assert!(windowed(&v, 5).is_empty());
    }
    #[test]
    fn windowed_zero() {
        let v = [1, 2];
        assert!(windowed(&v, 0).is_empty());
    }
}
