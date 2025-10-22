use std::rc::Rc;

use deno_core::Extension;
use deno_io;
use deno_permissions::PermissionsContainer;
use deno_resolver::npm::{
    ByonmInNpmPackageChecker, ByonmNpmResolver, ByonmNpmResolverCreateOptions,
};
use jstz_runtime::runtime::JstzPermissions;
use node_resolver::{
    cache::NodeResolutionSys, ConditionsFromResolutionMode,
    DenoIsBuiltInNodeModuleChecker, IsBuiltInNodeModuleChecker, NodeResolver,
    PackageJsonResolver,
};
use sys_traits::impls::RealSys;
use deno_process;
use crate::tty::deno_tty;

pub type DefaultNodeResolverRc = node_resolver::NodeResolverRc<
    ByonmInNpmPackageChecker,
    DenyNodeBuiltins,
    ByonmNpmResolver<RealSys>,
    RealSys,
>;

pub fn create_node_resolver() -> DefaultNodeResolverRc {
    let in_npm_pkg_checker = ByonmInNpmPackageChecker;
    let is_built_in_node_module_checker = DenyNodeBuiltins;
    let sys = NodeResolutionSys::new(RealSys, None);
    let pkg_json_resolver = Rc::new(PackageJsonResolver::new(RealSys, None));
    let npm_pkg_folder_resolver = ByonmNpmResolver::new(ByonmNpmResolverCreateOptions {
        root_node_modules_dir: Some(
            "/Users/ryan-tan/workspace/jstz/node_modules"
                .try_into()
                .unwrap(),
        ),
        sys: sys.clone(),
        pkg_json_resolver: pkg_json_resolver.clone(),
    });

    let node_resolver = NodeResolver::new(
        in_npm_pkg_checker,
        is_built_in_node_module_checker,
        npm_pkg_folder_resolver,
        pkg_json_resolver.clone(),
        sys,
        ConditionsFromResolutionMode::new(Rc::new(|_res| &[])),
    );
    Rc::new(node_resolver)
}

pub fn create_deno_node_ext() -> Vec<Extension> {
    let fs = Rc::new(deno_fs::RealFs);
    vec![
        deno_tty::init_ops_and_esm(),
        deno_io::deno_io::init_ops_and_esm(Some(Default::default())),
        deno_fs::deno_fs::init_ops_and_esm::<PermissionsContainer>(fs.clone()),
        deno_process::deno_process::init_ops_and_esm(None),
        deno_node::deno_node::init_ops_and_esm::<
            PermissionsContainer,
            ByonmInNpmPackageChecker,
            ByonmNpmResolver<RealSys>,
            RealSys,
        >(None, fs.clone()),
    ]
}

#[derive(Debug)]
pub struct DenyNodeBuiltins;

impl IsBuiltInNodeModuleChecker for DenyNodeBuiltins {
    fn is_builtin_node_module(&self, _module_name: &str) -> bool {
        false
    }
}
