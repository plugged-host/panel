use gix::protocol::transport::client::blocking_io::Transport;
use std::sync::Arc;

type BoxedTransport = Box<dyn Transport + Send>;
type GitConnection<'a, 'b, 'c> = gix::remote::Connection<'a, 'b, 'c, BoxedTransport>;

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
            let (username, auth) = match &self {
                GitCredentials::None => return Ok(()),
                GitCredentials::Password { username, password } if !over_ssh => {
                    connection
                        .transport_mut()
                        .set_identity(gix::sec::identity::Account {
                            username: username.to_string(),
                            password: password.to_string(),
                            oauth_refresh_token: None,
                        })
                        .map_err(gix::Error::from_error)?;

                    return Ok(());
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
