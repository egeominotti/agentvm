//! Gemini CLI's installed-app OAuth loopback flow, entirely on the Mac.
use super::super::{Session, checked_json, client, parse_session};
use super::{Flow as LoginFlow, google_client, nonce};
use crate::domain::accounts::{AccountKind, LoginStart};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
pub struct Flow {
    listener: tokio::net::TcpListener,
    state: String,
    redirect: String,
}

pub async fn start() -> Result<(LoginStart, LoginFlow), String> {
    let config = google_client::load().await?;
    let listener =
        tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|_| "cannot open Google login callback")?;
    let port = listener.local_addr().map_err(|_| "cannot inspect Google login callback")?.port();
    let redirect = format!("http://127.0.0.1:{port}/oauth2callback");
    let state = nonce()?;
    let mut url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").expect("fixed URL");
    url.query_pairs_mut().extend_pairs(&[
        ("client_id",config.client_id.as_str()),("redirect_uri",&redirect),("response_type","code"),
        ("scope","https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile"),
        ("access_type","offline"),("prompt","consent"),("state",&state),
    ]);
    Ok((
        LoginStart { id: nonce()?, verification_url: url.into(), user_code: None, expires_in: 600 },
        LoginFlow::Google(Flow { listener, state, redirect }),
    ))
}
pub async fn complete(flow: Flow) -> Result<Session, String> {
    let config = google_client::load().await?;
    tokio::time::timeout(std::time::Duration::from_secs(600),async {
        loop {
            let (mut stream,_)=flow.listener.accept().await.map_err(|_|"Google login callback failed")?;
            let mut request=String::new();
            let n={
                let mut reader=tokio::io::BufReader::new((&mut stream).take(8193));
                tokio::time::timeout(std::time::Duration::from_secs(5),reader.read_line(&mut request)).await
                    .map_err(|_|"Google callback timed out")?.map_err(|_|"Google callback read failed")?
            };
            if n>8192 {return Err("Google callback request too large".into());}
            let path=request.lines().next().and_then(|s|s.strip_prefix("GET ")).and_then(|s|s.split(' ').next()).unwrap_or("");
            let Some(code)=crate::domain::accounts::callback_code(path,&flow.state)? else {
                let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n").await;continue;
            };
            let _=stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nReturn to AgentVM to check sign-in.").await;
            drop(stream);
            let response=client()?.post("https://oauth2.googleapis.com/token").form(&[
                ("grant_type","authorization_code"),("client_id",config.client_id.as_str()),("client_secret",config.client_secret.as_str()),
                ("redirect_uri",flow.redirect.as_str()),("code",code.as_str()),
            ]).send().await.map_err(|_|"Google token exchange unavailable")?;
            return parse_session(AccountKind::Google,&checked_json(response).await?)
                .and_then(|session|if session.refresh_token.is_none(){Err("Google did not grant offline access; sign in again".into())}else{Ok(session)})
                ;
        }
    }).await.map_err(|_|"Google sign-in expired; start again")?
}
