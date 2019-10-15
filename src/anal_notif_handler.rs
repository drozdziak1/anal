use jack::{Client, NotificationHandler, PortId};

use crate::constants::ANAL_JACK_CLIENT_NAME;

pub struct AnalNotifHandler;

impl NotificationHandler for AnalNotifHandler {
    fn port_registration(&mut self, client: &Client, port_id: PortId, is_registered: bool) {
        if !is_registered {
            return;
        }
        if let Some(p) = client.port_by_id(port_id) {
            match p.name() {
                Ok(n) => {
                    debug!("New port {}", n);

                    let mut iter = n.split(':');
					let client_name = if let Some(c) = iter.next() {
						c
					} else {
						error!("Could not extract client name from port name");
						return;
					};
					if client_name == ANAL_JACK_CLIENT_NAME {
						info!("New anal port {}", n);

						// TODO: Connect the newfound port to system
					}
                }
                Err(e) => {
                    error!("Port name error: {:?}", e);
                }
            }
        } else {
            warn!("Could not retrieve the new port");
        }
    }
}
