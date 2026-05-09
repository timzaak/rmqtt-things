use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair, SanType,
};
use std::ops::Add;
use time::{Duration, OffsetDateTime};

pub fn issue_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
    product_id: &str,
    device_id: &str,
    start_at: OffsetDateTime,
    end_at: OffsetDateTime,
) -> anyhow::Result<(String, String)> {
    let ca_key_pair = KeyPair::from_pem(ca_key_pem)?;
    let issuer = Issuer::from_ca_cert_pem(ca_cert_pem, ca_key_pair)?;

    let ee_key = KeyPair::generate()?;
    // customise it if you needed.

    let mut ee_params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, format!("{}/{}", product_id, device_id));
    /*
    ee_params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[0x02, 0x26],
            format!("{}/{}", product_id, device_id).into_bytes(),
        ));
    */
    ee_params.not_before = start_at;
    ee_params.not_after = end_at;

    let ee_cert = ee_params.signed_by(&ee_key, &issuer)?;

    let cert_pem = ee_cert.pem();
    let key_pem = ee_key.serialize_pem();

    Ok((cert_pem, key_pem))
}

pub fn generate_ca(name: &str, valid_days: i64) -> anyhow::Result<(String, String)> {
    let key_pair = KeyPair::generate()?;
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, name);
    ca_params.distinguished_name = dn;
    let now = OffsetDateTime::now_utc();
    ca_params.not_after = now.add(Duration::days(valid_days));
    ca_params.not_before = now;

    let certificate = ca_params.self_signed(&key_pair)?;
    let cert_pem = certificate.pem();
    let key_pem = key_pair.serialize_pem();
    Ok((cert_pem, key_pem))
}

pub fn issue_server_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
    //domain: &str,
    san: Vec<SanType>,
    start_at: OffsetDateTime,
    end_at: OffsetDateTime,
) -> anyhow::Result<(String, String)> {
    let ca_key_pair = KeyPair::from_pem(ca_key_pem)?;
    let issuer = Issuer::from_ca_cert_pem(ca_cert_pem, ca_key_pair)?;

    let ee_key = KeyPair::generate()?;
    // customise it if you needed.

    let mut ee_params = CertificateParams::default();
    // SanType::DnsName(domain.try_into()?)
    // SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
    ee_params.subject_alt_names = san;

    ee_params.not_before = start_at;
    ee_params.not_after = end_at;

    let ee_cert = ee_params.signed_by(&ee_key, &issuer)?;

    let cert_pem = ee_cert.pem();
    let key_pem = ee_key.serialize_pem();

    Ok((cert_pem, key_pem))
}
