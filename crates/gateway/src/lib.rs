// SPDX-License-Identifier: Apache-2.0 OR MIT
//! # gateway
//!
//! Protocol/utility modules surfaced by `gateway-tools`. Each module is a
//! minimal but functional implementation sufficient to drive the CLI's
//! `--demo` paths and the test suite. A future commit can replace any
//! module's body with a full, audited implementation; the public surface
//! is intended to stay stable.

pub mod chunked_transfer;
pub mod dns_message_parser;
pub mod histogram_metrics;
pub mod json_patch;
pub mod jwt_hs256;
pub mod m3u_parser;
pub mod pem_codec;
pub mod pkcs7_padding;
pub mod prometheus_exposition;
pub mod redis_resp;
pub mod tls_record;
