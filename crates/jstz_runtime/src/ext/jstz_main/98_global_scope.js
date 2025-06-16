import { core, primordials } from "ext:core/mod.js";

import * as webidl from "ext:deno_webidl/00_webidl.js";
import jstzConsole from "ext:jstz_console/console.js";
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";
import * as jstzKv from "ext:jstz_kv/kv.js";

// https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { NotSupported } from "ext:jstz_main/01_errors.js";
import * as event from "ext:deno_web/02_event.js";
// import * as timers from "ext:deno_web/02_timers.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as base64 from "ext:deno_web/05_base64.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import * as file from "ext:deno_web/09_file.js";
import * as fileReader from "ext:deno_web/10_filereader.js";
import * as location from "ext:deno_web/12_location.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import * as compression from "ext:deno_web/14_compression.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as imageData from "ext:deno_web/16_image_data.js";
import * as headers from "ext:deno_fetch/20_headers.js";
import * as formData from "ext:deno_fetch/21_formdata.js";
import * as request from "ext:deno_fetch/23_request.js";
import * as response from "ext:deno_fetch/23_response.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";

let GlobalMath = Math;
GlobalMath.random = () => {
  return 0.42;
};

let NativeDate = Date;

const NOW = 1530380397121;

function JstzDate(...args) {
  if (this instanceof JstzDate) {
    if (args.length === 0) {
      // Constructor with no args ie. new Date()
      return new NativeDate(NOW);
    } else {
      return new NativeDate(...args);
    }
  } else {
    // Static constructor call ie. Date()
    return new NativeDate(NOW).toString();
  }
}

// Manually set static methods to reduce leaking inherited class. This ensures
// that `now()` and `constructor()` of NativeDate are not exposed
JstzDate.now = () => NOW;
JstzDate.parse = (...args) => NativeDate.parse(...args);
JstzDate.UTC = (...args) => NativeDate.UTC(...args);

// Forwards instance methods
JstzDate.prototype = NativeDate.prototype;
JstzDate.prototype.contructor = JstzDate;

// https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope
const workerGlobalScope = {
  AbortController: core.propNonEnumerable(abortSignal.AbortController),
  AbortSignal: core.propNonEnumerable(abortSignal.AbortSignal),
  Blob: core.propNonEnumerable(file.Blob),
  ByteLengthQueuingStrategy: core.propNonEnumerable(
    streams.ByteLengthQueuingStrategy,
  ),
  CloseEvent: core.propNonEnumerable(event.CloseEvent),
  CompressionStream: core.propNonEnumerable(compression.CompressionStream),
  CountQueuingStrategy: core.propNonEnumerable(streams.CountQueuingStrategy),
  CustomEvent: core.propNonEnumerable(event.CustomEvent),
  Date: core.propNonEnumerable(JstzDate),
  DecompressionStream: core.propNonEnumerable(compression.DecompressionStream),
  DedicatedWorkerGlobalScope:
    globalInterfaces.dedicatedWorkerGlobalScopeConstructorDescriptor,
  DOMException: core.propNonEnumerable(DOMException),
  ErrorEvent: core.propNonEnumerable(event.ErrorEvent),
  Event: core.propNonEnumerable(event.Event),
  EventTarget: core.propNonEnumerable(event.EventTarget),
  File: core.propNonEnumerable(file.File),
  FileReader: core.propNonEnumerable(fileReader.FileReader),
  FormData: core.propNonEnumerable(formData.FormData),
  Headers: core.propNonEnumerable(headers.Headers),
  ImageData: core.propNonEnumerable(imageData.ImageData),
  Math: core.propNonEnumerable(GlobalMath),
  NotSupported: core.propNonEnumerable(NotSupported),
  MessageChannel: core.propNonEnumerable(messagePort.MessageChannel),
  MessageEvent: core.propNonEnumerable(event.MessageEvent),
  MessagePort: core.propNonEnumerable(messagePort.MessagePort),
  Performance: core.propNonEnumerable(performance.Performance),
  PerformanceEntry: core.propNonEnumerable(performance.PerformanceEntry),
  PerformanceMark: core.propNonEnumerable(performance.PerformanceMark),
  PerformanceMeasure: core.propNonEnumerable(performance.PerformanceMeasure),
  PromiseRejectionEvent: core.propNonEnumerable(event.PromiseRejectionEvent),
  ProgressEvent: core.propNonEnumerable(event.ProgressEvent),
  ReadableStream: core.propNonEnumerable(streams.ReadableStream),
  ReadableStreamDefaultReader: core.propNonEnumerable(
    streams.ReadableStreamDefaultReader,
  ),
  Request: core.propNonEnumerable(request.Request),
  Response: core.propNonEnumerable(response.Response),
  TextDecoder: core.propNonEnumerable(encoding.TextDecoder),
  TextEncoder: core.propNonEnumerable(encoding.TextEncoder),
  TextDecoderStream: core.propNonEnumerable(encoding.TextDecoderStream),
  TextEncoderStream: core.propNonEnumerable(encoding.TextEncoderStream),
  TransformStream: core.propNonEnumerable(streams.TransformStream),
  URL: core.propNonEnumerable(url.URL),
  URLPattern: core.propNonEnumerable(urlPattern.URLPattern),
  WritableStream: core.propNonEnumerable(streams.WritableStream),
  WritableStreamDefaultWriter: core.propNonEnumerable(
    streams.WritableStreamDefaultWriter,
  ),
  WritableStreamDefaultController: core.propNonEnumerable(
    streams.WritableStreamDefaultController,
  ),
  ReadableByteStreamController: core.propNonEnumerable(
    streams.ReadableByteStreamController,
  ),
  ReadableStreamBYOBReader: core.propNonEnumerable(
    streams.ReadableStreamBYOBReader,
  ),
  ReadableStreamBYOBRequest: core.propNonEnumerable(
    streams.ReadableStreamBYOBRequest,
  ),
  ReadableStreamDefaultController: core.propNonEnumerable(
    streams.ReadableStreamDefaultController,
  ),
  TransformStreamDefaultController: core.propNonEnumerable(
    streams.TransformStreamDefaultController,
  ),
  WorkerGlobalScope: globalInterfaces.workerGlobalScopeConstructorDescriptor,
  WorkerLocation: location.workerLocationConstructorDescriptor,
  atob: core.propWritable(base64.atob),
  btoa: core.propWritable(base64.btoa),
  // clearInterval: core.propWritable(timers.clearInterval),
  clearInterval: core.propWritable((..._args) => {
    throw new NotSupported("'clearInterval()' is not supported");
  }),
  // clearTimeout: core.propWritable(timers.clearTimeout),
  clearTimeout: core.propWritable((..._args) => {
    throw new NotSupported("'clearTimeout()' is not supported");
  }),
  console: core.propNonEnumerable(jstzConsole),
  fetch: core.propWritable(fetch.fetch),
  location: location.workerLocationDescriptor,
  performance: core.propWritable(performance.performance),
  reportError: core.propWritable(event.reportError),
  // setInterval: core.propWritable(timers.setInterval),
  setInterval: core.propWritable((..._args) => {
    throw new NotSupported("'setInterval()' is not supported");
  }),
  // setTimeout: core.propWritable(timers.setTimeout)
  setTimeout: core.propWritable((..._args) => {
    throw new NotSupported("'setTimeout()' is not supported");
  }),
  structuredClone: core.propWritable(messagePort.structuredClone),
  [webidl.brand]: core.propNonEnumerable(webidl.brand),
  Kv: {
    value: jstzKv.Kv,
    enumerable: false,
    configurable: false,
    writable: false,
  },
};

export { workerGlobalScope };
