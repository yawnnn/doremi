use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net, path,
    time::{self, Duration, SystemTime},
};
use ureq::{
    self,
    typestate::{WithBody, WithoutBody},
};
use url::Url;

// TODO: some of these are provided in the client_secret or google's responses
const URL_OAUTH_AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const IP_LOOPBACK: &str = "127.0.0.1";
const PORT_LOOPBACK: u16 = 53682;
const URL_OAUTH_TOKEN: &str = "https://oauth2.googleapis.com/token";
const OAUTH_SCOPE_DRIVE_APPDATA: &str = "https://www.googleapis.com/auth/drive.appdata";
const URL_DRIVE_FILES: &str = "https://www.googleapis.com/drive/v3/files";
const URL_DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";

#[derive(Deserialize, Debug)]
struct ApiKeys {
    #[serde(rename = "client_id")]
    id: String,
    #[serde(rename = "client_secret")]
    secret: String,
    //auth_uri: String,
    //token_uri: String,
    //redirect_uris: Vec<String>,
}

impl ApiKeys {
    fn new<P: AsRef<path::Path>>(api_data_dir: &P) -> anyhow::Result<ApiKeys> {
        #[derive(Deserialize, Debug)]
        struct ClientSecret {
            installed: ApiKeys,
        }

        let mut fl = fs::File::open(api_data_dir.as_ref().join("client_secret.json"))?;
        let secret: ClientSecret = serde_json::from_reader(&mut fl)?;

        Ok(secret.installed)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TokenData {
    access_token: String,
    expires_in: u64,
    scope: String,
    token_type: String,
    refresh_token: String,
    refresh_token_expires_in: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    token: TokenData,
    token_ctime: SystemTime,
}

impl Credentials {
    fn flname<P: AsRef<path::Path>>(api_data_dir: &P) -> path::PathBuf {
        api_data_dir.as_ref().join("creds.json")
    }

    fn read<P: AsRef<path::Path>>(api_data_dir: &P) -> anyhow::Result<Option<Credentials>> {
        let path = Self::flname(api_data_dir);
        if !fs::exists(&path)? {
            return Ok(None);
        }
        let mut fl = fs::File::open(&path)?;
        let creds: Option<Credentials> = serde_json::from_reader(&mut fl)?;

        Ok(creds)
    }

    fn write<P: AsRef<path::Path>>(&self, api_data_dir: &P) -> anyhow::Result<()> {
        let mut fl = fs::File::create(Self::flname(api_data_dir))?;
        serde_json::to_writer(&mut fl, self)?;

        Ok(())
    }
}

fn mk_loopback_url() -> String {
    format!("http://{IP_LOOPBACK}:{PORT_LOOPBACK}")
}

fn mk_auth_url(api_keys: &ApiKeys) -> String {
    let mut url = Url::parse(URL_OAUTH_AUTH).unwrap();

    url.query_pairs_mut()
        .append_pair("client_id", api_keys.id.as_str())
        .append_pair("redirect_uri", mk_loopback_url().as_str())
        .append_pair("response_type", "code")
        .append_pair("scope", OAUTH_SCOPE_DRIVE_APPDATA);
    // TODO: state for security

    url.to_string()
}

fn listen_for_code() -> anyhow::Result<String> {
    // accept exactly one request
    let listener = net::TcpListener::bind((IP_LOOPBACK, PORT_LOOPBACK))?;
    log::debug!("Listening on {IP_LOOPBACK}:{PORT_LOOPBACK}");

    let (mut stream, _) = listener.accept()?;

    // TODO: better
    let mut buf = vec![0u8; 4096];
    let read = stream.read(&mut buf)?; // no read_to_string, cause that will wait for EOF and hang indefinitely
    assert!(read < buf.len()); // 4096 should have been more that enough
    let resp = str::from_utf8(&buf[..read])?;

    // eg: GET /?code=<code>&scope=<scope> HTTP/1.1
    let code = resp
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split('?').nth(1))
        .and_then(|qs| {
            form_urlencoded::parse(qs.as_bytes())
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v)
        })
        .context("parse device code")?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nYou can close this window.";
    stream.write_all(response.as_bytes())?;

