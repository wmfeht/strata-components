//! The highlighter thread: its requests, and the loop that feeds them to the engine.
//!
//! One thread owns the [`Engine`], and so every `ParseState` (S4, B1 option (a)). Requests
//! arrive over one channel; replies go back over each document's own channel, and the thread
//! wakes the window with Rune's `Waker` once per step that sent any. Before each step the
//! worker drains the queue, so the newest viewport per document wins and queued edits share
//! one foreground pass.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use rune::Waker;
use syntect::highlighting::Theme;

use super::engine::{Engine, OpenArgs, Reply};
use super::{LineEdit, Options};

/// What the UI side asks the highlighter thread.
pub(crate) enum Request {
    Open {
        id: u64,
        seq: u64,
        args: OpenArgs,
        reply: Sender<Reply>,
    },
    Show {
        id: u64,
        seq: u64,
        lines: Range<usize>,
    },
    Edit {
        id: u64,
        seq: u64,
        generation: u64,
        text: Arc<str>,
        edit: LineEdit,
        visible: Range<usize>,
    },
    OpenPool {
        id: u64,
        reply: Sender<Reply>,
    },
    Block {
        pool: u64,
        key: u64,
        lang: Box<str>,
        code: Arc<str>,
    },
    /// Closes a document or a block pool.
    Close {
        id: u64,
    },
    SetTheme(Arc<Theme>),
}

/// The waker the thread calls after sending replies, settable after the thread starts.
pub(crate) type WakerSlot = Arc<Mutex<Option<Waker>>>;

/// The engine plus where its replies go.
pub(crate) struct Worker {
    pub(crate) engine: Engine,
    replies: HashMap<u64, Sender<Reply>>,
    waker: WakerSlot,
}

impl Worker {
    pub(crate) fn new(theme: Arc<Theme>, opts: Options, waker: WakerSlot) -> Worker {
        Worker {
            engine: Engine::new(theme, opts),
            replies: HashMap::new(),
            waker,
        }
    }

    pub(crate) fn handle(&mut self, req: Request) {
        let e = &mut self.engine;
        match req {
            Request::Open {
                id,
                seq,
                args,
                reply,
            } => {
                self.replies.insert(id, reply);
                e.open(id, seq, args);
            }
            Request::Show { id, seq, lines } => e.show(id, seq, lines),
            Request::Edit {
                id,
                seq,
                generation,
                text,
                edit,
                visible,
            } => e.edit(id, seq, generation, text, edit, visible),
            Request::OpenPool { id, reply } => {
                self.replies.insert(id, reply);
                e.open_pool(id);
            }
            Request::Block {
                pool,
                key,
                lang,
                code,
            } => e.block(pool, key, &lang, code),
            Request::Close { id } => {
                self.replies.remove(&id);
                e.close(id);
            }
            Request::SetTheme(theme) => e.set_theme(theme),
        }
    }

    /// Sends the engine's replies, and wakes the window if any went out. A document whose
    /// receiver is gone is closed.
    pub(crate) fn flush(&mut self) {
        let mut sent = false;
        for (id, reply) in self.engine.take_out() {
            let alive = self
                .replies
                .get(&id)
                .is_some_and(|tx| tx.send(reply).is_ok());
            if alive {
                sent = true;
            } else {
                self.replies.remove(&id);
                self.engine.close(id);
            }
        }
        if sent {
            let waker = self.waker.lock().ok().and_then(|w| w.clone());
            if let Some(wake) = waker {
                wake();
            }
        }
    }
}

/// The thread's loop. It blocks while there is no work, and exits when every sender (the
/// `Highlighter` handles, documents and block pools) is gone.
pub(crate) fn run(rx: Receiver<Request>, mut worker: Worker) {
    loop {
        let first = if worker.engine.has_work() {
            match rx.try_recv() {
                Ok(req) => Some(req),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(req) => Some(req),
                Err(_) => return,
            }
        };
        if let Some(req) = first {
            worker.handle(req);
            while let Ok(req) = rx.try_recv() {
                worker.handle(req);
            }
        }
        worker.engine.step();
        worker.flush();
    }
}
