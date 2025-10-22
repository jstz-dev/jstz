use deno_core::{
    error::ModuleLoaderError, futures::FutureExt, FsModuleLoader, ModuleLoadResponse,
    ModuleLoader, ModuleResolutionError, ModuleSource, ModuleSourceCode, ModuleSpecifier,
    ModuleType, RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use resolver::DefaultNodeResolverRc;
use url::{ParseError, Url};

pub mod resolver;
mod tty;
pub mod transpile;
/// - Loads modules from file system
/// - Will reload modules if they changed
pub struct MemoModuleLoader {
    pub node_resolver: DefaultNodeResolverRc,
}

impl ModuleLoader for MemoModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        println!("Resolving specifier {} with base {}", specifier, referrer);
        let specifier =
            resolve_import(specifier, referrer, kind, self.node_resolver.clone())?;
        if specifier.scheme() != "file" {
            return Err(ModuleLoaderError::Unsupported {
                specifier: Box::new(specifier),
                maybe_referrer: None,
            });
        }
        Ok(specifier)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        if requested_module_type != RequestedModuleType::None {
            return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Unsupported {
                specifier: Box::new(module_specifier.clone()),
                maybe_referrer: maybe_referrer.map(|r| Box::new(r.clone())),
            }));
        }
        match module_specifier.to_file_path() {
            Err(_) => ModuleLoadResponse::Sync(Err(ModuleLoaderError::Unsupported {
                specifier: Box::new(module_specifier.clone()),
                maybe_referrer: maybe_referrer.map(|r| Box::new(r.clone())),
            })),
            Ok(filepath) => {
                let specifier = module_specifier.clone();
                let filepath = filepath.clone();
                let fut = async move {
                    tokio::fs::read_to_string(filepath)
                        .await
                        .map(|source| {
                            let module = ModuleSource::new(
                                ModuleType::JavaScript,
                                ModuleSourceCode::String(source.into()),
                                &specifier,
                                None,
                            );
                            Ok(module)
                        })
                        .map_err(|err| {
                            JsErrorBox::from_err(LoadFailedError {
                                specifier: specifier.clone(),
                                source: err,
                            })
                        })?
                }
                .boxed();
                ModuleLoadResponse::Async(fut)
            }
        }
    }
}

pub fn resolve_import(
    specifier: &str,
    base: &str,
    _resolution_kind: ResolutionKind,
    _node_resolver: DefaultNodeResolverRc,
) -> Result<ModuleSpecifier, ModuleResolutionError> {
    println!("Resolving {specifier} with base {base}");
    let url = match Url::parse(specifier) {
        Ok(url) => url,
        Err(ParseError::RelativeUrlWithoutBase)
            if !(specifier.starts_with('/')
                || specifier.starts_with("./")
                || specifier.starts_with("../")) =>
        {
            // let resolution_mode = match resolution_kind {
            //     ResolutionKind::MainModule => node_resolver::ResolutionMode::Import,
            //     ResolutionKind::Import => node_resolver::ResolutionMode::Import,
            //     ResolutionKind::DynamicImport => node_resolver::ResolutionMode::Require,
            // };
            // let referrer =
            //     Url::parse(base).map_err(ModuleResolutionError::InvalidBaseUrl)?;
            // let node_module = node_resolver
            //     .resolve(
            //         specifier,
            //         &referrer,
            //         resolution_mode,
            //         NodeResolutionKind::Execution,
            //     )
            //     .unwrap()
            //     .into_url()
            //     .unwrap();
            // node_module
            return Err(ModuleResolutionError::ImportPrefixMissing {
                specifier: specifier.to_string(),
                maybe_referrer: Some(base.to_string()),
            })
        }
        Err(ParseError::RelativeUrlWithoutBase) => {
            let base = Url::parse(base).map_err(ModuleResolutionError::InvalidBaseUrl)?;
            base.join(specifier)
                .map_err(ModuleResolutionError::InvalidBaseUrl)?
        }
        Err(err) => return Err(ModuleResolutionError::InvalidBaseUrl(err)),
    };

    Ok(url)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Failed to load {specifier}")]
pub struct LoadFailedError {
    specifier: ModuleSpecifier,
    #[source]
    #[inherit]
    source: std::io::Error,
}

#[cfg(test)]
mod test {}
