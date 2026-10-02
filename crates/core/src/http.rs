use base64::Engine;
use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use reqwest::{
    Method, Url,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;

use crate::{
    CancellationToken, Document, DocumentKind, Error, ExecutionContext, LoadedRequest,
    NetworkError, Result, VariableOrigin, collection::reject_active, environment::Variables,
};

pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Contains resolved credentials. Intentionally neither Debug nor Serialize.
pub struct PreparedRequest {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    timeout_ms: u64,
    follow_redirects: bool,
    max_redirects: usize,
    origins: BTreeMap<String, VariableOrigin>,
}

impl PreparedRequest {
    /// Provenance contains no resolved values or secrets.
    pub fn variable_origins(&self) -> &BTreeMap<String, VariableOrigin> {
        &self.origins
    }
}

pub struct ResponseHeader {
    pub name: String,
    pub value: Vec<u8>,
}

pub struct HttpResponse {
    pub status: u16,
    /// An array, not a map: duplicate headers and non-UTF-8 values survive.
    pub headers: Vec<ResponseHeader>,
    /// Raw bytes; callers choose decoding instead of losing binary data.
    pub body: Vec<u8>,
}

pub fn prepare(loaded: &LoadedRequest, context: &ExecutionContext) -> Result<PreparedRequest> {
    let doc = &loaded.request;
    doc.validate(DocumentKind::HttpRequest)?;
    if doc.value["info"]["type"] != "http" {
        return Err(doc.error("info.type", "Se requiere una petición HTTP"));
    }
    reject_active(
        doc,
        &doc.value["runtime"],
        &["scripts", "assertions", "actions"],
    )?;
    let variables = Variables::new(loaded, context)?;
    let interpolate = |value: &Value, field: &str| -> Result<String> {
        let text = value
            .as_str()
            .ok_or_else(|| doc.error(field, "Se requiere un string"))?;
        variables
            .interpolate(text, field)
            .map_err(|error| with_path(error, doc))
    };
    let method_text = doc.value["http"]["method"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| doc.error("http.method", "Método HTTP obligatorio"))?;
    let method = Method::from_bytes(method_text.as_bytes())
        .map_err(|_| doc.error("http.method", "Método HTTP inválido"))?;
    let url_text = interpolate(&doc.value["http"]["url"], "http.url")?;
    let mut url =
        Url::parse(&url_text).map_err(|_| doc.error("http.url", "URL absoluta inválida"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(doc.error(
            "http.url",
            "Solo URLs HTTP/HTTPS absolutas están soportadas",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(doc.error(
            "http.url",
            "Usa http.auth en lugar de credenciales dentro de la URL",
        ));
    }
    if url.fragment().is_some() {
        return Err(doc.error(
            "http.url",
            "Los fragmentos no forman parte de una petición HTTP",
        ));
    }

    let mut headers = HeaderMap::new();
    let mut settings = BTreeMap::new();
    let mut auth = None;
    for defaults in &loaded.defaults {
        reject_active(
            defaults,
            &defaults.value["request"],
            &["scripts", "metadata"],
        )?;
        merge_headers(
            defaults,
            &defaults.value["request"]["headers"],
            &variables,
            &mut headers,
        )?;
        merge_settings(
            &defaults.value["request"]["settings"]["http"],
            &mut settings,
        );
        let candidate = &defaults.value["request"]["auth"];
        if !candidate.is_null() && candidate != "inherit" {
            auth = Some(candidate);
        }
    }
    merge_headers(doc, &doc.value["http"]["headers"], &variables, &mut headers)?;
    merge_settings(&doc.value["settings"], &mut settings);
    let candidate = &doc.value["http"]["auth"];
    if !candidate.is_null() && candidate != "inherit" {
        auth = Some(candidate);
    }

    for param in doc.value["http"]["params"].as_array().into_iter().flatten() {
        if param["disabled"] == true {
            continue;
        }
        if param["type"] != "query" {
            return Err(doc.error("http.params.type", "Path params aún no están soportados"));
        }
        let name = interpolate(&param["name"], "http.params.name")?;
        let value = interpolate(&param["value"], "http.params.value")?;
        url.query_pairs_mut().append_pair(&name, &value);
    }

    let mut body = None;
    let raw_body = &doc.value["http"]["body"];
    if !raw_body.is_null() {
        let content_type = match raw_body["type"].as_str() {
            Some("json") => "application/json",
            Some("text") => "text/plain; charset=utf-8",
            Some("xml") => "application/xml",
            _ => {
                return Err(doc.error(
                    "http.body",
                    "Solo body json/text/xml sin variantes está soportado",
                ));
            }
        };
        let data = interpolate(&raw_body["data"], "http.body.data")?;
        if raw_body["type"] == "json" && serde_json::from_str::<Value>(&data).is_err() {
            return Err(doc.error(
                "http.body.data",
                "JSON inválido después de resolver variables",
            ));
        }
        if !headers.contains_key(CONTENT_TYPE) {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
        }
        body = Some(data.into_bytes());
    }
    if let Some(auth) = auth {
        apply_auth(doc, auth, &variables, &mut headers, &mut url)?;
    }
    for omitted in settings
        .get("omitHeaders")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let name = HeaderName::from_bytes(omitted.as_str().unwrap().as_bytes())
            .map_err(|_| doc.error("settings.omitHeaders", "Nombre de header inválido"))?;
        headers.remove(name);
    }
    // reqwest handles framing. Accepting manual framing can create ambiguous sends.
    for name in [
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
        "upgrade",
    ] {
        if headers.contains_key(name) {
            return Err(doc.error(
                "http.headers",
                "Headers de transporte manuales aún no soportados",
            ));
        }
    }
    let timeout_ms = settings
        .get("timeout")
        .map_or(Some(30_000), |v| v.as_u64())
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            doc.error(
                "settings.timeout",
                "El timeout debe ser un entero positivo en ms",
            )
        })?;
    let max_redirects = settings
        .get("maxRedirects")
        .map_or(Some(5), |v| v.as_u64())
        .filter(|v| *v <= 20)
        .ok_or_else(|| {
            doc.error(
                "settings.maxRedirects",
                "maxRedirects debe ser un entero entre 0 y 20",
            )
        })? as usize;
    if settings.get("forwardAuthorizationHeader").copied() == Some(&Value::Bool(true)) {
        return Err(doc.error(
            "settings.forwardAuthorizationHeader",
            "Reenviar auth a otro origen no está soportado",
        ));
    }
    if settings.get("encodeUrl").copied() == Some(&Value::Bool(false)) {
        return Err(doc.error(
            "settings.encodeUrl",
            "URLs sin codificación aún no están soportadas",
        ));
    }
    Ok(PreparedRequest {
        method,
        url,
        headers,
        body,
        timeout_ms,
        follow_redirects: settings.get("followRedirects").copied() != Some(&Value::Bool(false)),
        max_redirects,
        origins: variables.origins(),
    })
}

fn with_path(error: Error, doc: &Document) -> Error {
    match error {
        Error::Config { field, message, .. } => Error::config(doc.path(), field, message),
        other => other,
    }
}

fn merge_headers(
    doc: &Document,
    rows: &Value,
    variables: &Variables,
    headers: &mut HeaderMap,
) -> Result<()> {
    let mut groups: HashMap<HeaderName, Vec<HeaderValue>> = HashMap::new();
    for row in rows.as_array().into_iter().flatten() {
        let name = HeaderName::from_bytes(row["name"].as_str().unwrap().as_bytes())
            .map_err(|_| doc.error("headers.name", "Nombre de header inválido"))?;
        let group = groups.entry(name).or_default();
        if row["disabled"] == true {
            continue;
        }
        let value = variables
            .interpolate(row["value"].as_str().unwrap(), "headers.value")
            .map_err(|e| with_path(e, doc))?;
        group.push(HeaderValue::from_str(&value).map_err(|_| {
            doc.error(
                "headers.value",
                "Valor de header inválido (incluye CR/LF u otros caracteres prohibidos)",
            )
        })?);
    }
    for (name, values) in groups {
        headers.remove(&name);
        for value in values {
            headers.append(name.clone(), value);
        }
    }
    Ok(())
}

fn merge_settings<'a>(source: &'a Value, settings: &mut BTreeMap<&'a str, &'a Value>) {
    if let Some(object) = source.as_object() {
        for (name, value) in object {
            if value != "inherit" {
                settings.insert(name.as_str(), value);
            }
        }
    }
}

