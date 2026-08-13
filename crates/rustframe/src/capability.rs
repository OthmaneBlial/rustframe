use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Result, RuntimeError};

const DEFAULT_SHELL_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SHELL_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default)]
pub struct FsCapability {
    roots: Vec<PathBuf>,
    root_ids: Vec<String>,
    grants: Arc<Mutex<BTreeMap<String, StoredFsGrant>>>,
    next_grant_id: Arc<AtomicU64>,
    persistence_path: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FsGrantAccess {
    Read,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FsGrantKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGrant {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub access: FsGrantAccess,
    pub kind: FsGrantKind,
    pub persistent: bool,
}

#[derive(Clone, Debug)]
struct StoredFsGrant {
    grant: FsGrant,
    path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedFsGrant {
    id: String,
    path: PathBuf,
    access: FsGrantAccess,
    kind: FsGrantKind,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWalkOptions {
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub extensions: Vec<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub uri: String,
    pub path: String,
    pub name: String,
    pub parent: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
    pub extension: Option<String>,
    pub modified_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsBinaryContents {
    #[serde(flatten)]
    pub file: FsEntry,
    pub byte_length: usize,
    pub base64: String,
}

#[derive(Clone, Debug)]
struct ResolvedFsPath {
    root: PathBuf,
    absolute: PathBuf,
    relative: PathBuf,
    uri_prefix: Option<String>,
}

impl FsCapability {
    pub fn new<I, P>(roots: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut resolved_roots = Vec::new();
        let mut root_ids = Vec::new();
        let mut seen_root_ids = BTreeSet::new();
        for root in roots {
            let path = root.into();
            let canonical = path.canonicalize().map_err(|error| {
                RuntimeError::InvalidConfiguration(format!(
                    "fs root '{}' is invalid: {error}",
                    path.display()
                ))
            })?;

            if !canonical.is_dir() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "fs root '{}' is not a directory",
                    canonical.display()
                )));
            }

            let id = filesystem_root_id(&canonical, resolved_roots.len());
            if !seen_root_ids.insert(id.clone()) {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "filesystem roots resolve to duplicate id '{id}'"
                )));
            }
            resolved_roots.push(canonical);
            root_ids.push(id);
        }

        Ok(Self {
            roots: resolved_roots,
            root_ids,
            grants: Arc::new(Mutex::new(BTreeMap::new())),
            next_grant_id: Arc::new(AtomicU64::new(1)),
            persistence_path: Arc::new(Mutex::new(None)),
        })
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn root_uris(&self) -> Vec<String> {
        self.root_ids
            .iter()
            .map(|id| format!("root://{id}"))
            .collect()
    }

    pub fn with_persistence(self, path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Ok(source) = fs::read_to_string(&path) {
            let persisted: Vec<PersistedFsGrant> = serde_json::from_str(&source)?;
            let mut grants = self.grants.lock().map_err(|_| {
                RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
            })?;
            let mut max_id = 0;
            for entry in persisted {
                let Ok(canonical) = entry.path.canonicalize() else {
                    continue;
                };
                let numeric_id = entry
                    .id
                    .strip_prefix("grant-")
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(0);
                max_id = max_id.max(numeric_id);
                let name = canonical
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Selected folder".into());
                grants.insert(
                    entry.id.clone(),
                    StoredFsGrant {
                        grant: FsGrant {
                            uri: format!("grant://{}", entry.id),
                            id: entry.id,
                            name,
                            access: entry.access,
                            kind: entry.kind,
                            persistent: true,
                        },
                        path: canonical,
                    },
                );
            }
            self.next_grant_id.store(max_id + 1, Ordering::Relaxed);
        }
        *self.persistence_path.lock().map_err(|_| {
            RuntimeError::InvalidConfiguration("filesystem grant persistence is poisoned".into())
        })? = Some(path);
        Ok(self)
    }

    pub fn grant_path(
        &self,
        path: impl Into<PathBuf>,
        access: FsGrantAccess,
        persistent: bool,
    ) -> Result<FsGrant> {
        let path = path.into();
        let canonical = path.canonicalize().map_err(|error| {
            RuntimeError::InvalidParameter(format!(
                "selected path '{}' is unavailable: {error}",
                path.display()
            ))
        })?;
        let metadata = fs::metadata(&canonical)?;
        let kind = if metadata.is_dir() {
            FsGrantKind::Directory
        } else if metadata.is_file() {
            FsGrantKind::File
        } else {
            return Err(RuntimeError::InvalidParameter(
                "only files and directories can receive filesystem grants".into(),
            ));
        };
        let id = format!(
            "grant-{}",
            self.next_grant_id.fetch_add(1, Ordering::Relaxed)
        );
        let grant = FsGrant {
            id: id.clone(),
            uri: format!("grant://{id}"),
            name: canonical
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "Selected folder".into()),
            access,
            kind,
            persistent,
        };
        self.grants
            .lock()
            .map_err(|_| {
                RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
            })?
            .insert(
                id,
                StoredFsGrant {
                    grant: grant.clone(),
                    path: canonical,
                },
            );
        if persistent {
            self.save_persistent_grants()?;
        }
        Ok(grant)
    }

    pub fn grants(&self) -> Result<Vec<FsGrant>> {
        Ok(self
            .grants
            .lock()
            .map_err(|_| {
                RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
            })?
            .values()
            .map(|stored| stored.grant.clone())
            .collect())
    }

    pub fn revoke_grant(&self, id: &str) -> Result<bool> {
        let removed = self
            .grants
            .lock()
            .map_err(|_| {
                RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
            })?
            .remove(id)
            .is_some();
        if removed {
            self.save_persistent_grants()?;
        }
        Ok(removed)
    }

    fn save_persistent_grants(&self) -> Result<()> {
        let path = self
            .persistence_path
            .lock()
            .map_err(|_| {
                RuntimeError::InvalidConfiguration(
                    "filesystem grant persistence is poisoned".into(),
                )
            })?
            .clone();
        let Some(path) = path else {
            return Ok(());
        };
        let grants = self.grants.lock().map_err(|_| {
            RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
        })?;
        let persisted = grants
            .values()
            .filter(|stored| stored.grant.persistent)
            .map(|stored| PersistedFsGrant {
                id: stored.grant.id.clone(),
                path: stored.path.clone(),
                access: stored.grant.access,
                kind: stored.grant.kind,
            })
            .collect::<Vec<_>>();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&persisted)?)?;
        Ok(())
    }

    pub fn read_text<P>(&self, requested: P) -> Result<String>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        fs::read_to_string(resolved.absolute).map_err(Into::into)
    }

    pub fn read_binary<P>(&self, requested: P) -> Result<FsBinaryContents>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        let bytes = fs::read(&resolved.absolute)?;
        Ok(FsBinaryContents {
            file: self.entry_for_resolved(&resolved)?,
            byte_length: bytes.len(),
            base64: BASE64_STANDARD.encode(bytes),
        })
    }

    pub fn metadata<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        self.entry_for_resolved(&resolved)
    }

    pub fn list_dir<P>(&self, requested: P) -> Result<Vec<FsEntry>>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        if !resolved.absolute.is_dir() {
            return Err(RuntimeError::InvalidParameter(format!(
                "'{}' is not a directory",
                display_requested_path(requested.as_ref())
            )));
        }

        let mut entries = fs::read_dir(&resolved.absolute)?
            .map(|entry| {
                let entry = entry?;
                let child_absolute = entry.path();
                let child_relative = child_absolute
                    .strip_prefix(&resolved.root)
                    .map(PathBuf::from)
                    .map_err(|_| {
                        RuntimeError::PermissionDenied(format!(
                            "path '{}' is outside the configured filesystem roots",
                            child_absolute.display()
                        ))
                    })?;

                let mut entry =
                    self.entry_for_paths(&resolved.root, &child_absolute, &child_relative)?;
                if let Some(prefix) = &resolved.uri_prefix {
                    apply_uri_prefix(&mut entry, prefix, &child_relative);
                }
                Ok(entry)
            })
            .collect::<Result<Vec<_>>>()?;

        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.path.cmp(&right.path))
        });

        Ok(entries)
    }

    pub fn walk<P>(&self, requested: P, options: &FsWalkOptions) -> Result<Vec<FsEntry>>
    where
        P: AsRef<Path>,
    {
        let root = self.resolve_existing(requested.as_ref())?;
        if !root.absolute.is_dir() {
            return Err(RuntimeError::InvalidParameter(
                "fs.walk requires a directory URI".into(),
            ));
        }
        let limit = options.limit.unwrap_or(10_000);
        if limit == 0 || limit > 100_000 {
            return Err(RuntimeError::InvalidParameter(
                "fs.walk limit must be between 1 and 100000".into(),
            ));
        }
        let extensions = options
            .extensions
            .iter()
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        let mut directories = vec![root.absolute.clone()];
        let mut entries = Vec::new();
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory)? {
                let path = entry?.path();
                let canonical = path.canonicalize()?;
                if !canonical.starts_with(&root.root) {
                    return Err(RuntimeError::PermissionDenied(format!(
                        "filesystem walk encountered an escaping symlink at '{}'",
                        path.display()
                    )));
                }
                let relative = canonical
                    .strip_prefix(&root.root)
                    .map(PathBuf::from)
                    .map_err(|_| {
                        RuntimeError::PermissionDenied("filesystem walk escaped its grant".into())
                    })?;
                let mut item = self.entry_for_paths(&root.root, &canonical, &relative)?;
                if let Some(prefix) = &root.uri_prefix {
                    apply_uri_prefix(&mut item, prefix, &relative);
                }
                if item.is_dir && options.recursive {
                    directories.push(canonical);
                }
                let matches_extension = extensions.is_empty()
                    || item.is_dir
                    || item
                        .extension
                        .as_ref()
                        .is_some_and(|value| extensions.contains(&value.to_ascii_lowercase()));
                if matches_extension {
                    entries.push(item);
                    if entries.len() >= limit {
                        entries.sort_by(|left, right| left.uri.cmp(&right.uri));
                        return Ok(entries);
                    }
                }
            }
        }
        entries.sort_by(|left, right| left.uri.cmp(&right.uri));
        Ok(entries)
    }

    pub fn write_text<P>(&self, requested: P, contents: &str) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_for_write(requested.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::write(&resolved.absolute, contents)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn write_binary<P>(&self, requested: P, contents_base64: &str) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let bytes = BASE64_STANDARD.decode(contents_base64).map_err(|error| {
            RuntimeError::InvalidParameter(format!("binary payload is not valid base64: {error}"))
        })?;
        let resolved = self.resolve_for_write(requested.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::write(&resolved.absolute, bytes)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn copy_from<P, Q>(&self, source: P, destination: Q) -> Result<FsEntry>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let source = self.resolve_existing(source.as_ref())?;
        if !source.absolute.is_file() {
            return Err(RuntimeError::InvalidParameter(format!(
                "copy source '{}' is not a file",
                display_requested_path(&source.relative)
            )));
        }

        let resolved = self.resolve_for_write(destination.as_ref())?;
        self.prepare_write_target(&resolved)?;
        fs::copy(source.absolute, &resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn open_path<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        open_in_default_app(&resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn reveal_path<P>(&self, requested: P) -> Result<FsEntry>
    where
        P: AsRef<Path>,
    {
        let resolved = self.resolve_existing(requested.as_ref())?;
        reveal_in_file_manager(&resolved.absolute)?;
        self.entry_for_resolved(&resolved)
    }

    pub fn resolve(&self, requested: &Path) -> Result<PathBuf> {
        Ok(self.resolve_existing(requested)?.absolute)
    }

    fn resolve_existing(&self, requested: &Path) -> Result<ResolvedFsPath> {
        if let Some(resolved) = self.resolve_root_uri(requested, false)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.resolve_grant_uri(requested, false)? {
            return Ok(resolved);
        }
        if self.roots.is_empty() {
            return Err(RuntimeError::PermissionDenied(
                "no filesystem roots have been allowed".into(),
            ));
        }

        if requested.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "path must not be empty".into(),
            ));
        }

        if requested.is_absolute() {
            let canonical = requested.canonicalize().map_err(|error| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve '{}': {error}",
                    requested.display()
                ))
            })?;

            return self.ensure_allowed(canonical, requested);
        }

        for (index, root) in self.roots.iter().enumerate() {
            let joined = root.join(requested);
            let canonical = match joined.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };

            if canonical.starts_with(root) {
                return Ok(ResolvedFsPath {
                    root: root.clone(),
                    relative: canonical
                        .strip_prefix(root)
                        .map(PathBuf::from)
                        .unwrap_or_default(),
                    absolute: canonical,
                    uri_prefix: Some(format!("root://{}", self.root_ids[index])),
                });
            }
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn resolve_for_write(&self, requested: &Path) -> Result<ResolvedFsPath> {
        if let Some(resolved) = self.resolve_root_uri(requested, true)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.resolve_grant_uri(requested, true)? {
            return Ok(resolved);
        }
        if self.roots.is_empty() {
            return Err(RuntimeError::PermissionDenied(
                "no filesystem roots have been allowed".into(),
            ));
        }

        if requested.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidParameter(
                "path must not be empty".into(),
            ));
        }

        if requested.is_absolute() {
            for (index, root) in self.roots.iter().enumerate() {
                if let Ok(mut resolved) = resolve_candidate_for_write(root, requested) {
                    resolved.uri_prefix = Some(format!("root://{}", self.root_ids[index]));
                    return Ok(resolved);
                }
            }

            return Err(RuntimeError::PermissionDenied(format!(
                "path '{}' is outside the configured filesystem roots",
                requested.display()
            )));
        }

        for (index, root) in self.roots.iter().enumerate() {
            let candidate = root.join(requested);
            if let Ok(mut resolved) = resolve_candidate_for_write(root, &candidate) {
                resolved.uri_prefix = Some(format!("root://{}", self.root_ids[index]));
                return Ok(resolved);
            }
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn ensure_allowed(&self, canonical: PathBuf, requested: &Path) -> Result<ResolvedFsPath> {
        if let Some((index, root)) = self
            .roots
            .iter()
            .enumerate()
            .find(|(_, root)| canonical.starts_with(root.as_path()))
        {
            return Ok(ResolvedFsPath {
                root: root.clone(),
                relative: canonical
                    .strip_prefix(root)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                absolute: canonical,
                uri_prefix: Some(format!("root://{}", self.root_ids[index])),
            });
        }

        Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            requested.display()
        )))
    }

    fn resolve_root_uri(
        &self,
        requested: &Path,
        for_write: bool,
    ) -> Result<Option<ResolvedFsPath>> {
        let rendered = requested.to_string_lossy().replace('\\', "/");
        let Some(rest) = rendered.strip_prefix("root://") else {
            return Ok(None);
        };
        let (id, relative) = rest.split_once('/').unwrap_or((rest, ""));
        if id.is_empty() || relative.split('/').any(|segment| segment == "..") {
            return Err(RuntimeError::PermissionDenied(
                "filesystem root URI is invalid or escapes its root".into(),
            ));
        }
        let index = self
            .root_ids
            .iter()
            .position(|candidate| candidate == id)
            .ok_or_else(|| {
                RuntimeError::PermissionDenied(format!(
                    "filesystem root URI references unknown root '{id}'"
                ))
            })?;
        let root = &self.roots[index];
        let candidate = root.join(relative);
        let mut resolved = if for_write {
            resolve_candidate_for_write(root, &candidate)?
        } else {
            let absolute = candidate.canonicalize().map_err(|error| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve root URI '{rendered}': {error}"
                ))
            })?;
            if !absolute.starts_with(root) {
                return Err(RuntimeError::PermissionDenied(
                    "filesystem root URI escaped its declared root".into(),
                ));
            }
            ResolvedFsPath {
                relative: absolute
                    .strip_prefix(root)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                absolute,
                root: root.clone(),
                uri_prefix: None,
            }
        };
        resolved.uri_prefix = Some(format!("root://{id}"));
        Ok(Some(resolved))
    }

    fn resolve_grant_uri(
        &self,
        requested: &Path,
        for_write: bool,
    ) -> Result<Option<ResolvedFsPath>> {
        let rendered = requested.to_string_lossy().replace('\\', "/");
        let Some(rest) = rendered.strip_prefix("grant://") else {
            return Ok(None);
        };
        let (id, relative) = rest.split_once('/').unwrap_or((rest, ""));
        if id.is_empty() || relative.split('/').any(|segment| segment == "..") {
            return Err(RuntimeError::PermissionDenied(
                "filesystem grant URI is invalid or escapes its root".into(),
            ));
        }
        let grants = self.grants.lock().map_err(|_| {
            RuntimeError::InvalidConfiguration("filesystem grant store is poisoned".into())
        })?;
        let stored = grants.get(id).ok_or_else(|| {
            RuntimeError::PermissionDenied(format!("filesystem grant '{id}' is missing or revoked"))
        })?;
        if for_write && stored.grant.access != FsGrantAccess::ReadWrite {
            return Err(RuntimeError::PermissionDenied(format!(
                "filesystem grant '{id}' is read-only"
            )));
        }
        if stored.grant.kind == FsGrantKind::File {
            if !relative.is_empty() {
                return Err(RuntimeError::PermissionDenied(
                    "file grants cannot resolve child paths".into(),
                ));
            }
            return Ok(Some(ResolvedFsPath {
                root: stored.path.clone(),
                absolute: stored.path.clone(),
                relative: PathBuf::new(),
                uri_prefix: Some(format!("grant://{id}")),
            }));
        }

        let candidate = stored.path.join(relative);
        let mut resolved = if for_write {
            resolve_candidate_for_write(&stored.path, &candidate)?
        } else {
            let absolute = candidate.canonicalize().map_err(|error| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve grant URI '{rendered}': {error}"
                ))
            })?;
            if !absolute.starts_with(&stored.path) {
                return Err(RuntimeError::PermissionDenied(
                    "filesystem grant URI escaped its selected directory".into(),
                ));
            }
            ResolvedFsPath {
                relative: absolute
                    .strip_prefix(&stored.path)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                absolute,
                root: stored.path.clone(),
                uri_prefix: None,
            }
        };
        resolved.uri_prefix = Some(format!("grant://{id}"));
        Ok(Some(resolved))
    }

    fn prepare_write_target(&self, resolved: &ResolvedFsPath) -> Result<()> {
        if let Some(parent) = resolved.absolute.parent() {
            fs::create_dir_all(parent)?;
        }

        if resolved.absolute.exists() && resolved.absolute.is_dir() {
            return Err(RuntimeError::InvalidParameter(format!(
                "'{}' is a directory",
                display_requested_path(&resolved.relative)
            )));
        }

        Ok(())
    }

    fn entry_for_resolved(&self, resolved: &ResolvedFsPath) -> Result<FsEntry> {
        let mut entry =
            self.entry_for_paths(&resolved.root, &resolved.absolute, &resolved.relative)?;
        if let Some(prefix) = &resolved.uri_prefix {
            apply_uri_prefix(&mut entry, prefix, &resolved.relative);
        }
        Ok(entry)
    }

    fn entry_for_paths(&self, root: &Path, absolute: &Path, relative: &Path) -> Result<FsEntry> {
        let metadata = fs::metadata(absolute)?;
        let path = display_requested_path(relative);
        Ok(FsEntry {
            uri: path.clone(),
            path,
            name: absolute
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| root.to_string_lossy().to_string()),
            parent: relative
                .parent()
                .map(display_requested_path)
                .unwrap_or_else(|| ".".into()),
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            size: metadata.len(),
            extension: absolute
                .extension()
                .map(|value| value.to_string_lossy().to_string()),
            modified_at: modified_at_string(&metadata).ok(),
        })
    }
}

