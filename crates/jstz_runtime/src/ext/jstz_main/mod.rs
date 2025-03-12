use deno_core::extension;

extension!(
  jstz_main,
  deps = [deno_webidl, deno_console, jstz_console, deno_url],
  esm_entry_point = "ext:jstz_main/99_main.js",
  esm = [dir "src/ext/jstz_main", "98_global_scope.js", "99_main.js"],
);