fn apply_auth(
    doc: &Document,
    auth: &Value,
    variables: &Variables,
    headers: &mut HeaderMap,
    url: &mut Url,
) -> Result<()> {
    let field = |key: &str| -> Result<String> {
        let value = auth[key]
            .as_str()
            .ok_or_else(|| doc.error("http.auth", "Falta un campo requerido de autenticación"))?;
        variables
            .interpolate(value, "http.auth")
            .map_err(|e| with_path(e, doc))
    };
    match auth["type"].as_str() {
        Some("basic") => {
            if headers.contains_key(AUTHORIZATION) {
                return Err(doc.error("http.auth", "Auth choca con un header Authorization manual"));
            }
            let user = field("username")?;
            let password = field("password")?;
            if user.contains(':') {
                return Err(doc.error("http.auth.username", "Basic username no puede contener ':'"));
            }
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
            let mut value = HeaderValue::from_str(&format!("Basic {encoded}"))
                .map_err(|_| doc.error("http.auth", "Autenticación Basic inválida"))?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        Some("bearer") => {
            if headers.contains_key(AUTHORIZATION) {
                return Err(doc.error("http.auth", "Auth choca con un header Authorization manual"));
            }
            let token = field("token")?;
            if token.is_empty() {
                return Err(doc.error("http.auth.token", "Bearer token no puede estar vacío"));
            }
            let mut value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                doc.error(
                    "http.auth.token",
                    "Bearer token no es un valor de header válido",
                )
            })?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        Some("apikey") => {
            let key = field("key")?;
            let value = field("value")?;
            if key.is_empty() {
                return Err(doc.error("http.auth.key", "API key requiere un nombre"));
            }
            match auth["placement"].as_str() {
                Some("header") => {
                    let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
                        doc.error("http.auth.key", "Nombre de header API key inválido")
                    })?;
                    if headers.contains_key(&name) {
                        return Err(doc.error("http.auth", "API key choca con un header manual"));
                    }
                    let mut value = HeaderValue::from_str(&value)
                        .map_err(|_| doc.error("http.auth.value", "Valor de API key inválido"))?;
                    value.set_sensitive(true);
                    headers.insert(name, value);
                }
                Some("query") => {
                    if url.query_pairs().any(|(name, _)| name == key) {
                        return Err(
                            doc.error("http.auth", "API key choca con un query param manual")
                        );
                    }
                    url.query_pairs_mut().append_pair(&key, &value);
                }
                _ => {
                    return Err(doc.error(
                        "http.auth.placement",
                        "API key requiere placement header/query",
                    ));
                }
            }
        }
        _ => return Err(doc.error("http.auth.type", "Autenticación aún no soportada")),
    }
    Ok(())
}