fn resolve_candidate_for_write(root: &Path, candidate: &Path) -> Result<ResolvedFsPath> {
    let mut existing_ancestor = candidate.to_path_buf();
    let mut missing_segments = Vec::new();

    while !existing_ancestor.exists() {
        let segment = existing_ancestor.file_name().ok_or_else(|| {
            RuntimeError::InvalidParameter(format!(
                "unable to resolve '{}' for writing",
                candidate.display()
            ))
        })?;
        missing_segments.push(segment.to_os_string());
        existing_ancestor = existing_ancestor
            .parent()
            .ok_or_else(|| {
                RuntimeError::InvalidParameter(format!(
                    "unable to resolve '{}' for writing",
                    candidate.display()
                ))
            })?
            .to_path_buf();
    }

    let canonical_ancestor = existing_ancestor.canonicalize().map_err(|error| {
        RuntimeError::InvalidParameter(format!(
            "unable to resolve '{}': {error}",
            candidate.display()
        ))
    })?;

    if !canonical_ancestor.starts_with(root) {
        return Err(RuntimeError::PermissionDenied(format!(
            "path '{}' is outside the configured filesystem roots",
            candidate.display()
        )));
    }

    let mut absolute = canonical_ancestor.clone();
    for segment in missing_segments.iter().rev() {
        absolute.push(segment);
    }

    let relative = absolute
        .strip_prefix(root)
        .map(PathBuf::from)
        .map_err(|_| {
            RuntimeError::PermissionDenied(format!(
                "path '{}' is outside the configured filesystem roots",
                candidate.display()
            ))
        })?;

    Ok(ResolvedFsPath {
        root: root.to_path_buf(),
        absolute,
        relative,
        uri_prefix: None,
    })
}

