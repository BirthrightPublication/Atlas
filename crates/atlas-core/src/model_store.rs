//! Explicit, pinned native downloads. No Python, shell or executable from a model URL.
use crate::{ai, now, string, Atlas};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static PROGRESS: Mutex<Option<Value>> = Mutex::new(None);
fn progress(phase: &str, received: u64, total: u64, message: &str) {
    if let Ok(mut p) = PROGRESS.lock() {
        *p = Some(json!({"phase":phase,"downloaded":received,"total":total,"message":message}));
    }
}
fn platform() -> String {
    format!(
        "{}-{}",
        if cfg!(target_os = "macos") {
            "darwin"
        } else {
            std::env::consts::OS
        },
        std::env::consts::ARCH
    )
}
pub fn catalog() -> Value {
    let mut value: Value =
        serde_json::from_str(include_str!("../resources/ai-catalog.json")).expect("pinned catalog");
    value["runtime"] = value["runtimes"][platform()].clone();
    value["licenses"] = json!({"model":"Apache-2.0", "runtime":"MIT", "runtimeSource":"https://github.com/ggml-org/llama.cpp/releases/tag/b11827"});
    value["requirements"] = json!({"0.6b":"639 MB download; allow at least 2 GB free RAM plus operating system headroom", "1.7b":"1.83 GB download; allow at least 4 GB free RAM plus operating system headroom", "note":"Memory estimates are capacity guidance, not measured inference performance."});
    value
}
fn root(atlas: &Atlas) -> PathBuf {
    atlas.data_dir().join("local-ai")
}
fn model(c: &Value, id: &str) -> Result<Value, String> {
    c["models"]
        .get(id)
        .cloned()
        .ok_or("Choose a supported model".into())
}
fn digest(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|_| "Installed file is missing")?;
    let mut h = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f
            .read(&mut b)
            .map_err(|_| "Could not verify installed file")?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn checked_file(path: &Path, size: u64, sha: &str) -> bool {
    fs::metadata(path).map(|m| m.len() == size).unwrap_or(false)
        && digest(path).is_ok_and(|s| s == sha)
}
pub fn cancel_download() {
    CANCEL.store(true, Ordering::Release);
}
async fn cancelled() {
    loop {
        if CANCEL.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
fn fetch(url: &str, path: &Path, size: u64, sha: &str, phase: &str) -> Result<(), String> {
    if checked_file(path, size, sha) {
        return Ok(());
    }
    let part = path.with_extension("part");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Download worker unavailable")?;
    let result=runtime.block_on(async {
        let client=reqwest::Client::builder().https_only(true).connect_timeout(std::time::Duration::from_secs(30)).read_timeout(std::time::Duration::from_secs(30)).timeout(std::time::Duration::from_secs(1800)).redirect(reqwest::redirect::Policy::custom(|a|if a.previous().len()>8||a.url().scheme()!="https"{a.error("Unsafe artifact redirect")}else{a.follow()})).build().map_err(|_|"Could not start verified download")?;
        let mut response=tokio::select! {response=client.get(url).send()=>response.map_err(|_|"Pinned download connection failed; check network access and retry")?,_=cancelled()=>return Err("Download cancelled".into())};
        if !response.status().is_success(){return Err(format!("Pinned artifact server returned HTTP {}. No unverified file was installed.",response.status().as_u16()))}
        let mut file=fs::File::create(&part).map_err(|_|"Could not create model download; check disk space")?;
        let mut digest=Sha256::new();let mut received=0;
        loop {let chunk=tokio::select! {chunk=response.chunk()=>chunk.map_err(|_|"Download interrupted; retry to restart safely")?,_=cancelled()=>return Err("Download cancelled".into())};
            let Some(chunk)=chunk else{break};received+=chunk.len() as u64;if received>size{return Err("Download exceeds the pinned size".into())}digest.update(&chunk);file.write_all(&chunk).map_err(|_|"Could not save model download")?;progress(phase,received,size,"");
        }
        if received!=size||format!("{:x}",digest.finalize())!=sha{return Err("Pinned size or SHA-256 verification failed".into())}
        file.sync_all().map_err(|_|"Could not flush model download")?;drop(file);fs::rename(&part,path).map_err(|_|"Could not install verified download")?;Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_file(part);
    }
    result
}
fn unpack(archive: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|_| "Could not replace owned runtime staging")?;
    }
    fs::create_dir_all(dest).map_err(|_| "Could not prepare runtime")?;
    let file = fs::File::open(archive).map_err(|_| "Runtime archive missing")?;
    let mut total = 0u64;
    if archive.extension().and_then(|x| x.to_str()) == Some("zip") {
        let mut zip = zip::ZipArchive::new(file).map_err(|_| "Invalid pinned runtime ZIP")?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|_| "Invalid runtime entry")?;
            if f.is_symlink() {
                return Err("Runtime symlinks are not accepted".into());
            }
            let rel = f.enclosed_name().ok_or("Unsafe runtime archive path")?;
            if rel
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
            {
                return Err("Unsafe runtime archive path".into());
            }
            total = total
                .checked_add(f.size())
                .ok_or("Runtime archive too large")?;
            if total > 512 * 1024 * 1024 {
                return Err("Runtime archive too large".into());
            }
            let out = dest.join(rel);
            if f.is_dir() {
                fs::create_dir_all(out).map_err(|_| "Runtime directory failed")?;
            } else {
                fs::create_dir_all(out.parent().ok_or("Unsafe runtime path")?)
                    .map_err(|_| "Runtime directory failed")?;
                let mut target = fs::File::create(&out).map_err(|_| "Runtime file failed")?;
                std::io::copy(&mut f, &mut target).map_err(|_| "Runtime extraction failed")?;
            }
        }
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        for e in tar.entries().map_err(|_| "Invalid runtime archive")? {
            let mut e = e.map_err(|_| "Invalid runtime entry")?;
            let kind = e.header().entry_type();
            if kind.is_symlink() {
                let target = e
                    .link_name()
                    .map_err(|_| "Invalid runtime link")?
                    .ok_or("Missing runtime link target")?;
                if target.components().any(|c| {
                    !matches!(
                        c,
                        std::path::Component::Normal(_) | std::path::Component::CurDir
                    )
                }) {
                    return Err("Unsafe runtime link".into());
                }
            } else if !(kind.is_file() || kind.is_dir()) {
                return Err("Runtime special files are not accepted".into());
            }
            total = total.checked_add(e.size()).ok_or("Runtime too large")?;
            if total > 512 * 1024 * 1024 {
                return Err("Runtime too large".into());
            }
            if !e.unpack_in(dest).map_err(|_| "Unsafe runtime archive")? {
                return Err("Unsafe runtime path".into());
            }
        }
    }
    Ok(())
}
fn files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in fs::read_dir(dir).map_err(|_| "Cannot inspect runtime")? {
        let e = e.map_err(|_| "Cannot inspect runtime")?;
        let t = e.file_type().map_err(|_| "Cannot inspect runtime")?;
        if t.is_symlink() {
            let target = e
                .path()
                .canonicalize()
                .map_err(|_| "Runtime link target missing")?;
            if !target.starts_with(
                dir.canonicalize()
                    .map_err(|_| "Invalid runtime directory")?,
            ) || !target.is_file()
            {
                return Err("Unsafe runtime link".into());
            }
            out.push(e.path());
            continue;
        }
        if t.is_dir() {
            files(&e.path(), out)?
        } else {
            out.push(e.path());
        }
    }
    Ok(())
}
fn verify(root: &Path, id: &str) -> Result<Value, String> {
    let c = catalog();
    let m = model(&c, id)?;
    let r = &c["runtime"];
    let inventory: Value = serde_json::from_slice(
        &fs::read(root.join(format!("{id}.json"))).map_err(|_| "Model is not installed")?,
    )
    .map_err(|_| "Invalid model inventory")?;
    let config = &inventory["ai"];
    let model_file = root.join(format!("{}.gguf", string(&m, "name")));
    if config["modelName"] != m["name"]
        || config["modelSha256"] != m["sha256"]
        || Path::new(string(config, "modelPath")) != model_file
        || !checked_file(
            &model_file,
            m["bytes"].as_u64().ok_or("Invalid pinned model")?,
            string(&m, "sha256"),
        )
    {
        return Err("Model does not match the authoritative pinned catalog".into());
    }
    let archive = root.join(string(r, "filename"));
    if !checked_file(
        &archive,
        r["bytes"].as_u64().ok_or("Unsupported runtime")?,
        string(r, "sha256"),
    ) {
        return Err("Runtime archive does not match the pinned upstream checksum".into());
    }
    let stage = root.join("runtime-verification");
    unpack(&archive, &stage)?;
    let checked: Result<(), String> = (|| {
        let mut expected = vec![];
        files(&stage, &mut expected)?;
        let runtime = root.join("llama-b11827");
        let executable_name = if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        };
        let mut server = None;
        for file in expected {
            let installed = runtime.join(
                file.strip_prefix(&stage)
                    .map_err(|_| "Invalid verification path")?,
            );
            if !installed
                .canonicalize()
                .map_err(|_| "Runtime file missing")?
                .starts_with(runtime.canonicalize().map_err(|_| "Runtime missing")?)
            {
                return Err("Runtime path escapes the managed cache".into());
            }
            if digest(&installed)? != digest(&file)? {
                return Err("Installed runtime differs from the pinned archive".into());
            }
            if installed.file_name().and_then(|s| s.to_str()) == Some(executable_name) {
                server = Some(installed);
            }
        }
        let server = server.ok_or("Runtime server missing")?;
        if Path::new(string(config, "runtimePath")) != server
            || string(config, "runtimeSha256") != digest(&server)?
        {
            return Err("Runtime configuration differs from verified upstream files".into());
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(stage);
    checked?;
    Ok(inventory)
}
pub fn query(atlas: &Arc<Mutex<Atlas>>, method: &str, p: &Value) -> Result<Value, String> {
    let c = catalog();
    if method == "ai.models" {
        return Ok(c);
    }
    if method == "ai.install_status" {
        return Ok(
            json!({"busy":BUSY.load(Ordering::Acquire),"progress":PROGRESS.lock().ok().and_then(|p|p.clone())}),
        );
    }
    if method == "ai.install_cancel" {
        CANCEL.store(true, Ordering::Release);
        return Ok(json!({"cancelling":true}));
    }
    if method == "ai.test" {
        let settings = atlas
            .lock()
            .map_err(|_| "Service unavailable")?
            .call("settings.get", json!({}))?;
        return ai::explain(
            "Explain this saved plan without inventing facts.",
            &json!({"instruction":"This is a local model connectivity test; no saved records supplied."}),
            &settings,
        );
    }
    let _inference_guard = ai::maintenance()?;
    let id = string(p, "model");
    let m = model(&c, id)?;
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("A model operation is already running".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    CANCEL.store(false, Ordering::Release);
    let root = {
        let a = atlas.lock().map_err(|_| "Service unavailable")?;
        root(&a)
    };
    fs::create_dir_all(&root).map_err(|_| "Cannot create private model cache")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect model cache")?;
    }
    if method == "ai.remove" {
        if p["confirm"] != true {
            return Err("Confirm removal of this model".into());
        }
        ai::cancel();
        let mut a = atlas.lock().map_err(|_| "Service unavailable")?;
        let s = a.call("settings.get", json!({}))?;
        if s["ai"]["modelName"] == m["name"] {
            a.call("settings.save", json!({"ai":{"enabled":false}}))?;
        }
        let _ = fs::remove_file(root.join(format!("{}.gguf", string(&m, "name"))));
        let _ = fs::remove_file(root.join(format!("{id}.json")));
        return Ok(json!({"removed":true,"runtimeRetainedForOtherModels":true}));
    }
    if method == "ai.install" {
        if p["approveDownload"] != true {
            return Err("Approve the displayed model and runtime download sizes first".into());
        }
        let r = &c["runtime"];
        if r.is_null() {
            return Err("No verified runtime for this platform".into());
        }
        let result = (|| {
            let model_file = root.join(format!("{}.gguf", string(&m, "name")));
            fetch(
                &format!(
                    "https://huggingface.co/Qwen/{}/resolve/{}/{}.gguf?download=true",
                    string(&m, "repo"),
                    string(&m, "revision"),
                    string(&m, "name")
                ),
                &model_file,
                m["bytes"].as_u64().ok_or("Invalid catalog")?,
                string(&m, "sha256"),
                "Downloading model",
            )?;
            let archive = root.join(string(r, "filename"));
            fetch(
                &format!(
                    "https://github.com/ggml-org/llama.cpp/releases/download/b11827/{}",
                    string(r, "filename")
                ),
                &archive,
                r["bytes"].as_u64().ok_or("Invalid runtime catalog")?,
                string(r, "sha256"),
                "Downloading runtime",
            )?;
            ai::cancel();
            let runtime = root.join("llama-b11827");
            unpack(&archive, &runtime)?;
            let mut paths = vec![];
            files(&runtime, &mut paths)?;
            let expected = if cfg!(windows) {
                "llama-server.exe"
            } else {
                "llama-server"
            };
            let executables: Vec<_> = paths
                .iter()
                .filter(|p| p.file_name().and_then(|v| v.to_str()) == Some(expected))
                .collect();
            if executables.len() != 1 {
                return Err("Verified archive must contain one llama-server".into());
            }
            let executable = executables[0];
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(executable, fs::Permissions::from_mode(0o700))
                    .map_err(|_| "Could not set runtime permissions")?;
            }
            let entries:Vec<_>=paths.iter().map(|p|Ok(json!({"path":p.strip_prefix(&root).map_err(|_|"Invalid runtime path")?.to_string_lossy(),"sha256":digest(p)?}))).collect::<Result<_,String>>()?;
            let config = json!({"enabled":false,"modelName":m["name"],"runtimePath":executable,"runtimeSha256":digest(executable)?,"modelPath":model_file,"modelSha256":m["sha256"],"modelSource":format!("https://huggingface.co/Qwen/{}/tree/{}",string(&m,"repo"),string(&m,"revision")),"modelLicense":"Apache-2.0"});
            let inventory = json!({"ai":config,"runtimeFiles":entries,"installedAt":now()});
            fs::write(
                root.join(format!("{id}.json")),
                serde_json::to_vec_pretty(&inventory).map_err(|_| "Inventory failed")?,
            )
            .map_err(|_| "Could not save model inventory")?;
            Ok(json!({"installed":true,"enabled":false,"model":m["name"]}))
        })();
        progress(
            if result.is_ok() {
                "Installed"
            } else {
                "Failed"
            },
            0,
            0,
            result
                .as_ref()
                .err()
                .map(String::as_str)
                .unwrap_or("Model installed. Verify and enable it when ready."),
        );
        return result;
    }
    let installed = verify(&root, id)?;
    if method == "ai.enable" {
        let mut config = installed["ai"].clone();
        config["enabled"] = json!(true);
        atlas
            .lock()
            .map_err(|_| "Service unavailable")?
            .call("settings.save", json!({"ai":config}))?;
        return Ok(json!({"enabled":true,"verified":true}));
    }
    if method == "ai.verify" {
        return Ok(json!({"verified":true,"model":m["name"],"checkedAt":now()}));
    }
    Err("Unknown model operation".into())
}

pub fn chat(atlas: &Arc<Mutex<Atlas>>, p: &Value) -> Result<Value, String> {
    if string(p, "text")
        .trim()
        .to_lowercase()
        .starts_with("remind me:")
    {
        return Err("Use Atlas Planner for an explicit reminder request, or review journey proposals before saving.".into());
    }
    let (planner, settings) = {
        let mut a = atlas.lock().map_err(|_| "Service unavailable")?;
        (
            a.call(
                "assistant.ask",
                json!({"text":p["text"],"context":p["context"]}),
            )?,
            a.call("settings.get", json!({}))?,
        )
    };
    let explanation = ai::explain(string(p, "text"), &planner, &settings)?;
    let mut a = atlas.lock().map_err(|_| "Service unavailable")?;
    // Persist through the existing idempotent conversation path; generated prose
    // is never dispatched as an operation or used as record fields.
    a.save_local_chat(p, &explanation, &planner)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_archive_extraction_rejects_parent_paths() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bad.zip");
        let mut z = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        z.start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"synthetic").unwrap();
        z.finish().unwrap();
        assert!(unpack(&archive, &dir.path().join("out")).is_err());
        assert!(!dir.path().join("escape").exists());
    }
    #[test]
    fn managed_inventory_cannot_replace_pinned_catalog_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("fake.gguf");
        fs::write(&file, b"synthetic").unwrap();
        fs::write(
            dir.path().join("0.6b.json"),
            json!({"ai":{"modelPath":file,"modelSha256":digest(&file).unwrap()},"runtimeFiles":[]})
                .to_string(),
        )
        .unwrap();
        assert!(verify(dir.path(), "0.6b").is_err());
    }
}
