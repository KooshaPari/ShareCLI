//! Use-case test with in-memory fakes — proves the dispatch flow with no IO.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use substrate_app::DispatchService;
use substrate_core::{
    domain::{
        ConversationDump, EngineCapabilities, Mailbox, Message, Session, StructuredResult, Task,
        TaskState,
    },
    error::{Result, SubstrateError},
    ports::{DispatchApi, EnginePort, StorePort, TransportPort},
};
use uuid::Uuid;

struct FakeEngine;

#[async_trait]
impl EnginePort for FakeEngine {
    async fn start(&self, _task: &Task) -> Result<Session> {
        Ok(Session {
            conv_id: "conv-1".into(),
            pid: None,
            logfile: None,
        })
    }
    async fn resume(&self, conv_id: &str, _prompt: &str) -> Result<Session> {
        Ok(Session {
            conv_id: conv_id.into(),
            pid: None,
            logfile: None,
        })
    }
    async fn dump(&self, conv_id: &str) -> Result<ConversationDump> {
        Ok(ConversationDump {
            conversation_id: conv_id.into(),
            raw: "{}".into(),
        })
    }
    async fn cancel(&self, _conv_id: &str) -> Result<()> {
        Ok(())
    }
    async fn wire_mailbox(&self, _c: &str, _m: &Mailbox) -> Result<()> {
        Ok(())
    }
    fn extract_result(&self, _dump: &ConversationDump) -> Result<StructuredResult> {
        Ok(StructuredResult {
            text: "all done".into(),
            artifacts: vec![],
            pr_urls: vec![],
            status: TaskState::Completed,
        })
    }
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            supports_resume: true,
            supports_subagents: false,
            supports_mcp_import: false,
        }
    }
}

#[derive(Default)]
struct MemStore {
    tasks: Mutex<Vec<Task>>,
    results: Mutex<Vec<(Uuid, StructuredResult)>>,
}

#[async_trait]
impl StorePort for MemStore {
    async fn persist(&self, task: &Task) -> Result<()> {
        let mut g = self.tasks.lock().unwrap();
        g.retain(|t| t.id != task.id);
        g.push(task.clone());
        Ok(())
    }
    async fn load(&self, id: &Uuid) -> Result<Task> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| &t.id == id)
            .cloned()
            .ok_or_else(|| SubstrateError::NotFound(id.to_string()))
    }
    async fn persist_result(&self, task_id: &Uuid, result: &StructuredResult) -> Result<()> {
        self.results
            .lock()
            .unwrap()
            .push((*task_id, result.clone()));
        Ok(())
    }
    async fn claim_atomic(&self, id: &Uuid) -> Result<Task> {
        self.load(id).await
    }
}

struct NoopTransport;

#[async_trait]
impl TransportPort for NoopTransport {
    async fn publish(&self, _m: &Message) -> Result<()> {
        Ok(())
    }
    async fn subscribe(&self, _o: &str) -> Result<Vec<Message>> {
        Ok(vec![])
    }
    async fn claim(&self, _o: &str, _id: &Uuid) -> Result<Message> {
        Err(SubstrateError::NotFound("noop".into()))
    }
    async fn mailbox(&self, owner: &str) -> Result<Mailbox> {
        Ok(Mailbox {
            owner: owner.into(),
            messages: vec![],
        })
    }
}

#[tokio::test]
async fn dispatch_persists_and_completes() {
    let store = Arc::new(MemStore::default());
    let svc = DispatchService::new(Arc::new(FakeEngine), store.clone(), Arc::new(NoopTransport));

    let task = Task::new("echo hi", "/tmp");
    let id = task.id;
    let result = svc.dispatch(task).await.unwrap();

    assert_eq!(result.status, TaskState::Completed);
    assert_eq!(result.text, "all done");

    let persisted = svc.get(&id).await.unwrap();
    assert_eq!(persisted.state, TaskState::Completed);
    assert_eq!(store.results.lock().unwrap().len(), 1);
}
