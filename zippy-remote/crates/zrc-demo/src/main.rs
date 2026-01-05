use anyhow::Context;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bytes::Bytes;
use prost::Message;
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};

use zrc_core::{
    dispatch::{dispatch_controller_envelope, dispatch_host_envelope, ControllerEvent},
    http_mailbox::HttpMailboxClient,
    keys::generate_identity_keys,
    pairing::{PairingController, PairingHost},
    session::{SessionController, SessionHost},
    store::MemoryStore,
};

use zrc_proto::v1::{InviteV1, TransportV1, DirectIpHintV1};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  zrc-demo host <rendezvous_url> <quic_bind_addr> <quic_advertise_host:port>");
        eprintln!("  zrc-demo controller <rendezvous_url> <invite_b64>");
        std::process::exit(2);
    }
    match args[1].as_str() {
        "host" => {
            let rz = args.get(2).context("missing rendezvous_url")?;
            let quic_bind: SocketAddr = args.get(3).context("missing quic_bind_addr")?.parse()?;
            let advertise: SocketAddr = args.get(4).context("missing quic_advertise_host:port")?.parse()?;
            run_host(rz, quic_bind, advertise).await
        }
        "controller" => {
            let rz = args.get(2).context("missing rendezvous_url")?;
            let invite_b64 = args.get(3).context("missing invite_b64")?;
            run_controller(rz, invite_b64).await
        }
        _ => {
            anyhow::bail!("unknown mode");
        }
    }
}

