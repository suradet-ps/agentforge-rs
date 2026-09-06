## AXUM-1. Web Services

### AXUM-1.1 Handler Shape
- Keep handlers thin: extract state → call a domain function → map to
  `Response`. Never put business logic inside an `async fn` handler body.
- Prefer `Result<T, AppError>` return types so the error path flows through
  middleware, not hand-written `match` in every handler.

### AXUM-1.2 Error Middleware
- Define one `AppError` type implementing `IntoResponse`; map domain errors
  at the service boundary (see §5.2).
- Log errors with context and a correlation id; return safe, minimal error
  bodies to the client. Never echo internal details or stack traces.

### AXUM-1.3 State & Extensions
- Store shared state (pools, config, caches) in `AppState`, not in a global
  static. Clone cheap handles, never clone pools per request.
- Use `Extension`/`State` extractors explicitly; document the ownership of
  anything stored for the lifetime of the app.

### AXUM-1.4 Routing & Middleware Order
- Register middleware in dependency order (auth → tracing → error) and
  comment why the order matters. Keep routes flat and named.
- Set timeouts and limits on every layer: request body, connection, and
  task-level timeouts for long-running work.