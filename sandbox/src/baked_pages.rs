#![allow(dead_code)]

use pilcrow_web::axum::{
    extract::{Path, Query},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use pilcrow_web::{
    axum::http::{header::HeaderName, HeaderValue},
    StatusCode,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path as FsPath, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const SLOT: &str = "ticket_status";
const ROUTE: &str = "/tickets/:id";
const RENDER_LOAD_VERSION: &str = "baked-pages-poc-v3";

static TICKETS: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();

#[cfg(test)]
static TEST_BAKED_ROOT: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

type ReverseIndex = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
struct DependencyKey(String);

impl DependencyKey {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BakedSlotKind {
    Text,
    TrustedHtml,
}

impl BakedSlotKind {
    fn marker_kind(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::TrustedHtml => "html",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BakedSlot {
    name: String,
    kind: BakedSlotKind,
    dependency_keys: Vec<DependencyKey>,
}

impl BakedSlot {
    fn text(name: impl Into<String>, dependency_keys: Vec<DependencyKey>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::Text,
            dependency_keys,
        }
    }

    fn trusted_html(name: impl Into<String>, dependency_keys: Vec<DependencyKey>) -> Self {
        Self {
            name: name.into(),
            kind: BakedSlotKind::TrustedHtml,
            dependency_keys,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BakeEligibility {
    BuildTime,
    LazyOnFirstHit,
    NeverBake,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BakedArtifactMode {
    FullPage,
    FragmentComposed,
}

impl Default for BakedArtifactMode {
    fn default() -> Self {
        Self::FullPage
    }
}

#[derive(Debug, Clone)]
struct BakedRouteDeclaration {
    route_pattern: String,
    concrete_path: String,
    eligibility: BakeEligibility,
    artifact_mode: BakedArtifactMode,
    layout_key: Option<String>,
    slots: Vec<BakedSlot>,
}

impl BakedRouteDeclaration {
    fn new(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        eligibility: BakeEligibility,
    ) -> Self {
        Self {
            route_pattern: route_pattern.into(),
            concrete_path: concrete_path.into(),
            eligibility,
            artifact_mode: BakedArtifactMode::FullPage,
            layout_key: None,
            slots: Vec::new(),
        }
    }

    fn lazy_on_first_hit(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
    ) -> Self {
        Self::new(
            route_pattern,
            concrete_path,
            BakeEligibility::LazyOnFirstHit,
        )
    }

    fn build_time(route_pattern: impl Into<String>, concrete_path: impl Into<String>) -> Self {
        Self::new(route_pattern, concrete_path, BakeEligibility::BuildTime)
    }

    fn never_bake(route_pattern: impl Into<String>, concrete_path: impl Into<String>) -> Self {
        Self::new(route_pattern, concrete_path, BakeEligibility::NeverBake)
    }

    fn full_page(mut self) -> Self {
        self.artifact_mode = BakedArtifactMode::FullPage;
        self.layout_key = None;
        self
    }

    fn fragment_composed(mut self, layout_key: impl Into<String>) -> Self {
        self.artifact_mode = BakedArtifactMode::FragmentComposed;
        self.layout_key = Some(layout_key.into());
        self
    }

    fn text_slot(mut self, name: impl Into<String>, dependency_keys: Vec<DependencyKey>) -> Self {
        self.slots.push(BakedSlot::text(name, dependency_keys));
        self
    }

    fn trusted_html_slot(
        mut self,
        name: impl Into<String>,
        dependency_keys: Vec<DependencyKey>,
    ) -> Self {
        self.slots
            .push(BakedSlot::trusted_html(name, dependency_keys));
        self
    }

    fn to_page(&self, store: &BakedPageStore) -> BakedPage {
        match self.artifact_mode {
            BakedArtifactMode::FullPage => BakedPage::new(
                self.route_pattern.clone(),
                self.concrete_path.clone(),
                self.slots.clone(),
                store,
            ),
            BakedArtifactMode::FragmentComposed => BakedPage::fragment_composed(
                self.route_pattern.clone(),
                self.concrete_path.clone(),
                self.layout_key.clone().unwrap_or_default(),
                self.slots.clone(),
                store,
            ),
        }
    }

    fn slot(&self, name: &str) -> Option<&BakedSlot> {
        self.slots.iter().find(|slot| slot.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StaleState {
    stale: bool,
    reason: Option<String>,
}

impl StaleState {
    fn fresh() -> Self {
        Self {
            stale: false,
            reason: None,
        }
    }

    fn stale(reason: impl Into<String>) -> Self {
        Self {
            stale: true,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BakedPage {
    route_pattern: String,
    concrete_path: String,
    #[serde(default)]
    artifact_mode: BakedArtifactMode,
    html_path: String,
    #[serde(default)]
    body_path: String,
    metadata_path: String,
    #[serde(default)]
    layout_key: Option<String>,
    slots: Vec<BakedSlot>,
    dependency_keys: Vec<DependencyKey>,
    baked_at: u64,
    last_accessed_at: u64,
    render_load_version: String,
    stale_state: StaleState,
}

impl BakedPage {
    fn new(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        slots: Vec<BakedSlot>,
        store: &BakedPageStore,
    ) -> Self {
        let concrete_path = concrete_path.into();
        let mut dependency_keys = Vec::new();
        for slot in &slots {
            for key in &slot.dependency_keys {
                if !dependency_keys.iter().any(|existing| existing == key) {
                    dependency_keys.push(key.clone());
                }
            }
        }
        let now = unix_timestamp();
        let body_path = store
            .html_path(&concrete_path)
            .to_string_lossy()
            .to_string();
        Self {
            route_pattern: route_pattern.into(),
            artifact_mode: BakedArtifactMode::FullPage,
            html_path: body_path.clone(),
            body_path,
            metadata_path: store
                .metadata_path(&concrete_path)
                .to_string_lossy()
                .to_string(),
            concrete_path,
            layout_key: None,
            slots,
            dependency_keys,
            baked_at: now,
            last_accessed_at: now,
            render_load_version: RENDER_LOAD_VERSION.to_string(),
            stale_state: StaleState::fresh(),
        }
    }

    fn fragment_composed(
        route_pattern: impl Into<String>,
        concrete_path: impl Into<String>,
        layout_key: impl Into<String>,
        slots: Vec<BakedSlot>,
        store: &BakedPageStore,
    ) -> Self {
        let mut page = Self::new(route_pattern, concrete_path, slots, store);
        page.artifact_mode = BakedArtifactMode::FragmentComposed;
        page.layout_key = Some(layout_key.into());
        page.body_path = store
            .body_path(&page.concrete_path)
            .to_string_lossy()
            .to_string();
        page.html_path = page.body_path.clone();
        page
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BakedLayout {
    key: String,
    artifact_path: String,
    slots: Vec<BakedSlot>,
    version_hash: String,
    baked_at: u64,
}

impl BakedLayout {
    fn new(key: impl Into<String>, slots: Vec<BakedSlot>, store: &BakedPageStore) -> Self {
        let key = key.into();
        Self {
            artifact_path: store.layout_path(&key).to_string_lossy().to_string(),
            key,
            slots,
            version_hash: RENDER_LOAD_VERSION.to_string(),
            baked_at: unix_timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BakedFragment {
    key: String,
    artifact_path: String,
    slots: Vec<BakedSlot>,
    version_hash: String,
    baked_at: u64,
}

impl BakedFragment {
    fn new(key: impl Into<String>, slots: Vec<BakedSlot>, store: &BakedPageStore) -> Self {
        let key = key.into();
        Self {
            artifact_path: store.fragment_path(&key).to_string_lossy().to_string(),
            key,
            slots,
            version_hash: RENDER_LOAD_VERSION.to_string(),
            baked_at: unix_timestamp(),
        }
    }
}

struct RenderedBakedPage {
    page: BakedPage,
    html: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServeState {
    Hit,
    MissRendered,
    StaleRebaked,
    RenderedUnbaked,
}

impl ServeState {
    fn header_value(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::MissRendered => "miss-rendered",
            Self::StaleRebaked => "stale-rebaked",
            Self::RenderedUnbaked => "never-bake-rendered",
        }
    }

    fn ssr_load_header(self) -> &'static str {
        match self {
            Self::Hit => "skipped",
            Self::MissRendered | Self::StaleRebaked | Self::RenderedUnbaked => "ran",
        }
    }
}

#[derive(Clone)]
struct BakedPageStore {
    root: PathBuf,
}

impl BakedPageStore {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn default() -> Self {
        Self::new(baked_root())
    }

    fn get_or_render<F>(
        &self,
        concrete_path: &str,
        render_fn: F,
    ) -> io::Result<(String, ServeState)>
    where
        F: FnOnce(&Self) -> io::Result<RenderedBakedPage>,
    {
        self.ensure_reverse_index()?;
        let was_stale = self
            .read_page(concrete_path)?
            .map(|page| page.stale_state.stale)
            .unwrap_or(false);

        if let Some(html) = self.serve_if_fresh(concrete_path)? {
            return Ok((html, ServeState::Hit));
        }

        let rendered = render_fn(self)?;
        self.write_artifact(&rendered.page, &rendered.html)?;
        let page = self
            .read_page(&rendered.page.concrete_path)?
            .unwrap_or(rendered.page);
        let html = self.read_serving_artifact(&page)?;
        let state = if was_stale {
            ServeState::StaleRebaked
        } else {
            ServeState::MissRendered
        };
        Ok((html, state))
    }

    fn get_or_render_declared<F>(
        &self,
        declaration: &BakedRouteDeclaration,
        render_fn: F,
    ) -> io::Result<(String, ServeState)>
    where
        F: FnOnce(&Self, &BakedRouteDeclaration) -> io::Result<RenderedBakedPage>,
    {
        match declaration.eligibility {
            BakeEligibility::LazyOnFirstHit | BakeEligibility::BuildTime => {
                self.get_or_render(&declaration.concrete_path, |store| {
                    let mut rendered = render_fn(store, declaration)?;
                    rendered.page = declaration.to_page(store);
                    Ok(rendered)
                })
            }
            BakeEligibility::NeverBake => {
                let rendered = render_fn(self, declaration)?;
                Ok((rendered.html, ServeState::RenderedUnbaked))
            }
        }
    }

    fn prebake_declared<F>(
        &self,
        declaration: &BakedRouteDeclaration,
        render_fn: F,
    ) -> io::Result<String>
    where
        F: FnOnce(&Self, &BakedRouteDeclaration) -> io::Result<RenderedBakedPage>,
    {
        match declaration.eligibility {
            BakeEligibility::BuildTime => {
                self.ensure_reverse_index()?;
                let mut rendered = render_fn(self, declaration)?;
                rendered.page = declaration.to_page(self);
                let html = rendered.html.clone();
                self.write_artifact(&rendered.page, &rendered.html)?;
                Ok(html)
            }
            BakeEligibility::LazyOnFirstHit => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "lazy declarations are baked on first request, not during prebake",
            )),
            BakeEligibility::NeverBake => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "never_bake declarations cannot be prebaked",
            )),
        }
    }

    fn serve_if_fresh(&self, concrete_path: &str) -> io::Result<Option<String>> {
        let Some(page) = self.read_page(concrete_path)? else {
            return Ok(None);
        };
        if page.stale_state.stale {
            return Ok(None);
        }

        match self.read_serving_artifact(&page) {
            Ok(html) => {
                self.touch(concrete_path)?;
                Ok(Some(html))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn write_artifact(&self, page: &BakedPage, html: &str) -> io::Result<()> {
        let mut page = page.clone();
        page.body_path = match page.artifact_mode {
            BakedArtifactMode::FullPage => self.html_path(&page.concrete_path),
            BakedArtifactMode::FragmentComposed => self.body_path(&page.concrete_path),
        }
        .to_string_lossy()
        .to_string();
        page.html_path = page.body_path.clone();
        page.metadata_path = self
            .metadata_path(&page.concrete_path)
            .to_string_lossy()
            .to_string();
        page.stale_state = StaleState::fresh();
        self.write_atomic(FsPath::new(&page.body_path), html.as_bytes())?;
        self.write_page(&page)?;
        for slot in &page.slots {
            for dep in &slot.dependency_keys {
                self.add_reverse_index_entry(dep, &page.concrete_path, &slot.name)?;
            }
        }
        Ok(())
    }

    fn write_layout(&self, layout: &BakedLayout, html: &str) -> io::Result<()> {
        self.write_atomic(FsPath::new(&layout.artifact_path), html.as_bytes())
    }

    fn write_fragment(&self, fragment: &BakedFragment, html: &str) -> io::Result<()> {
        self.write_atomic(FsPath::new(&fragment.artifact_path), html.as_bytes())
    }

    fn read_serving_artifact(&self, page: &BakedPage) -> io::Result<String> {
        match page.artifact_mode {
            BakedArtifactMode::FullPage => fs::read_to_string(&page.body_path),
            BakedArtifactMode::FragmentComposed => self.compose_page(page),
        }
    }

    fn compose_page(&self, page: &BakedPage) -> io::Result<String> {
        let layout_key = page.layout_key.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "fragment-composed page is missing layout key",
            )
        })?;
        let layout = fs::read_to_string(self.layout_path(layout_key))?;
        let body = fs::read_to_string(&page.body_path)?;
        replace_slot_content(&layout, "page_body", &BakedSlotKind::TrustedHtml, &body)
    }

    fn mark_stale(&self, concrete_path: &str, reason: &str) -> io::Result<()> {
        let mut page = self.read_page(concrete_path)?.unwrap_or_else(|| BakedPage {
            route_pattern: PageShape::from_page_key(concrete_path).route().to_string(),
            concrete_path: concrete_path.to_string(),
            artifact_mode: BakedArtifactMode::FullPage,
            html_path: self.html_path(concrete_path).to_string_lossy().to_string(),
            body_path: self.html_path(concrete_path).to_string_lossy().to_string(),
            metadata_path: self
                .metadata_path(concrete_path)
                .to_string_lossy()
                .to_string(),
            layout_key: None,
            slots: Vec::new(),
            dependency_keys: Vec::new(),
            baked_at: 0,
            last_accessed_at: 0,
            render_load_version: RENDER_LOAD_VERSION.to_string(),
            stale_state: StaleState::fresh(),
        });
        page.stale_state = StaleState::stale(reason);
        self.write_page(&page)
    }

    fn clear_stale(&self, concrete_path: &str) -> io::Result<()> {
        if let Some(mut page) = self.read_page(concrete_path)? {
            page.stale_state = StaleState::fresh();
            self.write_page(&page)?;
        }
        Ok(())
    }

    fn patch_slot(
        &self,
        concrete_path: &str,
        slot: &BakedSlot,
        replacement: &str,
    ) -> io::Result<()> {
        validate_replacement(&slot.kind, replacement)?;
        let html_path = self
            .read_page(concrete_path)?
            .map(|page| PathBuf::from(page.body_path))
            .unwrap_or_else(|| self.html_path(concrete_path));
        let html = fs::read_to_string(&html_path)?;
        let patched = replace_slot_content(&html, &slot.name, &slot.kind, replacement)?;
        self.write_atomic(&html_path, patched.as_bytes())?;
        self.clear_stale(concrete_path)?;
        Ok(())
    }

    fn rebuild_reverse_index_from_metadata(&self) -> io::Result<ReverseIndex> {
        let mut index: ReverseIndex = BTreeMap::new();
        for path in self.metadata_files()? {
            let raw = fs::read_to_string(path)?;
            let page: BakedPage = serde_json::from_str(&raw)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let page = self.normalize_page(page);
            for slot in &page.slots {
                for dep in &slot.dependency_keys {
                    add_index_slot(&mut index, dep, &page.concrete_path, &slot.name);
                }
            }
        }

        Ok(index)
    }

    fn metadata_files(&self) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_metadata_files(&self.root.join("pages"), &mut files)?;
        self.collect_metadata_files(&self.metadata_dir(), &mut files)?;
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn collect_metadata_files(&self, dir: &FsPath, files: &mut Vec<PathBuf>) -> io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_metadata_files(&path, files)?;
            } else if path.file_name().and_then(|name| name.to_str()) == Some("metadata.json")
                || (path
                    .parent()
                    .map(|parent| parent == self.metadata_dir())
                    .unwrap_or(false)
                    && path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            {
                files.push(path);
            }
        }
        Ok(())
    }

    fn ensure_reverse_index(&self) -> io::Result<ReverseIndex> {
        match self.reverse_index() {
            Ok(Some(index)) => Ok(index),
            Ok(None) => {
                let index = self.rebuild_reverse_index_from_metadata()?;
                self.write_reverse_index(&index)?;
                Ok(index)
            }
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                pilcrow_web::tracing::warn!(
                    error = %err,
                    "reverse index is invalid; rebuilding from baked metadata"
                );
                let index = self.rebuild_reverse_index_from_metadata()?;
                self.write_reverse_index(&index)?;
                Ok(index)
            }
            Err(err) => Err(err),
        }
    }

    fn reverse_index_or_empty(&self) -> io::Result<ReverseIndex> {
        match self.reverse_index() {
            Ok(Some(index)) => Ok(index),
            Ok(None) => Ok(ReverseIndex::new()),
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                pilcrow_web::tracing::warn!(
                    error = %err,
                    "reverse index is invalid; replacing with a clean index"
                );
                Ok(ReverseIndex::new())
            }
            Err(err) => Err(err),
        }
    }

    fn add_reverse_index_entry(
        &self,
        dep: &DependencyKey,
        concrete_path: &str,
        slot: &str,
    ) -> io::Result<()> {
        let mut index = self.reverse_index_or_empty()?;
        add_index_slot(&mut index, dep, concrete_path, slot);
        self.write_reverse_index(&index)
    }

    fn reverse_index(&self) -> io::Result<Option<ReverseIndex>> {
        match fs::read_to_string(self.reverse_index_path()) {
            Ok(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn write_reverse_index(&self, index: &ReverseIndex) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(index).map_err(io::Error::other)?;
        self.write_atomic(&self.reverse_index_path(), &json)
    }

    fn read_page(&self, concrete_path: &str) -> io::Result<Option<BakedPage>> {
        let raw = match fs::read_to_string(self.metadata_path(concrete_path)) {
            Ok(raw) => raw,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                match fs::read_to_string(self.legacy_metadata_path(concrete_path)) {
                    Ok(raw) => raw,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(err) => return Err(err),
                }
            }
            Err(err) => return Err(err),
        };
        serde_json::from_str(&raw)
            .map(|page| Some(self.normalize_page(page)))
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    fn write_page(&self, page: &BakedPage) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(page).map_err(io::Error::other)?;
        self.write_atomic(&self.metadata_path(&page.concrete_path), &json)
    }

    fn touch(&self, concrete_path: &str) -> io::Result<()> {
        if let Some(mut page) = self.read_page(concrete_path)? {
            page.last_accessed_at = unix_timestamp();
            self.write_page(&page)?;
        }
        Ok(())
    }

    fn normalize_page(&self, mut page: BakedPage) -> BakedPage {
        if page.body_path.is_empty() {
            page.body_path = if page.html_path.is_empty() {
                match page.artifact_mode {
                    BakedArtifactMode::FullPage => self.html_path(&page.concrete_path),
                    BakedArtifactMode::FragmentComposed => self.body_path(&page.concrete_path),
                }
                .to_string_lossy()
                .to_string()
            } else {
                page.html_path.clone()
            };
        }
        if page.html_path.is_empty() {
            page.html_path = page.body_path.clone();
        }
        page.metadata_path = self
            .metadata_path(&page.concrete_path)
            .to_string_lossy()
            .to_string();
        page
    }

    fn write_atomic(&self, path: &FsPath, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp = temp_path_for(path);
        {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent() {
            if let Ok(dir) = OpenOptions::new().read(true).open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    }

    fn html_path(&self, concrete_path: &str) -> PathBuf {
        self.root.join("pages").join(storage_name(concrete_path))
    }

    fn body_path(&self, concrete_path: &str) -> PathBuf {
        self.route_dir(concrete_path).join("body.html")
    }

    fn metadata_path(&self, concrete_path: &str) -> PathBuf {
        self.route_dir(concrete_path).join("metadata.json")
    }

    fn legacy_metadata_path(&self, concrete_path: &str) -> PathBuf {
        self.metadata_dir()
            .join(storage_name(concrete_path).replace(".html", ".json"))
    }

    fn metadata_dir(&self) -> PathBuf {
        self.root.join("metadata")
    }

    fn layout_path(&self, key: &str) -> PathBuf {
        self.root
            .join("layouts")
            .join(format!("{}.html", safe_key(key)))
    }

    fn fragment_path(&self, key: &str) -> PathBuf {
        self.root
            .join("fragments")
            .join(format!("{}.html", safe_key(key)))
    }

    fn route_dir(&self, concrete_path: &str) -> PathBuf {
        let trimmed = concrete_path.trim_start_matches('/');
        if trimmed.is_empty() {
            self.root.join("pages").join("index")
        } else {
            trimmed
                .split('/')
                .fold(self.root.join("pages"), |path, segment| path.join(segment))
        }
    }

    fn reverse_index_path(&self) -> PathBuf {
        self.root.join("reverse-index.json")
    }
}

fn add_index_slot(index: &mut ReverseIndex, dep: &DependencyKey, concrete_path: &str, slot: &str) {
    let slots = index
        .entry(dep.as_str().to_string())
        .or_default()
        .entry(concrete_path.to_string())
        .or_default();
    if !slots.iter().any(|existing| existing == slot) {
        slots.push(slot.to_string());
        slots.sort();
    }
}

enum SlotValue {
    Text(String),
    TrustedHtml(TrustedHtml),
}

impl SlotValue {
    fn render_for_slot(self, slot: &BakedSlot) -> io::Result<String> {
        match (slot.kind.clone(), self) {
            (BakedSlotKind::Text, Self::Text(value)) => Ok(text_slot_content(&slot.name, &value)),
            (BakedSlotKind::TrustedHtml, Self::TrustedHtml(value)) => {
                Ok(trusted_html_slot_content(&slot.name, value))
            }
            (BakedSlotKind::Text, Self::TrustedHtml(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "trusted HTML cannot be written into a text slot",
            )),
            (BakedSlotKind::TrustedHtml, Self::Text(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "text cannot be written into a trusted HTML slot without an explicit wrapper",
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct TrustedHtml(String);

impl TrustedHtml {
    fn from_sanitized(html: String) -> Self {
        Self(html)
    }
}

type RecomputeFn = Box<dyn Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync>;

struct BakedPatchRegistry {
    store: BakedPageStore,
    recompute_fns: BTreeMap<String, RecomputeFn>,
}

impl BakedPatchRegistry {
    fn new(store: BakedPageStore) -> Self {
        Self {
            store,
            recompute_fns: BTreeMap::new(),
        }
    }

    fn register_slot_recompute<F>(&mut self, slot: impl Into<String>, recompute: F)
    where
        F: Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync + 'static,
    {
        self.recompute_fns.insert(slot.into(), Box::new(recompute));
    }

    fn register_declared_slot_recompute<F>(
        &mut self,
        declaration: &BakedRouteDeclaration,
        slot: &str,
        recompute: F,
    ) -> io::Result<()>
    where
        F: Fn(&DependencyKey, &str) -> io::Result<SlotValue> + Send + Sync + 'static,
    {
        if declaration.slot(slot).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "slot `{slot}` is not declared for route `{}`",
                    declaration.route_pattern
                ),
            ));
        }
        self.register_slot_recompute(slot, recompute);
        Ok(())
    }

    fn patch_dependency(&self, key: DependencyKey) -> io::Result<UpdateOutcome> {
        let index = self.store.ensure_reverse_index()?;
        let pages = index.get(key.as_str()).cloned().unwrap_or_default();
        let mut outcome = UpdateOutcome::default();

        for (concrete_path, slot_names) in pages {
            let Some(page) = self.store.read_page(&concrete_path)? else {
                continue;
            };

            for slot_name in slot_names {
                let Some(slot) = page.slots.iter().find(|slot| slot.name == slot_name) else {
                    self.store.mark_stale(
                        &concrete_path,
                        &format!("slot `{slot_name}` missing from baked metadata"),
                    )?;
                    outcome.stale_pages.push(concrete_path.clone());
                    continue;
                };
                let Some(recompute) = self.recompute_fns.get(&slot.name) else {
                    self.store.mark_stale(
                        &concrete_path,
                        &format!("slot `{}` has no registered recompute function", slot.name),
                    )?;
                    outcome.stale_pages.push(concrete_path.clone());
                    continue;
                };

                let replacement =
                    match recompute(&key, &concrete_path).and_then(|v| v.render_for_slot(slot)) {
                        Ok(replacement) => replacement,
                        Err(err) => {
                            self.store.mark_stale(&concrete_path, &err.to_string())?;
                            outcome.stale_pages.push(concrete_path.clone());
                            continue;
                        }
                    };

                match self.store.patch_slot(&concrete_path, slot, &replacement) {
                    Ok(()) => outcome.patched_pages.push(concrete_path.clone()),
                    Err(err) => {
                        self.store.mark_stale(&concrete_path, &err.to_string())?;
                        outcome.stale_pages.push(concrete_path.clone());
                    }
                }
            }
        }

        outcome.patched_pages.sort();
        outcome.patched_pages.dedup();
        outcome.stale_pages.sort();
        outcome.stale_pages.dedup();
        Ok(outcome)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct UpdateOutcome {
    patched_pages: Vec<String>,
    stale_pages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StatusUpdate {
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateResult {
    dependency: String,
    patched_pages: Vec<String>,
    stale_pages: Vec<String>,
    status: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/tickets/:id", get(ticket_page))
        .route("/tickets/:id/summary", get(ticket_summary_page))
        .route("/__poc/tickets/:id/status", post(update_ticket_status))
}

async fn ticket_page(Path(id): Path<String>) -> Response {
    serve_baked_page(PageShape::Detail, id).into_response()
}

async fn ticket_summary_page(Path(id): Path<String>) -> Response {
    serve_baked_page(PageShape::Summary, id).into_response()
}

async fn update_ticket_status(
    Path(id): Path<String>,
    Query(update): Query<StatusUpdate>,
) -> Response {
    let status = update.status.unwrap_or_else(|| "Closed".to_string());
    set_ticket_status(&id, &status);

    match emit_ticket_status_changed(&id) {
        Ok(outcome) => (
            StatusCode::OK,
            Json(UpdateResult {
                dependency: dependency_key(&id).as_str().to_string(),
                patched_pages: outcome.patched_pages,
                stale_pages: outcome.stale_pages,
                status,
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("baked page update failed: {err}"),
        )
            .into_response(),
    }
}

fn serve_baked_page(shape: PageShape, ticket_id: String) -> Response {
    match serve_baked_page_result(shape, &ticket_id) {
        Ok((html, state)) => baked_html_response(html, state),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("baked page failed: {err}"),
        )
            .into_response(),
    }
}

fn serve_baked_page_result(shape: PageShape, ticket_id: &str) -> io::Result<(String, ServeState)> {
    let declaration = ticket_route_declaration(shape, ticket_id);
    BakedPageStore::default().get_or_render_declared(&declaration, |store, declaration| {
        pilcrow_web::tracing::info!(
            page_key = declaration.concrete_path,
            "baked page miss/stale; running POC SSR/load"
        );
        render_ticket_baked_page(store, declaration, shape, ticket_id)
    })
}

fn baked_html_response(html: String, state: ServeState) -> Response {
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        HeaderName::from_static("x-pilcrow-baked"),
        HeaderValue::from_static(state.header_value()),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-pilcrow-ssr-load"),
        HeaderValue::from_static(state.ssr_load_header()),
    );
    response
}

fn render_ticket_baked_page(
    store: &BakedPageStore,
    declaration: &BakedRouteDeclaration,
    shape: PageShape,
    ticket_id: &str,
) -> io::Result<RenderedBakedPage> {
    let html = render_ticket_page(shape, ticket_id);
    let page = declaration.to_page(store);
    Ok(RenderedBakedPage { page, html })
}

fn rebake_page(shape: PageShape, ticket_id: &str) -> io::Result<String> {
    let store = BakedPageStore::default();
    let declaration = ticket_route_declaration(shape, ticket_id);
    let rendered = render_ticket_baked_page(&store, &declaration, shape, ticket_id)?;
    let html = rendered.html.clone();
    store.write_artifact(&rendered.page, &rendered.html)?;
    Ok(html)
}

fn ticket_route_declaration(shape: PageShape, ticket_id: &str) -> BakedRouteDeclaration {
    BakedRouteDeclaration::lazy_on_first_hit(shape.route(), shape.page_key(ticket_id))
        .text_slot(SLOT, vec![dependency_key(ticket_id)])
}

fn render_ticket_page(shape: PageShape, ticket_id: &str) -> String {
    let status = ticket_status(ticket_id);
    let slot = text_slot(SLOT, &status);
    let title = match shape {
        PageShape::Detail => format!("Ticket {ticket_id}"),
        PageShape::Summary => format!("Ticket {ticket_id} Summary"),
    };
    let body = match shape {
        PageShape::Detail => format!(
            "<h1>{}</h1><p>Status: {}</p><form method=\"post\" action=\"/__poc/tickets/{}/status?status=Closed\"><button type=\"submit\">Close ticket</button></form>",
            escape_html(&title),
            slot,
            escape_attr(ticket_id)
        ),
        PageShape::Summary => format!(
            "<h1>{}</h1><p>Compact status: {}</p>",
            escape_html(&title),
            slot
        ),
    };

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\" /><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" /><title>{}</title></head><body><nav><a href=\"/\">Home</a> <a href=\"/tickets/123\">Ticket POC</a> <a href=\"/tickets/123/summary\">Ticket Summary</a></nav><main>{}</main></body></html>",
        escape_html(&title),
        body
    )
}

fn text_slot(name: &str, value: &str) -> String {
    let slot = BakedSlot::text(name, Vec::new());
    let content = SlotValue::Text(value.to_string())
        .render_for_slot(&slot)
        .expect("text slot rendering is infallible for text values");
    format!("<!--pilcrow-slot:start {name} kind=text-->{content}<!--pilcrow-slot:end {name}-->")
}

fn text_slot_content(name: &str, value: &str) -> String {
    format!(
        "<span data-pilcrow-slot=\"{}\">{}</span>",
        escape_attr(name),
        escape_html(value)
    )
}

fn trusted_html_slot_content(_name: &str, html: TrustedHtml) -> String {
    html.0
}

fn emit_ticket_status_changed(ticket_id: &str) -> io::Result<UpdateOutcome> {
    ticket_patch_registry().patch_dependency(dependency_key(ticket_id))
}

fn ticket_patch_registry() -> BakedPatchRegistry {
    let mut registry = BakedPatchRegistry::new(BakedPageStore::default());
    let declaration = ticket_route_declaration(PageShape::Detail, "123");
    registry
        .register_declared_slot_recompute(&declaration, SLOT, |key, _page| {
            let ticket_id = ticket_id_from_dependency(key)?;
            Ok(SlotValue::Text(ticket_status(ticket_id)))
        })
        .expect("ticket status slot is declared");
    registry
}

fn ticket_id_from_dependency(key: &DependencyKey) -> io::Result<&str> {
    key.as_str()
        .strip_prefix("TicketStatus:ticket_id=")
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported ticket dependency key `{}`", key.as_str()),
            )
        })
}

fn patch_slot(
    page_key: &str,
    slot: &str,
    kind: BakedSlotKind,
    replacement: &str,
) -> io::Result<()> {
    BakedPageStore::default().patch_slot(
        page_key,
        &BakedSlot {
            name: slot.to_string(),
            kind,
            dependency_keys: Vec::new(),
        },
        replacement,
    )
}

fn validate_replacement(kind: &BakedSlotKind, replacement: &str) -> io::Result<()> {
    if replacement.contains("<!--pilcrow-slot:start")
        || replacement.contains("<!--pilcrow-slot:end")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "slot replacement must not contain Pilcrow slot markers",
        ));
    }

    if matches!(kind, BakedSlotKind::Text) && replacement.contains("<script") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "text slot replacement must be escaped text, not trusted HTML",
        ));
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct SlotBoundary {
    content_start: usize,
    content_end: usize,
}

fn find_slot_boundary(html: &str, slot: &str, kind: &BakedSlotKind) -> io::Result<SlotBoundary> {
    let start = format!(
        "<!--pilcrow-slot:start {slot} kind={}-->",
        kind.marker_kind()
    );
    let end = format!("<!--pilcrow-slot:end {slot}-->");
    let start_pos = single_marker_pos(html, &start)?;
    let end_pos = single_marker_pos(html, &end)?;

    if start_pos >= end_pos {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "slot start marker appears after end marker",
        ));
    }

    Ok(SlotBoundary {
        content_start: start_pos + start.len(),
        content_end: end_pos,
    })
}

fn single_marker_pos(html: &str, marker: &str) -> io::Result<usize> {
    let mut matches = html.match_indices(marker);
    let Some((pos, _)) = matches.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing marker {marker}"),
        ));
    };
    if matches.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("duplicate marker {marker}"),
        ));
    }
    Ok(pos)
}

fn replace_slot_content(
    html: &str,
    slot: &str,
    kind: &BakedSlotKind,
    replacement: &str,
) -> io::Result<String> {
    let boundary = find_slot_boundary(html, slot, kind)?;
    let mut patched = String::with_capacity(html.len() + replacement.len());
    patched.push_str(&html[..boundary.content_start]);
    patched.push_str(replacement);
    patched.push_str(&html[boundary.content_end..]);
    Ok(patched)
}

fn read_metadata(page_key: &str) -> io::Result<Option<BakedPage>> {
    BakedPageStore::default().read_page(page_key)
}

fn ensure_reverse_index() -> io::Result<ReverseIndex> {
    BakedPageStore::default().ensure_reverse_index()
}

fn add_reverse_index_entry(dep: &str, page_key: &str, slot: &str) -> io::Result<()> {
    BakedPageStore::default().add_reverse_index_entry(&DependencyKey::new(dep), page_key, slot)
}

fn reverse_index() -> io::Result<Option<ReverseIndex>> {
    BakedPageStore::default().reverse_index()
}

fn ticket_status(ticket_id: &str) -> String {
    let mut tickets = tickets().lock().expect("ticket store lock poisoned");
    tickets
        .entry(ticket_id.to_string())
        .or_insert_with(|| "Open".to_string())
        .clone()
}

fn set_ticket_status(ticket_id: &str, status: &str) {
    tickets()
        .lock()
        .expect("ticket store lock poisoned")
        .insert(ticket_id.to_string(), status.to_string());
}

fn tickets() -> &'static Mutex<BTreeMap<String, String>> {
    TICKETS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn dependency_key(ticket_id: &str) -> DependencyKey {
    DependencyKey::new(format!("TicketStatus:ticket_id={ticket_id}"))
}

fn temp_path_for(path: &FsPath) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("baked-artifact");
    path.with_file_name(format!(
        ".{file_name}.{}.tmp",
        unix_timestamp_nanos().unwrap_or_default()
    ))
}

fn html_path(page_key: &str) -> PathBuf {
    BakedPageStore::default().html_path(page_key)
}

fn metadata_path(page_key: &str) -> PathBuf {
    BakedPageStore::default().metadata_path(page_key)
}

fn reverse_index_path() -> PathBuf {
    BakedPageStore::default().reverse_index_path()
}

fn baked_root() -> PathBuf {
    #[cfg(test)]
    if let Some(root) = TEST_BAKED_ROOT
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("test baked root lock poisoned")
        .clone()
    {
        return root;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".pilcrow-baked")
}

fn storage_name(page_key: &str) -> String {
    let trimmed = page_key.trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        format!("{}.html", trimmed.replace('/', "__"))
    }
}

