//! Selected code files are opened through directory descriptors. Neither source
//! symlinks nor destination symlinks can redirect an operation outside its root.
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use aionui_api_types::{ProductFactorySnapshotFileRequest, ProductFactorySourceFile, ProductFactorySourceManifest};
use sha2::{Digest, Sha256};

use super::{VersionError, VersionResult};

pub const MAX_FILES: usize = 512;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;

/// Establish a configured/new run root once, before operations begin. Existing
/// OS aliases such as macOS /tmp and /var are resolved here, never while copying.
pub(crate) fn established_root(path: PathBuf) -> PathBuf {
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().map(|cwd| cwd.join(&path)).unwrap_or(path)
    };
    let mut existing = absolute.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let Some(leaf) = existing.file_name() else {
            return absolute;
        };
        suffix.push(leaf.to_os_string());
        let Some(parent) = existing.parent() else {
            return absolute;
        };
        existing = parent;
    }
    let Ok(mut root) = std::fs::canonicalize(existing) else {
        return absolute;
    };
    for part in suffix.into_iter().rev() {
        root.push(part);
    }
    root
}

pub fn validate_relative(path: &str) -> VersionResult<()> {
    if path.is_empty()
        || path.len() > 512
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.len() > 255)
        || path.split('/').count() > 32
    {
        return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"));
    }
    Ok(())
}

fn excluded_component(component: &str) -> bool {
    let value = component.to_ascii_lowercase();
    matches!(
        value.as_str(),
        ".tasks"
            | ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "out"
            | "runtime"
            | "data"
            | "evidence"
            | "logs"
            | "cache"
            | ".cache"
            | "__pycache__"
            | "tmp"
            | ".ssh"
            | ".aws"
            | ".codex"
            | ".claude"
            | ".gemini"
            | ".agents"
            | ".config"
            | ".local"
            | "product-factory-versions"
    ) || value.starts_with(".env")
        || value.starts_with("credentials")
        || value.starts_with("secrets")
        || matches!(
            value.as_str(),
            ".npmrc"
                | ".pypirc"
                | ".netrc"
                | "auth.json"
                | "authentication.json"
                | "tokens.json"
                | "token.json"
                | "provider-config.json"
                | "id_rsa"
                | "id_ed25519"
        )
        || (value.starts_with('.') && !matches!(value.as_str(), ".gitignore" | ".gitattributes" | ".editorconfig"))
}

fn allowed_file(path: &str) -> bool {
    if path.split('/').any(excluded_component) {
        return false;
    }
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "makefile" | "dockerfile" | "license" | "notice" | ".gitignore" | ".gitattributes" | ".editorconfig"
    ) {
        return true;
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let static_fixture = path.starts_with("examples/")
        || path.starts_with("tests/fixtures/")
        || path.starts_with("assets/")
        || path.starts_with("public/")
        || path.starts_with("static/");
    if extension == "csv" {
        return path.starts_with("examples/") || path.starts_with("tests/fixtures/");
    }
    if extension == "json" {
        if static_fixture || path.starts_with("src/") || path.starts_with("schemas/") || path.starts_with("config/") {
            return true;
        }
        return !path.contains('/')
            && matches!(
                name.as_str(),
                "package.json"
                    | "package-lock.json"
                    | "tsconfig.json"
                    | "jsconfig.json"
                    | "manifest.json"
                    | "schema.json"
                    | "openapi.json"
                    | "swagger.json"
                    | "composer.json"
                    | "config.json"
                    | "settings.json"
                    | "app.config.json"
            );
    }
    matches!(
        extension.as_str(),
        "rs" | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "html"
            | "css"
            | "scss"
            | "vue"
            | "svelte"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "java"
            | "kt"
            | "swift"
            | "rb"
            | "cs"
            | "php"
            | "sh"
            | "bash"
            | "zsh"
            | "ps1"
            | "sql"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "md"
            | "txt"
            | "svg"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "woff"
            | "woff2"
            | "ttf"
            | "eot"
            | "wasm"
            | "lock"
    )
}

