use crate::state::AppState;
use std::sync::Arc;

pub fn handle_register_cert(
    state: &Arc<AppState>,
    sni: String,
    cert_pem: String,
    key_pem: String,
) -> (bool, String) {
    let mut certs = (**state.certs.load()).clone();
    match certs.register(
        &sni,
        cert_pem.as_bytes(),
        key_pem.as_bytes(),
    ) {
        Ok(_) => {
            state.certs.store(Arc::new(certs));
            state.events.publish(crate::event::Event::CertRegistered {
                sni: sni.clone(),
            });
            (true, format!("Certificate registered successfully for {}", sni))
        }
        Err(e) => (false, format!("Failed to register certificate: {}", e)),
    }
}
