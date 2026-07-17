use crate::state::AppState;
use instant_acme::{Account, ChallengeType, Identifier, NewAccount, NewOrder, OrderStatus};
use rcgen::{CertificateParams, DistinguishedName, DnType};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

const ACME_POLL_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(2);
const ACME_VALIDATION_ATTEMPTS: usize = 15;
const ACME_CERTIFICATE_ATTEMPTS: usize = 30;

struct ChallengeCleanup<'a> {
    challenges: &'a RwLock<HashMap<String, String>>,
    tokens: Vec<String>,
}

impl<'a> ChallengeCleanup<'a> {
    fn new(challenges: &'a RwLock<HashMap<String, String>>) -> Self {
        Self {
            challenges,
            tokens: Vec::new(),
        }
    }

    fn insert(&mut self, token: String, key_authorization: String) {
        self.challenges
            .write()
            .unwrap()
            .insert(token.clone(), key_authorization);
        self.tokens.push(token);
    }
}

impl Drop for ChallengeCleanup<'_> {
    fn drop(&mut self) {
        let mut challenges = self.challenges.write().unwrap();
        for token in &self.tokens {
            challenges.remove(token);
        }
    }
}

pub async fn trigger_refresh(state: Arc<AppState>, domain: String, email: Option<String>) {
    tokio::spawn(async move {
        let _ = state.events.send(crate::event::Event::Log {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            text: format!("Starting ACME flow for domain: {}", domain),
        });

        let contact_email = email.or_else(|| {
            state.cert_store.load_all().ok().and_then(|certs| {
                certs
                    .into_iter()
                    .find(|c| c.sni == domain)
                    .and_then(|c| c.acme_email)
            })
        });

        match run_acme_flow(&state, &domain, contact_email.as_deref()).await {
            Ok(_) => {
                let _ = state.events.send(crate::event::Event::Log {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    text: format!(
                        "Successfully renewed/obtained TLS certificate for domain: {}",
                        domain
                    ),
                });
                if let Ok(certs) = crate::control::certs::cert_snapshot(&state) {
                    let _ = state.events.send(crate::event::Event::CertRegistered {
                        sni: domain.clone(),
                        certs,
                    });
                }
            }
            Err(e) => {
                tracing::error!("ACME flow failed for {}: {:?}", domain, e);
                let _ = state.events.send(crate::event::Event::Log {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    text: format!("ACME flow failed for {}: {:?}", domain, e),
                });
            }
        }
    });
}

async fn run_acme_flow(state: &AppState, domain: &str, email: Option<&str>) -> anyhow::Result<()> {
    tracing::info!("Running ACME flow for domain: {}", domain);
    let mut challenge_cleanup = ChallengeCleanup::new(&state.acme_challenges);

    // 1. Create an ACME account with Let's Encrypt production.
    let contact_email = match email {
        Some(e) => format!("mailto:{}", e),
        None => format!("mailto:admin@{}", domain),
    };
    let contact = vec![contact_email.as_str()];

    let server_url = "https://acme-v02.api.letsencrypt.org/directory";

    let (account, _credentials) = Account::create(
        &NewAccount {
            contact: &contact,
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        server_url,
        None,
    )
    .await?;

    // 2. Create Order
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[Identifier::Dns(domain.to_string())],
        })
        .await?;

    // 3. Retrieve Authorizations and complete HTTP-01 Challenges
    let mut authorizations = order.authorizations().await?;
    for auth in &mut authorizations {
        let challenge = auth
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or_else(|| anyhow::anyhow!("No HTTP-01 challenge found"))?;

        let token = challenge.token.clone();
        let key_auth = order.key_authorization(challenge).as_str().to_string();

        // Write challenge token and key auth payload to shared AppState so Pingora serves it
        challenge_cleanup.insert(token.clone(), key_auth);

        let _ = state.events.send(crate::event::Event::Log {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            text: format!("Prepared HTTP-01 challenge for token: {}", token),
        });

        // Signal ACME server that we are ready
        order.set_challenge_ready(&challenge.url).await?;
    }

    // Poll order status
    let mut attempts = 0;
    loop {
        tokio::time::sleep(ACME_POLL_INTERVAL).await;
        let order_state = order.refresh().await?;
        if matches!(order_state.status, OrderStatus::Ready | OrderStatus::Valid) {
            break;
        }
        if matches!(order_state.status, OrderStatus::Invalid) {
            return Err(anyhow::anyhow!(
                "ACME order validation failed (status invalid)"
            ));
        }
        attempts += 1;
        if attempts >= ACME_VALIDATION_ATTEMPTS {
            return Err(anyhow::anyhow!("ACME challenge validation timed out"));
        }
    }

    // Generate CSR with rcgen
    let mut params = CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, domain);
    let cert_key = rcgen::KeyPair::generate()?;
    let csr = params.serialize_request(&cert_key)?;
    let csr_der = csr.der();

    // Finalize order
    order.finalize(csr_der).await?;

    // Let’s Encrypt may keep the finalized order in "processing" briefly.
    // Poll until the certificate is available instead of treating that state as a failure.
    let mut cert_chain_pem = None;
    for _ in 0..ACME_CERTIFICATE_ATTEMPTS {
        if let Some(certificate) = order.certificate().await? {
            cert_chain_pem = Some(certificate);
            break;
        }
        tokio::time::sleep(ACME_POLL_INTERVAL).await;
    }
    let cert_chain_pem =
        cert_chain_pem.ok_or_else(|| anyhow::anyhow!("ACME certificate issuance timed out"))?;

    let private_key_pem = cert_key.serialize_pem();

    // Save certificate and key to the data directory
    state.cert_store.save(
        domain,
        cert_chain_pem.as_bytes(),
        private_key_pem.as_bytes(),
        email,
    )?;

    // Reload in-memory registry
    let all_certs = state.cert_store.load_all()?;
    let mut registry = crate::registry::CertificateRegistry::new();
    for db_cert in all_certs {
        let _ = registry.register(&db_cert.sni, &db_cert.cert_pem, &db_cert.key_pem);
    }
    state.certs.store(Arc::new(registry));

    Ok(())
}



#[cfg(test)]
mod tests {
    use super::ChallengeCleanup;
    use std::collections::HashMap;
    use std::sync::RwLock;

    #[test]
    fn challenge_cleanup_removes_only_its_own_tokens() {
        let challenges = RwLock::new(HashMap::from([(
            "another-order".to_string(),
            "keep-me".to_string(),
        )]));

        {
            let mut cleanup = ChallengeCleanup::new(&challenges);
            cleanup.insert("this-order".to_string(), "remove-me".to_string());
            assert!(challenges.read().unwrap().contains_key("this-order"));
        }

        let challenges = challenges.read().unwrap();
        assert!(!challenges.contains_key("this-order"));
        assert_eq!(challenges.get("another-order").unwrap(), "keep-me");
    }
}