fn display_requested_path(path: &Path) -> String {
    let rendered = path.to_string_lossy().replace('\\', "/");
    if rendered.is_empty() {
        ".".into()
    } else {
        rendered
    }
}

fn uri_join(prefix: &str, relative: &Path) -> String {
    let relative = display_requested_path(relative);
    if relative == "." {
        prefix.to_string()
    } else {
        format!("{prefix}/{relative}")
    }
}

fn apply_uri_prefix(entry: &mut FsEntry, prefix: &str, relative: &Path) {
    entry.uri = uri_join(prefix, relative);
    entry.parent = relative
        .parent()
        .map(|parent| uri_join(prefix, parent))
        .unwrap_or_else(|| prefix.to_string());
}

fn filesystem_root_id(root: &Path, index: usize) -> String {
    let source = root
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();
    let mut id = String::new();
    let mut separator = false;
    for character in source.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            if separator && !id.is_empty() {
                id.push('-');
            }
            separator = false;
            id.push(character.to_ascii_lowercase());
        } else {
            separator = true;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        format!("root-{}", index + 1)
    } else {
        id
    }
}

fn modified_at_string(metadata: &fs::Metadata) -> std::io::Result<String> {
    let modified_at = metadata.modified()?;
    let timestamp = OffsetDateTime::from(modified_at)
        .format(&Rfc3339)
        .map_err(std::io::Error::other)?;
    Ok(timestamp)
}

