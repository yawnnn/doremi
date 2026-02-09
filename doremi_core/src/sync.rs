use datetime::DateTime;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net, thread,
    time::{self, Duration, SystemTime},
};
use ureq::{
    self, RequestBuilder,
    typestate::{WithBody, WithoutBody},
    unversioned::transport,
};
use url::Url;

const URL_OAUTH_AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const IP_LOOPBACK: &str = "127.0.0.1";
const PORT_LOOPBACK: u16 = 53682;
const URL_OAUTH_DEVICE_CODE: &str = "https://oauth2.googleapis.com/device/code";
const URL_OAUTH_TOKEN: &str = "https://oauth2.googleapis.com/token";
const OAUTH_SCOPE_DRIVE_APPDATA: &str = "https://www.googleapis.com/auth/drive.appdata";
const URL_DRIVE_FILES: &str = "https://www.googleapis.com/drive/v3/files";
const URL_DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";

#[derive(Deserialize, Debug)]
struct ClientSecret {
    installed: DriveApiData,
}

#[derive(Deserialize, Debug)]
struct DriveApiData {
    #[serde(rename = "client_id")]
    id: String,
    #[serde(rename = "client_secret")]
    secret: String,
}

#[derive(Deserialize, Debug)]
struct DeviceCodeResp {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenResp {
    access_token: String,
    expires_in: u64,
    refresh_token: String,
    refresh_token_expires_in: Option<u64>,
    scope: String,
    token_type: String,
    #[serde(skip_deserializing)]
    ctime: Option<SystemTime>,
}

fn oauth_device_code(api: &DriveApiData, scope: &str) -> DeviceCodeResp {
    ureq::post(URL_OAUTH_DEVICE_CODE)
        .send_form([("client_id", api.id.as_str()), ("scope", scope)])
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap()
}

fn oauth_poll_token(api: &DriveApiData, device_code: &DeviceCodeResp) -> Result<TokenResp, String> {
    let start = time::Instant::now();

    loop {
        thread::sleep(time::Duration::new(device_code.interval, 0));

        let mut resp = ureq::post(URL_OAUTH_TOKEN).send_form([
            ("client_id", api.id.as_str()),
            ("client_secret", api.secret.as_str()),
            ("device_code", &device_code.device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ]);

        match resp {
            Ok(mut r) => {
                let mut token: TokenResp = r.body_mut().read_json().unwrap();
                token.ctime = Some(SystemTime::now());

                return Ok(token);
            }
            // Err(ureq::Error::StatusCode(400)) => {
            //     // authorization_pending, slow_down, etc.
            //     let body = r.into_string().unwrap();
            //     if body.contains("authorization_pending") {
            //         thread::sleep(Duration::from_secs(device_code.interval));
            //         continue;
            //     }
            //     return Err(body.into());
            // }
            Err(e) => {
                println!("poll without success");
                if start.elapsed().as_secs() > device_code.expires_in {
                    return Err("Waited too long".into());
                }
            }
        }
    }
}

fn oatuh_refresh_token(api: &DriveApiData, refresh_token: &str) -> TokenResp {
    ureq::post(URL_OAUTH_TOKEN)
        .send_form([
            ("client_id", api.id.as_str()),
            ("client_secret", api.secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap()
}

fn flname_client_secret() -> &'static str {
    "cache/client_secret.json"
}

fn drive_api_data() -> DriveApiData {
    let mut fl = fs::File::open(flname_client_secret()).unwrap();
    let secret: ClientSecret = serde_json::from_reader(&mut fl).unwrap();

    secret.installed
}

fn flname_token() -> &'static str {
    "cache/.token.json"
}

fn read_token() -> Option<TokenResp> {
    let mut fl = fs::File::open(flname_token()).ok()?;

    serde_json::from_reader(&mut fl).ok()
}

fn write_token(token: &TokenResp) {
    let mut fl = fs::File::create(flname_token()).unwrap();

    serde_json::to_writer(&mut fl, &token).unwrap();
}

fn mk_auth_url(api: &DriveApiData) -> String {
    let mut url = Url::parse(URL_OAUTH_AUTH).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", api.id.as_str())
        .append_pair("redirect_uri", &format!("http://{IP_LOOPBACK}:{PORT_LOOPBACK}"))
        .append_pair("response_type", "code")
        .append_pair("scope", OAUTH_SCOPE_DRIVE_APPDATA);
    // TODO: state for security

    url.to_string()
}

fn listen_for_code() -> String {
    let listener = net::TcpListener::bind((IP_LOOPBACK, PORT_LOOPBACK)).unwrap();
    println!("Listening on {IP_LOOPBACK}:{PORT_LOOPBACK}");

    // accept exactly one request
    let (mut stream, _) = listener.accept().unwrap();

    let mut buffer = vec![0u8; 4096];
    let n = stream.read(&mut buffer).unwrap();
    buffer.truncate(n);
    let req = String::from_utf8(buffer).unwrap();

    let code = req
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split('?').nth(1))
        .and_then(|qs| form_urlencoded::parse(qs.as_bytes()).find(|(k, _)| k == "code").map(|(_, v)| v))
        .unwrap();

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nYou can close this window.";
    stream.write_all(response.as_bytes()).unwrap();

    code.to_string()
    // parse GET /?code=XYZ
    // let mut url = Url::parse(IP_LOOPBACK).unwrap();
    // url.set_query(req.split_whitespace().nth(1));
    // let code = url
    //     .query_pairs()
    //     .find(|(k, _)| k == "code")
    //     .map(|(_, v)| v)
    //     .unwrap();
    //-----------
    // let code = req
    //     .split_whitespace()
    //     .nth(1)
    //     .and_then(|path| path.split('?').nth(1))
    //     .and_then(|qs| qs.split('&').find(|kv| kv.starts_with("code=")))
    //     .and_then(|kv| kv.strip_prefix("code="))
    //     .unwrap();
}

pub fn auth_user() -> Result<TokenResp, String> {
    let api = drive_api_data();

    if let Some(token) = read_token() {
        let now = time::SystemTime::now();

        if now
            .duration_since(token.ctime.unwrap())
            .unwrap_or(Duration::ZERO)
            .as_secs()
            >= token.expires_in
        {
            if let Some(expires_in) = token.refresh_token_expires_in {
                fs::remove_file(flname_token()).unwrap();

                auth_user()
            } else {
                let new_token = oatuh_refresh_token(&api, &token.refresh_token);
                write_token(&new_token);

                Ok(new_token)
            }
        } else {
            Ok(token)
        }
    } else {
        let url = mk_auth_url(&api);
        println!("{url}");
        if webbrowser::open(&url).is_err() {
            println!("Open this url in your browser: {url}");
        }

        let code = listen_for_code();

        Err(code)

        // let device_code = oauth_device_code(&api, OAUTH_SCOPE_DRIVE_APPDATA);

        // println!("{device_code:?}");

        // println!("Browse to {}", device_code.verification_url);
        // println!("Enter code '{}'", device_code.user_code);

        // let token = oauth_poll_token(&api, &device_code)?;
        // write_token(&token);

        // Ok(token)
    }
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

pub struct DriveClient {
    token: String,
}

impl DriveClient {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    fn req_get(&self, url: &str) -> ureq::RequestBuilder<WithoutBody> {
        ureq::get(url).header("Authorization", &format!("Bearer {}", self.token))
    }

    fn req_post(&self, url: &str) -> ureq::RequestBuilder<WithBody> {
        ureq::post(url).header("Authorization", &format!("Bearer {}", self.token))
    }

    pub fn req_multipart(
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

        let mut req = self
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

    pub fn list(&self) -> Result<Vec<DriveFile>, anyhow::Error> {
        let resp: ListFilesResp = self
            .req_get(URL_DRIVE_FILES)
            .query("spaces", "appDataFolder")
            .query("fields", "files(id,name,modifiedTime,size)")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(resp.files)
    }

    pub fn download(&self, file_id: &str) -> Result<Vec<u8>, ureq::Error> {
        let resp = self
            .req_get(&format!("{URL_DRIVE_FILES}/{file_id}"))
            .query("alt", "media")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(resp)
    }

    pub fn upload(&self, name: &str, payload: &[u8]) -> Result<String, ureq::Error> {
        let metadata = serde_json::json!({
            "name": name,
            "parents": ["appDataFolder"]
        });
        let (req, body) = self.req_multipart(URL_DRIVE_UPLOAD, metadata, payload);
        let resp: serde_json::Value = req.send(&body)?.body_mut().read_json()?;

        Ok(resp["id"].as_str().unwrap().to_string())
    }

    pub fn update(&self, file_id: &str, payload: &[u8]) -> Result<(), ureq::Error> {
        let metadata = serde_json::json!({});
        let (req, body) = self.req_multipart(URL_DRIVE_UPLOAD, metadata, payload);
        let _: serde_json::Value = req.send(&body)?.body_mut().read_json()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url() {
        let api = drive_api_data();
        let url = mk_auth_url(&api);
        println!("\n{url}");
        // env_logger::init();
        // println!();

        // let api = drive_api_data();
        // let drive = oauth_device_code(&api, OAUTH_SCOPE_DRIVE_APPDATA);
        // println!("{drive:?}");
    }
}