async fn run_host(rendezvous_url: &str, quic_bind: SocketAddr, quic_advertise: SocketAddr) -> anyhow::Result<()> {
    let device_keys = generate_identity_keys();
    let store_host = MemoryStore::new();

    // Start QUIC server
    let alpn = b"zrc/1";
    let quic_server = zrc_core::quic::QuicServer::bind(quic_bind, alpn).await?;
    let cert_der = quic_server.cert_der.clone();

    println!("DEVICE_ID_HEX={}", hex::encode(device_keys.id32));
    println!("QUIC_CERT_DER_B64={}", B64.encode(&cert_der));

    // Create invite and persist secret on host
    let now_unix = 1_760_000_000u64 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?.as_secs() % 10_000);

    let (invite, invite_rec) = zrc_core::harness::make_invite(now_unix, &device_keys);
    store_host.put_invite(invite_rec).await;

    let mut invite_bytes = Vec::with_capacity(invite.encoded_len());
    invite.encode(&mut invite_bytes)?;
    println!("INVITE_B64={}", B64.encode(invite_bytes));

    // Pairing/session hosts
    let pairing_host = PairingHost::new(store_host.clone(), zrc_core::harness::AutoApprove, device_keys.clone());

    let mut session_host = SessionHost::new(store_host.clone(), device_keys.clone());
    session_host.quic_endpoints = vec![DirectIpHintV1 { host: quic_advertise.ip().to_string(), port: quic_advertise.port() as u32 }];
    session_host.alpn = Some("zrc/1".to_string());
    session_host.quic_server_cert_der = cert_der;

    // Spawn QUIC session loop (accept, establish control, then stream frames)
    let device_keys2 = device_keys.clone();
    let quic_endpoint = quic_server.endpoint.clone();

    tokio::spawn(async move {
        loop {
            let Some(connecting) = quic_endpoint.accept().await else { break };
            let Ok(conn) = connecting.await else { continue };
            let device_keys3 = device_keys2.clone();
            tokio::spawn(async move {
                // Accept control and get ticket (plaintext over pinned QUIC TLS)
                let (ticket_packet, mut control) = match zrc_core::quic_mux::host_accept_control_handshake(&conn, now_unix).await {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("control accept failed: {e}");
                        return;
                    }
                };

                // Verify ticket signature/binding if you want here (recommended).
                // (You already have verify_ticket_v1 in zrc-crypto::ticket.)

                let ticket_id = ticket_packet.ticket.as_ref().unwrap().ticket_id.as_ref().unwrap().id.clone();
                let crypto = zrc_crypto::session_crypto::derive_session_crypto_v1(
                    &ticket_packet.ticket.as_ref().unwrap().session_binding,
                    &ticket_id
                );

                // Handle input events
                #[cfg(windows)]
                {
                    let apply_input = |evt: zrc_proto::v1::InputEventV1| -> anyhow::Result<()> {
                        use zrc_proto::v1::input_event_v1::Kind;
                        match evt.kind {
                            Some(Kind::MouseMove(m)) => {
                                zrc_platform_win::input_sendinput::mouse_move_abs(m.x, m.y)
                                    .map_err(|e| anyhow::anyhow!("mouse_move: {e}"))?;
                            }
                            Some(Kind::MouseButton(b)) => {
                                zrc_platform_win::input_sendinput::mouse_button(b.button, b.down)
                                    .map_err(|e| anyhow::anyhow!("mouse_button: {e}"))?;
                            }
                            Some(Kind::Key(k)) => {
                                zrc_platform_win::input_sendinput::key_vk(k.keycode, k.down)
                                    .map_err(|e| anyhow::anyhow!("key_vk: {e}"))?;
                            }
                            Some(Kind::Text(_t)) => {
                                // MVP: ignore or implement via SendInput unicode events later
                            }
                            None => {}
                        }
                        Ok(())
                    };

                    let mut control2 = control;
                    tokio::spawn(async move {
                        loop {
                            let Some(msg) = control2.recv_msg().await.ok().flatten() else { break; };
                            if let Some(zrc_proto::v1::control_msg_v1::Msg::Input(input)) = msg.msg {
                                let _ = apply_input(input);
                            }
                        }
                    });
                }

                // Stream frames
                let mut frame_no = 0u64;
                let _ = zrc_core::quic_mux::host_stream_frames(&conn, &crypto, move || {
                    frame_no += 1;

                    // Windows capture if available; otherwise send dummy frame
                    #[cfg(windows)]
                    {
                        let f = zrc_platform_win::capture_gdi::capture_primary_bgra()
                            .map_err(|e| anyhow::anyhow!("{e}"))?;
                        Ok(zrc_core::quic_mux::FramePacketV1 {
                            width: f.width,
                            height: f.height,
                            stride: f.stride,
                            format: 1,
                            pixels: f.bgra,
                        })
                    }
                    #[cfg(not(windows))]
                    {
                        let w = 320u32;
                        let h = 180u32;
                        let stride = w * 4;
                        let mut pixels = vec![0u8; (stride * h) as usize];
                        // tiny moving pattern
                        let x = (frame_no % w as u64) as u32;
                        let y = (frame_no % h as u64) as u32;
                        let idx = (y * stride + x * 4) as usize;
                        if idx + 4 <= pixels.len() {
                            pixels[idx + 0] = 255; // B
                            pixels[idx + 1] = 0;   // G
                            pixels[idx + 2] = 0;   // R
                            pixels[idx + 3] = 255; // A
                        }
                        Ok(zrc_core::quic_mux::FramePacketV1 { width: w, height: h, stride, format: 1, pixels })
                    }
                }).await;
            });
        }
    });

    // HTTP mailbox loop
    let http = HttpMailboxClient::new(rendezvous_url)?;
    loop {
        if let Some(env_bytes) = http.poll(&device_keys.id32, 25_000).await? {
            let out = dispatch_host_envelope(
                &pairing_host,
                &session_host,
                &device_keys.kex_priv,
                now_unix,
                env_bytes,
            )
            .await?;

            if let Some(out) = out {
                let mut rid32 = [0u8; 32];
                rid32.copy_from_slice(&out.recipient_id);
                http.post(&rid32, &out.envelope_bytes).await?;
            }
        }
    }
}

