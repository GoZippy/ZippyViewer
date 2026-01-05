#![cfg(feature = "quic")]
#![forbid(unsafe_code)]

use prost::Message;

use crate::quic::{read_frame, write_frame};
use zrc_crypto::session_crypto::{open_v1, seal_v1, SessionCryptoV1};

/// Logical channels over QUIC streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelV1 {
    Control = 1,
    Frames = 2,
    Clipboard = 3,
    Files = 4,
}

impl ChannelV1 {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Control),
            2 => Some(Self::Frames),
            3 => Some(Self::Clipboard),
            4 => Some(Self::Files),
            _ => None,
        }
    }
}

/// First frame on every stream: [version=1][channel_id]
fn hello_bytes(ch: ChannelV1) -> [u8; 2] {
    [1u8, ch as u8]
}

async fn send_hello(send: &mut quinn::SendStream, ch: ChannelV1) -> anyhow::Result<()> {
    write_frame(send, &hello_bytes(ch)).await.map_err(|e| anyhow::anyhow!("{e}"))
}

async fn recv_hello(recv: &mut quinn::RecvStream) -> anyhow::Result<ChannelV1> {
    let b = read_frame(recv).await.map_err(|e| anyhow::anyhow!("{e}"))?
        .ok_or_else(|| anyhow::anyhow!("EOF before hello"))?;
    if b.len() != 2 || b[0] != 1 {
        return Err(anyhow::anyhow!("bad hello"));
    }
    ChannelV1::from_u8(b[1]).ok_or_else(|| anyhow::anyhow!("unknown channel"))
}

/// A simple frame packet: width/height/stride/format + pixels
/// Encoded inside the QUIC frame payload.
#[derive(Debug, Clone)]
pub struct FramePacketV1 {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u8, // 1=BGRA
    pub pixels: Vec<u8>,
}

pub fn encode_frame_packet(pkt: &FramePacketV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(17 + pkt.pixels.len());
    out.extend_from_slice(&pkt.width.to_be_bytes());
    out.extend_from_slice(&pkt.height.to_be_bytes());
    out.extend_from_slice(&pkt.stride.to_be_bytes());
    out.push(pkt.format);
    out.extend_from_slice(&(pkt.pixels.len() as u32).to_be_bytes());
    out.extend_from_slice(&pkt.pixels);
    out
}

pub fn decode_frame_packet(b: &[u8]) -> Option<FramePacketV1> {
    if b.len() < 17 { return None; }
    let width  = u32::from_be_bytes(b[0..4].try_into().ok()?);
    let height = u32::from_be_bytes(b[4..8].try_into().ok()?);
    let stride = u32::from_be_bytes(b[8..12].try_into().ok()?);
    let format = b[12];
    let len = u32::from_be_bytes(b[13..17].try_into().ok()?) as usize;
    if b.len() != 17 + len { return None; }
    Some(FramePacketV1 { width, height, stride, format, pixels: b[17..].to_vec() })
}

/// AAD is just channel id for now; you can extend later (session_id, counter, etc).
fn aad_for_channel(ch: ChannelV1) -> [u8; 1] {
    [ch as u8]
}

