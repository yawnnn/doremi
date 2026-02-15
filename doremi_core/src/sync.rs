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

fn secure_storage_path() -> path::PathBuf {
    path::PathBuf::from("data")
}

#[derive(Deserialize, Debug)]
struct ApiCreds {
    #[serde(rename = "client_id")]
    id: String,
    #[serde(rename = "client_secret")]
    secret: String,
    //auth_uri: String,
    //token_uri: String,
    //redirect_uris: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct ClientSecret {
    installed: ApiCreds,
}

impl ApiCreds {
    fn path() -> path::PathBuf {
        secure_storage_path().join("client_secret.json")
    }

    fn read() -> anyhow::Result<ApiCreds> {
        let mut fl = fs::File::open(Self::path())?;
        let secret: ClientSecret = serde_json::from_reader(&mut fl)?;

        Ok(secret.installed)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RespToken {
    access_token: String,
    expires_in: u64,
    scope: String,
    token_type: String,
    refresh_token: String,
    refresh_token_expires_in: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RespRefreshToken {
    access_token: String,
    expires_in: u64,
    scope: String,
    token_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleCreds {
    token: RespToken,
    token_ctime: SystemTime,
}

impl GoogleCreds {
    fn path() -> path::PathBuf {
        secure_storage_path().join("creds.json")
    }

    fn read() -> anyhow::Result<Option<GoogleCreds>> {
        let path = Self::path();
        if !fs::exists(path)? {
            return Ok(None);
        }
        let mut fl = fs::File::open(Self::path())?;
        let creds: Option<GoogleCreds> = serde_json::from_reader(&mut fl)?;

        Ok(creds)
    }

    fn write(&self) -> anyhow::Result<()> {
        let mut fl = fs::File::create(Self::path())?;
        serde_json::to_writer(&mut fl, self)?;

        Ok(())
    }

    fn delete(self) -> anyhow::Result<()> {
        fs::remove_file(Self::path())?;

        Ok(())
    }
}

fn mk_loopback_url() -> String {
    format!("http://{IP_LOOPBACK}:{PORT_LOOPBACK}")
}

fn mk_auth_url(api: &ApiCreds) -> String {
    let mut url = Url::parse(URL_OAUTH_AUTH).unwrap();

    url.query_pairs_mut()
        .append_pair("client_id", api.id.as_str())
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

fn exchange_code_for_token(api: &ApiCreds, code: String) -> anyhow::Result<RespToken> {
    let token: RespToken = ureq::post(URL_OAUTH_TOKEN)
        .send_form([
            ("code", code.as_str()),
            ("client_id", api.id.as_str()),
            ("client_secret", api.secret.as_str()),
            ("redirect_uri", mk_loopback_url().as_str()),
            ("grant_type", "authorization_code"),
        ])?
        .body_mut()
        .read_json()?;

    Ok(token)
}

fn refresh_token(google: &mut GoogleCreds, api: &ApiCreds) -> anyhow::Result<()> {
    let refresh_token: RespRefreshToken = ureq::post(URL_OAUTH_TOKEN)
        .send_form([
            ("client_id", api.id.as_str()),
            ("client_secret", api.secret.as_str()),
            ("grant_type", "authorization_code"),
            ("refresh_token", google.token.refresh_token.as_str()),
        ])?
        .body_mut()
        .read_json()?;

    let RespRefreshToken {
        access_token,
        expires_in,
        scope,
        token_type,
    } = refresh_token;

    google.token = RespToken {
        access_token,
        expires_in,
        scope,
        token_type,
        ..google.token.clone()
    };

    Ok(())
}

pub fn get_google_api_creds() -> anyhow::Result<GoogleCreds> {
    let api = ApiCreds::read()?;

    let google = match GoogleCreds::read()? {
        Some(mut google) => {
            let now = time::SystemTime::now();

            if now
                .duration_since(google.token_ctime)
                .unwrap_or(Duration::ZERO)
                .as_secs()
                >= google.token.expires_in
            {
                log::debug!("Token expired: {google:?}");

                let res = refresh_token(&mut google, &api);
                if res.is_err() {
                    log::debug!("Couldn't refresh token: {res:?}");
                    // starting over
                    google.delete()?;

                    return get_google_api_creds();
                }
            }

            google
        }
        _ => {
            let url = mk_auth_url(&api);
            if webbrowser::open(&url).is_err() {
                println!("Open this url in your browser: {url}");
            }
            let code = listen_for_code()?;
            log::debug!("code: {code}");

            let token = exchange_code_for_token(&api, code)?;

            let google = GoogleCreds {
                token,
                token_ctime: time::SystemTime::now(),
            };
            google.write()?;

            google
        }
    };

    Ok(google)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub modified_time: Option<String>,
    pub size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListFilesResp {
    files: Vec<DriveFile>,
}

impl GoogleCreds {
    fn req_get(&self, url: &str) -> ureq::RequestBuilder<WithoutBody> {
        ureq::get(url).header("Authorization", &format!("Bearer {}", self.token.access_token))
    }

    fn req_post(&self, url: &str) -> ureq::RequestBuilder<WithBody> {
        ureq::post(url).header("Authorization", &format!("Bearer {}", self.token.access_token))
    }

    fn req_multipart(
        &self,
        url: &str,
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
            .req_post(url)
            .query("uploadType", "multipart")
            .content_type(format!("multipart/related; boundary={boundary}"));

        part_init(&mut body, boundary);
        part_header(&mut body, "Content-Type", "application/json; charset=UTF-8");
        part_body(&mut body, metadata.as_str().unwrap().as_bytes());
        part_init(&mut body, boundary);
        part_header(&mut body, "Content-Type", "application/octet-stream");
        part_body(&mut body, payload);

        body.extend(format!("--{boundary}--\r\n").as_bytes());

        (req, body)
    }

    pub fn list(&self) -> anyhow::Result<Vec<DriveFile>> {
        let resp: ListFilesResp = self
            .req_get(URL_DRIVE_FILES)
            .query("spaces", "appDataFolder")
            .query("fields", "files(id,name,modifiedTime,size)")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(resp.files)
    }

    pub fn download(&self, file_id: &str) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .req_get(&format!("{URL_DRIVE_FILES}/{file_id}"))
            .query("alt", "media")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(resp)
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
}
