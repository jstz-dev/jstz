use std::path::Path;

use deno_core::SourceMapData;
use deno_core::{ModuleCodeString, ModuleName};

pub fn transpile_extension_ts(
    specifier: ModuleName,
    code: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), deno_error::JsErrorBox> {
    use deno_ast::{
        parse_module, EmitOptions, ImportsNotUsedAsValues, MediaType, ParseParams,
        SourceMapOption, TranspileModuleOptions, TranspileOptions,
    };

    let spec_str = specifier.as_str().to_string();
    // deno_core provides ModuleCodeString which can be borrowed as &str
    let code_str: &str = code.as_str();

    // Guess media type from the specifier string
    let media_type = if specifier.starts_with("node:") {
        MediaType::TypeScript
    } else {
        MediaType::from_path(Path::new(&specifier))
    };

    match media_type {
        MediaType::TypeScript => {}
        MediaType::JavaScript => return Ok((code, None)),
        MediaType::Mjs => return Ok((code, None)),
        MediaType::Cjs => return Ok((code, None)),
        _ => panic!(
            "Unsupported media type {specifier}",
        ),
    }

    let parsed = parse_module(ParseParams {
        specifier: spec_str.parse().unwrap(),
        text: code_str.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })
    .map_err(deno_error::JsErrorBox::from_err)?;

    let res = parsed
        .transpile(
            &TranspileOptions {
                imports_not_used_as_values: ImportsNotUsedAsValues::Remove,
                use_decorators_proposal: true,
                ..Default::default()
            },
            &TranspileModuleOptions { module_kind: None },
            &EmitOptions {
                source_map: SourceMapOption::Separate,
                inline_sources: true,
                ..Default::default()
            },
        )
        .map_err(deno_error::JsErrorBox::from_err)?;

    let out = res.into_source();
    let map = out
        .source_map
        .map(|m| deno_core::SourceMapData::from(m.into_bytes()));

    Ok((ModuleCodeString::from(out.text), map))
}