/// Cancelling drops the in-flight HTTP future; it cannot undo server-side effects.
pub async fn execute(
    prepared: PreparedRequest,
    cancel: &CancellationToken,
) -> Result<HttpResponse> {
    let timeout_ms = prepared.timeout_ms;
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = tokio::time::timeout(Duration::from_millis(timeout_ms), send(prepared)) =>
            result.map_err(|_| Error::Timeout { timeout_ms })?,
    }
}

async fn send(prepared: PreparedRequest) -> Result<HttpResponse> {
    // Keep all redirects on the original origin. This also protects custom API-key
    // headers/query credentials which reqwest cannot identify as auth by name.
    let original = prepared.url.clone();
    let follow = prepared.follow_redirects;
    let limit = prepared.max_redirects;
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if !follow {
            return attempt.stop();
        }
        if attempt.url().origin() != original.origin() {
            return attempt.stop();
        }
        if attempt.previous().len() > limit {
            return attempt.error("redirect limit");
        }
        attempt.follow()
    });
    // ponytail: one client per execution supports per-request redirect settings;
    // reuse a small client pool when measured connection overhead warrants it.
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(policy)
        .build()
        .map_err(network_error)?;
    let mut request = client
        .request(prepared.method, prepared.url)
        .headers(prepared.headers);
    if let Some(body) = prepared.body {
        request = request.body(body);
    }
    let mut response = request.send().await.map_err(network_error)?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| ResponseHeader {
            name: name.as_str().to_owned(),
            value: value.as_bytes().to_vec(),
        })
        .collect();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(Error::BodyTooLarge {
            limit: MAX_RESPONSE_BYTES,
        });
    }
    // ponytail: bounded in-memory response for the initial engine; add file-backed
    // streaming before supporting bodies larger than 16 MiB or sending them over IPC.
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if chunk.len() > MAX_RESPONSE_BYTES - body.len() {
            return Err(Error::BodyTooLarge {
                limit: MAX_RESPONSE_BYTES,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn network_error(error: reqwest::Error) -> Error {
    Error::Network(if error.is_connect() {
        NetworkError::Connect
    } else if error.is_redirect() {
        NetworkError::Redirect
    } else if error.is_body() || error.is_decode() {
        NetworkError::Response
    } else {
        NetworkError::Transport
    })
}
