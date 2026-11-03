// SPDX-License-Identifier: Apache-2.0 OR MIT
pub struct Deque<T> {
    items: Vec<T>,
}
impl<T> Deque<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    pub fn push_back(&mut self, v: T) {
        self.items.push(v);
    }
    pub fn push_front(&mut self, v: T) {
        self.items.insert(0, v);
    }
    pub fn pop_back(&mut self) -> Option<T> {
        self.items.pop()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items.remove(0))
        }
    }
    pub fn front(&self) -> Option<&T> {
        self.items.first()
    }
    pub fn back(&self) -> Option<&T> {
        self.items.last()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_push_back() {
        let mut d: Deque<i32> = Deque::new();
        d.push_back(1);
        d.push_back(2);
        assert_eq!(d.front(), Some(&1));
        assert_eq!(d.back(), Some(&2));
    }
    #[test]
    fn test_push_front() {
        let mut d: Deque<i32> = Deque::new();
        d.push_front(2);
        d.push_front(1);
        assert_eq!(d.front(), Some(&1));
    }
    #[test]
    fn test_pop() {
        let mut d: Deque<i32> = Deque::new();
        d.push_back(1);
        d.push_back(2);
        assert_eq!(d.pop_front(), Some(1));
        assert_eq!(d.pop_back(), Some(2));
        assert_eq!(d.pop_front(), None);
    }
    #[test]
    fn test_empty() {
        let d: Deque<i32> = Deque::new();
        assert!(d.is_empty());
        assert_eq!(d.front(), None);
    }
}