#[derive(Clone, Debug)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
    allowed_extra_args: BTreeSet<String>,
    cwd: Option<PathBuf>,
    env: BTreeMap<String, String>,
    clear_env: bool,
    timeout: Duration,
    max_output_bytes: usize,
}

impl ShellCommand {
    pub fn new<I, S>(program: impl Into<String>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            allowed_extra_args: BTreeSet::new(),
            cwd: None,
            env: BTreeMap::new(),
            clear_env: false,
            timeout: DEFAULT_SHELL_TIMEOUT,
            max_output_bytes: DEFAULT_SHELL_MAX_OUTPUT_BYTES,
        }
    }

    pub fn allow_extra_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_extra_args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn clear_env(mut self) -> Self {
        self.clear_env = true;
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    fn validate(&self, name: &str) -> Result<()> {
        if self.program.trim().is_empty() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' must declare a program"
            )));
        }

        if self.timeout.is_zero() {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' timeout must be greater than zero"
            )));
        }

        if self.max_output_bytes == 0 {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' max output bytes must be greater than zero"
            )));
        }

        if let Some(cwd) = &self.cwd {
            if !cwd.exists() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' cwd '{}' does not exist",
                    cwd.display()
                )));
            }

            if !cwd.is_dir() {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' cwd '{}' is not a directory",
                    cwd.display()
                )));
            }
        }

        for key in self.env.keys() {
            if key.is_empty() || key.contains('=') || key.contains('\0') {
                return Err(RuntimeError::InvalidConfiguration(format!(
                    "shell command '{name}' defines invalid env key '{key}'"
                )));
            }
        }

        if self.env.values().any(|value| value.contains('\0')) {
            return Err(RuntimeError::InvalidConfiguration(format!(
                "shell command '{name}' defines env values containing NUL bytes"
            )));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShellCapability {
    commands: BTreeMap<String, ShellCommand>,
}

impl ShellCapability {
    pub fn new(commands: BTreeMap<String, ShellCommand>) -> Self {
        Self { commands }
    }

    pub fn try_new(commands: BTreeMap<String, ShellCommand>) -> Result<Self> {
        for (name, command) in &commands {
            command.validate(name)?;
        }

        Ok(Self { commands })
    }

    pub fn insert(&mut self, name: impl Into<String>, command: ShellCommand) {
        self.commands.insert(name.into(), command);
    }

    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(String::as_str).collect()
    }

    pub fn exec(&self, name: &str, extra_args: &[String]) -> Result<ShellOutput> {
        let Some(command) = self.commands.get(name) else {
            return Err(RuntimeError::PermissionDenied(format!(
                "shell command '{name}' is not allowed"
            )));
        };

        command.validate(name)?;

        if !extra_args.is_empty() && command.allowed_extra_args.is_empty() {
            return Err(RuntimeError::PermissionDenied(format!(
                "shell command '{name}' does not allow frontend arguments"
            )));
        }

        for arg in extra_args {
            if !command.allowed_extra_args.contains(arg) {
                return Err(RuntimeError::PermissionDenied(format!(
                    "shell command '{name}' does not allow argument '{arg}'"
                )));
            }
        }

        let mut child = Command::new(&command.program);
        child
            .args(&command.args)
            .args(extra_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = &command.cwd {
            child.current_dir(cwd);
        }

        if command.clear_env {
            child.env_clear();
        }

        if !command.env.is_empty() {
            child.envs(command.env.iter());
        }

        let mut child = child.spawn().map_err(|error| {
            audit_shell_event(
                name,
                command,
                extra_args,
                None,
                None,
                None,
                Some(format!("failed to spawn process: {error}")),
            );
            error
        })?;
        let stdout_reader = spawn_reader(child.stdout.take(), command.max_output_bytes);
        let stderr_reader = spawn_reader(child.stderr.take(), command.max_output_bytes);
        let status = wait_for_exit(&mut child, command.timeout)?;

        let status = match status {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait()?;
                let stdout = collect_reader(stdout_reader)?;
                let stderr = collect_reader(stderr_reader)?;
                audit_shell_event(
                    name,
                    command,
                    extra_args,
                    None,
                    Some(&stdout),
                    Some(&stderr),
                    Some(format!(
                        "shell command '{name}' timed out after {} ms",
                        command.timeout.as_millis()
                    )),
                );
                return Err(RuntimeError::TimedOut(format!(
                    "shell command '{name}' timed out after {} ms",
                    command.timeout.as_millis()
                )));
            }
        };

        let stdout = collect_reader(stdout_reader)?;
        let stderr = collect_reader(stderr_reader)?;
        let output = ShellOutput {
            stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
            exit_code: status.code().unwrap_or_default(),
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            timeout_ms: command.timeout.as_millis() as u64,
            max_output_bytes: command.max_output_bytes,
        };
        audit_shell_event(
            name,
            command,
            extra_args,
            Some(output.exit_code),
            Some(&stdout),
            Some(&stderr),
            None,
        );

        Ok(output)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

fn spawn_reader(
    stream: Option<impl Read + Send + 'static>,
    max_output_bytes: usize,
) -> Option<thread::JoinHandle<std::io::Result<CapturedStream>>> {
    stream.map(|mut stream| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 8192];
            let mut truncated = false;

            loop {
                let read = stream.read(&mut buffer)?;
                if read == 0 {
                    break;
                }

                let remaining = max_output_bytes.saturating_sub(bytes.len());
                let kept = remaining.min(read);

                if kept > 0 {
                    bytes.extend_from_slice(&buffer[..kept]);
                }

                if kept < read {
                    truncated = true;
                }
            }

            Ok(CapturedStream { bytes, truncated })
        })
    })
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }

        if Instant::now() >= deadline {
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn collect_reader(
    reader: Option<thread::JoinHandle<std::io::Result<CapturedStream>>>,
) -> Result<CapturedStream> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| RuntimeError::InvalidConfiguration("shell output reader panicked".into()))?
            .map_err(Into::into),
        None => Ok(CapturedStream {
            bytes: Vec::new(),
            truncated: false,
        }),
    }
}

