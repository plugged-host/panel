use gix::{
    protocol::transport::client::{
        AuthenticationRequired, TransportWithoutIO,
        blocking_io::{
            Transport,
            http::{self, GetResponse, Http, PostBodyDataKind, PostResponse},
        },
    },
    utils::io::pipe,
};
use std::{
    io::{Read, Write},
    str::FromStr,
    sync::{Arc, OnceLock},
};

type BoxedTransport = Box<dyn Transport + Send>;
type GitConnection<'a, 'b, 'c> = gix::remote::Connection<'a, 'b, 'c, BoxedTransport>;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

struct GitHttp {
    env: Arc<crate::env::Env>,
    client: &'static reqwest::blocking::Client,
}

impl GitHttp {
    fn new(env: &Arc<crate::env::Env>) -> Self {
        Self {
            env: Arc::clone(env),
            client: HTTP_CLIENT.get_or_init(|| {
                reqwest::blocking::Client::builder()
                    .user_agent(format!("github.com/calagopus/panel {}", crate::VERSION))
                    .timeout(std::time::Duration::from_secs(60))
                    .connect_timeout(std::time::Duration::from_secs(20))
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .dns_resolver(Arc::new(crate::net::BlockedIpResolver::new(env, "git")))
                    .build()
                    .expect("Failed to create git HTTP client")
            }),
        }
    }

    fn request(
        &mut self,
        url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
        body_kind: Option<PostBodyDataKind>,
    ) -> gix::Result<PostResponse<pipe::Reader, pipe::Reader, pipe::Writer>> {
        let url = reqwest::Url::parse(url).map_err(gix::Error::from_error)?;
        if let Some(host) = url.host_str()
            && let Some(ip) = crate::net::host_to_ip(host)
            && crate::net::is_blocked_ip(&self.env.app_blocked_cidrs, &ip)
        {
            tracing::warn!("blocking internal IP address in git: {}", ip);

            return Err(gix::Error::from_boxed(
                anyhow::anyhow!("IP address {ip} is blocked").into(),
            ));
        }

        let mut header_map = reqwest::header::HeaderMap::new();
        for line in headers {
            if let Some((name, value)) = line.as_ref().split_once(':')
                && let Ok(name) = reqwest::header::HeaderName::from_str(name)
                && let Ok(value) = reqwest::header::HeaderValue::from_str(value.trim())
            {
                header_map.append(name, value);
            }
        }

        let (post_body_tx, mut post_body_rx) = pipe::unidirectional(0);
        let (mut headers_tx, headers_rx) = pipe::unidirectional(0);
        let (mut body_tx, body_rx) = pipe::unidirectional(0);
        let client = self.client;

        std::thread::spawn(move || {
            let request = match body_kind {
                None => client.get(url),
                Some(PostBodyDataKind::BoundedAndFitsIntoMemory) => {
                    let mut body = Vec::new();
                    if let Err(err) = post_body_rx.read_to_end(&mut body) {
                        headers_tx.channel.send(Err(err)).ok();
                        return;
                    }

                    client.post(url).body(body)
                }
                Some(PostBodyDataKind::Unbounded) => client
                    .post(url)
                    .body(reqwest::blocking::Body::new(post_body_rx)),
            };

            let mut response = match request.headers(header_map).send() {
                Ok(response) => response,
                Err(err) => {
                    headers_tx
                        .channel
                        .send(Err(std::io::Error::other(err)))
                        .ok();
                    return;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let err = if status == reqwest::StatusCode::UNAUTHORIZED {
                    std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        AuthenticationRequired {
                            www_authenticate: response
                                .headers()
                                .get_all(reqwest::header::WWW_AUTHENTICATE)
                                .iter()
                                .map(|value| value.as_bytes().into())
                                .collect(),
                        },
                    )
                } else if status.is_redirection() {
                    std::io::Error::other(format!(
                        "repository redirected to {}, use that url instead",
                        response
                            .headers()
                            .get(reqwest::header::LOCATION)
                            .and_then(|location| location.to_str().ok())
                            .unwrap_or("another location")
                    ))
                } else if status.is_server_error() && status != reqwest::StatusCode::NOT_IMPLEMENTED
                {
                    std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        format!("received HTTP status {status}"),
                    )
                } else {
                    std::io::Error::other(format!("received HTTP status {status}"))
                };

                headers_tx.channel.send(Err(err)).ok();
                return;
            }

            for (name, value) in response.headers() {
                if headers_tx
                    .write_all(name.as_str().as_bytes())
                    .and_then(|_| headers_tx.write_all(b":"))
                    .and_then(|_| headers_tx.write_all(value.as_bytes()))
                    .and_then(|_| headers_tx.write_all(b"\n"))
                    .is_err()
                {
                    return;
                }
            }
            drop(headers_tx);

            if let Err(err) = std::io::copy(&mut response, &mut body_tx) {
                body_tx.channel.send(Err(err)).ok();
            }
        });