fn sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "api_key"
            | "apikey"
            | "access_token"
            | "accesstoken"
            | "refresh_token"
            | "refreshtoken"
            | "client_secret"
            | "clientsecret"
            | "password"
            | "password_hash"
            | "secret"
            | "secret_key"
            | "api_secret"
            | "token"
    )
}
fn placeholder(value: &str) -> bool {
    let value = value.trim();
    let upper = value.to_ascii_uppercase();
    value.is_empty()
        || value.starts_with("${")
        || value.starts_with("{{")
        || value.starts_with('<')
        || upper.starts_with("YOUR_")
        || matches!(
            upper.as_str(),
            "CHANGEME"
                | "CHANGE_ME"
                | "REPLACE_ME"
                | "EXAMPLE"
                | "EXAMPLE-KEY"
                | "FIXTURE-SECRET"
                | "FIXTURE-ONLY"
                | "NULL"
                | "NONE"
                | "~"
                | "TRUE"
                | "FALSE"
        )
        || (value.starts_with('$') && value[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}
fn json_has_credential(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            let literal = match value {
                serde_json::Value::String(value) => !placeholder(value),
                serde_json::Value::Number(value) => value.as_u64() != Some(0),
                _ => false,
            };
            (sensitive_key(key) && literal) || json_has_credential(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(json_has_credential),
        _ => false,
    }
}
fn contains_credential(path: &str, bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    if text.contains("PRIVATE KEY-----") {
        return true;
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "json"
        && let Ok(value) = serde_json::from_str(text)
        && json_has_credential(&value)
    {
        return true;
    }
    for line in text.lines() {
        for prefix in ["sk-", "ghp_", "github_pat_"] {
            if let Some((_, suffix)) = line.split_once(prefix)
                && suffix
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .count()
                    >= 16
            {
                return true;
            }
        }
        let Some((left, right)) = line.split_once(':').or_else(|| line.split_once('=')) else {
            continue;
        };
        let key = left
            .split_whitespace()
            .last()
            .unwrap_or("")
            .rsplit('.')
            .next()
            .unwrap_or("")
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
        if !sensitive_key(key) {
            continue;
        }
        let value = right
            .split(" #")
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches(',')
            .trim();
        let quoted = value.starts_with(['\'', '"']);
        if !quoted && !matches!(extension.as_str(), "yaml" | "yml" | "toml" | "sh" | "bash" | "zsh") {
            continue;
        }
        let literal = value.trim_matches(['\'', '"']);
        if !placeholder(literal) && !literal.starts_with("$(") && !literal.starts_with('`') {
            return true;
        }
    }
    false
}

fn file_from_bytes(path: String, bytes: &[u8]) -> ProductFactorySourceFile {
    ProductFactorySourceFile {
        path,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size_bytes: bytes.len() as u64,
    }
}

pub fn selected_manifest(
    root: &Path,
    selected: &[ProductFactorySnapshotFileRequest],
) -> VersionResult<ProductFactorySourceManifest> {
    if selected.is_empty() || selected.len() > MAX_FILES {
        return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
    }
    let root = platform::Directory::open(root)?;
    let mut paths = BTreeSet::new();
    let mut files = Vec::new();
    let mut total_bytes = 0;
    for selected in selected {
        validate_relative(&selected.path)?;
        if !allowed_file(&selected.path) {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"));
        }
        if selected.sha256.len() != 64
            || !selected
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !paths.insert(selected.path.clone())
        {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"));
        }
        let (bytes, _) = root.read_file(&selected.path)?;
        if contains_credential(&selected.path, &bytes) {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_CREDENTIAL_FILE"));
        }
        let file = file_from_bytes(selected.path.clone(), &bytes);
        if file.sha256 != selected.sha256 {
            return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
        }
        if matches!(file.path.as_str(), "START.md" | "ACCEPTANCE.md") && bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
        }
        total_bytes += file.size_bytes;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
        }
        files.push(file);
    }
    if !paths.contains("START.md") || !paths.contains("ACCEPTANCE.md") {
        return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ProductFactorySourceManifest {
        version: 1,
        scope: "code_only".into(),
        files,
        total_bytes,
    })
}

pub fn source_manifest(root: &Path) -> VersionResult<(ProductFactorySourceManifest, u64)> {
    let root = platform::Directory::open(root)?;
    let mut pending = vec![String::new()];
    let mut files = Vec::new();
    let mut excluded = 0;
    let mut entries = 0;
    let mut total_bytes = 0;
    while let Some(parent) = pending.pop() {
        for (name, directory) in root.entries(&parent)? {
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
            }
            if excluded_component(&name) {
                excluded += 1;
                continue;
            }
            let path = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            validate_relative(&path)?;
            if directory {
                pending.push(path);
                continue;
            }
            if !allowed_file(&path) {
                excluded += 1;
                continue;
            }
            let (bytes, _) = root.read_file(&path)?;
            if contains_credential(&path, &bytes) {
                excluded += 1;
                continue;
            }
            let file = file_from_bytes(path, &bytes);
            total_bytes += file.size_bytes;
            if files.len() >= MAX_FILES || total_bytes > MAX_TOTAL_BYTES {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
            }
            files.push(file);
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((
        ProductFactorySourceManifest {
            version: 1,
            scope: "code_only".into(),
            files,
            total_bytes,
        },
        excluded,
    ))
}

pub fn validate_manifest(manifest: &ProductFactorySourceManifest) -> VersionResult<()> {
    if manifest.version != 1
        || manifest.scope != "code_only"
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_FILES
        || manifest.total_bytes > MAX_TOTAL_BYTES
    {
        return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"));
    }
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for file in &manifest.files {
        validate_relative(&file.path)?;
        if !allowed_file(&file.path)
            || !paths.insert(&file.path)
            || file.size_bytes > MAX_FILE_BYTES
            || file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"));
        }
        total = total
            .checked_add(file.size_bytes)
            .ok_or(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"))?;
    }
    if total != manifest.total_bytes
        || !paths.contains(&"START.md".to_owned())
        || !paths.contains(&"ACCEPTANCE.md".to_owned())
    {
        return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"));
    }
    Ok(())
}

pub fn verify_manifest(root: &Path, manifest: &ProductFactorySourceManifest) -> VersionResult<()> {
    validate_manifest(manifest)?;
    let root = platform::Directory::open(root)?;
    for file in &manifest.files {
        let (bytes, _) = root.read_file(&file.path)?;
        if file_from_bytes(file.path.clone(), &bytes) != *file {
            return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
        }
        if contains_credential(&file.path, &bytes) {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_CREDENTIAL_FILE"));
        }
    }
    Ok(())
}

pub fn verify_exact_manifest(path: &Path, manifest: &ProductFactorySourceManifest) -> VersionResult<()> {
    verify_manifest(path, manifest)?;
    let root = platform::Directory::open(path)?;
    let expected = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut expected_directories = BTreeSet::new();
    for file in &manifest.files {
        let mut parent = Path::new(&file.path).parent();
        while let Some(path) = parent {
            if let Some(value) = path.to_str().filter(|value| !value.is_empty()) {
                expected_directories.insert(value.to_owned());
            }
            parent = path.parent();
        }
    }
    let mut pending = vec![String::new()];
    let mut actual = BTreeSet::new();
    while let Some(parent) = pending.pop() {
        for (name, directory) in root.entries(&parent)? {
            let path = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            if directory {
                if !expected_directories.contains(&path) {
                    return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
                }
                pending.push(path);
            } else if !expected.contains(&path) || !actual.insert(path) {
                return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
            }
        }
    }
    if actual != expected {
        return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
    }
    Ok(())
}

/// The caller has already reserved this exact leaf durably. It is created once;
/// any existing file/folder makes the operation fail without overwriting it.
pub fn copy_manifest(
    source: &Path,
    target_root: &Path,
    target_relative: &str,
    manifest: &ProductFactorySourceManifest,
    sealed: bool,
) -> VersionResult<PathBuf> {
    validate_manifest(manifest)?;
    validate_relative(target_relative)?;
    let source = platform::Directory::open(source)?;
    let (root, target_root_path) = platform::Directory::prepare_root(target_root)?;
    let target = root.create_leaf(target_relative)?;
    for file in &manifest.files {
        let (bytes, executable) = source.read_file(&file.path)?;
        if file_from_bytes(file.path.clone(), &bytes) != *file {
            return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
        }
        if contains_credential(&file.path, &bytes) {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_CREDENTIAL_FILE"));
        }
        target.write_file(&file.path, &bytes, executable, sealed)?;
    }
    if sealed {
        target.seal()?;
    }
    Ok(target_root_path.join(target_relative))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod platform {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::fs::{File, Permissions};
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    pub struct Directory {
        file: File,
    }
    fn failure() -> VersionError {
        VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED")
    }
    fn name(value: &str) -> VersionResult<CString> {
        CString::new(value).map_err(|_| VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"))
    }
    fn open_at(parent: &File, value: &str, directory: bool, create: bool) -> VersionResult<File> {
        let value = name(value)?;
        let flags = libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if create {
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL
            } else {
                libc::O_RDONLY | libc::O_NONBLOCK
            }
            | if directory { libc::O_DIRECTORY } else { 0 };
        // CString is live for the call; openat returns a new owned descriptor.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), value.as_ptr(), flags, 0o600) };
        if fd < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::ELOOP) {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"));
            }
            let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
            if unsafe {
                libc::fstatat(
                    parent.as_raw_fd(),
                    value.as_ptr(),
                    metadata.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
                && unsafe { metadata.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFLNK
            {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"));
            }
            return Err(failure());
        }
        // A successful descriptor is transferred exactly once to File ownership.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    impl Directory {
        pub fn open(path: &Path) -> VersionResult<Self> {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().map_err(|_| failure())?.join(path)
            };
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
                .open("/")
                .map_err(|_| failure())?;
            let mut root = Self { file };
            for component in absolute.components() {
                match component {
                    std::path::Component::RootDir | std::path::Component::CurDir => {}
                    std::path::Component::Normal(name) => {
                        let name = name
                            .to_str()
                            .ok_or(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"))?;
                        root.file = open_at(&root.file, name, true, false)?;
                    }
                    _ => return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH")),
                }
            }
            Ok(root)
        }
        pub fn prepare_root(path: &Path) -> VersionResult<(Self, PathBuf)> {
            let parent_path = path.parent().ok_or_else(failure)?;
            let leaf = path.file_name().and_then(|name| name.to_str()).ok_or_else(failure)?;
            let parent = Self::open(parent_path)?;
            let root = parent.child(leaf, true)?;
            Ok((root, parent_path.join(leaf)))
        }
        fn child(&self, relative: &str, create: bool) -> VersionResult<Self> {
            let mut file = self.file.try_clone().map_err(|_| failure())?;
            for part in relative.split('/').filter(|part| !part.is_empty()) {
                if create {
                    let cname = name(part)?;
                    // Directory creation is anchored to the current descriptor.
                    if unsafe { libc::mkdirat(file.as_raw_fd(), cname.as_ptr(), 0o700) } < 0
                        && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                    {
                        return Err(failure());
                    }
                }
                file = open_at(&file, part, true, false)?;
            }
            Ok(Self { file })
        }
        pub fn create_leaf(&self, relative: &str) -> VersionResult<Self> {
            let path = Path::new(relative);
            let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");
            let leaf = path.file_name().and_then(|p| p.to_str()).ok_or_else(failure)?;
            let parent = self.child(parent, true)?;
            let leaf_name = name(leaf)?;
            if unsafe { libc::mkdirat(parent.file.as_raw_fd(), leaf_name.as_ptr(), 0o700) } < 0 {
                return Err(failure());
            }
            parent.file.sync_all().map_err(|_| failure())?;
            parent.child(leaf, false)
        }
        pub fn read_file(&self, relative: &str) -> VersionResult<(Vec<u8>, bool)> {
            let path = Path::new(relative);
            let parent = self.child(path.parent().and_then(|p| p.to_str()).unwrap_or(""), false)?;
            let leaf = path.file_name().and_then(|p| p.to_str()).ok_or_else(failure)?;
            let cname = name(leaf)?;
            let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
            if unsafe {
                libc::fstatat(
                    parent.file.as_raw_fd(),
                    cname.as_ptr(),
                    metadata.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } < 0
            {
                return Err(failure());
            }
            let metadata = unsafe { metadata.assume_init() };
            if metadata.st_mode & libc::S_IFMT == libc::S_IFLNK {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"));
            }
            if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"));
            }
            let mut file = open_at(
                &parent.file,
                path.file_name().and_then(|p| p.to_str()).ok_or_else(failure)?,
                false,
                false,
            )?;
            let before = file.metadata().map_err(|_| failure())?;
            if !before.is_file() || before.nlink() != 1 {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"));
            }
            if before.len() > MAX_FILE_BYTES {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| failure())?;
            let after = file.metadata().map_err(|_| failure())?;
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
            }
            if before.len() != after.len()
                || before.mtime() != after.mtime()
                || before.mtime_nsec() != after.mtime_nsec()
            {
                return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"));
            }
            Ok((bytes, before.mode() & 0o111 != 0))
        }
        pub fn write_file(&self, relative: &str, bytes: &[u8], executable: bool, sealed: bool) -> VersionResult<()> {
            let path = Path::new(relative);
            let parent = self.child(path.parent().and_then(|p| p.to_str()).unwrap_or(""), true)?;
            let mut file = open_at(
                &parent.file,
                path.file_name().and_then(|p| p.to_str()).ok_or_else(failure)?,
                false,
                true,
            )?;
            file.write_all(bytes).map_err(|_| failure())?;
            file.sync_all().map_err(|_| failure())?;
            file.set_permissions(Permissions::from_mode(if sealed {
                if executable { 0o500 } else { 0o400 }
            } else if executable {
                0o700
            } else {
                0o600
            }))
            .map_err(|_| failure())?;
            parent.file.sync_all().map_err(|_| failure())?;
            Ok(())
        }
        pub fn entries(&self, relative: &str) -> VersionResult<Vec<(String, bool)>> {
            let directory = self.child(relative, false)?;
            let fd = directory.file.try_clone().map_err(|_| failure())?.into_raw_fd();
            // fdopendir takes descriptor ownership on success only.
            let raw = unsafe { libc::fdopendir(fd) };
            if raw.is_null() {
                unsafe { libc::close(fd) };
                return Err(failure());
            }
            struct Stream(*mut libc::DIR);
            impl Drop for Stream {
                fn drop(&mut self) {
                    unsafe { libc::closedir(self.0) };
                }
            }
            let stream = Stream(raw);
            let mut names = Vec::new();
            loop {
                #[cfg(target_os = "macos")]
                let errno = unsafe { libc::__error() };
                #[cfg(target_os = "linux")]
                let errno = unsafe { libc::__errno_location() };
                unsafe { *errno = 0 };
                let entry = unsafe { libc::readdir(stream.0) };
                if entry.is_null() {
                    if unsafe { *errno } != 0 {
                        return Err(failure());
                    }
                    break;
                }
                let value = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                    .to_str()
                    .map_err(|_| VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"))?
                    .to_owned();
                if value == "." || value == ".." {
                    continue;
                }
                if excluded_component(&value) {
                    names.push((value, false));
                    continue;
                }
                let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
                let value_c = name(&value)?;
                if unsafe {
                    libc::fstatat(
                        directory.file.as_raw_fd(),
                        value_c.as_ptr(),
                        stat.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } < 0
                {
                    return Err(failure());
                }
                let stat = unsafe { stat.assume_init() };
                let kind = stat.st_mode & libc::S_IFMT;
                if kind == libc::S_IFLNK {
                    return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"));
                }
                if kind != libc::S_IFDIR && kind != libc::S_IFREG {
                    return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"));
                }
                names.push((value, kind == libc::S_IFDIR));
                if names.len() > MAX_ENTRIES {
                    return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"));
                }
            }
            names.sort();
            Ok(names)
        }
        pub fn seal(&self) -> VersionResult<()> {
            for (name, directory) in self.entries("")? {
                if directory {
                    self.child(&name, false)?.seal()?;
                }
            }
            self.file
                .set_permissions(Permissions::from_mode(0o500))
                .map_err(|_| failure())?;
            self.file.sync_all().map_err(|_| failure())?;
            Ok(())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use super::*;
    pub struct Directory;
    fn unsupported<T>() -> VersionResult<T> {
        Err(VersionError::Unavailable(
            "PRODUCT_FACTORY_VERSION_PLATFORM_UNSUPPORTED",
        ))
    }
    impl Directory {
        pub fn open(_: &Path) -> VersionResult<Self> {
            unsupported()
        }
        pub fn prepare_root(_: &Path) -> VersionResult<(Self, PathBuf)> {
            unsupported()
        }
        pub fn create_leaf(&self, _: &str) -> VersionResult<Self> {
            unsupported()
        }
        pub fn read_file(&self, _: &str) -> VersionResult<(Vec<u8>, bool)> {
            unsupported()
        }
        pub fn write_file(&self, _: &str, _: &[u8], _: bool, _: bool) -> VersionResult<()> {
            unsupported()
        }
        pub fn entries(&self, _: &str) -> VersionResult<Vec<(String, bool)>> {
            unsupported()
        }
        pub fn seal(&self) -> VersionResult<()> {
            unsupported()
        }
    }
}