/// Control channel handle (post-handshake, encrypted ControlMsgV1).
pub struct ControlChannelV1 {
    pub crypto: SessionCryptoV1,
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl ControlChannelV1 {
    pub async fn send_msg(&mut self, msg: &zrc_proto::v1::ControlMsgV1) -> anyhow::Result<()> {
        let mut buf = Vec::with_capacity(msg.encoded_len());
        msg.encode(&mut buf)?;
        let sealed = seal_v1(&self.crypto, &buf, &aad_for_channel(ChannelV1::Control))
            .map_err(|e| anyhow::anyhow!("seal failed: {:?}", e))?;
        write_frame(&mut self.send, &sealed).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn recv_msg(&mut self) -> anyhow::Result<Option<zrc_proto::v1::ControlMsgV1>> {
        let sealed = match read_frame(&mut self.recv).await {
            Ok(Some(b)) => b,
            Ok(None) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("{e}")),
        };
        let pt = open_v1(&self.crypto, &sealed, &aad_for_channel(ChannelV1::Control))
            .ok_or_else(|| anyhow::anyhow!("control decrypt failed"))?;
        let msg = zrc_proto::v1::ControlMsgV1::decode(pt.as_slice())?;
        Ok(Some(msg))
    }
}

/// Controller: open Control bi-stream, send plaintext ControlTicketV1, then upgrade to E2EE.
pub async fn controller_control_handshake(
    conn: &quinn::Connection,
    ticket_packet: &zrc_proto::v1::ControlTicketV1,
) -> anyhow::Result<ControlChannelV1> {
    let (mut send, mut recv) = conn.open_bi().await?;
    send_hello(&mut send, ChannelV1::Control).await?;

    // 1) Send ticket packet plaintext (still protected by QUIC TLS pinning)
    let mut tp = Vec::with_capacity(ticket_packet.encoded_len());
    ticket_packet.encode(&mut tp)?;
    write_frame(&mut send, &tp).await.map_err(|e| anyhow::anyhow!("{e}"))?;

    // 2) Derive session crypto (E2EE) from binding + ticket_id
    let t = ticket_packet.ticket.as_ref().ok_or_else(|| anyhow::anyhow!("missing ticket"))?;
    let tid = &t.ticket_id;
    if tid.is_empty() {
        return Err(anyhow::anyhow!("missing ticket_id"));
    }
    let crypto = zrc_crypto::session_crypto::derive_session_crypto_v1(&t.session_binding, tid);

    Ok(ControlChannelV1 { crypto, send, recv })
}

/// Host: accept Control stream, read plaintext ControlTicketV1, verify ticket/binding, upgrade to E2EE.
pub async fn host_accept_control_handshake(
    conn: &quinn::Connection,
    now_unix: u64,
) -> anyhow::Result<(zrc_proto::v1::ControlTicketV1, ControlChannelV1)> {
    loop {
        let (send, mut recv) = conn.accept_bi().await?;
        let ch = recv_hello(&mut recv).await?;
        if ch != ChannelV1::Control { continue; }

        // 1) Read plaintext ticket packet
        let tp = read_frame(&mut recv).await.map_err(|e| anyhow::anyhow!("{e}"))?
            .ok_or_else(|| anyhow::anyhow!("EOF before ticket packet"))?;
        let ticket_packet = zrc_proto::v1::ControlTicketV1::decode(tp.as_ref())?;

        // 2) Verify session binding matches packet fields
        let sid = ticket_packet.session_id.as_ref().ok_or_else(|| anyhow::anyhow!("missing session_id"))?;
        let did = ticket_packet.device_id.as_ref().ok_or_else(|| anyhow::anyhow!("missing device_id"))?;
        let oid = ticket_packet.operator_id.as_ref().ok_or_else(|| anyhow::anyhow!("missing operator_id"))?;
        if ticket_packet.ticket_binding_nonce.len() != 16 {
            return Err(anyhow::anyhow!("ticket_binding_nonce must be 16 bytes"));
        }

        let expected = zrc_crypto::ticket::compute_session_binding_v1(
            &sid.id, &oid.id, &did.id, &ticket_packet.ticket_binding_nonce
        );

        let t = ticket_packet.ticket.as_ref().ok_or_else(|| anyhow::anyhow!("missing ticket"))?;
        zrc_crypto::ticket::verify_ticket_v1(t, now_unix, &expected)
            .map_err(|e| anyhow::anyhow!("ticket invalid: {e}"))?;

        // 3) Upgrade to E2EE
        let tid = &t.ticket_id;
        if tid.is_empty() {
            return Err(anyhow::anyhow!("missing ticket_id"));
        }
        let crypto = zrc_crypto::session_crypto::derive_session_crypto_v1(&t.session_binding, tid);

        let cc = ControlChannelV1 { crypto, send, recv };
        return Ok((ticket_packet, cc));
    }
}

/// Host: open Frames stream (uni) and continuously send encrypted FramePacketV1 blobs.
pub async fn host_stream_frames(
    conn: &quinn::Connection,
    crypto: &SessionCryptoV1,
    mut next_frame: impl FnMut() -> anyhow::Result<FramePacketV1> + Send + 'static,
) -> anyhow::Result<()> {
    let mut send = conn.open_uni().await?;
    send_hello(&mut send, ChannelV1::Frames).await?;

    loop {
        let pkt = next_frame()?;
        let raw = encode_frame_packet(&pkt);
        let sealed = seal_v1(crypto, &raw, &aad_for_channel(ChannelV1::Frames))
            .map_err(|e| anyhow::anyhow!("seal failed: {:?}", e))?;
        write_frame(&mut send, &sealed).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
}

/// Controller: accept uni streams, when Frames stream arrives, read/decrypt packets and call callback.
pub async fn controller_recv_frames(
    conn: &quinn::Connection,
    crypto: &SessionCryptoV1,
    mut on_frame: impl FnMut(FramePacketV1) + Send + 'static,
) -> anyhow::Result<()> {
    loop {
        let mut recv = conn.accept_uni().await?;
        let ch = recv_hello(&mut recv).await?;
        if ch != ChannelV1::Frames {
            continue;
        }
        loop {
            let sealed = match read_frame(&mut recv).await {
                Ok(Some(b)) => b,
                Ok(None) => break,
                Err(e) => return Err(anyhow::anyhow!("{e}")),
            };
            let pt = open_v1(crypto, &sealed, &aad_for_channel(ChannelV1::Frames))
                .ok_or_else(|| anyhow::anyhow!("frame decrypt failed"))?;
            if let Some(pkt) = decode_frame_packet(&pt) {
                on_frame(pkt);
            }
        }
    }
}

