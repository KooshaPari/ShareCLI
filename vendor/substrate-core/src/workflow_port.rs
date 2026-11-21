//! WorkflowPort — DAG orchestration over dependent tasks.
//!
//! Core defines workflow shapes; `substrate-dag` implements graph algorithms
//! (topological order, ready-set, cycle detection) via petgraph.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A node in a workflow DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNode {
    /// Stable node identifier.
    pub id: String,
}

/// A directed dependency edge (`from` must complete before `to` may start).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    /// Upstream task id.
    pub from: String,
    /// Downstream task id.
    pub to: String,
}

/// A workflow: a DAG of dependent tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    /// All task nodes.
    pub nodes: Vec<WorkflowNode>,
    /// Dependency edges.
    pub edges: Vec<WorkflowEdge>,
}

/// DAG orchestration port.
pub trait WorkflowPort: Send + Sync {
    /// Validate the workflow is acyclic; reject cycles.
    fn validate_acyclic(&self, workflow: &Workflow) -> Result<()>;

    /// Return a topological execution order (dependencies first).
    fn topological_order(&self, workflow: &Workflow) -> Result<Vec<String>>;

    /// Return node ids whose dependencies are all in `completed` and not yet done.
    fn ready_set(&self, workflow: &Workflow, completed: &[String]) -> Result<Vec<String>>;
}
