# Security and Limits

## CSRF

CSRF protection is always on for form submissions. Do not implement custom CSRF middleware for the standard form path unless there is a framework gap.

## Request Body Limit

Requests are capped at a configurable body size. The default is 2 MiB. Requests over the limit are rejected before reaching handlers.

## Request Timeout

Requests that exceed 30 seconds return `408 Request Timeout`.

## Compression

HTML, JavaScript, and CSS responses are compressed according to the client's `Accept-Encoding` header.

## Tracing

Pilcrow installs Tower HTTP tracing. It emits output when the app configures a tracing subscriber.

## Silcrow Safety

Silcrow protects client-side surfaces through sanitization, URL protocol validation, prototype pollution blocking, same-origin live connections, and `_blank` link hardening.
