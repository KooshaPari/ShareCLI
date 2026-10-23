// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::collections::{HashMap, HashSet, VecDeque};

pub struct Graph {
    adj: HashMap<usize, Vec<(usize, u32)>>,
}
impl Graph {
    pub fn new() -> Self {
        Self { adj: HashMap::new() }
    }
    pub fn add_node(&mut self, n: usize) {
        self.adj.entry(n).or_default();
    }
    pub fn add_edge(&mut self, from: usize, to: usize, weight: u32) {
        self.adj.entry(from).or_default().push((to, weight));
        self.adj.entry(to).or_default();
    }
    pub fn neighbors(&self, n: usize) -> Vec<usize> {
        self.adj.get(&n).cloned().unwrap_or_default().into_iter().map(|(n, _)| n).collect()
    }
    pub fn bfs(&self, start: usize) -> Vec<usize> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut order = Vec::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(n) = queue.pop_front() {
            order.push(n);
            for &(next, _) in &self.adj[&n] {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        order
    }
    pub fn nodes(&self) -> usize {
        self.adj.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        let mut g = Graph::new();
        g.add_edge(1, 2, 1);
        g.add_edge(2, 3, 1);
        assert_eq!(g.nodes(), 3);
    }
    #[test]
    fn test_neighbors() {
        let mut g = Graph::new();
        g.add_edge(1, 2, 1);
        g.add_edge(1, 3, 1);
        assert_eq!(g.neighbors(1).len(), 2);
    }
    #[test]
    fn test_bfs() {
        let mut g = Graph::new();
        g.add_edge(1, 2, 1);
        g.add_edge(2, 3, 1);
        g.add_edge(1, 3, 1);
        let order = g.bfs(1);
        assert_eq!(order[0], 1);
        assert!(order.contains(&3));
    }
    #[test]
    fn test_isolated() {
        let mut g = Graph::new();
        g.add_node(99);
        assert_eq!(g.bfs(99), vec![99]);
    }
}
