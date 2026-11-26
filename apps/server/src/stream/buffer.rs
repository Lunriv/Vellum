use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{Mutex, Notify};

use crate::kernel::PendingInterrupt;
use crate::persistence::event_repo::DirectiveDto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedEvent {
    pub seq: u64,
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct BufferLimits {
    pub max_events: usize,
    pub max_buffer_bytes: usize,
    pub max_narration_bytes: usize,
    pub ttl: Duration,
}

impl Default for BufferLimits {
    fn default() -> Self {
        Self {
            max_events: 2048,
            max_buffer_bytes: 512 * 1024,
            max_narration_bytes: 256 * 1024,
            ttl: Duration::from_secs(300),
        }
    }
}

pub struct ActiveTurnBufferState {
    pub turn_id: String,
    pub generation_id: i64,
    pub status: String,
    pub events: VecDeque<BufferedEvent>,
    pub total_event_bytes: usize,
    pub next_seq: u64,
    pub narration: String,
    pub pending_interrupt: Option<PendingInterrupt>,
    pub directives: Vec<DirectiveDto>,
    pub terminal: bool,
    pub terminal_at: Option<Instant>,
}

pub struct ActiveTurnBuffer {
    pub state: Mutex<ActiveTurnBufferState>,
    pub changed: Notify,
    limits: BufferLimits,
}

#[derive(Debug, Clone)]
pub enum SubscribeStep {
    Events {
        events: Vec<BufferedEvent>,
        terminal: bool,
    },
    StreamReset {
        seq: u64,
        payload: Value,
        terminal: bool,
    },
}

impl ActiveTurnBuffer {
    pub fn new(turn_id: impl Into<String>, limits: BufferLimits) -> Self {
        Self {
            state: Mutex::new(ActiveTurnBufferState {
                turn_id: turn_id.into(),
                generation_id: 1,
                status: "received".into(),
                events: VecDeque::new(),
                total_event_bytes: 0,
                next_seq: 1,
                narration: String::new(),
                pending_interrupt: None,
                directives: Vec::new(),
                terminal: false,
                terminal_at: None,
            }),
            changed: Notify::new(),
            limits,
        }
    }

    pub fn with_defaults(turn_id: impl Into<String>) -> Self {
        Self::new(turn_id, BufferLimits::default())
    }

    pub async fn push(&self, event_type: impl Into<String>, payload: Value) -> u64 {
        let mut s = self.state.lock().await;
        let seq = s.next_seq;
        s.next_seq += 1;
        let ev = BufferedEvent {
            seq,
            event_type: event_type.into(),
            payload,
        };
        let ev_bytes = serde_json::to_vec(&ev).map(|v| v.len()).unwrap_or(64);
        s.total_event_bytes += ev_bytes;
        s.events.push_back(ev);
        while s.events.len() > self.limits.max_events
            || s.total_event_bytes > self.limits.max_buffer_bytes
        {
            if let Some(evicted) = s.events.pop_front() {
                let evicted_bytes = serde_json::to_vec(&evicted).map(|v| v.len()).unwrap_or(64);
                s.total_event_bytes = s.total_event_bytes.saturating_sub(evicted_bytes);
            } else {
                break;
            }
        }
        drop(s);
        self.changed.notify_waiters();
        seq
    }

    pub async fn set_status(&self, status: impl Into<String>) {
        let mut s = self.state.lock().await;
        s.status = status.into();
    }

    pub async fn set_pending_interrupt(&self, interrupt: Option<PendingInterrupt>) {
        let mut s = self.state.lock().await;
        s.pending_interrupt = interrupt;
    }

    pub async fn record_directive(&self, directive: DirectiveDto) {
        let mut s = self.state.lock().await;
        if let Some(existing) = s
            .directives
            .iter_mut()
            .find(|d| d.directive_id == directive.directive_id)
        {
            *existing = directive;
        } else {
            s.directives.push(directive);
        }
    }

    pub async fn reset_narration_for_generation(&self, generation_id: i64) {
        let mut s = self.state.lock().await;
        s.generation_id = generation_id;
        s.narration.clear();
        s.status = "recovering".into();
        s.terminal = false;
        s.terminal_at = None;
        for d in &mut s.directives {
            if d.source == "kernel_owned" && !d.displayed {
                d.narration_generation = generation_id;
            }
        }
        drop(s);
        self.changed.notify_waiters();
    }

    pub async fn reopen(&self, status: impl Into<String>) {
        let mut s = self.state.lock().await;
        s.terminal = false;
        s.terminal_at = None;
        s.status = status.into();
        drop(s);
        self.changed.notify_waiters();
    }

    pub async fn append_narration(&self, chunk: &str) {
        let mut s = self.state.lock().await;
        if s.narration.len() + chunk.len() <= self.limits.max_narration_bytes {
            s.narration.push_str(chunk);
        }
    }

    pub async fn complete(&self, final_status: impl Into<String>) {
        let mut s = self.state.lock().await;
        s.status = final_status.into();
        s.terminal = true;
        s.terminal_at = Some(Instant::now());
        drop(s);
        self.changed.notify_waiters();
    }

    /// Zero-lost-wakeup pull step: registers `Notified` BEFORE acquiring the Mutex.
    pub async fn next_batch(&self, cursor: u64) -> SubscribeStep {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            {
                let s = self.state.lock().await;
                let first_seq = s.events.front().map(|e| e.seq).unwrap_or(s.next_seq);
                if cursor.saturating_add(1) < first_seq {
                    let last_seq = s.next_seq.saturating_sub(1);
                    let payload = json!({
                        "type": "stream_reset",
                        "turnId": s.turn_id,
                        "snapshot": {
                            "turnId": s.turn_id,
                            "generationId": s.generation_id,
                            "status": s.status,
                            "lastSeq": last_seq,
                            "narrationSnapshot": s.narration,
                            "pendingInterrupt": s.pending_interrupt,
                            "directives": s.directives,
                        }
                    });
                    return SubscribeStep::StreamReset {
                        seq: last_seq,
                        payload,
                        terminal: s.terminal,
                    };
                }

                let events: Vec<BufferedEvent> = s
                    .events
                    .iter()
                    .filter(|e| e.seq > cursor)
                    .cloned()
                    .collect();
                if !events.is_empty() || s.terminal {
                    return SubscribeStep::Events {
                        events,
                        terminal: s.terminal,
                    };
                }
            }

            notified.await;
        }
    }

    pub async fn is_expired(&self) -> bool {
        let s = self.state.lock().await;
        s.terminal
            && s.terminal_at
                .map(|at| at.elapsed() >= self.limits.ttl)
                .unwrap_or(false)
    }
}

