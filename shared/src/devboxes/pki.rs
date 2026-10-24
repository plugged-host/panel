//! plugged.host: the devbox CA for VS Code in the browser (docs/features/devboxes.md), private to
//! this panel:
//!
//! - the edge certificate: the client certificate the edge (nginx) presents to a devbox. `ph`
//!   only serves its web side to that certificate, so code-server is reachable through the edge
//!   alone.
//! - a server certificate per devbox start for `<slug>.<web domain>`, which the edge verifies, so
//!   nothing between the edge and a node can stand in for a devbox.
//!
//! EC P-256 throughout. The CA and edge keys are encrypted settings; the CA key never leaves the
//! panel, the edge files are written for nginx by `panel-rs devboxes edge-certificates`.

use crate::settings::devboxes::AppSettingsDevboxes;
use anyhow::Context;
use openssl::{
    asn1::Asn1Time,
    bn::{BigNum, MsbOption},
    ec::{EcGroup, EcKey},
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    x509::{
        X509, X509NameBuilder,
        extension::{
            AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage,
            SubjectAlternativeName, SubjectKeyIdentifier,
        },
    },
};

const CA_DAYS: u32 = 3650;
const EDGE_DAYS: u32 = 3650;
const SERVER_DAYS: u32 = 30;
/// Certificates are valid from a few minutes ago: a node whose clock runs a little behind still
/// takes a fresh one.
const BACKDATE_SECONDS: i64 = 300;

/// A new CA with its edge certificate, PEM.
pub struct Authority {
    pub ca_cert: String,
    pub ca_key: String,
    pub edge_cert: String,
    pub edge_key: String,
}

/// A devbox's server certificate, its key and the CA, PEM.
pub struct Issued {
    pub cert: String,
    pub key: String,
    pub ca: String,
}

#[derive(Clone, Copy)]
enum Kind {
    Ca,
    Edge,
    Server,
}

/// Makes a new CA and edge certificate. Replacing them breaks the browser version until nginx
/// has the new edge files and running devboxes started again.
pub fn create_authority(app_name: &str) -> Result<Authority, anyhow::Error> {
    let ca_key = new_key()?;
    let ca = sign(
        &common_name(app_name, "devbox CA"),
        &ca_key,
        None,
        Kind::Ca,
        CA_DAYS,
    )?;

    let edge_key = new_key()?;
    let edge = sign(
        &common_name(app_name, "devbox edge"),
        &edge_key,
        Some((&ca, &ca_key)),
        Kind::Edge,
        EDGE_DAYS,
    )?;

    Ok(Authority {
        ca_cert: pem(ca.to_pem()?)?,
        ca_key: pem(ca_key.private_key_to_pem_pkcs8()?)?,
        edge_cert: pem(edge.to_pem()?)?,
        edge_key: pem(edge_key.private_key_to_pem_pkcs8()?)?,
    })
}

/// A server certificate for one devbox start at `host`.
pub fn issue(settings: &AppSettingsDevboxes, host: &str) -> Result<Issued, anyhow::Error> {
    let (Some(ca_cert), Some(ca_key)) = (settings.ca_cert.as_deref(), settings.ca_key.as_deref())
    else {
        anyhow::bail!("the devbox CA is not set up (Admin › Settings › Hosting › Devboxes)");
    };

    issue_with(ca_cert, ca_key, host)
}

fn issue_with(ca_cert: &str, ca_key: &str, host: &str) -> Result<Issued, anyhow::Error> {
    if !valid_host(host) {
        anyhow::bail!("not a host name: {host}");
    }

    let ca = X509::from_pem(ca_cert.as_bytes()).context("unreadable devbox CA certificate")?;
    let ca_key =
        PKey::private_key_from_pem(ca_key.as_bytes()).context("unreadable devbox CA key")?;
    let key = new_key()?;
    let cert = sign(host, &key, Some((&ca, &ca_key)), Kind::Server, SERVER_DAYS)?;

    Ok(Issued {
        cert: pem(cert.to_pem()?)?,
        key: pem(key.private_key_to_pem_pkcs8()?)?,
        ca: ca_cert.to_string(),
    })
}

pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
}

/// "<app name> devbox CA": only a label (nothing checks it), cut to X.509's 64 characters.
fn common_name(app_name: &str, what: &str) -> String {
    let app_name: String = app_name.chars().filter(|c| !c.is_control()).collect();
    let name = format!("{} {what}", app_name.trim());
    let name = if name.chars().count() > 64 {
        what.to_string()
    } else {
        name.trim().to_string()
    };

    name.chars().take(64).collect()
}

fn new_key() -> Result<PKey<Private>, anyhow::Error> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;

    Ok(PKey::from_ec_key(EcKey::generate(&group)?)?)
}

fn pem(bytes: Vec<u8>) -> Result<String, anyhow::Error> {
    Ok(String::from_utf8(bytes)?)
}

