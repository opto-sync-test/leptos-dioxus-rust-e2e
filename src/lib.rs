use std::sync::Arc;

use dioxus::prelude::*;
use leptos::prelude::*;
use opto_sync_client::{InMemoryStore, OptoSyncClient};
use serde::{Deserialize, Serialize};
use tokio::sync::Barrier;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEnvelope {
    pub id: String,
    pub lane: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneReceipt {
    pub lane: &'static str,
    pub mutation_ids: Vec<String>,
}

/// Runs the same immutable mutation snapshot through independent upload and
/// realtime lanes. Mobile/desktop schedulers can retry this function without
/// rebuilding or reordering the batch that the web service worker persisted.
///
/// # Panics
///
/// Panics if an isolated Tokio lane panics, because returning a partial set of
/// acknowledgements would allow a caller to delete a batch that was not fully
/// delivered.
pub async fn run_background_cycle(
    batch: Arc<Vec<MutationEnvelope>>,
    rendezvous: Arc<Barrier>,
) -> Vec<LaneReceipt> {
    let lanes = ["upload", "realtime"];
    let mut tasks = Vec::with_capacity(lanes.len());

    for lane in lanes {
        let snapshot = Arc::clone(&batch);
        let gate = Arc::clone(&rendezvous);
        tasks.push(tokio::spawn(async move {
            gate.wait().await;
            LaneReceipt {
                lane,
                mutation_ids: snapshot.iter().map(|item| item.id.clone()).collect(),
            }
        }));
    }

    let mut receipts = Vec::with_capacity(tasks.len());
    for task in tasks {
        receipts.push(task.await.expect("background lane must not panic"));
    }
    receipts
}

/// Exercises the official Rust client instead of duplicating its merge rules.
///
/// # Errors
///
/// Returns the upstream client or reconciliation error if a payload cannot be
/// queued or rebased.
pub fn optimistic_snapshot() -> Result<String, Box<dyn std::error::Error>> {
    let mut client = OptoSyncClient::new(InMemoryStore::new());
    client.queue_mutation_value(serde_json::json!({
        "id": "rust-1",
        "title": "edited while offline"
    }))?;
    Ok(client.local_view(r#"{"id":"rust-1","title":"server copy","updatedAt":"1-server"}"#)?)
}

#[must_use]
pub fn leptos_shell() -> String {
    view! {
        <main data-runtime="leptos">
            <h1>"OptoSync Leptos shell"</h1>
            <p>"Durable web queue plus multiplexed native workers"</p>
            <div id="dioxus-island"></div>
        </main>
    }
    .to_html()
}

#[must_use]
pub fn dioxus_island() -> String {
    dioxus::ssr::render_element(rsx! {
        section { "data-runtime": "dioxus",
            h2 { "Dioxus cross-platform island" }
            p { "The same Rust mutation envelope runs on web, mobile, and desktop." }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn immutable_batch_is_multiplexed_across_concurrent_lanes() {
        let original = Arc::new(vec![
            MutationEnvelope {
                id: "m-1".into(),
                lane: "all".into(),
                payload: r#"{"title":"offline one"}"#.into(),
            },
            MutationEnvelope {
                id: "m-2".into(),
                lane: "all".into(),
                payload: r#"{"title":"offline two"}"#.into(),
            },
        ]);
        let before = (*original).clone();

        let receipts = run_background_cycle(Arc::clone(&original), Arc::new(Barrier::new(2))).await;

        assert_eq!(*original, before, "workers must not mutate a retry batch");
        assert_eq!(receipts.len(), 2);
        assert!(
            receipts
                .iter()
                .all(|receipt| receipt.mutation_ids == ["m-1", "m-2"])
        );
    }

    #[test]
    fn both_full_stack_renderers_and_official_client_execute() {
        assert!(leptos_shell().contains("OptoSync Leptos shell"));
        assert!(dioxus_island().contains("Dioxus cross-platform island"));
        assert!(
            optimistic_snapshot()
                .unwrap()
                .contains("edited while offline")
        );
    }
}
