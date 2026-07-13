# Architecture

## Layer 1: transport core

`Client` owns immutable configuration, a reqwest client and a `TokenManager`. `ApiRequest` carries HTTP method, path/query variables, headers, body and access-token semantics. The transport:

1. renders and percent-encodes path variables;
2. obtains app or tenant tokens from a pluggable cache;
3. applies request-level and client-level headers;
4. serializes JSON, form, bytes or multipart bodies;
5. captures request IDs from response headers;
6. converts non-zero OpenAPI codes to structured errors;
7. invalidates managed tokens and retries once for official invalid-token codes.

## Layer 2: typed services

Typed services are lightweight views borrowing `Client`. They only build `ApiRequest` values and delegate transport behavior to the core. This prevents generated API modules from duplicating auth, retry and serialization logic.

The first typed service is `im.v1.message.create`. Future generated services follow the same pattern.

## Scene modules

Scene modules implement workflows that are not ordinary authenticated OpenAPI calls:

- `registration`: device-code application creation and polling;
- `event`: callback signature verification, AES decryption and payload parsing.

They are feature-gated so minimal clients can exclude them.

## Compatibility strategy

The project aligns observable behavior with the official SDKs, while exposing Rust-native APIs:

- builders instead of functional options;
- enums for token and status types;
- async functions and cancellation by dropping futures;
- trait-based token cache;
- typed errors rather than string matching;
- borrowed service handles instead of a large self-referential client tree.
