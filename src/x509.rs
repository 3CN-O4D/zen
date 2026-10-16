//! Zen `x509` module — offline X.509 certificate inspection.
//!
//! ```python
//! import x509
//! cert = x509.parse_pem(open("site.pem").read())
//! print(cert["subject"])      # CN=audit-lab.example, O=Zen Audit, C=US
//! print(cert["not_after"])    # 2036-10-13T02:32:04Z
//! print(cert["san"])          # ["audit-lab.example", "www.audit-lab.example"]
//! ```
//!
//! Structure only: no chain building, no signature verification, no network.
//! Live handshakes need a TLS connector (a later milestone); this module
//! answers "what does this certificate say?" for PEM you already hold.

use crate::runtime::{Value, Vm};
use base64::Engine;
use std::sync::Arc;

struct Tlv<'a> {
    tag: u8,
    body: &'a [u8],
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!("x509: truncated {what}"));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn tlv(&mut self, what: &str) -> Result<Tlv<'a>, String> {
        let tag = *self.take(1, what)?.first().unwrap_or(&0);
        let first = *self.take(1, &format!("{what} length"))?.first().unwrap_or(&0);
        let len = if first < 0x80 {
            first as usize
        } else {
            let n = (first & 0x7F) as usize;
            if n == 0 || n > 4 {
                return Err(format!("x509: bad {what} length"));
            }
            let mut len = 0usize;
            for b in self.take(n, &format!("{what} long length"))? {
                len = (len << 8) | (*b as usize);
            }
            len
        };
        Ok(Tlv { tag, body: self.take(len, what)? })
    }

    fn seq(&mut self, what: &str) -> Result<Cursor<'a>, String> {
        let t = self.tlv(what)?;
        if t.tag != 0x30 {
            return Err(format!("x509: expected SEQUENCE for {what}"));
        }
        Ok(Cursor::new(t.body))
    }

    fn finished(&self) -> bool {
        self.pos >= self.data.len()
    }
}

fn oid_string(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }
    let mut parts = vec![(body[0] / 40).to_string(), (body[0] % 40).to_string()];
    let mut val = 0u64;
    for (i, b) in body.iter().enumerate() {
        if i == 0 {
            continue;
        }
        val = (val << 7) | ((b & 0x7F) as u64);
        if b & 0x80 == 0 {
            parts.push(val.to_string());
            val = 0;
        }
    }
    parts.join(".")
}

