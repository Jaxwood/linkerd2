#![deny(warnings, rust_2018_idioms)]
#![forbid(unsafe_code)]

pub mod labels;
pub mod policy;
// mod watch;

pub use self::{labels::Labels, watcher::Event};
use futures::prelude::*;
pub use k8s_openapi::api::{
    self,
    core::v1::{Namespace, Node, NodeSpec, Pod, PodSpec, PodStatus},
};
pub use kube::api::{ObjectMeta, ResourceExt};
use kube::{
    api::{Api, ListParams},
    runtime::watcher,
};
use parking_lot::Mutex;
use std::sync::Arc;

// pub use kube::runtime::reflector::Store;

pub type EventResult<T> = watcher::Result<watcher::Event<T>>;

/// We only need to watch pods that are injected with a Linkerd sidecar because these are the only
/// pods that can have inbound policy. We avoid indexing information about uninjected pods.
pub fn watch_all_injected_pods(client: kube::Client) -> impl Stream<Item = EventResult<Pod>> {
    let params = ListParams::default().labels("linkerd.io/control-plane-ns");
    watcher(Api::all(client), params)
}

pub fn watch_all_servers(client: kube::Client) -> impl Stream<Item = EventResult<policy::Server>> {
    watcher(Api::all(client), ListParams::default())
}

pub fn watch_all_serverauthorizations(
    client: kube::Client,
) -> impl Stream<Item = EventResult<policy::ServerAuthorization>> {
    watcher(Api::all(client), ListParams::default())
}

/* TODO
pub fn cached<T, S>(
    events: S,
) -> (
    Store<T>,
    impl Stream<Item = watcher::Result<watcher::Event<T>>>,
)
where
    T: kube::Resource + Clone + 'static,
    T::DynamicType: std::hash::Hash + Eq + Clone + Default,
    S: Stream<Item = EventResult<T>>,
{
    let writer = kube::runtime::reflector::store::Writer::<T>::default();
    let store = writer.as_reader();
    let events = kube::runtime::reflector::reflector(writer, events);
    (store, events)
}
 */

/// Processes an `E`-typed event stream of `T`-typed resources.
///
/// The `F`-typed processor is called for each event with exclusive mutable accecss to an `S`-typed
/// store.
///
/// The `H`-typed initialization handle is dropped after the first event is processed to signal to
/// the application that the index has been updated.
///
/// It is assumed that the event stream is infinite. If an error is encountered, the stream is
/// immediately polled again; and if this attempt to read from the stream fails, a backoff is
/// employed before attempting to read from the stream again.
pub async fn index<T, E, H, S, F>(
    events: E,
    initialized: H,
    backoff: std::time::Duration,
    store: Arc<Mutex<S>>,
    process: F,
) where
    E: Stream<Item = EventResult<T>>,
    F: Fn(&mut S, watcher::Event<T>),
{
    tokio::pin!(events);

    // A handle to be dropped when the index has been initialized.
    let mut initialized = Some(initialized);

    let mut failed = false;
    while let Some(ev) = events.next().await {
        match ev {
            Ok(ev) => {
                process(&mut *store.lock(), ev);

                // Drop the initialization handle if it's set.
                drop(initialized.take());
                failed = false;
            }

            Err(error) => {
                tracing::info!(%error, "stream failed");
                // If the stream had previously failed, backoff before polling the stream again.
                if failed {
                    // TODO: Use an exponential backoff.
                    tracing::debug!(?backoff);
                    tokio::time::sleep(backoff).await;
                }
                failed = true;
            }
        }
    }

    tracing::warn!("k8s event stream terminated");
}