    Ok(code.to_string())
}

fn exchange_code_for_token(api_keys: &ApiKeys, code: String) -> anyhow::Result<TokenData> {
    let token: TokenData = ureq::post(URL_OAUTH_TOKEN)
        .send_form([
            ("code", code.as_str()),
            ("client_id", api_keys.id.as_str()),
            ("client_secret", api_keys.secret.as_str()),
            ("redirect_uri", mk_loopback_url().as_str()),
            ("grant_type", "authorization_code"),
        ])?
        .body_mut()
        .read_json()?;

    Ok(token)
}

fn refresh_token(google: &mut Credentials, api_keys: &ApiKeys) -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, Debug)]
    pub struct RefreshTokenData {
        access_token: String,
        expires_in: u64,
        scope: String,
        token_type: String,
    }

    let refresh_token: RefreshTokenData = ureq::post(URL_OAUTH_TOKEN)
        .send_form([
            ("client_id", api_keys.id.as_str()),
            ("client_secret", api_keys.secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", google.token.refresh_token.as_str()),
        ])?
        .body_mut()
        .read_json()?;

    let RefreshTokenData {
        access_token,
        expires_in,
        scope,
        token_type,
    } = refresh_token;

    google.token = TokenData {
        access_token,
        expires_in,
        scope,
        token_type,
        ..google.token.clone()
    };
    google.token_ctime = time::SystemTime::now();

    Ok(())
}

fn request_credentials(api_keys: &ApiKeys) -> anyhow::Result<Credentials> {
    let url = mk_auth_url(api_keys);
    if webbrowser::open(&url).is_err() {
        println!("Open this url in your browser: {url}");
    }
    let code = listen_for_code()?;
    log::debug!("code: {code}");

    let token = exchange_code_for_token(api_keys, code)?;

    let creds = Credentials {
        token,
        token_ctime: time::SystemTime::now(),
    };

    Ok(creds)
}

fn refresh_credentials(api_keys: &ApiKeys, creds: &mut Credentials) -> anyhow::Result<()> {
    let now = time::SystemTime::now();

    if let Some(refresh_token_expires_in) = creds.token.refresh_token_expires_in
        && now
            .duration_since(creds.token_ctime)
            .unwrap_or(Duration::ZERO)
            .as_secs()
            >= refresh_token_expires_in
    {
        log::debug!("Refresh token expired");
        *creds = request_credentials(api_keys)?;
    } else {
        let res = refresh_token(creds, api_keys);
        if let Err(e) = res {
            log::debug!("Couldn't refresh token: {e:?}");
            *creds = request_credentials(api_keys)?;
        }
    }

    Ok(())
}

fn read_or_request_credentials<P: AsRef<path::Path>>(
    api_data_dir: &P,
) -> anyhow::Result<Credentials> {
    let api_keys = ApiKeys::new(api_data_dir)?;

    let creds = match Credentials::read(api_data_dir)? {
        Some(mut creds) => {
            let now = time::SystemTime::now();

            if now
                .duration_since(creds.token_ctime)
                .unwrap_or(Duration::ZERO)
                .as_secs()
                >= creds.token.expires_in
            {
                refresh_credentials(&api_keys, &mut creds)?;
            }

            creds
        }
        _ => request_credentials(&api_keys)?,
    };

    creds.write(api_data_dir)?;

    Ok(creds)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub modified_time: Option<String>,
    pub size: Option<String>,
}

pub struct DriveApi(Credentials);

impl DriveApi {
    pub fn new<P: AsRef<path::Path>>(api_data_dir: &P) -> anyhow::Result<Self> {
        read_or_request_credentials(api_data_dir).map(DriveApi)
    }

