use ores_api_docs_client::{PageContext, PageDocument, PageResult};
use ores_api_docs_macros::ores_page;

#[ores_page(
    renderer = "leptos",
    delivery = "ssr_only",
    render = "dynamic",
    title = "Opto sync smoke",
    summary = "Exercises Leptos route discovery and RPC admission through ores-stack",
    data_sources("rpc:GetSyncStatus"),
    tags("opto-sync", "ores-stack-smoke")
)]
pub async fn page(_ctx: PageContext) -> PageResult {
    Ok(PageDocument::html("<main>opto sync ores-stack smoke</main>"))
}
