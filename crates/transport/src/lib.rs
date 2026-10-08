//! Per-network transport for Fuselane.
//!
//! Creates sockets pinned to one network (`SO_BINDTODEVICE`, `IP_BOUND_IF`,
//! `IP_UNICAST_IF`), resolves names through that network, and builds one HTTP
//! client per network. Design: `docs/03-architecture/NETWORKING.md` §2–6.
//! Rules: L-09, L-11, L-56–L-59, L-65, L-66.
