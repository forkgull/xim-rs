use std::convert::TryFrom;

use x11rb::connection::Connection;
use x11rb::protocol::{xproto::*, Event};
use xim::x11rb::{Client, ClientError};
use xim_parser::{AttributeName, CommitData, InputStyle, Request, Spot};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let (conn, screen_num) = x11rb::connect(None).expect("Connect X");
    let screen = &conn.setup().roots[screen_num];
    let window = conn.generate_id()?;
    conn.create_window(
        screen.root_depth,
        window,
        screen.root,
        0,
        0,
        800,
        600,
        0,
        WindowClass::InputOutput,
        screen.root_visual,
        &CreateWindowAux::default()
            .background_pixel(screen.black_pixel)
            .event_mask(EventMask::KeyPress | EventMask::KeyRelease),
    )?;
    conn.map_window(window)?;
    conn.flush()?;

    let mut client = Client::init(&conn, screen, None)?;

    log::info!("Start event loop");

    let mut im_id = None;
    let mut ic_id = None;

    let mut end = false;

    while !end {
        let e = conn.wait_for_event()?;

        if client.filter_event(&e, |client, req| {
            log::trace!("Recv req: {:?}", req);

            match req {
                Request::ConnectReply {
                    server_major_protocol_version: _,
                    server_minor_protocol_version: _,
                } => client.send_req(Request::Open {
                    locale: "en_US".into(),
                }),
                Request::OpenReply {
                    input_method_id,
                    im_attrs,
                    ic_attrs,
                } => {
                    client.set_attrs(im_attrs, ic_attrs);
                    client.send_req(Request::QueryExtension {
                        input_method_id,
                        extensions: vec![],
                    })
                }
                Request::QueryExtensionReply {
                    input_method_id, ..
                } => client.send_req(Request::EncodingNegotiation {
                    encodings: vec!["COMPOUND_TEXT".into(), "".into()],
                    encoding_infos: vec![],
                    input_method_id,
                }),
                Request::EncodingNegotiationReply {
                    category: _,
                    index: _,
                    input_method_id,
                } => {
                    let ic_attributes = client
                        .build_ic_attributes()
                        .push(
                            AttributeName::InputStyle,
                            InputStyle::PREEDITPOSITION | InputStyle::STATUSNOTHING,
                        )
                        .push(AttributeName::ClientWindow, window)
                        .push(AttributeName::FocusWindow, window)
                        .nested_list(AttributeName::PreeditAttributes, |b| {
                            b.push(AttributeName::SpotLocation, Spot { x: 0, y: 0 });
                        })
                        .build();

                    client.send_req(Request::CreateIc {
                        input_method_id,
                        ic_attributes,
                    })
                }
                Request::CreateIcReply {
                    input_method_id,
                    input_context_id,
                } => {
                    log::info!(
                        "IC Created im: {}, ic: {}",
                        input_method_id,
                        input_context_id
                    );

                    im_id = Some(input_method_id);
                    ic_id = Some(input_context_id);

                    client.send_req(Request::SetIcFocus {
                        input_method_id,
                        input_context_id,
                    })
                }
                Request::GetIcValuesReply {
                    input_method_id: _,
                    input_context_id: _,
                    ic_attributes: _,
                } => Ok(()),
                Request::SetEventMask {
                    input_method_id: _,
                    input_context_id: _,
                    forward_event_mask,
                    synchronous_event_mask,
                } => {
                    client.set_event_mask(forward_event_mask, synchronous_event_mask);
                    Ok(())
                }
                Request::CloseReply { input_method_id: _ } => {
                    client.send_req(Request::Disconnect {})
                }
                Request::DisconnectReply {} => {
                    end = true;
                    Ok(())
                }
                Request::Error { code, detail, .. } => Err(ClientError::XimError(code, detail)),
                Request::ForwardEvent { xev, .. } => {
                    let xev = &xev[..];
                    if let Ok(press) = KeyPressEvent::try_from(xev) {
                        log::info!("forward press: 0x{:X}", press.detail);
                        Ok(())
                    } else {
                        Err(ClientError::InvalidReply)
                    }
                }
                Request::Commit {
                    input_method_id,
                    input_context_id,
                    data,
                } => match data {
                    CommitData::Keysym { keysym, .. } => {
                        log::info!("Commited keysym: {}", keysym);

                        Ok(())
                    }
                    CommitData::Chars { commited, .. } => {
                        log::info!(
                            "Commited {}",
                            ctext::compound_text_to_utf8(&commited).unwrap()
                        );

                        Ok(())
                    }
                    _ => todo!(),
                },
                _ => Err(ClientError::InvalidReply),
            }
        })? {
            log::trace!("event consumed");
        } else if let Event::Error(err) = e {
            return Err(ClientError::X11Error(err).into());
        } else {
            match e {
                Event::KeyPress(e) | Event::KeyRelease(e) => match (im_id, ic_id) {
                    (Some(im), Some(ic)) => {
                        client.forward_key_press(im, ic, &e)?;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(())
}
