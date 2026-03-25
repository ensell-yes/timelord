// gRPC clients are used for optional introspection (non-hot-path).
// Hot-path auth uses local RS256 JWT verification + Redis jti denylist.
// These clients are reserved for admin tooling and introspection use cases.

pub mod auth_client;