pub struct BufferRegistry {
    buffers: std::sync::Mutex<HashMap<String, Arc<ActiveTurnBuffer>>>,
    default_limits: BufferLimits,
}

impl Default for BufferRegistry {
    fn default() -> Self {
        Self::with_limits(BufferLimits::default())
    }
}

impl BufferRegistry {
    pub fn with_limits(limits: BufferLimits) -> Self {
        Self {
            buffers: std::sync::Mutex::new(HashMap::new()),
            default_limits: limits,
        }
    }

    pub fn get(&self, turn_id: &str) -> Option<Arc<ActiveTurnBuffer>> {
        self.buffers.lock().unwrap().get(turn_id).cloned()
    }

    pub fn get_or_create(&self, turn_id: &str) -> Arc<ActiveTurnBuffer> {
        let mut g = self.buffers.lock().unwrap();
        g.entry(turn_id.to_string())
            .or_insert_with(|| {
                Arc::new(ActiveTurnBuffer::new(turn_id, self.default_limits.clone()))
            })
            .clone()
    }

    pub fn insert(&self, turn_id: &str, buffer: Arc<ActiveTurnBuffer>) {
        self.buffers
            .lock()
            .unwrap()
            .insert(turn_id.to_string(), buffer);
    }

    pub async fn cleanup_expired(&self) {
        let candidates: Vec<(String, Arc<ActiveTurnBuffer>)> = {
            let g = self.buffers.lock().unwrap();
            g.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };
        let mut to_remove = Vec::new();
        for (id, buf) in candidates {
            if buf.is_expired().await {
                to_remove.push(id);
            }
        }
        if !to_remove.is_empty() {
            let mut g = self.buffers.lock().unwrap();
            for id in to_remove {
                g.remove(&id);
            }
        }
    }
}
