//! Serde helpers for types that do not implement `serde` themselves.
//!
//! The application error taxonomy carries `std::io::ErrorKind` values,
//! which have no `Serialize`/`Deserialize` impls of their own. The wire
//! representation chosen here is the `Debug` variant name (e.g.
//! `"NotFound"`); unknown names deserialize to `ErrorKind::Other`.

use serde::{Deserialize, Deserializer, Serializer};
use std::io::ErrorKind;

/// Map an `ErrorKind` to its stable wire name (the `Debug` spelling).
pub fn name(kind: &ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "NotFound",
        ErrorKind::PermissionDenied => "PermissionDenied",
        ErrorKind::ConnectionRefused => "ConnectionRefused",
        ErrorKind::ConnectionReset => "ConnectionReset",
        ErrorKind::HostUnreachable => "HostUnreachable",
        ErrorKind::NetworkUnreachable => "NetworkUnreachable",
        ErrorKind::ConnectionAborted => "ConnectionAborted",
        ErrorKind::NotConnected => "NotConnected",
        ErrorKind::AddrInUse => "AddrInUse",
        ErrorKind::AddrNotAvailable => "AddrNotAvailable",
        ErrorKind::NetworkDown => "NetworkDown",
        ErrorKind::BrokenPipe => "BrokenPipe",
        ErrorKind::AlreadyExists => "AlreadyExists",
        ErrorKind::WouldBlock => "WouldBlock",
        ErrorKind::NotADirectory => "NotADirectory",
        ErrorKind::IsADirectory => "IsADirectory",
        ErrorKind::DirectoryNotEmpty => "DirectoryNotEmpty",
        ErrorKind::ReadOnlyFilesystem => "ReadOnlyFilesystem",
        ErrorKind::StaleNetworkFileHandle => "StaleNetworkFileHandle",
        ErrorKind::InvalidInput => "InvalidInput",
        ErrorKind::InvalidData => "InvalidData",
        ErrorKind::TimedOut => "TimedOut",
        ErrorKind::WriteZero => "WriteZero",
        ErrorKind::StorageFull => "StorageFull",
        ErrorKind::NotSeekable => "NotSeekable",
        ErrorKind::QuotaExceeded => "QuotaExceeded",
        ErrorKind::FileTooLarge => "FileTooLarge",
        ErrorKind::ResourceBusy => "ResourceBusy",
        ErrorKind::ExecutableFileBusy => "ExecutableFileBusy",
        ErrorKind::Deadlock => "Deadlock",
        ErrorKind::CrossesDevices => "CrossesDevices",
        ErrorKind::TooManyLinks => "TooManyLinks",
        ErrorKind::InvalidFilename => "InvalidFilename",
        ErrorKind::ArgumentListTooLong => "ArgumentListTooLong",
        ErrorKind::Interrupted => "Interrupted",
        ErrorKind::Unsupported => "Unsupported",
        ErrorKind::UnexpectedEof => "UnexpectedEof",
        ErrorKind::OutOfMemory => "OutOfMemory",
        // `FilesystemLoop`/`InProgress` are unstable-gated on current
        // stable; fall back to the generic name for anything unlisted.
        _ => "Other",
    }
}

/// Map a wire name back to an `ErrorKind`; unknown names fall back to
/// `ErrorKind::Other`.
pub fn from_name(name: &str) -> ErrorKind {
    match name {
        "NotFound" => ErrorKind::NotFound,
        "PermissionDenied" => ErrorKind::PermissionDenied,
        "ConnectionRefused" => ErrorKind::ConnectionRefused,
        "ConnectionReset" => ErrorKind::ConnectionReset,
        "HostUnreachable" => ErrorKind::HostUnreachable,
        "NetworkUnreachable" => ErrorKind::NetworkUnreachable,
        "ConnectionAborted" => ErrorKind::ConnectionAborted,
        "NotConnected" => ErrorKind::NotConnected,
        "AddrInUse" => ErrorKind::AddrInUse,
        "AddrNotAvailable" => ErrorKind::AddrNotAvailable,
        "NetworkDown" => ErrorKind::NetworkDown,
        "BrokenPipe" => ErrorKind::BrokenPipe,
        "AlreadyExists" => ErrorKind::AlreadyExists,
        "WouldBlock" => ErrorKind::WouldBlock,
        "NotADirectory" => ErrorKind::NotADirectory,
        "IsADirectory" => ErrorKind::IsADirectory,
        "DirectoryNotEmpty" => ErrorKind::DirectoryNotEmpty,
        "ReadOnlyFilesystem" => ErrorKind::ReadOnlyFilesystem,
        "StaleNetworkFileHandle" => ErrorKind::StaleNetworkFileHandle,
        "InvalidInput" => ErrorKind::InvalidInput,
        "InvalidData" => ErrorKind::InvalidData,
        "TimedOut" => ErrorKind::TimedOut,
        "WriteZero" => ErrorKind::WriteZero,
        "StorageFull" => ErrorKind::StorageFull,
        "NotSeekable" => ErrorKind::NotSeekable,
        "QuotaExceeded" => ErrorKind::QuotaExceeded,
        "FileTooLarge" => ErrorKind::FileTooLarge,
        "ResourceBusy" => ErrorKind::ResourceBusy,
        "ExecutableFileBusy" => ErrorKind::ExecutableFileBusy,
        "Deadlock" => ErrorKind::Deadlock,
        "CrossesDevices" => ErrorKind::CrossesDevices,
        "TooManyLinks" => ErrorKind::TooManyLinks,
        "InvalidFilename" => ErrorKind::InvalidFilename,
        "ArgumentListTooLong" => ErrorKind::ArgumentListTooLong,
        "Interrupted" => ErrorKind::Interrupted,
        "Unsupported" => ErrorKind::Unsupported,
        "UnexpectedEof" => ErrorKind::UnexpectedEof,
        "OutOfMemory" => ErrorKind::OutOfMemory,
        "Other" => ErrorKind::Other,
        _ => ErrorKind::Other,
    }
}

/// `#[serde(with = "crate::error_serde::io_kind")]` adapter for
/// `std::io::ErrorKind` fields.
pub mod io_kind {
    use super::*;

    pub fn serialize<S: Serializer>(
        kind: &ErrorKind,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(name(kind))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ErrorKind, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(from_name(&name))
    }
}