fn name_string(body: &[u8]) -> Result<String, String> {
    // RDNSequence content: a list of SETs, each of SEQUENCE { OID, value }.
    // (`body` is already the SEQUENCE content — no extra unwrap.)
    let mut rdns = Cursor::new(body);
    let mut out = Vec::new();
    while !rdns.finished() {
        let set = rdns.tlv("rdn set")?;
        let mut set = Cursor::new(set.body);
        while !set.finished() {
            let atv = set.tlv("attribute")?;
            let mut atv = Cursor::new(atv.body);
            let oid = atv.tlv("attribute type")?;
            let oid = oid_string(oid.body);
            let val = atv.tlv("attribute value")?;
            let text = String::from_utf8_lossy(val.body).into_owned();
            let short = match oid.as_str() {
                "2.5.4.3" => "CN",
                "2.5.4.6" => "C",
                "2.5.4.7" => "L",
                "2.5.4.8" => "ST",
                "2.5.4.10" => "O",
                "2.5.4.11" => "OU",
                "2.5.4.12" => "title",
                "1.2.840.113549.1.9.1" => "email",
                _ => continue,
            };
            out.push(format!("{short}={text}"));
        }
    }
    Ok(out.join(", "))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn time_value(body: &[u8], tag: u8) -> Result<(String, i64), String> {
    // UTCTime "YYMMDDHHMMSSZ", GeneralizedTime "YYYYMMDDHHMMSSZ".
    let s = std::str::from_utf8(body).map_err(|_| "x509: bad time encoding".to_string())?;
    let (ys, rest) = if tag == 0x17 {
        let yy: i64 = s[0..2].parse().map_err(|_| "x509: bad UTCTime".to_string())?;
        (if yy < 50 { 2000 + yy } else { 1900 + yy }, &s[2..])
    } else {
        (
            s[0..4].parse().map_err(|_| "x509: bad GeneralizedTime".to_string())?,
            &s[4..],
        )
    };
    let mo: i64 = rest[0..2].parse().map_err(|_| "x509: bad time".to_string())?;
    let dy: i64 = rest[2..4].parse().map_err(|_| "x509: bad time".to_string())?;
    let hh: i64 = rest[4..6].parse().map_err(|_| "x509: bad time".to_string())?;
    let mm: i64 = rest[6..8].parse().map_err(|_| "x509: bad time".to_string())?;
    let ss: i64 = rest[8..10].parse().map_err(|_| "x509: bad time".to_string())?;
    let epoch = days_from_civil(ys, mo, dy) * 86400 + hh * 3600 + mm * 60 + ss;
    let iso = format!("{ys:04}-{mo:02}-{dy:02}T{hh:02}:{mm:02}:{ss:02}Z");
    Ok((iso, epoch))
}

fn sig_name(oid: &str) -> &str {
    match oid {
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption",
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption",
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption",
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption",
        "1.2.840.113549.1.1.4" => "md5WithRSAEncryption",
        "1.2.840.10045.4.3.2" => "ecdsaWithSHA256",
        "1.2.840.10045.4.3.3" => "ecdsaWithSHA384",
        "1.2.840.10045.4.1" => "ecdsaWithSHA1",
        _ => "unknown",
    }
}

fn parse_cert(der: &[u8]) -> Result<indexmap::IndexMap<String, Value>, String> {
    let mut top = Cursor::new(der);
    let mut cert = top.seq("certificate")?;
    let mut tbs = cert.seq("tbsCertificate")?;

    // [0] EXPLICIT version is optional (absent means v1).
    let first = tbs.tlv("tbs head")?;
    let (version, serial_body) = if first.tag == 0xA0 {
        let mut v = Cursor::new(first.body);
        let inner = v.tlv("version")?;
        let serial = tbs.tlv("serial")?;
        (inner.body.first().copied().unwrap_or(0) as i64 + 1, serial.body)
    } else {
        if first.tag != 0x02 {
            return Err("x509: expected serial INTEGER".to_string());
        }
        (1, first.body)
    };
    let mut serial_hex = String::new();
    for b in serial_body {
        serial_hex.push_str(&format!("{b:02X}"));
    }

    // Signature algorithm of the TBS.
    let mut sigseq = tbs.seq("tbs signature")?;
    let sigoid = sigseq.tlv("signature oid")?;
    let sig_algo = sig_name(&oid_string(sigoid.body)).to_string();

    // Issuer, validity, subject.
    let issuer = tbs.tlv("issuer")?;
    let issuer = name_string(issuer.body)?;
    let mut validity = tbs.seq("validity")?;
    let nb = validity.tlv("notBefore")?;
    let (not_before, not_before_epoch) = time_value(nb.body, nb.tag)?;
    let na = validity.tlv("notAfter")?;
    let (not_after, not_after_epoch) = time_value(na.body, na.tag)?;
    let subject = tbs.tlv("subject")?;
    let subject = name_string(subject.body)?;

    // SubjectPublicKeyInfo: algorithm + key size.
    let mut spki = tbs.seq("subjectPublicKeyInfo")?;
    let mut alg = spki.seq("key algorithm")?;
    let keyoid = alg.tlv("key oid")?;
    let keyoid = oid_string(keyoid.body);
    let (key_algo, key_bits) = match keyoid.as_str() {
        "1.2.840.113549.1.1.1" => {
            let bits = spki.tlv("public key")?;
            let keybytes = if bits.body.first() == Some(&0) { &bits.body[1..] } else { bits.body };
            let mut k = Cursor::new(keybytes);
            let mut rsaseq = k.seq("rsa key")?;
            let modulus = rsaseq.tlv("modulus")?;
            let mut m = modulus.body;
            while m.len() > 1 && m[0] == 0 {
                m = &m[1..];
            }
            ("rsa".to_string(), (m.len() as i64) * 8)
        }
        "1.2.840.10045.2.1" => {
            // EC params name the curve (absent means implicit; report unknown size).
            let rest = alg.rest();
            let mut bits = 0i64;
            if !rest.is_empty() {
                let mut p = Cursor::new(rest);
                if let Ok(t) = p.tlv("ec params") {
                    if t.tag == 0x06 {
                        bits = match oid_string(t.body).as_str() {
                            "1.2.840.10045.3.1.7" => 256,
                            "1.3.132.0.34" => 384,
                            "1.3.132.0.35" => 521,
                            _ => 0,
                        };
                    }
                }
            }
            ("ec".to_string(), bits)
        }
        "1.2.840.10040.4.1" => ("dsa".to_string(), 0),
        _ => ("unknown".to_string(), 0),
    };

    // Extensions live in [3] EXPLICIT after the subject-unique-ID fields.
    let mut san: Vec<Value> = Vec::new();
    let mut is_ca = false;
    while !tbs.finished() {
        let t = tbs.tlv("tbs tail")?;
        if t.tag != 0xA3 {
            continue;
        }
        let mut exts = Cursor::new(t.body);
        let exts = exts.seq("extensions")?;
        let mut exts = exts;
        while !exts.finished() {
            let e = exts.tlv("extension")?;
            let mut e = Cursor::new(e.body);
            let oid = e.tlv("extension oid")?;
            let oid = oid_string(oid.body);
            // Optional critical BOOLEAN sits between OID and value.
            let mut val = e.tlv("extension value")?;
            if val.tag == 0x01 {
                val = e.tlv("extension value")?;
            }
            if oid == "2.5.29.17" {
                let mut names = Cursor::new(val.body);
                if let Ok(seq) = names.seq("san") {
                    let mut seq = seq;
                    while !seq.finished() {
                        if let Ok(n) = seq.tlv("general name") {
                            if n.tag == 0x82 {
                                san.push(Value::String(
                                    String::from_utf8_lossy(n.body).into_owned(),
                                ));
                            } else if n.tag == 0x87 && n.body.len() == 4 {
                                // iPAddress (IPv4): four octets in network order.
                                san.push(Value::String(format!(
                                    "{}.{}.{}.{}",
                                    n.body[0], n.body[1], n.body[2], n.body[3]
                                )));
                            }
                        } else {
                            break;
                        }
                    }
                }
            } else if oid == "2.5.29.19" {
                is_ca = val.body.windows(3).any(|w| w == [0x01, 0x01, 0xFF]);
            }
        }
    }

    let mut out = indexmap::IndexMap::new();
    out.insert("subject".into(), Value::String(subject));
    out.insert("issuer".into(), Value::String(issuer));
    out.insert("serial_hex".into(), Value::String(serial_hex));
    out.insert("version".into(), Value::Number(version as f64));
    out.insert("not_before".into(), Value::String(not_before));
    out.insert("not_before_epoch".into(), Value::Number(not_before_epoch as f64));
    out.insert("not_after".into(), Value::String(not_after));
    out.insert("not_after_epoch".into(), Value::Number(not_after_epoch as f64));
    out.insert("san".into(), Value::List(Arc::new(san)));
    out.insert("is_ca".into(), Value::Bool(is_ca));
    out.insert("sig_algo".into(), Value::String(sig_algo));
    out.insert("key_algo".into(), Value::String(key_algo));
    out.insert("key_bits".into(), Value::Number(key_bits as f64));
    Ok(out)
}

fn string_arg(args: &[Value], idx: usize, what: &str) -> Result<String, String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(format!("x509.{what}: expected a string argument")),
    }
}

