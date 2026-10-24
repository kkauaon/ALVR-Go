mod control_socket;
mod packets;
mod stream_socket;

use alvr_common::prelude::*;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

pub use control_socket::*;
pub use packets::*;
pub use stream_socket::*;

pub const LOCAL_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
pub const CONTROL_PORT: u16 = 9943;
pub const MAX_HANDSHAKE_PACKET_SIZE_BYTES: usize = 4_000;

/// Wire encoding helpers. bincode 2 with the `legacy` configuration (fixed-width integers,
/// little endian) produces the same bytes as bincode 1, so the protocol is unchanged.
pub mod wire {
    use serde::{de::DeserializeOwned, Serialize};
    use std::io::{Read, Write};

    pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
        bincode::serde::encode_to_vec(value, bincode::config::legacy())
    }

    pub fn serialized_size<T: Serialize>(value: &T) -> Result<usize, bincode::error::EncodeError> {
        serialize(value).map(|bytes| bytes.len())
    }

    pub fn serialize_into<T: Serialize>(
        writer: &mut impl Write,
        value: &T,
    ) -> Result<usize, bincode::error::EncodeError> {
        bincode::serde::encode_into_std_write(value, writer, bincode::config::legacy())
    }

    pub fn deserialize<T: DeserializeOwned>(
        bytes: &[u8],
    ) -> Result<T, bincode::error::DecodeError> {
        bincode::serde::decode_from_slice(bytes, bincode::config::legacy()).map(|(value, _)| value)
    }

    pub fn deserialize_from<T: DeserializeOwned>(
        reader: &mut impl Read,
    ) -> Result<T, bincode::error::DecodeError> {
        bincode::serde::decode_from_std_read(reader, bincode::config::legacy())
    }
}

type Ldc = tokio_util::codec::LengthDelimitedCodec;

#[derive(Serialize, Deserialize, Clone)]
pub struct PublicIdentity {
    pub hostname: String,
    pub certificate_pem: Option<String>,
}

pub struct PrivateIdentity {
    pub hostname: String,
    pub certificate_pem: String,
    pub key_pem: String,
}

pub fn create_identity(hostname: Option<String>) -> StrResult<PrivateIdentity> {
    let hostname = hostname.unwrap_or(format!(
        "{}{}{}{}.client.alvr",
        rand::rng().random_range(0..10),
        rand::rng().random_range(0..10),
        rand::rng().random_range(0..10),
        rand::rng().random_range(0..10),
    ));

    let certificate = trace_err!(rcgen::generate_simple_self_signed([hostname.clone()]))?;

    Ok(PrivateIdentity {
        hostname,
        certificate_pem: certificate.cert.pem(),
        key_pem: certificate.signing_key.serialize_pem(),
    })
}

mod util {
    use alvr_common::prelude::*;
    use std::future::Future;
    use tokio::{sync::oneshot, task};

    // Tokio tasks are not cancelable. This function awaits a cancelable task.
    pub async fn spawn_cancelable(
        future: impl Future<Output = StrResult> + Send + 'static,
    ) -> StrResult {
        // this channel is actually never used. cancel_receiver will be notified when _cancel_sender
        // is dropped
        let (_cancel_sender, cancel_receiver) = oneshot::channel::<()>();

        trace_err!(
            task::spawn(async {
                tokio::select! {
                    res = future => res,
                    _ = cancel_receiver => Ok(()),
                }
            })
            .await
        )?
    }
}
pub use util::*;
