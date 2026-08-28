use anyhow::{Context, Result, anyhow};
use pumpkinpi_protocol::{DocumentCoverage, SourceDocument, SourceOfIntentBundle};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Component, Path, PathBuf},
};

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn normalize_relative(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(anyhow!("authoritative document escapes Project root"));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("authoritative document path must be relative"));
            }
        }
    }
    Ok(normalized)
}

fn markdown_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        let target = remaining[..end].trim();
        remaining = &remaining[end + 1..];
        let target = target.split('#').next().unwrap_or_default().trim();
        if target.to_ascii_lowercase().ends_with(".md")
            && !target.contains("://")
            && !target.starts_with('#')
        {
            links.push(target.to_string());
        }
    }
    links
}

fn discover_markdown(root: &Path, directory: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            discover_markdown(root, &entry.path(), out)?;
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            out.insert(entry.path().strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

pub(crate) fn import(root: &Path) -> Result<Option<SourceOfIntentBundle>> {
    let root = std::fs::canonicalize(root).context("Project root is unavailable")?;
    let manifest = ["design.md", "docs/design/README.md"]
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| root.join(candidate).is_file());
    let Some(manifest) = manifest else {
        return Ok(None);
    };

    let mut queue = VecDeque::from([manifest.clone()]);
    let mut documents = BTreeMap::<PathBuf, SourceDocument>::new();
    while let Some(relative) = queue.pop_front() {
        let relative = normalize_relative(&relative)?;
        if documents.contains_key(&relative) {
            continue;
        }
        let full = std::fs::canonicalize(root.join(&relative)).with_context(|| {
            format!(
                "authoritative document {} is unavailable",
                relative.display()
            )
        })?;
        if !full.starts_with(&root) || !full.is_file() {
            return Err(anyhow!(
                "authoritative document {} escapes Project root",
                relative.display()
            ));
        }
        let bytes = std::fs::read(&full)?;
        let content = String::from_utf8(bytes.clone()).with_context(|| {
            format!("authoritative document {} is not UTF-8", relative.display())
        })?;
        for target in markdown_links(&content) {
            let linked = relative
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(target);
            queue.push_back(normalize_relative(&linked)?);
        }
        let path = relative.to_string_lossy().replace('\\', "/");
        documents.insert(
            relative,
            SourceDocument {
                path,
                content_hash: digest(&bytes),
                byte_len: bytes.len() as u64,
                content,
            },
        );
    }

    // The design directory is a closed manifest domain: an unlinked Markdown file is an
    // activation error rather than silently omitted intent.
    let mut declared_design_docs = BTreeSet::new();
    discover_markdown(&root, &root.join("docs/design"), &mut declared_design_docs)?;
    let reached = documents.keys().cloned().collect::<BTreeSet<_>>();
    let unreferenced = declared_design_docs
        .difference(&reached)
        .collect::<Vec<_>>();
    if !unreferenced.is_empty() {
        return Err(anyhow!(
            "authoritative design manifest omits: {}",
            unreferenced
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let documents = documents.into_values().collect::<Vec<_>>();
    let mut bundle_digest = Sha256::new();
    for document in &documents {
        bundle_digest.update(document.path.as_bytes());
        bundle_digest.update([0]);
        bundle_digest.update(document.content_hash.as_bytes());
        bundle_digest.update([0]);
    }
    Ok(Some(SourceOfIntentBundle {
        manifest_path: manifest.to_string_lossy().replace('\\', "/"),
        bundle_hash: hex::encode(bundle_digest.finalize()),
        documents,
    }))
}

pub(crate) fn coverage(bundle: &SourceOfIntentBundle) -> Vec<DocumentCoverage> {
    bundle
        .documents
        .iter()
        .map(|document| DocumentCoverage {
            path: document.path.clone(),
            content_hash: document.content_hash.clone(),
        })
        .collect()
}

pub(crate) fn validate_coverage(
    bundle: Option<&SourceOfIntentBundle>,
    actual: &[DocumentCoverage],
) -> Result<()> {
    let expected = bundle.map(coverage).unwrap_or_default();
    let expected = expected
        .into_iter()
        .map(|item| (item.path, item.content_hash))
        .collect::<BTreeMap<_, _>>();
    let mut observed = BTreeMap::new();
    for item in actual {
        if observed
            .insert(item.path.clone(), item.content_hash.clone())
            .is_some()
        {
            return Err(anyhow!(
                "duplicate Source of Intent coverage for {}",
                item.path
            ));
        }
    }
    if observed != expected {
        let missing = expected
            .keys()
            .filter(|path| !observed.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = observed
            .keys()
            .filter(|path| !expected.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        return Err(anyhow!(
            "incomplete Source of Intent coverage; missing [{}], unexpected or changed [{}]",
            missing.join(", "),
            unexpected.join(", ")
        ));
    }
    Ok(())
}

pub(crate) fn verify_on_disk(root: &Path, expected: &SourceOfIntentBundle) -> Result<()> {
    let actual = import(root)?.context("authoritative Source of Intent manifest disappeared")?;
    if actual.bundle_hash != expected.bundle_hash {
        return Err(anyhow!(
            "authoritative source material changed: expected {}, observed {}",
            expected.bundle_hash,
            actual.bundle_hash
        ));
    }
    Ok(())
}

pub(crate) fn restore(root: &Path, bundle: &SourceOfIntentBundle) -> Result<()> {
    for document in &bundle.documents {
        let relative = normalize_relative(Path::new(&document.path))?;
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, document.content.as_bytes())?;
    }
    verify_on_disk(root, bundle)
}

pub(crate) fn source_hash(payload: &str, bundle: Option<&SourceOfIntentBundle>) -> String {
    let mut value = payload.as_bytes().to_vec();
    value.push(0);
    if let Some(bundle) = bundle {
        value.extend_from_slice(bundle.bundle_hash.as_bytes());
    }
    digest(&value)
}

pub(crate) fn manifest_for_prompt(bundle: Option<&SourceOfIntentBundle>) -> String {
    match bundle {
        Some(bundle) => {
            let documents = bundle
                .documents
                .iter()
                .map(|document| {
                    format!(
                        "- {} [{}] ({} bytes)",
                        document.path, document.content_hash, document.byte_len
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Manifest: {}\nBundle hash: {}\nDocuments:\n{}",
                bundle.manifest_path, bundle.bundle_hash, documents
            )
        }
        None => "No authoritative document bundle is attached.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_is_exact_content_addressed_and_closed_over_design_docs() {
        let root = std::env::temp_dir().join(format!("pumpkinpi-bundle-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("docs/design")).unwrap();
        std::fs::write(root.join("design.md"), "[Design](docs/design/README.md)\n").unwrap();
        std::fs::write(
            root.join("docs/design/README.md"),
            "[Behavior](behavior.md)\n",
        )
        .unwrap();
        std::fs::write(root.join("docs/design/behavior.md"), "Exact requirement.\n").unwrap();

        let bundle = import(&root).unwrap().unwrap();
        assert_eq!(bundle.documents.len(), 3);
        assert_eq!(bundle.documents[2].content, "Exact requirement.\n");
        validate_coverage(Some(&bundle), &coverage(&bundle)).unwrap();

        std::fs::write(root.join("docs/design/behavior.md"), "weakened\n").unwrap();
        assert!(verify_on_disk(&root, &bundle).is_err());
        restore(&root, &bundle).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("docs/design/behavior.md")).unwrap(),
            "Exact requirement.\n"
        );

        std::fs::write(root.join("docs/design/unlinked.md"), "lost intent\n").unwrap();
        assert!(import(&root).unwrap_err().to_string().contains("omits"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pumpkinpi_design_manifest_is_closed_and_imports_the_complete_corpus() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let bundle = import(root).unwrap().unwrap();
        let paths = bundle
            .documents
            .iter()
            .map(|document| document.path.as_str())
            .collect::<BTreeSet<_>>();
        assert!(paths.contains("design.md"));
        assert!(paths.contains("docs/design/README.md"));
        assert!(paths.contains("docs/design/16-intent-orchestration.md"));
        assert!(paths.contains("docs/design/IMPLEMENTATION-MIGRATION.md"));
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.starts_with("docs/design/"))
                .count(),
            std::fs::read_dir(root.join("docs/design"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|value| value == "md"))
                .count()
        );
        validate_coverage(Some(&bundle), &coverage(&bundle)).unwrap();
    }

    #[test]
    fn missing_coverage_is_rejected() {
        let bundle = SourceOfIntentBundle {
            manifest_path: "design.md".into(),
            bundle_hash: "bundle".into(),
            documents: vec![SourceDocument {
                path: "design.md".into(),
                content_hash: "hash".into(),
                byte_len: 1,
                content: "x".into(),
            }],
        };
        assert!(validate_coverage(Some(&bundle), &[]).is_err());
    }
}