fn open_in_default_app(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        run_desktop_command("open", [path.as_os_str()])
    }

    #[cfg(target_os = "windows")]
    {
        run_desktop_command(
            "cmd",
            [
                std::ffi::OsStr::new("/C"),
                std::ffi::OsStr::new("start"),
                std::ffi::OsStr::new(""),
                path.as_os_str(),
            ],
        )
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        run_desktop_command("xdg-open", [path.as_os_str()])
    }
}

fn reveal_in_file_manager(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        run_desktop_command("open", [std::ffi::OsStr::new("-R"), path.as_os_str()])
    }

    #[cfg(target_os = "windows")]
    {
        let select_arg = format!("/select,{}", path.display());
        run_desktop_command("explorer", [std::ffi::OsStr::new(select_arg.as_str())])
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = path.parent().unwrap_or(path);
        run_desktop_command("xdg-open", [parent.as_os_str()])
    }
}

fn run_desktop_command<I, S>(program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = process::Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(RuntimeError::InvalidParameter(format!(
            "desktop helper '{program}' exited with status {status}"
        )))
    }
}

fn audit_shell_event(
    name: &str,
    command: &ShellCommand,
    extra_args: &[String],
    exit_code: Option<i32>,
    stdout: Option<&CapturedStream>,
    stderr: Option<&CapturedStream>,
    error: Option<String>,
) {
    let Some(path) = env::var_os("RUSTFRAME_AUDIT_LOG") else {
        return;
    };

    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let payload = serde_json::json!({
        "timestamp": timestamp,
        "name": name,
        "program": command.program,
        "fixedArgCount": command.args.len(),
        "extraArgCount": extra_args.len(),
        "argumentsRedacted": true,
        "cwdConfigured": command.cwd.is_some(),
        "envKeys": command.env.keys().cloned().collect::<Vec<_>>(),
        "clearEnv": command.clear_env,
        "timeoutMs": command.timeout.as_millis() as u64,
        "maxOutputBytes": command.max_output_bytes,
        "exitCode": exit_code,
        "stdoutBytes": stdout.map(|value| value.bytes.len()).unwrap_or(0),
        "stderrBytes": stderr.map(|value| value.bytes.len()).unwrap_or(0),
        "stdoutTruncated": stdout.map(|value| value.truncated).unwrap_or(false),
        "stderrTruncated": stderr.map(|value| value.truncated).unwrap_or(false),
        "error": error,
    });

    if let Some(parent) = Path::new(&path).parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{payload}");
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    #[cfg(unix)]
    use std::path::Path;

    use base64::Engine as _;
    use tempfile::tempdir;

    use super::{
        BASE64_STANDARD, FsCapability, FsGrantAccess, FsWalkOptions, ShellCapability, ShellCommand,
    };

    #[test]
    fn reads_file_inside_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("note.txt");
        fs::write(&file, "console.log('hello');").unwrap();

        let capability = FsCapability::new([root]).unwrap();

        let content = capability.read_text("note.txt").unwrap();

        assert_eq!(content, "console.log('hello');");
    }

    #[test]
    fn rejects_parent_escape_outside_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("safe.txt"), "safe").unwrap();
        fs::write(temp.path().join("secret.txt"), "secret").unwrap();

        let capability = FsCapability::new([root]).unwrap();
        let error = capability.read_text("../secret.txt").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("outside the configured filesystem roots")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_read_write_and_walk_through_escaping_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        let rooted = FsCapability::new([root.clone()]).unwrap();
        assert!(rooted.read_text("escape/secret.txt").is_err());
        assert!(rooted.write_text("escape/new.txt", "blocked").is_err());
        assert!(!outside.join("new.txt").exists());
        assert!(
            rooted
                .walk(
                    ".",
                    &FsWalkOptions {
                        recursive: true,
                        extensions: Vec::new(),
                        limit: Some(100),
                    },
                )
                .is_err()
        );

        let grants = FsCapability::new(Vec::<PathBuf>::new()).unwrap();
        let grant = grants
            .grant_path(&root, FsGrantAccess::ReadWrite, false)
            .unwrap();
        assert!(
            grants
                .read_text(format!("{}/escape/secret.txt", grant.uri))
                .is_err()
        );
        assert!(
            grants
                .write_text(format!("{}/escape/new.txt", grant.uri), "blocked")
                .is_err()
        );
    }

    #[test]
    fn lists_directory_entries_and_metadata() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/brief.md"), "# Brief").unwrap();
        fs::write(root.join("summary.txt"), "ready").unwrap();

        let capability = FsCapability::new([root]).unwrap();
        let entries = capability.list_dir(".").unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "notes");
        assert_eq!(entries[0].uri, "root://frontend/notes");
        assert_eq!(entries[0].parent, "root://frontend");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].path, "summary.txt");
        assert!(entries[1].is_file);
        assert_eq!(entries[1].extension.as_deref(), Some("txt"));
    }

    #[test]
    fn writes_text_inside_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        let written = capability
            .write_text("imports/brief.md", "# Imported")
            .unwrap();

        assert_eq!(written.path, "imports/brief.md");
        assert_eq!(written.uri, "root://frontend/imports/brief.md");
        assert_eq!(
            fs::read_to_string(root.join("imports/brief.md")).unwrap(),
            "# Imported"
        );
    }

    #[test]
    fn reads_and_writes_binary_payloads() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        capability
            .write_binary("assets/icon.bin", &BASE64_STANDARD.encode([0_u8, 1, 2, 3]))
            .unwrap();

        let binary = capability.read_binary("assets/icon.bin").unwrap();

        assert_eq!(binary.byte_length, 4);
        assert_eq!(binary.base64, BASE64_STANDARD.encode([0_u8, 1, 2, 3]));
        assert_eq!(
            fs::read(root.join("assets/icon.bin")).unwrap(),
            vec![0_u8, 1, 2, 3]
        );
    }

    #[test]
    fn copies_granted_files_into_allowed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("frontend");
        fs::create_dir_all(&root).unwrap();
        let external = temp.path().join("source.md");
        fs::write(&external, "# External").unwrap();

        let capability = FsCapability::new([root.clone()]).unwrap();
        let grant = capability
            .grant_path(&external, FsGrantAccess::Read, false)
            .unwrap();
        let copied = capability
            .copy_from(&grant.uri, "root://frontend/imports/source.md")
            .unwrap();

        assert_eq!(copied.path, "imports/source.md");
        assert_eq!(
            fs::read_to_string(root.join("imports/source.md")).unwrap(),
            "# External"
        );
    }

    #[test]
    fn root_uris_resolve_without_exposing_absolute_paths() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join("reports")).unwrap();
        fs::write(root.join("reports/q3.md"), "Q3").unwrap();

        let capability = FsCapability::new([root]).unwrap();
        assert_eq!(capability.root_uris(), vec!["root://workspace"]);
        assert_eq!(
            capability
                .read_text("root://workspace/reports/q3.md")
                .unwrap(),
            "Q3"
        );
        let metadata = capability
            .metadata("root://workspace/reports/q3.md")
            .unwrap();
        assert_eq!(metadata.uri, "root://workspace/reports/q3.md");
        assert_eq!(metadata.parent, "root://workspace/reports");
        assert!(
            !metadata
                .uri
                .contains(temp.path().to_string_lossy().as_ref())
        );
        assert!(
            capability
                .read_text("root://workspace/../secret.txt")
                .is_err()
        );
        assert!(
            capability
                .read_text("root://unknown/reports/q3.md")
                .is_err()
        );
    }

    #[test]
    fn rejects_duplicate_derived_root_ids() {
        let temp = tempdir().unwrap();
        let first = temp.path().join("one/workspace");
        let second = temp.path().join("two/workspace");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();

        let error = FsCapability::new([first, second]).unwrap_err();
        assert!(error.to_string().contains("duplicate id 'workspace'"));
    }

    #[test]
    fn rejects_unknown_shell_command() {
        let capability = ShellCapability::new(Default::default());
        let error = capability.exec("missing", &[]).unwrap_err();

        assert!(error.to_string().contains("is not allowed"));
    }

    #[test]
    fn executes_allowlisted_shell_command() {
        let mut capability = ShellCapability::default();
        capability.insert("print", ShellCommand::new("printf", ["rustframe"]));

        let output = capability.exec("print", &[]).unwrap();

        assert_eq!(output.stdout, "rustframe");
        assert_eq!(output.exit_code, 0);
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
        assert_eq!(output.timeout_ms, 10_000);
        assert_eq!(output.max_output_bytes, 64 * 1024);
    }

    #[test]
    fn rejects_frontend_args_when_none_are_allowlisted() {
        let mut capability = ShellCapability::default();
        capability.insert("print", ShellCommand::new("printf", ["rustframe"]));

        let error = capability
            .exec("print", &[String::from("--json")])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not allow frontend arguments")
        );
    }

    #[test]
    fn rejects_frontend_args_outside_allowlist() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["rustframe"]).allow_extra_args(["--json"]),
        );

        let error = capability
            .exec("print", &[String::from("--yaml")])
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not allow argument '--yaml'")
        );
    }

    #[test]
    fn executes_allowlisted_frontend_args() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["%s%s", "rustframe"]).allow_extra_args(["--json"]),
        );

        let output = capability.exec("print", &[String::from("--json")]).unwrap();

        assert_eq!(output.stdout, "rustframe--json");
    }

    #[test]
    fn rejects_invalid_shell_configuration() {
        let error = ShellCapability::try_new(BTreeMap::from([(
            "print".to_string(),
            ShellCommand::new("printf", ["rustframe"]).timeout_ms(0),
        )]))
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("timeout must be greater than zero")
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_can_run_in_declared_cwd() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir_all(&nested).unwrap();

        let mut capability = ShellCapability::default();
        capability.insert(
            "pwd",
            ShellCommand::new("pwd", std::iter::empty::<&str>()).current_dir(&nested),
        );

        let output = capability.exec("pwd", &[]).unwrap();

        assert_eq!(
            Path::new(output.stdout.trim()).canonicalize().unwrap(),
            nested.canonicalize().unwrap()
        );
    }

    #[test]
    fn grants_use_opaque_uris_and_enforce_access_and_revocation() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("notes.md"), "private notes").unwrap();
        let capability = FsCapability::new(Vec::<PathBuf>::new()).unwrap();
        let grant = capability
            .grant_path(temp.path(), FsGrantAccess::Read, false)
            .unwrap();

        assert!(grant.uri.starts_with("grant://grant-"));
        assert_eq!(
            capability
                .read_text(format!("{}/notes.md", grant.uri))
                .unwrap(),
            "private notes"
        );
        assert!(
            capability
                .write_text(format!("{}/notes.md", grant.uri), "changed")
                .unwrap_err()
                .to_string()
                .contains("read-only")
        );
        assert!(capability.revoke_grant(&grant.id).unwrap());
        assert!(
            capability
                .read_text(format!("{}/notes.md", grant.uri))
                .unwrap_err()
                .to_string()
                .contains("revoked")
        );
    }

    #[test]
    fn persistent_grants_survive_capability_restart_and_revocation() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let store = temp.path().join("private/grants.json");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("notes.md"), "persisted").unwrap();

        let first = FsCapability::new(Vec::<PathBuf>::new())
            .unwrap()
            .with_persistence(&store)
            .unwrap();
        let grant = first
            .grant_path(&workspace, FsGrantAccess::Read, true)
            .unwrap();
        drop(first);

        let restored = FsCapability::new(Vec::<PathBuf>::new())
            .unwrap()
            .with_persistence(&store)
            .unwrap();
        assert_eq!(restored.grants().unwrap().len(), 1);
        assert_eq!(
            restored
                .read_text(format!("{}/notes.md", grant.uri))
                .unwrap(),
            "persisted"
        );
        assert!(restored.revoke_grant(&grant.id).unwrap());

        let after_revocation = FsCapability::new(Vec::<PathBuf>::new())
            .unwrap()
            .with_persistence(&store)
            .unwrap();
        assert!(after_revocation.grants().unwrap().is_empty());
    }

    #[test]
    fn recursive_walk_filters_extensions_and_limits_entries() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("a.md"), "a").unwrap();
        fs::write(temp.path().join("nested/b.txt"), "b").unwrap();
        fs::write(temp.path().join("nested/c.png"), "c").unwrap();
        let capability = FsCapability::new(Vec::<PathBuf>::new()).unwrap();
        let grant = capability
            .grant_path(temp.path(), FsGrantAccess::Read, false)
            .unwrap();
        let entries = capability
            .walk(
                &grant.uri,
                &FsWalkOptions {
                    recursive: true,
                    extensions: vec!["md".into(), ".txt".into()],
                    limit: Some(10),
                },
            )
            .unwrap();
        let files = entries
            .iter()
            .filter(|entry| entry.is_file)
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|entry| entry.uri.starts_with(&grant.uri)));
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_apply_explicit_env_overrides() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "printenv",
            ShellCommand::new("printenv", ["RUSTFRAME_TEST_ENV"]).env("RUSTFRAME_TEST_ENV", "ok"),
        );

        let output = capability.exec("printenv", &[]).unwrap();

        assert_eq!(output.stdout.trim(), "ok");
    }

    #[cfg(unix)]
    #[test]
    fn shell_commands_time_out() {
        let mut capability = ShellCapability::default();
        capability.insert("sleep", ShellCommand::new("sleep", ["1"]).timeout_ms(25));

        let error = capability.exec("sleep", &[]).unwrap_err();

        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn shell_output_is_truncated_to_limit() {
        let mut capability = ShellCapability::default();
        capability.insert(
            "print",
            ShellCommand::new("printf", ["rustframe"]).max_output_bytes(4),
        );

        let output = capability.exec("print", &[]).unwrap();

        assert_eq!(output.stdout, "rust");
        assert!(output.stdout_truncated);
        assert!(!output.stderr_truncated);
        assert_eq!(output.max_output_bytes, 4);
    }
}