fn sign(
    common_name: &str,
    key: &PKey<Private>,
    issuer: Option<(&X509, &PKey<Private>)>,
    kind: Kind,
    days: u32,
) -> Result<X509, anyhow::Error> {
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_nid(Nid::COMMONNAME, common_name)?;
    let name = name.build();

    let mut serial = BigNum::new()?;
    serial.rand(127, MsbOption::MAYBE_ZERO, false)?;
    let now = chrono::Utc::now().timestamp();

    let serial = serial.to_asn1_integer()?;
    let not_before = Asn1Time::from_unix(now - BACKDATE_SECONDS)?;
    let not_after = Asn1Time::days_from_now(days)?;

    let mut builder = X509::builder()?;
    builder.set_version(2)?;
    builder.set_serial_number(&serial)?;
    builder.set_subject_name(&name)?;
    builder.set_issuer_name(issuer.map_or(&*name, |(ca, _)| ca.subject_name()))?;
    builder.set_pubkey(key)?;
    builder.set_not_before(&not_before)?;
    builder.set_not_after(&not_after)?;

    match kind {
        Kind::Ca => {
            builder
                .append_extension(BasicConstraints::new().critical().ca().pathlen(0).build()?)?;
            builder.append_extension(
                KeyUsage::new()
                    .critical()
                    .key_cert_sign()
                    .crl_sign()
                    .build()?,
            )?;
        }
        Kind::Edge => {
            builder.append_extension(BasicConstraints::new().critical().build()?)?;
            builder.append_extension(KeyUsage::new().critical().digital_signature().build()?)?;
            builder.append_extension(ExtendedKeyUsage::new().client_auth().build()?)?;
        }
        Kind::Server => {
            builder.append_extension(BasicConstraints::new().critical().build()?)?;
            builder.append_extension(KeyUsage::new().critical().digital_signature().build()?)?;
            builder.append_extension(ExtendedKeyUsage::new().server_auth().build()?)?;
            let san = SubjectAlternativeName::new()
                .dns(common_name)
                .build(&builder.x509v3_context(issuer.map(|(ca, _)| &**ca), None))?;
            builder.append_extension(san)?;
        }
    }

    let subject_key_id = SubjectKeyIdentifier::new().build(&builder.x509v3_context(None, None))?;
    builder.append_extension(subject_key_id)?;
    if let Some((ca, _)) = issuer {
        let authority_key_id = AuthorityKeyIdentifier::new()
            .keyid(false)
            .build(&builder.x509v3_context(Some(&**ca), None))?;
        builder.append_extension(authority_key_id)?;
    }

    builder.sign(
        issuer.map_or(key, |(_, ca_key)| ca_key),
        MessageDigest::sha256(),
    )?;

    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        stack::Stack,
        x509::{X509StoreContext, store::X509StoreBuilder},
    };

    fn verifies(ca: &str, cert: &str) -> bool {
        let ca = X509::from_pem(ca.as_bytes()).unwrap();
        let cert = X509::from_pem(cert.as_bytes()).unwrap();
        let mut store = X509StoreBuilder::new().unwrap();
        store.add_cert(ca).unwrap();
        let store = store.build();

        let mut context = X509StoreContext::new().unwrap();
        context
            .init(&store, &cert, &Stack::new().unwrap(), |context| {
                context.verify_cert()
            })
            .unwrap()
    }

    #[test]
    fn the_ca_signs_the_edge_and_devbox_certificates() {
        let authority = create_authority("Example").unwrap();
        assert!(verifies(&authority.ca_cert, &authority.edge_cert));

        let issued =
            issue_with(&authority.ca_cert, &authority.ca_key, "abc123.example.net").unwrap();
        assert!(verifies(&authority.ca_cert, &issued.cert));
        assert_eq!(issued.ca, authority.ca_cert);

        let cert = X509::from_pem(issued.cert.as_bytes()).unwrap();
        let names: Vec<String> = cert
            .subject_alt_names()
            .unwrap()
            .iter()
            .filter_map(|name| name.dnsname().map(str::to_string))
            .collect();
        assert_eq!(names, vec!["abc123.example.net".to_string()]);

        let other = create_authority("Example").unwrap();
        assert!(!verifies(&other.ca_cert, &issued.cert));
    }

    #[test]
    fn only_host_names_get_a_certificate() {
        let authority = create_authority("Example").unwrap();

        assert!(issue_with(&authority.ca_cert, &authority.ca_key, "Bad Host").is_err());
        assert!(issue_with(&authority.ca_cert, &authority.ca_key, "").is_err());
    }

    #[test]
    fn common_names_fit_x509() {
        assert_eq!(common_name("Example", "devbox CA"), "Example devbox CA");
        assert_eq!(common_name(&"x".repeat(80), "devbox CA"), "devbox CA");
        assert_eq!(common_name("", "devbox CA"), "devbox CA");
    }
}
