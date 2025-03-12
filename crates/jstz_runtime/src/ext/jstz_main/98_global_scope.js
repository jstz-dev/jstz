import { core } from "ext:core/mod.js";

import * as webidl from "ext:deno_webidl/00_webidl.js";
import jstzConsole from "ext:jstz_console/console.js";
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";
import * as jstzKv from "ext:jstz_kv/kv.js";

// https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope
const workerGlobalScope = {
  URL: core.propNonEnumerable(url.URL),
  URLPattern: core.propNonEnumerable(urlPattern.URLPattern),
  console: core.propNonEnumerable(jstzConsole),
  [webidl.brand]: core.propNonEnumerable(webidl.brand),
  Kv: {
    value: jstzKv.Kv,
    enumerable: false,
    configurable: false,
    writable: false,
  },
};

export { workerGlobalScope };
