use x11rb::connection::Connection;
use xim::{x11rb::X11rbServer, InputContext, Server, ServerError, ServerHandler, XimConnections};
use xim_parser::InputStyle;

#[derive(Default)]
struct Handler {}

impl Handler {}

impl<S: Server> ServerHandler<S> for Handler {
    type InputStyleArray = [InputStyle; 1];

    fn input_styles(&self) -> Self::InputStyleArray {
        [InputStyle::PREEDITNOTHING | InputStyle::STATUSNOTHING]
    }

    fn handle_connect(&mut self, _server: &mut S) -> Result<(), ServerError> {
        log::info!("Connected!");
        Ok(())
    }

    fn handle_create_ic(
        &mut self,
        server: &mut S,
        input_context: &mut InputContext,
    ) -> Result<(), ServerError> {
        server.commit(
            input_context.client_win(),
            input_context.input_method_id(),
            input_context.input_context_id(),
            "가나다",
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    let mut server = X11rbServer::init(&conn, screen, "test_server")?;
    let mut connections = XimConnections::new();
    let mut handler = Handler::default();

    loop {
        let e = conn.wait_for_event()?;
        log::trace!("event: {:?}", e);
        server.filter_event(&e, &mut connections, &mut handler)?;
    }
}