fn safe_key(key: &str) -> String {
    let key = key
        .trim_matches('/')
        .replace(['/', '\\', ':'], "__")
        .trim()
        .to_string();
    if key.is_empty() {
        "default".to_string()
    } else {
        key
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn unix_timestamp_nanos() -> io::Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(io::Error::other)
}

#[derive(Debug, Clone, Copy)]
enum PageShape {
    Detail,
    Summary,
}

impl PageShape {
    fn page_key(self, ticket_id: &str) -> String {
        match self {
            Self::Detail => format!("/tickets/{ticket_id}"),
            Self::Summary => format!("/tickets/{ticket_id}/summary"),
        }
    }

    fn route(self) -> &'static str {
        match self {
            Self::Detail => ROUTE,
            Self::Summary => "/tickets/:id/summary",
        }
    }

    fn from_page_key(page_key: &str) -> Self {
        if page_key.ends_with("/summary") {
            Self::Summary
        } else {
            Self::Detail
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TestRoot {
        path: PathBuf,
        _guard: MutexGuard<'static, ()>,
    }

    impl TestRoot {
        fn new(name: &str) -> Self {
            let guard = TEST_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("test lock poisoned");
            let path = std::env::temp_dir().join(format!(
                "pilcrow-baked-{name}-{}",
                unix_timestamp_nanos().unwrap_or_default()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test baked root");
            *TEST_BAKED_ROOT
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("test root lock poisoned") = Some(path.clone());
            tickets().lock().expect("ticket lock poisoned").clear();
            Self {
                path,
                _guard: guard,
            }
        }

        fn store(&self) -> BakedPageStore {
            BakedPageStore::new(self.path.clone())
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            *TEST_BAKED_ROOT
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("test root lock poisoned") = None;
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn response_header(response: &Response, name: &str) -> String {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn miss_bakes_page() {
        let _root = TestRoot::new("miss");

        let response = serve_baked_page(PageShape::Detail, "123".to_string());

        assert_eq!(
            response_header(&response, "x-pilcrow-baked"),
            "miss-rendered"
        );
        assert_eq!(response_header(&response, "x-pilcrow-ssr-load"), "ran");
        assert!(html_path("/tickets/123").exists());
        assert!(metadata_path("/tickets/123").exists());
    }

    #[test]
    fn hit_skips_ssr_load() {
        let _root = TestRoot::new("hit");
        let _ = serve_baked_page(PageShape::Detail, "123".to_string());

        let response = serve_baked_page(PageShape::Detail, "123".to_string());

        assert_eq!(response_header(&response, "x-pilcrow-baked"), "hit");
        assert_eq!(response_header(&response, "x-pilcrow-ssr-load"), "skipped");
    }

    #[test]
    fn mutation_patches_one_page() {
        let _root = TestRoot::new("one-page");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        set_ticket_status("123", "Resolved");

        let outcome = emit_ticket_status_changed("123").expect("emit dependency");
        let html = fs::read_to_string(html_path("/tickets/123")).expect("read baked html");

        assert_eq!(outcome.patched_pages, vec!["/tickets/123"]);
        assert!(outcome.stale_pages.is_empty());
        assert!(html.contains(">Resolved<"));
    }

    #[test]
    fn mutation_patches_two_pages() {
        let _root = TestRoot::new("two-pages");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        rebake_page(PageShape::Summary, "123").expect("rebake summary");
        set_ticket_status("123", "Resolved");

        let outcome = emit_ticket_status_changed("123").expect("emit dependency");
        let detail = fs::read_to_string(html_path("/tickets/123")).expect("read detail");
        let summary = fs::read_to_string(html_path("/tickets/123/summary")).expect("read summary");

        assert_eq!(
            outcome.patched_pages,
            vec!["/tickets/123", "/tickets/123/summary"]
        );
        assert!(detail.contains(">Resolved<"));
        assert!(summary.contains(">Resolved<"));
    }

    #[test]
    fn missing_marker_marks_stale_and_next_get_rebakes() {
        let _root = TestRoot::new("missing-marker");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        fs::write(html_path("/tickets/123"), "<html>broken</html>").expect("corrupt html");
        set_ticket_status("123", "Resolved");

        let outcome = emit_ticket_status_changed("123").expect("emit dependency");
        let metadata = read_metadata("/tickets/123")
            .expect("read metadata")
            .expect("metadata exists");
        let response = serve_baked_page(PageShape::Detail, "123".to_string());
        let html = fs::read_to_string(html_path("/tickets/123")).expect("read rebaked html");

        assert_eq!(outcome.stale_pages, vec!["/tickets/123"]);
        assert!(metadata.stale_state.stale);
        assert_eq!(
            response_header(&response, "x-pilcrow-baked"),
            "stale-rebaked"
        );
        assert!(html.contains(">Resolved<"));
    }

    #[test]
    fn duplicate_marker_refuses_patch() {
        let _root = TestRoot::new("duplicate-marker");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        let html = fs::read_to_string(html_path("/tickets/123")).expect("read html");
        fs::write(html_path("/tickets/123"), format!("{html}{html}")).expect("duplicate html");

        let err = patch_slot(
            "/tickets/123",
            SLOT,
            BakedSlotKind::Text,
            &text_slot_content(SLOT, "Resolved"),
        )
        .expect_err("duplicate marker must fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("duplicate marker"));
    }

    #[test]
    fn malformed_marker_order_refuses_patch() {
        let _root = TestRoot::new("malformed-marker");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        fs::write(
            html_path("/tickets/123"),
            "<!--pilcrow-slot:end ticket_status--><span>Open</span><!--pilcrow-slot:start ticket_status kind=text-->",
        )
        .expect("write malformed html");

        let err = patch_slot(
            "/tickets/123",
            SLOT,
            BakedSlotKind::Text,
            &text_slot_content(SLOT, "Resolved"),
        )
        .expect_err("malformed marker order must fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("after end marker"));
    }

    #[test]
    fn text_slot_escapes_html() {
        let _root = TestRoot::new("escape");
        set_ticket_status("123", "<script>alert('x')</script>");

        let html = rebake_page(PageShape::Detail, "123").expect("rebake detail");

        assert!(html.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert"));
    }

    #[test]
    fn atomic_temp_file_is_not_served() {
        let _root = TestRoot::new("atomic");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        fs::write(html_path("/tickets/123").with_extension("tmp"), "TMP ONLY").expect("write tmp");

        let (html, state) =
            serve_baked_page_result(PageShape::Detail, "123").expect("serve baked page");

        assert_eq!(state, ServeState::Hit);
        assert!(!html.contains("TMP ONLY"));
        assert!(html.contains("Ticket 123"));
    }

    #[test]
    fn reverse_index_rebuilds_from_metadata_when_missing() {
        let _root = TestRoot::new("rebuild-index");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        fs::remove_file(reverse_index_path()).expect("remove reverse index");

        let index = ensure_reverse_index().expect("rebuild index");
        let slots = index
            .get("TicketStatus:ticket_id=123")
            .and_then(|pages| pages.get("/tickets/123"))
            .expect("rebuilt target");

        assert_eq!(slots, &vec![SLOT.to_string()]);
    }

    #[test]
    fn reverse_index_avoids_duplicate_entries() {
        let _root = TestRoot::new("dedupe-index");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        add_reverse_index_entry("TicketStatus:ticket_id=123", "/tickets/123", SLOT)
            .expect("add duplicate index entry");

        let index = reverse_index()
            .expect("read reverse index")
            .expect("reverse index exists");
        let slots = index
            .get("TicketStatus:ticket_id=123")
            .and_then(|pages| pages.get("/tickets/123"))
            .expect("indexed slots");

        assert_eq!(slots, &vec![SLOT.to_string()]);
    }

    #[test]
    fn old_reverse_index_shape_rebuilds_instead_of_500() {
        let _root = TestRoot::new("old-index-shape");
        rebake_page(PageShape::Detail, "123").expect("rebake detail");
        fs::write(
            reverse_index_path(),
            r#"{
  "TicketStatus:ticket_id=123": [
    { "page_key": "/tickets/123", "slot": "ticket_status" }
  ]
}"#,
        )
        .expect("write old reverse index shape");

        let response = serve_baked_page(PageShape::Detail, "123".to_string());
        let index = reverse_index()
            .expect("read rebuilt reverse index")
            .expect("reverse index exists");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_header(&response, "x-pilcrow-baked"), "hit");
        assert!(index
            .get("TicketStatus:ticket_id=123")
            .and_then(|pages| pages.get("/tickets/123"))
            .is_some());
    }

    #[test]
    fn store_get_or_render_is_reusable_boundary() {
        let root = TestRoot::new("store-api");
        let store = root.store();
        let dep = DependencyKey::new("Example:1");
        let slot = BakedSlot::text("status", vec![dep]);

        let (html, first_state) = store
            .get_or_render("/example/1", |store| {
                let page = BakedPage::new("/example/:id", "/example/1", vec![slot], store);
                Ok(RenderedBakedPage {
                    page,
                    html: "<!--pilcrow-slot:start status kind=text--><span data-pilcrow-slot=\"status\">Open</span><!--pilcrow-slot:end status-->".to_string(),
                })
            })
            .expect("render first page");
        let (cached_html, second_state) = store
            .get_or_render("/example/1", |_store| panic!("cache hit must not render"))
            .expect("serve cached page");

        assert_eq!(first_state, ServeState::MissRendered);
        assert_eq!(second_state, ServeState::Hit);
        assert_eq!(html, cached_html);
    }

    #[test]
    fn declaration_layer_declares_lazy_text_slot_dep_and_recompute() {
        let root = TestRoot::new("declaration-api");
        let store = root.store();
        let dep = DependencyKey::new("DeclaredStatus:id=1");
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/declared/:id", "/declared/1")
            .text_slot("status", vec![dep.clone()]);

        let (html, first_state) = store
            .get_or_render_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: declaration.to_page(store),
                    html: text_slot("status", "Open"),
                })
            })
            .expect("render declared route");
        let (cached, second_state) = store
            .get_or_render_declared(&declaration, |_store, _declaration| {
                panic!("lazy baked hit must not render")
            })
            .expect("serve declared route from baked html");

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry
            .register_declared_slot_recompute(&declaration, "status", |_key, _path| {
                Ok(SlotValue::Text("Closed".to_string()))
            })
            .expect("declared slot accepts recompute");
        let outcome = registry.patch_dependency(dep).expect("patch declared dep");
        let patched = fs::read_to_string(store.html_path("/declared/1")).expect("read patched");

        assert_eq!(declaration.eligibility, BakeEligibility::LazyOnFirstHit);
        assert_eq!(declaration.slots[0].kind, BakedSlotKind::Text);
        assert_eq!(
            declaration.slots[0].dependency_keys[0].as_str(),
            "DeclaredStatus:id=1"
        );
        assert_eq!(first_state, ServeState::MissRendered);
        assert_eq!(second_state, ServeState::Hit);
        assert_eq!(html, cached);
        assert_eq!(outcome.patched_pages, vec!["/declared/1"]);
        assert!(patched.contains(">Closed<"));
    }

    #[test]
    fn declaration_defaults_to_full_page_and_can_make_mode_explicit() {
        let default = BakedRouteDeclaration::lazy_on_first_hit("/default/:id", "/default/1");
        let explicit =
            BakedRouteDeclaration::lazy_on_first_hit("/explicit/:id", "/explicit/1").full_page();

        assert_eq!(default.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(default.layout_key, None);
        assert_eq!(explicit.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(explicit.layout_key, None);
    }

    #[test]
    fn declaration_can_select_fragment_composed_layout() {
        let declaration = BakedRouteDeclaration::lazy_on_first_hit("/composed/:id", "/composed/1")
            .fragment_composed("app")
            .text_slot("status", vec![DependencyKey::new("ComposedStatus:id=1")]);

        assert_eq!(
            declaration.artifact_mode,
            BakedArtifactMode::FragmentComposed
        );
        assert_eq!(declaration.layout_key.as_deref(), Some("app"));
    }

    #[test]
    fn declared_fragment_composed_route_uses_layout_and_body_strategy() {
        let root = TestRoot::new("declared-composed");
        let store = root.store();
        let dep = DependencyKey::new("DeclaredComposedStatus:id=1");
        let declaration = BakedRouteDeclaration::lazy_on_first_hit(
            "/declared-composed/:id",
            "/declared-composed/1",
        )
        .fragment_composed("app")
        .text_slot("status", vec![dep.clone()]);
        let layout = BakedLayout::new(
            "app",
            vec![BakedSlot::trusted_html("page_body", Vec::new())],
            &store,
        );
        let layout_v1 = "<!doctype html><html><body><header>Declared layout v1</header><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>";
        let layout_v2 = "<!doctype html><html><body><header>Declared layout v2</header><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>";
        store
            .write_layout(&layout, layout_v1)
            .expect("write declared layout");

        let (html_v1, first_state) = store
            .get_or_render_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: BakedPage::new(
                        declaration.route_pattern.clone(),
                        declaration.concrete_path.clone(),
                        declaration.slots.clone(),
                        store,
                    ),
                    html: format!(
                        "<main><h1>Declared composed</h1>{}</main>",
                        text_slot("status", "Open")
                    ),
                })
            })
            .expect("render declared composed page");
        let metadata = store
            .read_page("/declared-composed/1")
            .expect("read metadata")
            .expect("metadata exists");
        let body_before =
            fs::read_to_string(store.body_path("/declared-composed/1")).expect("read body");

        store
            .write_layout(&layout, layout_v2)
            .expect("rebake declared layout");
        let (html_v2, second_state) = store
            .get_or_render_declared(&declaration, |_store, _declaration| {
                panic!("declared composed hit must not rerun SSR/load")
            })
            .expect("serve declared composed hit");
        let body_after =
            fs::read_to_string(store.body_path("/declared-composed/1")).expect("read body");

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry
            .register_declared_slot_recompute(&declaration, "status", |_key, _path| {
                Ok(SlotValue::Text("Patched".to_string()))
            })
            .expect("register declared composed recompute");
        let outcome = registry.patch_dependency(dep).expect("patch composed dep");
        let (patched_html, patched_state) = store
            .get_or_render_declared(&declaration, |_store, _declaration| {
                panic!("patched declared composed page should still be baked")
            })
            .expect("serve patched declared composed page");

        assert_eq!(first_state, ServeState::MissRendered);
        assert_eq!(second_state, ServeState::Hit);
        assert_eq!(patched_state, ServeState::Hit);
        assert_eq!(metadata.artifact_mode, BakedArtifactMode::FragmentComposed);
        assert_eq!(metadata.layout_key.as_deref(), Some("app"));
        assert!(store.body_path("/declared-composed/1").exists());
        assert!(store.metadata_path("/declared-composed/1").exists());
        assert!(html_v1.contains("Declared layout v1"));
        assert!(html_v1.contains(">Open<"));
        assert!(html_v2.contains("Declared layout v2"));
        assert!(!html_v2.contains("Declared layout v1"));
        assert_eq!(body_before, body_after);
        assert_eq!(outcome.patched_pages, vec!["/declared-composed/1"]);
        assert!(outcome.stale_pages.is_empty());
        assert!(patched_html.contains("Declared layout v2"));
        assert!(patched_html.contains(">Patched<"));
    }

    #[test]
    fn build_time_declaration_prebakes_before_request() {
        let root = TestRoot::new("build-time-prebake");
        let store = root.store();
        let dep = DependencyKey::new("BuildStatus:id=1");
        let declaration = BakedRouteDeclaration::build_time("/build/:id", "/build/1")
            .text_slot("status", vec![dep.clone()]);

        let html = store
            .prebake_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: declaration.to_page(store),
                    html: text_slot("status", "Prebaked"),
                })
            })
            .expect("prebake build-time page");
        let metadata = store
            .read_page("/build/1")
            .expect("read build-time metadata")
            .expect("metadata exists");
        let index = store
            .reverse_index()
            .expect("read reverse index")
            .expect("reverse index exists");

        assert_eq!(html, text_slot("status", "Prebaked"));
        assert!(store.html_path("/build/1").exists());
        assert_eq!(metadata.route_pattern, "/build/:id");
        assert_eq!(metadata.concrete_path, "/build/1");
        assert_eq!(metadata.slots[0].name, "status");
        assert_eq!(metadata.dependency_keys, vec![dep.clone()]);
        assert_eq!(
            index
                .get(dep.as_str())
                .and_then(|pages| pages.get("/build/1")),
            Some(&vec!["status".to_string()])
        );
    }

    #[test]
    fn build_time_prebaked_first_get_is_hit_and_skips_ssr_load() {
        let root = TestRoot::new("build-time-hit");
        let store = root.store();
        let declaration = BakedRouteDeclaration::build_time("/build/:id", "/build/1")
            .text_slot("status", vec![DependencyKey::new("BuildStatus:id=1")]);

        store
            .prebake_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: declaration.to_page(store),
                    html: text_slot("status", "Prebaked"),
                })
            })
            .expect("prebake build-time page");

        let (html, state) = store
            .get_or_render_declared(&declaration, |_store, _declaration| {
                panic!("prebaked hit must not run SSR/load")
            })
            .expect("serve prebaked page");

        assert_eq!(state, ServeState::Hit);
        assert_eq!(state.ssr_load_header(), "skipped");
        assert!(html.contains(">Prebaked<"));
    }

    #[test]
    fn dependency_patching_updates_build_time_baked_page() {
        let root = TestRoot::new("build-time-patch");
        let store = root.store();
        let dep = DependencyKey::new("BuildStatus:id=1");
        let declaration = BakedRouteDeclaration::build_time("/build/:id", "/build/1")
            .text_slot("status", vec![dep.clone()]);

        store
            .prebake_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: declaration.to_page(store),
                    html: text_slot("status", "Prebaked"),
                })
            })
            .expect("prebake build-time page");

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry
            .register_declared_slot_recompute(&declaration, "status", |_key, _path| {
                Ok(SlotValue::Text("Patched".to_string()))
            })
            .expect("register build-time recompute");
        let outcome = registry
            .patch_dependency(dep)
            .expect("patch build-time dep");
        let patched = fs::read_to_string(store.html_path("/build/1")).expect("read patched");

        assert_eq!(outcome.patched_pages, vec!["/build/1"]);
        assert!(outcome.stale_pages.is_empty());
        assert!(patched.contains(">Patched<"));
    }

    #[test]
    fn fragment_composed_page_uses_shared_layout_without_rebaking_body() {
        let root = TestRoot::new("fragment-composed");
        let store = root.store();
        let dep = DependencyKey::new("ComposedStatus:id=1");
        let page = BakedPage::fragment_composed(
            "/composed/:id",
            "/composed/1",
            "app",
            vec![BakedSlot::text("status", vec![dep.clone()])],
            &store,
        );
        let layout = BakedLayout::new(
            "app",
            vec![BakedSlot::trusted_html("page_body", Vec::new())],
            &store,
        );
        let header = BakedFragment::new("header", Vec::new(), &store);
        let layout_v1 = "<!doctype html><html><body><header>Layout v1</header><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>";
        let layout_v2 = "<!doctype html><html><body><header>Layout v2</header><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>";
        let body = format!(
            "<main><h1>Composed</h1>{}</main>",
            text_slot("status", "Open")
        );

        store
            .write_layout(&layout, layout_v1)
            .expect("write layout");
        store
            .write_fragment(&header, "<header>Shared header</header>")
            .expect("write shared fragment");
        store.write_artifact(&page, &body).expect("write body");

        let (html_v1, state_v1) = store
            .get_or_render("/composed/1", |_store| {
                panic!("composed hit must not rerun body renderer")
            })
            .expect("compose first response");
        let body_before = fs::read_to_string(store.body_path("/composed/1")).expect("read body");

        store
            .write_layout(&layout, layout_v2)
            .expect("rebake shared layout");
        let (html_v2, state_v2) = store
            .get_or_render("/composed/1", |_store| {
                panic!("layout-only change must not rebake page body")
            })
            .expect("compose changed layout");
        let body_after = fs::read_to_string(store.body_path("/composed/1")).expect("read body");

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _path| {
            Ok(SlotValue::Text("Patched".to_string()))
        });
        let outcome = registry.patch_dependency(dep).expect("patch body slot");
        let (patched_html, patched_state) = store
            .get_or_render("/composed/1", |_store| {
                panic!("patched composed page should still serve baked body")
            })
            .expect("compose patched body");

        assert_eq!(page.artifact_mode, BakedArtifactMode::FragmentComposed);
        assert_eq!(state_v1, ServeState::Hit);
        assert_eq!(state_v2, ServeState::Hit);
        assert_eq!(patched_state, ServeState::Hit);
        assert!(store.body_path("/composed/1").exists());
        assert!(store.fragment_path("header").exists());
        assert!(store.metadata_path("/composed/1").exists());
        assert!(html_v1.contains("Layout v1"));
        assert!(html_v1.contains(">Open<"));
        assert!(html_v2.contains("Layout v2"));
        assert!(!html_v2.contains("Layout v1"));
        assert_eq!(body_before, body_after);
        assert_eq!(outcome.patched_pages, vec!["/composed/1"]);
        assert!(outcome.stale_pages.is_empty());
        assert!(patched_html.contains("Layout v2"));
        assert!(patched_html.contains(">Patched<"));
    }

    #[test]
    fn never_bake_refuses_prebake_and_baked_serving() {
        let root = TestRoot::new("never-bake-refuses");
        let store = root.store();
        let declaration = BakedRouteDeclaration::never_bake("/preview/:id", "/preview/1")
            .text_slot("status", vec![DependencyKey::new("PreviewStatus:id=1")]);

        let err = store
            .prebake_declared(&declaration, |_store, _declaration| {
                panic!("never_bake prebake must not render")
            })
            .expect_err("never_bake must refuse prebake");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(!store.html_path("/preview/1").exists());
        assert!(!store.metadata_path("/preview/1").exists());

        let stale_page = declaration.to_page(&store);
        store
            .write_artifact(&stale_page, text_slot("status", "Cached").as_str())
            .expect("seed cached artifact");
        let (html, state) = store
            .get_or_render_declared(&declaration, |store, declaration| {
                Ok(RenderedBakedPage {
                    page: declaration.to_page(store),
                    html: text_slot("status", "Rendered"),
                })
            })
            .expect("serve never_bake declaration");

        assert_eq!(state, ServeState::RenderedUnbaked);
        assert_eq!(state.ssr_load_header(), "ran");
        assert!(html.contains(">Rendered<"));
        assert!(!html.contains(">Cached<"));
    }

    #[test]
    fn declaration_layer_supports_policy_and_trusted_html_declarations() {
        let build = BakedRouteDeclaration::build_time("/build/:id", "/build/1")
            .full_page()
            .text_slot("status", vec![DependencyKey::new("Build:1")]);
        let never = BakedRouteDeclaration::never_bake("/preview/:id", "/preview/1");
        let html = BakedRouteDeclaration::lazy_on_first_hit("/html/:id", "/html/1")
            .fragment_composed("app")
            .trusted_html_slot("body", vec![DependencyKey::new("Html:1")]);

        assert_eq!(build.eligibility, BakeEligibility::BuildTime);
        assert_eq!(build.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(never.eligibility, BakeEligibility::NeverBake);
        assert_eq!(never.artifact_mode, BakedArtifactMode::FullPage);
        assert_eq!(html.layout_key.as_deref(), Some("app"));
        assert_eq!(html.slots[0].kind, BakedSlotKind::TrustedHtml);
    }

    #[test]
    fn patch_registry_recomputes_explicit_slot() {
        let root = TestRoot::new("patch-registry-api");
        let store = root.store();
        let dep = DependencyKey::new("ExampleStatus:id=1");
        let slot = BakedSlot::text("status", vec![dep.clone()]);
        let page = BakedPage::new("/example/:id", "/example/1", vec![slot], &store);
        store
            .write_artifact(
                &page,
                "<!--pilcrow-slot:start status kind=text--><span data-pilcrow-slot=\"status\">Open</span><!--pilcrow-slot:end status-->",
            )
            .expect("write page");

        let mut registry = BakedPatchRegistry::new(store.clone());
        registry.register_slot_recompute("status", |_key, _path| {
            Ok(SlotValue::Text("<Closed>".to_string()))
        });

        let outcome = registry.patch_dependency(dep).expect("patch dependency");
        let html = fs::read_to_string(store.html_path("/example/1")).expect("read patched html");

        assert_eq!(outcome.patched_pages, vec!["/example/1"]);
        assert!(outcome.stale_pages.is_empty());
        assert!(html.contains("&lt;Closed&gt;"));
        assert!(!html.contains("<Closed>"));
    }

    #[test]
    fn trusted_html_requires_explicit_wrapper() {
        let html_slot = BakedSlot {
            name: "body".to_string(),
            kind: BakedSlotKind::TrustedHtml,
            dependency_keys: vec![DependencyKey::new("Body:1")],
        };

        let trusted = SlotValue::TrustedHtml(TrustedHtml::from_sanitized(
            "<strong>ok</strong>".to_string(),
        ))
        .render_for_slot(&html_slot)
        .expect("trusted html renders");
        let plain_text = SlotValue::Text("<strong>no</strong>".to_string())
            .render_for_slot(&html_slot)
            .expect_err("plain text cannot render into html slot");

        assert_eq!(trusted, "<strong>ok</strong>");
        assert_eq!(plain_text.kind(), io::ErrorKind::InvalidInput);
    }
}
