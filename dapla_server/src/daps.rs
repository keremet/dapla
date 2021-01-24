use std::{
    fs,
    path::{Path, PathBuf},
};

use actix_files::Files;
use actix_web::web;
pub use dapla_common::dap::access::*;
use log::error;
use serde::{Deserialize, Serialize};
use wasmer::{imports, Instance, Module, Store};

pub use self::{manager::*, service::*, settings::*};
use crate::error::ServerResult;

pub mod handler;
mod manager;
mod service;
mod settings;

type CommonDap = dapla_common::dap::Dap<PathBuf>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Dap(CommonDap);

impl Dap {
    pub fn new(name: impl Into<String>, root_dir: impl Into<PathBuf>) -> Self {
        let mut dap = Self(CommonDap::new(name.into(), root_dir.into(), Default::default()));
        if !dap.is_main() {
            if let Err(err) = dap.reload_settings() {
                error!("Error when load settings for dap '{}': {:?}", dap.name(), err);
            }
        }
        dap
    }

    pub const fn settings_file_name() -> &'static str {
        "settings.toml"
    }

    pub const fn static_dir_name() -> &'static str {
        CommonDap::static_dir_name()
    }

    pub const fn index_file_name() -> &'static str {
        CommonDap::index_file_name()
    }

    pub const fn main_name() -> &'static str {
        CommonDap::main_name()
    }

    pub fn main_static_uri() -> String {
        CommonDap::main_static_uri()
    }

    pub fn main_uri(tail: impl AsRef<str>) -> String {
        CommonDap::main_uri(tail)
    }

    pub fn is_main(&self) -> bool {
        self.0.is_main()
    }

    pub fn reload_settings(&mut self) -> DapSettingsResult<()> {
        self.0
            .set_settings(DapSettings::load(self.root_dir().join(Self::settings_file_name()))?);
        Ok(())
    }

    pub fn save_settings(&mut self) -> DapSettingsResult<()> {
        let path = self.root_dir().join(Self::settings_file_name());
        self.0.settings().save(path)
    }

    pub fn enabled(&self) -> bool {
        self.0.enabled()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.0.set_enabled(enabled);
    }

    pub fn title(&self) -> &str {
        self.0.title()
    }

    pub fn name(&self) -> &str {
        self.0.name()
    }

    pub fn root_dir(&self) -> &Path {
        self.0.root_dir()
    }

    pub fn root_uri(&self) -> String {
        self.0.root_uri()
    }

    pub fn static_uri(&self) -> String {
        self.0.static_uri()
    }

    pub fn static_dir(&self) -> PathBuf {
        self.root_dir().join(Self::static_dir_name())
    }

    pub fn index_file(&self) -> PathBuf {
        self.static_dir().join(Self::index_file_name())
    }

    pub fn server_module_file(&self) -> PathBuf {
        self.root_dir().join(&format!("{}_server.wasm", self.name()))
    }

    pub fn http_configure(&self) -> impl FnOnce(&mut web::ServiceConfig) + '_ {
        let name = self.name().to_string();
        let root_uri = self.root_uri();
        let static_uri = self.static_uri();
        let static_dir = self.static_dir();
        let is_main_client = self.is_main();

        move |config| {
            config
                .route(
                    &root_uri,
                    web::get().to({
                        let name = name.clone();
                        move |daps_service, request| handler::index_file(daps_service, request, name.clone())
                    }),
                )
                .service(Files::new(&static_uri, static_dir).index_file(Self::index_file_name()));

            if !is_main_client {
                config.service(web::scope(&root_uri).route(
                    "/*",
                    web::get().to(move |daps_service, request| handler::get(daps_service, request, name.clone())),
                ));
            }
        }
    }

    pub fn instantiate(&self) -> ServerResult<Instance> {
        let wasm = fs::read(self.server_module_file())?;

        let store = Store::default();
        let module = Module::new(&store, &wasm)?;
        let import_object = imports! {};
        Instance::new(&module, &import_object).map_err(Into::into)
    }

    pub fn update(&mut self, query: DapUpdateQuery) -> DapSettingsResult<bool> {
        let DapUpdateQuery { enabled } = query;
        if let Some(enabled) = enabled {
            if self.enabled() != enabled {
                self.set_enabled(enabled);
                self.save_settings()?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[derive(Debug, Deserialize)]
pub struct DapUpdateQuery {
    pub enabled: Option<bool>,
}
