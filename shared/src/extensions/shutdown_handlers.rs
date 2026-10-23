use crate::State;
use futures_util::FutureExt;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::{OwnedRwLockReadGuard, RwLock};

pub type ShutdownFunc =
    dyn Fn(State) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync;

pub struct ShutdownHandler {
    pub name: &'static str,
    pub task: Box<ShutdownFunc>,
}

pub struct ShutdownHandlerBuilder {
    state: State,
    tasks: Arc<RwLock<HashMap<&'static str, ShutdownHandler>>>,
}

impl ShutdownHandlerBuilder {
    pub fn new(state: State) -> Self {
        Self {
            state,
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn share(&self) -> Self {
        Self {
            state: self.state.clone(),
            tasks: Arc::clone(&self.tasks),
        }
    }

    pub(crate) async fn handler_names(&self) -> HashSet<&'static str> {
        self.tasks.read().await.keys().copied().collect()
    }

    /// Removes every handler not in `keep`.
    pub(crate) async fn rollback(&self, keep: &HashSet<&'static str>) {
        self.tasks
            .write()
            .await
            .retain(|name, _| keep.contains(name));
    }

    /// Adds a shutdown handler that will be executed when a gradual shutdown is initiated.
    /// This will run on primary and on non-primary instances, so be aware of that when implementing your handler and
    /// perhaps check `state.env.app_primary`.
    pub async fn add_handler<
        F: Fn(State) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), anyhow::Error>> + Send + 'static,
    >(
        &self,
        name: &'static str,
        shutdown_fn: F,
    ) {
        let state = self.state.clone();
        let tasks = Arc::clone(&self.tasks);

        let previous = self.tasks.write().await.insert(
            name,
            ShutdownHandler {
                name,
                task: Box::new(move |state: State| Box::pin(shutdown_fn(state))),
            },
        );

        if previous.is_some() {
            tracing::warn!(
                name,
                "shutdown handler registered twice, replacing the previous one"
            );
        }
    }
}

#[derive(Default)]
pub struct ShutdownHandlerManager {
    builder: RwLock<Option<ShutdownHandlerBuilder>>,
}

impl ShutdownHandlerManager {
    pub async fn merge_builder(&self, builder: ShutdownHandlerBuilder) {
        self.builder.write().await.replace(builder);
    }

    pub async fn get_handlers(
        &self,
    ) -> OwnedRwLockReadGuard<HashMap<&'static str, ShutdownHandler>> {
        let inner = self.builder.read().await;

        inner.as_ref().unwrap().tasks.clone().read_owned().await
    }

    pub async fn handle_shutdown(&self) {
        let handlers = self.get_handlers().await;

        for (name, handler) in handlers.iter() {
            tracing::info!(name, "running shutdown task");
            let result = AssertUnwindSafe((handler.task)(
                self.builder.read().await.as_ref().unwrap().state.clone(),
            ))
            .catch_unwind()
            .await;

            match result {
                Ok(result) => {
                    if let Err(err) = result {
                        tracing::error!(name, %err, "shutdown task failed");
                        sentry_anyhow::capture_anyhow(&err);
                    }
                }
                Err(err) => {
                    let err_msg: Cow<'_, str> = if let Some(s) = err.downcast_ref::<&str>() {
                        (*s).into()
                    } else if let Some(s) = err.downcast_ref::<String>() {
                        s.as_str().into()
                    } else {
                        "unknown panic".into()
                    };

                    tracing::error!(name, %err_msg, "shutdown task panicked");
                    sentry::capture_message(
                        &format!("Shutdown handler '{}' panicked: {}", name, err_msg),
                        sentry::Level::Error,
                    );
                }
            }
        }
    }
}
