declare interface PairIterable<K, V> {
  keys(): IterableIterator<K>;
  values(): IterableIterator<V>;
  entries(): IterableIterator<[K, V]>;
  [Symbol.iterator](): IterableIterator<[K, V]>;
  forEach(
    callback: (value: V, key: K, parent: this) => void,
    thisArg?: any,
  ): void;
}

declare interface URLSearchParams extends PairIterable<string, string> {
  append(name: string, value: string): void;
  delete(name: string, value?: string): void;
  getAll(name: string): string[];
  get(name: string): string | null;
  has(name: string, value?: string): boolean;
  set(name: string, value: string): void;
  sort(): void;
  toString(): string;
  size: number;
}

declare var URLSearchParams: {
  readonly prototype: URLSearchParams;
  new (
    init?: [string, string][] | Record<string, string> | string,
  ): URLSearchParams;
};

declare interface URL {
  hash: string;
  host: string;
  hostname: string;
  href: string;
  readonly origin: string;
  password: string;
  pathname: string;
  port: string;
  protocol: string;
  search: string;
  readonly searchParams: URLSearchParams;
  username: string;
  toString(): string;
  toJSON(): string;
}

declare var URL: {
  readonly prototype: URL;
  new (url: string, base?: string): URL;
  canParse(url: string, base?: string): boolean;
};

declare interface URLPatternInit {
  protocol?: string;
  username?: string;
  password?: string;
  hostname?: string;
  port?: string;
  pathname?: string;
  search?: string;
  hash?: string;
  baseURL?: string;
}

declare type URLPatternInput = string | URLPatternInit;

declare interface URLPatternComponentResult {
  input: string;
  groups: Record<string, string | undefined>;
}

declare interface URLPatternResult {
  inputs: URLPatternInit[];
  protocol: URLPatternComponentResult;
  username: URLPatternComponentResult;
  password: URLPatternComponentResult;
  hostname: URLPatternComponentResult;
  port: URLPatternComponentResult;
  pathname: URLPatternComponentResult;
  search: URLPatternComponentResult;
  hash: URLPatternComponentResult;
}

declare interface URLPattern {
  test(input?: URLPatternInput, baseURL?: string): boolean;
  exec(input?: URLPatternInput, baseURL?: string): URLPatternResult | null;
  readonly hash: string;
  readonly hostname: string;
  readonly password: string;
  readonly pathname: string;
  readonly port: string;
  readonly protocol: string;
  readonly search: string;
  readonly username: string;
}

declare var URLPattern: {
  readonly prototype: URLPattern;
  new (input?: URLPatternInput, baseURL?: string): URLPattern;
};

declare type BufferSource = ArrayBufferView | ArrayBuffer;

declare type BodyInit = string | BufferSource;

declare interface Body {
  readonly bodyUsed: boolean;
  arrayBuffer(): Promise<ArrayBuffer>;
  json(): Promise<any>;
  text(): Promise<string>;
}

declare type HeadersInit =
  | [string, string][]
  | Record<string, string>
  | Headers;

declare interface Headers extends PairIterable<string, string> {
  append(name: string, value: string): void;
  delete(name: string): void;
  get(name: string): string | null;
  has(name: string): boolean;
  set(name: string, value: string): void;
  getSetCookie(): string[];
}

declare var Headers: {
  readonly prototype: Headers;
  new (init?: HeadersInit): Headers;
};

declare type RequestInfo = Request | string;

declare interface RequestInit {
  body?: BodyInit | null;
  headers?: HeadersInit;
  method?: string;
}

declare interface Request extends Body {
  readonly headers: Headers;
  readonly method: string;
  readonly url: string;
}

declare var Request: {
  readonly prototype: Request;
  new (input: RequestInfo, init?: RequestInit): Request;
};

declare interface ResponseInit {
  headers?: HeadersInit;
  status?: number;
}

declare interface Response extends Body {
  readonly headers: Headers;
  readonly ok: boolean;
  readonly status: number;
  readonly statusText: string;
  readonly url: string;
}

declare var Response: {
  readonly prototype: Response;
  new (body?: BodyInit | null, init?: ResponseInit): Response;
  json(data: unknown): Response;
  error(): Response;
};

declare interface Console {
  log(...data: any[]): void;
  error(...data: any[]): void;
  debug(...data: any[]): void;
  warn(...data: any[]): void;
  info(...data: any[]): void;
  assert(condition?: boolean, ...data: any[]): void;
  group(...data: any[]): void;
  groupCollapsed(...data: any[]): void;
  groupEnd(): void;
  clear(): void;
}

declare var console: Console;

declare type Address = string;

declare interface Kv {
  get<T = unknown>(key: string): T | null;
  set(key: string, value: unknown): void;
  delete(key: string): void;
  has(key: string): boolean;
}

declare var Kv: Kv;

declare type Mutez = number;

declare interface Ledger {
  readonly selfAddress: Address;
  balance(address: Address): Mutez;
  transfer(address: Address, amount: Mutez): void;
}

declare var Ledger: Ledger;

declare interface SmartFunction {
  create(code: String): Promise<Address>;
  call(request: Request): Promise<Response>;
}

declare var SmartFunction: SmartFunction;

declare function fetch(request: Request): Promise<Response>;

declare function atob(s: string): string;
declare function btoa(s: string): string;

declare interface TextDecoderOptions {
  fatal?: boolean;
  ignoreBOM?: boolean;
}

declare interface TextDecodeOptions {
  stream?: boolean;
}

declare interface TextDecoder {
  readonly encoding: string;
  readonly fatal: boolean;
  readonly ignoreBOM: boolean;
  decode(input?: BufferSource, options?: TextDecodeOptions): string;
}

declare var TextDecoder: {
  readonly prototype: TextDecoder;
  new (label?: string, options?: TextDecoderOptions): TextDecoder;
};

declare interface TextEncoderEncodeIntoResult {
  read: number;
  written: number;
}

declare interface TextEncoder {
  readonly encoding: "utf-8";
  encode(input?: string): Uint8Array;
  encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
}

declare var TextEncoder: {
  readonly prototype: TextEncoder;
  new (): TextEncoder;
};

declare type BlobPart = BufferSource | Blob | string;

declare interface BlobPropertyBag {
  type?: string;
  endings?: "transparent" | "native";
}

declare interface Blob {
  readonly size: number;
  readonly type: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start?: number, end?: number, contentType?: string): Blob;
  text(): Promise<string>;
}

declare var Blob: {
  readonly prototype: Blob;
  new (blobParts?: BlobPart[], options?: BlobPropertyBag): Blob;
};

declare interface FilePropertyBag extends BlobPropertyBag {
  lastModified?: number;
}

declare interface File extends Blob {
  readonly lastModified: number;
  readonly name: string;
}

declare var File: {
  readonly prototype: File;
  new (fileBits: BlobPart[], fileName: string, options?: FilePropertyBag): File;
};
