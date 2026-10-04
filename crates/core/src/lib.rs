//! Shared, headless OpenCollection execution engine.
#![doc = include_str!("../README.md")]

mod collection;
mod environment;
mod error;
mod history;
mod http;
mod storage;
mod workspace;

pub use collection::{
    Document, DocumentKind, LoadedRequest, load_environment, load_request, read_within,
};
pub use environment::{ExecutionContext, VariableOrigin, VariablePreview, inherited_variables};
pub use error::{Error, NetworkError, Result};
pub use history::{History, HistoryEntry};
pub use http::{HttpResponse, PreparedRequest, ResponseHeader, execute, prepare};
pub use storage::{
    FieldEdit, create_collection, create_environment, create_folder, create_request,
    create_request_from_draft, create_request_with_edits, delete_request, duplicate_request,
    rename_folder, rename_request, save_document,
};
pub use tokio_util::sync::CancellationToken;
pub use workspace::{CollectionIndex, RequestInfo, collection_index};