        Ok(PostResponse {
            post_body: post_body_tx,
            headers: headers_rx,
            body: body_rx,
        })
    }
}

impl Http for GitHttp {
    type Headers = pipe::Reader;
    type ResponseBody = pipe::Reader;
    type PostBody = pipe::Writer;

    fn get(
        &mut self,
        url: &str,
        _base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> gix::Result<GetResponse<Self::Headers, Self::ResponseBody>> {
        self.request(url, headers, None).map(Into::into)
    }

    fn post(
        &mut self,
        url: &str,
        _base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
        body: PostBodyDataKind,
    ) -> gix::Result<PostResponse<Self::Headers, Self::ResponseBody, Self::PostBody>> {
        self.request(url, headers, Some(body))
    }

    fn configure(&mut self, _config: &dyn std::any::Any) -> gix::Result {
        Ok(())
    }
}

pub enum GitCredentials {
    None,
    Password {
        username: compact_str::CompactString,
        password: compact_str::CompactString,
    },
    PrivateKey {
        username: compact_str::CompactString,
        private_key: String,
        passphrase: Option<compact_str::CompactString>,
    },
}

struct AcceptAnyServerKey;

impl russh::client::Handler for AcceptAnyServerKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct SessionBound<T> {
    inner: T,
    _session: russh::client::Handle<AcceptAnyServerKey>,
}

impl<T: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for SessionBound<T> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

pub async fn resolve_addresses(
    env: &Arc<crate::env::Env>,
    url: &gix::Url,
) -> Result<Vec<std::net::SocketAddr>, anyhow::Error> {
    if !matches!(
        url.scheme,
        gix::url::Scheme::Http | gix::url::Scheme::Https | gix::url::Scheme::Ssh
    ) {
        return Err(anyhow::anyhow!(
            "only http, https and ssh repository urls are supported"
        ));
    }

    let host = url
        .host()
        .ok_or_else(|| anyhow::anyhow!("repository url is missing a host"))?;
    let port = url.port.unwrap_or(match url.scheme {
        gix::url::Scheme::Ssh => 22,
        gix::url::Scheme::Https => 443,
        _ => 80,
    });

    crate::net::resolve_allowed_addresses(env, host, port, "git").await
}

pub fn parse_private_key(
    private_key: &str,
    passphrase: Option<&str>,
) -> Result<russh::keys::PrivateKey, anyhow::Error> {
    let key = russh::keys::PrivateKey::from_openssh(private_key)?;

    if key.is_encrypted() {
        let passphrase = passphrase.ok_or_else(|| {
            anyhow::anyhow!("private key is encrypted, but no passphrase was set")
        })?;

        Ok(key.decrypt(passphrase)?)
    } else {
        Ok(key)
    }
}

pub fn validate_private_key(private_key: &str, _context: &()) -> Result<(), garde::Error> {
    match russh::keys::PrivateKey::from_openssh(private_key) {
        Ok(_) => Ok(()),
        Err(err) => Err(garde::Error::new(format!("Invalid private key: {err}"))),
    }
}

enum SshAuth {
    Password(compact_str::CompactString),
    PrivateKey(Arc<russh::keys::PrivateKey>),
}

async fn ssh_upload_pack(
    env: &Arc<crate::env::Env>,
    url: &gix::Url,
    username: &str,
    auth: SshAuth,
) -> Result<BoxedTransport, anyhow::Error> {
    let path = String::from_utf8_lossy(&url.path).into_owned();
    let addresses = resolve_addresses(env, url).await?;

    let mut session = russh::client::connect(
        Arc::new(russh::client::Config {
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        }),
        addresses.as_slice(),
        AcceptAnyServerKey,
    )
    .await?;

    let authenticated = match auth {
        SshAuth::Password(password) => {
            session
                .authenticate_password(username, password.as_str())
                .await?
        }
        SshAuth::PrivateKey(key) => {
            let hash_alg = session.best_supported_rsa_hash().await?.flatten();

            session
                .authenticate_publickey(
                    username,
                    russh::keys::PrivateKeyWithHashAlg::new(key, hash_alg),
                )
                .await?
        }
    };

    if !authenticated.success() {
        return Err(anyhow::anyhow!("ssh authentication was rejected"));
    }

    let channel = session.channel_open_session().await?;

    let _ = channel
        .set_env(false, "GIT_PROTOCOL", "version=2")
        .await
        .inspect_err(|err| tracing::debug!("failed to request GIT_PROTOCOL over ssh: {err:#}"));

    channel
        .exec(
            true,
            format!("git-upload-pack '{}'", path.replace('\'', "'\\''")),
        )
        .await?;

    let (read, write) = tokio::io::split(channel.into_stream());
    let handle = tokio::runtime::Handle::current();

    Ok(Box::new(
        gix::protocol::transport::client::git::blocking_io::Connection::new(
            tokio_util::io::SyncIoBridge::new_with_handle(
                SessionBound {
                    inner: read,
                    _session: session,
                },
                handle.clone(),
            ),
            tokio_util::io::SyncIoBridge::new_with_handle(write, handle),
            gix::protocol::transport::Protocol::V2,
            path,
            None::<(String, Option<u16>)>,
            gix::protocol::transport::client::git::ConnectMode::Process,
            false,
        ),
    ))
}

impl GitCredentials {
    pub fn into_connection_configurator(
        self,
        env: Arc<crate::env::Env>,
        url: gix::Url,
    ) -> impl FnMut(&mut GitConnection<'_, '_, '_>) -> gix::Result + 'static {
        let over_ssh = url.scheme == gix::url::Scheme::Ssh;

        move |connection| {
            if !over_ssh {
                let mut transport = http::connect_http(
                    GitHttp::new(&env),
                    url.clone(),
                    gix::protocol::transport::Protocol::V2,
                    false,
                );

                if let GitCredentials::Password { username, password } = &self {
                    transport
                        .set_identity(gix::sec::identity::Account {
                            username: username.to_string(),
                            password: password.to_string(),
                            oauth_refresh_token: None,
                        })
                        .map_err(gix::Error::from_error)?;
                }

                *connection.transport_mut() = Box::new(transport);

                return Ok(());
            }

            let (username, auth) = match &self {
                GitCredentials::None => {
                    return Err(gix::Error::from_boxed(
                        anyhow::anyhow!("ssh repositories need credentials").into(),
                    ));
                }
                GitCredentials::Password { username, password } => {
                    (username, SshAuth::Password(password.clone()))
                }
                GitCredentials::PrivateKey {
                    username,
                    private_key,
                    passphrase,
                } => (
                    username,
                    SshAuth::PrivateKey(Arc::new(
                        parse_private_key(private_key, passphrase.as_deref())
                            .map_err(|err| gix::Error::from_boxed(err.into()))?,
                    )),
                ),
            };

            let transport = tokio::runtime::Handle::current()
                .block_on(ssh_upload_pack(&env, &url, username, auth))
                .map_err(|err| gix::Error::from_boxed(err.into()))?;

            *connection.transport_mut() = transport;

            Ok(())
        }
    }
}
