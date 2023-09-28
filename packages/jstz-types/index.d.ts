declare interface URLSearchParams {
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
    new(
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
    new(url: string, base?: string): URL;
    canParse(url: string, base?: string): boolean;
};

declare type BufferSource = ArrayBufferView | ArrayBuffer;

declare type BodyInit =
    | string
    | BufferSource;

declare interface Body {
    readonly bodyUsed: boolean;
    arrayBuffer(): Promise<ArrayBuffer>;
    json(): Promise<any>;
    text(): Promise<string>;
}

declare type HeadersInit = [string, string][] | Record<string, string> | Headers;

declare interface Headers {
    append(name: string, value: string): void;
    delete(name: string): void;
    get(name: string): string | string[] | null;
    has(name: string): boolean;
    set(name: string, value: string): void;
}

declare var Headers: {
    readonly prototype: Headers;
    new(init?: HeadersInit): Headers;
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
    new(input: RequestInfo, init?: RequestInit): Request;
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
    new(body?: BodyInit | null, init?: ResponseInit): Response;
    json(data: unknown): Response;
    error(): Response;
};

declare interface TextEncoder {
    atob(data: string): string;
    btoa(data: string): string;
}

declare var TextEncoder: TextEncoder

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

declare interface Ledger {
    selfAddress(): Address;
    createContract(code: String): Promise<Address>;
}

declare var Ledger: Ledger;

declare interface Contract {
    call(request: Request): Promise<Response>;
}

declare var Contract: Contract