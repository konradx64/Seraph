use std::sync::Arc;
use crate::state::AppState;
use instant_acme::{Account, NewAccount, NewOrder, Identifier, OrderStatus, ChallengeType};
use rcgen::{CertificateParams, DistinguishedName, DnType};

pub async fn start_acme_worker(state: Arc<AppState>) {
    // Check routes periodically every 24 hours
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
    loop {
        interval.tick().await;
        tracing::info!("Running periodic ACME checks...");
        let routes = state.routes.load();
        for route in routes.all() {
            if matches!(route.tls, crate::route::TlsMode::Auto) {
                let domain = route.hostname.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = run_acme_flow(&state_clone, &domain).await {
                        tracing::error!("ACME flow failed for {}: {:?}", domain, e);
                    }
                });
            }
        }
    }
}

pub async fn trigger_refresh(state: Arc<AppState>, domain: String) {
    tokio::spawn(async move {
        let _ = state.events.send(crate::event::Event::Log {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            text: format!("Starting manual ACME refresh for domain: {}", domain),
        });
        match run_acme_flow(&state, &domain).await {
            Ok(_) => {
                let _ = state.events.send(crate::event::Event::Log {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    text: format!("Successfully renewed SSL certificate for domain: {}", domain),
                });
                let _ = state.events.send(crate::event::Event::CertRegistered {
                    sni: domain.clone(),
                });
            }
            Err(e) => {
                tracing::error!("Manual ACME refresh failed for {}: {:?}", domain, e);
                let _ = state.events.send(crate::event::Event::Log {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    text: format!("ACME refresh failed for {}: {:?}", domain, e),
                });
            }
        }
    });
}

async fn run_acme_flow(state: &AppState, domain: &str) -> anyhow::Result<()> {
    tracing::info!("Running ACME flow for domain: {}", domain);
    
    // 1. Create ACME Account using Let's Encrypt Staging by default
    let contact_email = format!("mailto:admin@{}", domain);
    let contact = vec![contact_email.as_str()];
    
    let server_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
    
    let (account, _credentials) = Account::create(
        &NewAccount {
            contact: &contact,
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        server_url,
        None,
    ).await?;
    
    // 2. Create Order
    let mut order = account.new_order(&NewOrder {
        identifiers: &[Identifier::Dns(domain.to_string())],
    }).await?;
    
    // 3. Retrieve Authorizations and complete HTTP-01 Challenges
    let mut authorizations = order.authorizations().await?;
    for auth in &mut authorizations {
        let challenge = auth.challenges.iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or_else(|| anyhow::anyhow!("No HTTP-01 challenge found"))?;
            
        let token = challenge.token.clone();
        let key_auth = order.key_authorization(challenge).as_str().to_string();
        
        // Write challenge token and key auth payload to shared AppState so Pingora serves it
        {
            let mut challenges = state.acme_challenges.write().unwrap();
            challenges.insert(token.clone(), key_auth);
        }
        
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
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let order_state = order.refresh().await?;
        if matches!(order_state.status, OrderStatus::Ready | OrderStatus::Valid) {
            break;
        }
        if matches!(order_state.status, OrderStatus::Invalid) {
            return Err(anyhow::anyhow!("ACME order validation failed (status invalid)"));
        }
        attempts += 1;
        if attempts > 15 {
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
    order.finalize(&csr_der).await?;
    
    // Download certificate
    let cert_chain_pem = order.certificate().await?
        .ok_or_else(|| anyhow::anyhow!("No certificate chain returned"))?;
        
    let private_key_pem = cert_key.serialize_pem();
    
    // Save to SQLite certificates table
    state.db.save_cert(domain, cert_chain_pem.as_bytes(), private_key_pem.as_bytes())?;
    
    // Reload in-memory registry
    let all_certs = state.db.load_certs()?;
    let mut registry = crate::registry::certs::CertificateRegistry::new();
    for (sni, cert_pem, key_pem) in all_certs {
        let _ = registry.register(&sni, &cert_pem, &key_pem);
    }
    state.certs.store(Arc::new(registry));
    
    // Clear acme challenges map
    {
        let mut challenges = state.acme_challenges.write().unwrap();
        challenges.clear();
    }
    
    Ok(())
}