    fn req_get(&self, uri: &str) -> ureq::RequestBuilder<WithoutBody> {
        ureq::get(uri).header(
            "Authorization",
            &format!("Bearer {}", self.0.token.access_token),
        )
    }

    fn req_post(&self, uri: &str) -> ureq::RequestBuilder<WithBody> {
        ureq::post(uri).header(
            "Authorization",
            &format!("Bearer {}", self.0.token.access_token),
        )
    }

    fn req_delete(&self, uri: &str) -> ureq::RequestBuilder<WithoutBody> {
        ureq::delete(uri).header(
            "Authorization",
            &format!("Bearer {}", self.0.token.access_token),
        )
    }

    fn req_multipart(
        &self,
        uri: &str,
        metadata: serde_json::Value,
        payload: &[u8],
    ) -> (ureq::RequestBuilder<WithBody>, Vec<u8>) {
        fn part_init(body: &mut Vec<u8>, boundary: &str) {
            body.extend(format!("--{boundary}\r\n").as_bytes());
        }

        fn part_header(body: &mut Vec<u8>, name: &str, value: &str) {
            body.extend(format!("{name}: {value}\r\n").as_bytes());
        }

        fn part_body(body: &mut Vec<u8>, payload: &[u8]) {
            body.extend(b"\r\n");
            body.extend(payload);
            body.extend(b"\r\n");
        }

        let boundary = "BOUNDARY1234567890";
        let mut body = Vec::new();

        let req = self
            .req_post(uri)
            .query("uploadType", "multipart")
            .content_type(format!("multipart/related; boundary={boundary}"));

        part_init(&mut body, boundary);
        part_header(&mut body, "Content-Type", "application/json; charset=UTF-8");
        part_body(&mut body, serde_json::to_vec(&metadata).unwrap().as_slice());
        part_init(&mut body, boundary);
        part_header(&mut body, "Content-Type", "application/octet-stream");
        part_body(&mut body, payload);

        body.extend(format!("--{boundary}--\r\n").as_bytes());

        (req, body)
    }

    pub fn list(&self) -> anyhow::Result<Vec<DriveFile>> {
        #[derive(Debug, Deserialize)]
        pub struct FileList {
            pub files: Vec<DriveFile>,
        }

        let resp: FileList = self
            .req_get(URL_DRIVE_FILES)
            .query("spaces", "appDataFolder")
            .query("fields", "files(id,name,modifiedTime,size)")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(resp.files)
    }

    pub fn download(&self, file_id: &str) -> anyhow::Result<Vec<u8>> {
        let contents = self
            .req_get(&format!("{URL_DRIVE_FILES}/{file_id}"))
            .query("alt", "media")
            .call()?
            .body_mut()
            .read_to_vec()?;

        Ok(contents)
    }

    pub fn upload(&self, name: &str, payload: &[u8]) -> anyhow::Result<String> {
        let metadata = serde_json::json!({
            "name": name,
            "parents": ["appDataFolder"]
        });
        let (req, body) = self.req_multipart(URL_DRIVE_UPLOAD, metadata, payload);
        let resp: serde_json::Value = req.send(&body)?.body_mut().read_json()?;

        Ok(resp["id"].as_str().unwrap().to_string())
    }

    pub fn update(&self, _file_id: &str, payload: &[u8]) -> anyhow::Result<()> {
        let metadata = serde_json::json!({});
        let (req, body) = self.req_multipart(URL_DRIVE_UPLOAD, metadata, payload);
        let _: serde_json::Value = req.send(&body)?.body_mut().read_json()?;

        Ok(())
    }

    pub fn delete(&self, file_id: &str) -> anyhow::Result<()> {
        self.req_delete(&format!("{URL_DRIVE_FILES}/{file_id}"))
            .call()?; // 204 No Content

        Ok(())
    }
}
