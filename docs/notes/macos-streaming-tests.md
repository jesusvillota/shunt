# macOS streaming test failures

TASK-005 of `agy-pool` fixes two TCP fixture assumptions and an HTTP/1
streaming defect in the locked Hyper dependency. Protocol assertions and
request guards remain unchanged.

## Refused connections

`tests/responses_chain_stream.rs` previously reserved a port with a bound,
non-listening TCP socket. On macOS, this socket drops connection attempts
instead of rejecting them. The resulting timeout is not `ConnectionRefused`.

The fixture now holds both ends of an established loopback connection. Its
client-side port has no listener, so new connections are refused. Holding the
connection prevents the OS from reassigning the port. The test also checks that
an exclusive socket cannot bind the reserved port.

## Truncated response bodies

Both `a_body_error_*` responders closed their sockets without consuming the
HTTP request. On macOS, closing with unread request bytes sends a TCP reset.
The reset could discard the response frames before Hyper exposed them.

The responders now read the request headers and the complete request body
before writing the response. They still advertise 4096 response bytes and
close early. This preserves the intended truncated-body fault and all SSE
assertions, including the distinction between errors before and after
`message_stop`.

## Antigravity terminal frames

The intermittent Antigravity request timeout was an HTTP/1 flush defect in
Hyper 1.10.1. Tracing showed that the adapter killed and reaped its child,
produced the terminal frames, and reached EOF. The response observer saw all
828 bytes, while the client received only the first 521 bytes.

Hyper's write re-check could buffer the remaining 307 bytes and the chunked
terminator, then yield without flushing them or arranging another wake-up.
The client therefore waited until the existing 20-second request guard fired.

`Cargo.lock` now selects Hyper 1.11.1, which includes the upstream fix in
[Hyper PR #4143](https://github.com/hyperium/hyper/pull/4143).
See the [1.11.1 release](https://github.com/hyperium/hyper/releases/tag/v1.11.1).
Keep this fix when updating dependencies. The Antigravity fixture and its
child-cleanup assertions require no changes.