/// Parse one PEM certificate into a dict of subject/issuer/validity/SAN and
/// key facts. Throws a plain error string on malformed input.
pub fn x509_parse_pem(args: &[Value]) -> Result<Value, String> {
    let pem = string_arg(args, 0, "parse_pem")?;
    let begin = pem.find("-----BEGIN CERTIFICATE-----").ok_or_else(|| {
        "x509.parse_pem: no PEM certificate block found".to_string()
    })?;
    let after = &pem[begin + "-----BEGIN CERTIFICATE-----".len()..];
    let end = after.find("-----END CERTIFICATE-----").ok_or_else(|| {
        "x509.parse_pem: unterminated PEM certificate block".to_string()
    })?;
    let body: String = after[..end].chars().filter(|c| !c.is_whitespace()).collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body.as_bytes())
        .map_err(|e| format!("x509.parse_pem: bad base64: {e}"))?;
    Ok(Value::Dict(Arc::new(parse_cert(&der)?)))
}

pub fn init_x509_module(vm: &mut Vm) {
    let x509 = Value::Dict(Arc::new(indexmap::IndexMap::from([(
        "parse_pem".into(),
        Value::NativeFunction("x509_parse_pem".into()),
    )])));
    vm.vars.insert("x509".into(), x509);
}
