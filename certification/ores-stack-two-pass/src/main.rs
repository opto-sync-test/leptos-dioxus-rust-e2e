use std::{env, fs, path::PathBuf, process};

use ores_api_docs::{
    materialize_finalized_page_build, read_page_build_manifest, write_page_build_manifest,
    write_page_build_outputs,
};

fn main() {
    let root = unique_temp();
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }

    write_browser_page(&root, "leptos", "leptos", "ssr_hydrate");
    write_browser_page(&root, "dioxus", "dioxus", "ssr_hydrate");

    let first_pass = root.join(".ores-stack/first-pass");
    let outputs = write_page_build_outputs(&root, &first_pass).expect("first pass");
    let mut manifest = read_page_build_manifest(&outputs.manifest_path).expect("manifest");
    assert_eq!(manifest.routes.len(), 2);

    for route in &manifest.routes {
        let wasm = route.wasm.as_ref().expect("browser route must have wasm plan");
        assert_eq!(wasm.source_sha256.len(), 64);
        assert!(wasm.final_wasm_sha256.is_none());
        assert!(wasm.wasm_output_file.is_none());
        assert!(wasm.public_path.is_none());
        assert!(wasm.js_sha256.is_none());
        assert!(wasm.js_output_file.is_none());
        assert!(wasm.js_public_path.is_none());
    }
    assert_eq!(manifest.routes[0].canonical_path, "/dioxus/{id}");
    assert_eq!(manifest.routes[0].renderer, "dioxus");
    assert_eq!(manifest.routes[1].canonical_path, "/leptos/{id}");
    assert_eq!(manifest.routes[1].renderer, "leptos");

    let first_error = materialize_finalized_page_build(
        &root,
        &root.join(".ores-stack/materialized-too-early"),
        &outputs.manifest_path,
        &outputs.manifest_path.parent().unwrap().join("page-assets"),
    )
    .expect_err("native server materialization must reject first-pass browser plans");
    assert!(first_error.to_string().contains("unfinalized WASM output"));

    let final_assets = root.join(".ores-stack/final-assets");
    fs::create_dir_all(&final_assets).unwrap();
    for (index, route) in manifest.routes.iter_mut().enumerate() {
        let wasm = route.wasm.as_mut().unwrap();
        let wasm_sha = if index == 0 { "a".repeat(64) } else { "b".repeat(64) };
        let js_sha = if index == 0 { "c".repeat(64) } else { "d".repeat(64) };
        let wasm_file = format!("page-{wasm_sha}.wasm");
        let js_file = format!("page-{js_sha}.js");
        fs::write(final_assets.join(&wasm_file), format!("wasm:{}", route.renderer)).unwrap();
        fs::write(final_assets.join(&js_file), format!("js:{}", route.renderer)).unwrap();
        wasm.final_wasm_sha256 = Some(wasm_sha);
        wasm.wasm_output_file = Some(wasm_file.clone());
        wasm.public_path = Some(format!("/__ores/assets/{wasm_file}"));
        wasm.js_sha256 = Some(js_sha);
        wasm.js_output_file = Some(js_file.clone());
        wasm.js_public_path = Some(format!("/__ores/assets/{js_file}"));
    }

    let final_manifest = root.join(".ores-stack/final/ores-page-manifest.json");
    fs::create_dir_all(final_manifest.parent().unwrap()).unwrap();
    write_page_build_manifest(&final_manifest, &manifest).unwrap();
    let materialized = materialize_finalized_page_build(
        &root,
        &root.join(".ores-stack/materialized"),
        &final_manifest,
        &final_assets,
    )
    .expect("fully finalized browser plan must materialize");

    let copied_assets = sorted_file_names(&materialized.compile_glue_path.parent().unwrap().join("page-assets"));
    assert_eq!(copied_assets.len(), 4);
    assert!(copied_assets.iter().all(|name| name.starts_with("page-")));
    assert!(materialized.compile_glue_path.is_file());
    assert!(materialized.manifest_path.is_file());
    assert_eq!(materialized.rerun_if_changed.len(), 2);
    assert!(materialized.rerun_if_changed.contains(&final_manifest.canonicalize().unwrap()));
    assert!(materialized.rerun_if_changed.contains(&final_assets.canonicalize().unwrap()));

    fs::remove_dir_all(&root).unwrap();
    println!("opto-sync-test leptos+dioxus ores-stack two-pass smoke passed");
}

fn write_browser_page(root: &std::path::Path, segment: &str, renderer: &str, delivery: &str) {
    let dir = root.join("src/pages").join(segment).join("[id]");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("page.rs"),
        format!(
            "#[ores_page(renderer = \"{renderer}\", delivery = \"{delivery}\", render = \"dynamic\", client = \"client.rs\", data_sources(\"rpc:GetItem\"))]\npub async fn page() {{}}\n"
        ),
    )
    .unwrap();
    fs::write(dir.join("client.rs"), format!("pub fn hydrate_{segment}() {{}}\n")).unwrap();
}

fn sorted_file_names(dir: &std::path::Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn unique_temp() -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!("opto-ores-stack-two-pass-{}-{now}", process::id()))
}