async fn run_controller(rendezvous_url: &str, invite_b64: &str) -> anyhow::Result<()> {
    let operator_keys = generate_identity_keys();
    let store_ctrl = MemoryStore::new();

    let http = HttpMailboxClient::new(rendezvous_url)?;

    // Decode invite
    let invite_bytes = B64.decode(invite_b64)?;
    let invite = InviteV1::decode(invite_bytes.as_slice())?;
    let device_id = invite.device_id.as_ref().context("invite missing device_id")?;
    let device_kex_pub = invite.device_kex_pub.clone().context("invite missing device_kex_pub")?;
    let pinned_device_sign_pub = invite.device_sign_pub.clone().context("invite missing device_sign_pub")?;

    let pairing_ctrl = PairingController::new(store_ctrl.clone(), operator_keys.clone());
    let session_ctrl = SessionController::new(store_ctrl.clone(), operator_keys.clone());

    let now_unix = 1_760_000_000u64 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?.as_secs() % 10_000);

    // 1) Pair: make + seal request, post to device mailbox
    let pair_req = pairing_ctrl.make_pair_request_from_invite(&invite, now_unix, true)?;
    let mut dev_id32 = [0u8; 32];
    dev_id32.copy_from_slice(&device_id.id);
    let pair_env = pairing_ctrl.seal_pair_request(&device_kex_pub, &dev_id32, now_unix, &pair_req)?;

    http.post(&dev_id32, &pair_env).await?;

    // 2) Wait for receipt on operator mailbox
    let receipt = loop {
        if let Some(env) = http.poll(&operator_keys.id32, 25_000).await? {
            match dispatch_controller_envelope(&pairing_ctrl, &session_ctrl, &operator_keys.kex_priv, now_unix, env).await? {
                ControllerEvent::PairReceipt(r) => break r,
                _ => {}
            }
        }
    };
    pairing_ctrl.accept_pair_receipt(receipt.clone(), now_unix).await?;

    // 3) Session init (request mesh-preferred, allow fallback)
    let session_id = rand16();
    let ticket_nonce = rand16();
    let init = session_ctrl.make_session_init(
        &dev_id32,
        session_id,
        ticket_nonce,
        now_unix,
        vec![
            TransportV1::TransportV1MeshMailbox,
            TransportV1::TransportV1Rendezvous,
            TransportV1::TransportV1DirectIp,
            TransportV1::TransportV1Relay,
        ],
    );

    let init_env = zrc_core::harness::seal_session_init(
        &operator_keys,
        &dev_id32,
        receipt.device_kex_pub.as_ref().context("receipt missing device_kex_pub")?,
        now_unix,
        &init,
    )?;
    http.post(&dev_id32, &init_env).await?;

    // 4) Wait for session init response
    let resp = loop {
        if let Some(env) = http.poll(&operator_keys.id32, 25_000).await? {
            match dispatch_controller_envelope(&pairing_ctrl, &session_ctrl, &operator_keys.kex_priv, now_unix, env).await? {
                ControllerEvent::SessionInitResponse(r) => break r,
                _ => {}
            }
        }
    };

    // Verify ticket and get it
    let maybe_ticket = session_ctrl
        .accept_session_init_response(now_unix, &init, &resp, &pinned_device_sign_pub)
        .await?;
    let ticket = maybe_ticket.context("expected issued ticket (unattended)")?;
    println!("Got ticket_id={}", hex::encode(ticket.ticket_id.as_ref().unwrap().id.clone()));

    // 5) QUIC: connect, send ticket on Control channel, then receive Frames
    if let Some(zrc_proto::v1::session_init_response_v1::Negotiation::QuicParams(qp)) = resp.negotiation {
        let cert_der = qp.server_cert_der;
        let alpn = qp.alpn.as_bytes();

        let ep = qp.endpoints.get(0).context("no quic endpoints")?;
        let remote: SocketAddr = format!("{}:{}", ep.host, ep.port).parse()?;

        let client = zrc_core::quic::QuicClient::new("0.0.0.0:0".parse()?, alpn, &cert_der)?;
        let conn = client.connect(remote, "zrc.local").await?;

        // Build ControlTicketV1
        let ticket_packet = zrc_proto::v1::ControlTicketV1 {
            session_id: init.session_id.clone(),
            device_id: init.device_id.clone(),
            operator_id: init.operator_id.clone(),
            ticket_binding_nonce: init.ticket_binding_nonce.clone(),
            ticket: Some(ticket.clone()),
        };

        // Open control and send the ticket (plaintext over pinned QUIC TLS)
        let mut control = zrc_core::quic_mux::controller_control_handshake(&conn, &ticket_packet).await?;

        // Receive frames (encrypted) - use crypto from control channel
        let crypto = control.crypto.clone();
        zrc_core::quic_mux::controller_recv_frames(&conn, &crypto, |pkt| {
            println!(
                "FRAME {}x{} stride={} fmt={} bytes={}",
                pkt.width, pkt.height, pkt.stride, pkt.format, pkt.pixels.len()
            );
        }).await?;
    } else {
        println!("No QUIC negotiation provided (requires unattended enabled + endpoints configured).");
    }

    // keep process alive briefly for output
    sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn rand16() -> [u8; 16] {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("rng");
    b
}

