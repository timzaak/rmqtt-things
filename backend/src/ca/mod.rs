use crate::config::CAConfig;
use rcgen::{Issuer, KeyPair, SanType};
use std::path::Path;
use time::{Duration, OffsetDateTime};
use tokio::fs;
use tracing::info;

pub mod generator;

pub async fn init_ca(config: &CAConfig) -> anyhow::Result<()> {
    let ca_dir = Path::new(&config.ca_dir);

    let ca_cert_path = ca_dir.join("ca.pem");
    let ca_key_path = ca_dir.join("ca.key");
    let server_cert_path = ca_dir.join("server.pem");
    let server_key_path = ca_dir.join("server.key");

    let regen_ca = if !ca_cert_path.exists() || !ca_key_path.exists() {
        info!("CA certificate or key not found.");
        true
    } else {
        let cert_pem = fs::read_to_string(&ca_cert_path).await?;
        let key_pem = fs::read_to_string(&ca_key_path).await?;
        let key_pair = KeyPair::from_pem(&key_pem);
        let cert = key_pair.and_then(|kp| Issuer::from_ca_cert_pem(&cert_pem, kp));
        if cert.is_err() {
            info!("CA certificate or key is invalid.");
            true
        } else {
            false
        }
    };

    if regen_ca {
        info!("Generating new CA certificate and key...");
        if !ca_dir.exists() {
            fs::create_dir_all(ca_dir).await?;
        }
        let (ca_cert_pem, ca_key_pem) = generator::generate_ca(&config.name, config.valid_days)?;
        let now = OffsetDateTime::now_utc();
        let (server_pem, server_key_pem) = generator::issue_server_cert(
            &ca_cert_pem,
            &ca_key_pem,
            vec![SanType::DnsName(config.domain.clone().try_into()?)],
            now,
            now + Duration::days(config.valid_days),
        )?;
        fs::write(&ca_cert_path, ca_cert_pem).await?;
        fs::write(&ca_key_path, ca_key_pem).await?;
        fs::write(&server_key_path, server_key_pem).await?;
        fs::write(&server_cert_path, server_pem).await?;
        info!("New CA generated and saved in {}", config.ca_dir);
    } else {
        info!("CA certificate and key are valid.");
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::config::CAConfig;
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
        PKCS_ED25519, SanType, SerialNumber,
    };
    use reqwest::{Certificate, Identity};
    use std::net::{IpAddr, Ipv4Addr};
    use time::{Duration, OffsetDateTime};

    #[test]
    fn generate_ca() -> anyhow::Result<()> {
        let key_pair = rcgen::KeyPair::generate_for(&PKCS_ED25519)?;
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained); // 表示这是 CA
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "My Test CA");
        ca_params.distinguished_name = dn;
        ca_params.not_after = rcgen::date_time_ymd(2026, 1, 1);
        ca_params.not_before = rcgen::date_time_ymd(2025, 1, 1);

        ca_params.subject_alt_names = vec![SanType::DnsName("example.com".try_into()?)];
        let certificate = ca_params.self_signed(&key_pair)?;
        println!("{}", certificate.pem());
        println!("{}", key_pair.serialize_pem());

        Ok(())
    }

    #[test]
    fn read_cert() -> anyhow::Result<()> {
        let cert = r#"-----BEGIN CERTIFICATE-----
MIIBNTCB6KADAgECAhQmeq/fGS0P0owXB7I88g847pAkiDAFBgMrZXAwFTETMBEG
A1UEAwwKTXkgVGVzdCBDQTAeFw0yNTAxMDEwMDAwMDBaFw0yNjAxMDEwMDAwMDBa
MBUxEzARBgNVBAMMCk15IFRlc3QgQ0EwKjAFBgMrZXADIQB5qrm1kKg6F8PUME9O
eTOZ5qBaM9v2pMmNYLClffpUYqNKMEgwFgYDVR0RBA8wDYILZXhhbXBsZS5jb20w
HQYDVR0OBBYEFAYpwC0cyKVKj0PrmtK153ZYDreIMA8GA1UdEwEB/wQFMAMBAf8w
BQYDK2VwA0EAcF+gS93lWR8BY8HgR6Z4n1MXqunmXyD/jl3cnROH4N1AMkeYSN0t
DB6F6Vhnh/A3O42QKy4Fzf3zMIxmmlpkCA==
-----END CERTIFICATE-----"#;

        let private_key = r#"-----BEGIN PRIVATE KEY-----
MFECAQEwBQYDK2VwBCIEIBZDWsCZm49MPA3R5tF6eegEw+H7SPxa0/NgcvtfXowC
gSEAeaq5tZCoOhfD1DBPTnkzmeagWjPb9qTJjWCwpX36VGI=
-----END PRIVATE KEY-----"#;
        let ca_key_pair = KeyPair::from_pem(private_key)?;
        // let t = pem::parse(cert)?.contents();
        let issuer = Issuer::from_ca_cert_pem(cert, ca_key_pair)?;
        let ee_key = KeyPair::generate()?;
        let mut ee_params = CertificateParams::default();

        ee_params.serial_number = Some(SerialNumber::from_slice("123-1234-123".as_bytes()));

        ee_params.use_authority_key_identifier_extension = true;
        let ee_cert = ee_params.signed_by(&ee_key, &issuer)?;
        println!("{}", ee_key.serialize_pem());
        println!("{}", ee_cert.pem());
        Ok(())
    }

    #[tokio::test]
    async fn ca_server_client_test() -> anyhow::Result<()> {
        let ca_config = CAConfig::default();
        let (ca_cert, ca_key) =
            super::generator::generate_ca(&ca_config.name, ca_config.valid_days)?;
        let (server_cert, server_key) = super::generator::issue_server_cert(
            &ca_cert,
            &ca_key,
            vec![
                SanType::DnsName("127.0.0.1".try_into()?),
                SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            ],
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc() + Duration::days(ca_config.valid_days),
        )?;

        let (client_cert1, client_key1) = super::generator::issue_cert(
            &ca_cert,
            &ca_key,
            "test_one",
            "test_two",
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc() + Duration::days(366),
        )?;

        let (client_cert2, client_key2) = super::generator::issue_cert(
            &ca_cert,
            &ca_key,
            "test_one",
            "test_two2",
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc() + Duration::days(366),
        )?;

        let pem = client_cert1.clone() + client_key1.as_str();
        let _identity = Identity::from_pem(pem.as_bytes())?;
        let _identity2 =
            Identity::from_pem((client_cert2.clone() + client_key2.as_str()).as_bytes())?;
        let _ca = Certificate::from_pem(ca_cert.as_bytes())?;
        let _server_identity =
            Identity::from_pem((server_cert.clone() + server_key.as_str()).as_bytes())?;

        println!("===========CA============");
        println!("{ca_cert}\n\n{ca_key}");

        println!("===========Server=============");
        println!("{server_cert}\n\n{server_key}");

        println!("===========Client1=============");
        println!("{client_cert1}\n\n{client_key1}");
        println!("===========Client2=============");
        println!("{client_cert2}\n\n{server_key}");

        // let parent_path = "C:\\code\\work\\deploy\\tmp\\certs";
        // fs::write(format!("{parent_path}\\ca.pem"), ca_cert).await?;
        // fs::write(format!("{parent_path}\\ca.key"), ca_key).await?;
        // fs::write(format!("{parent_path}\\server.pem"), server_cert).await?;
        // fs::write(format!("{parent_path}\\server.key"), server_key).await?;
        // fs::write(format!("{parent_path}\\client.pem"), client_cert1 + client_key1.as_str()).await?;

        Ok(())
    }
}
