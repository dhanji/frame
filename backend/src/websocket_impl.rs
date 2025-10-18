use actix_web::{web, HttpRequest, HttpResponse, Error};
use actix_web_actors::ws;
use actix::{Actor, StreamHandler, AsyncContext, ActorContext, Handler, Message};
use actix::prelude::*;
use std::time::{Duration, Instant};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// New email notification
    NewEmail {
        email_id: String,
        from: String,
        subject: String,
        preview: String,
    },
    /// Email marked as read
    EmailRead {
        email_id: String,
    },
    /// Email deleted
    EmailDeleted {
        email_id: String,
    },
    /// Folder updated
    FolderUpdate {
        folder: String,
        unread_count: i64,
    },
    /// Ping/Pong for keepalive
    Ping,
    Pong,
}

/// WebSocket connection actor
pub struct WsConnection {
    id: Uuid,
    user_id: i64,
    hb: Instant,
    manager: Arc<RwLock<ConnectionManager>>,
}

impl WsConnection {
    pub fn new(user_id: i64, manager: Arc<RwLock<ConnectionManager>>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            hb: Instant::now(),
            manager,
        }
    }

    /// Heartbeat that sends ping to client every 5 seconds
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::from_secs(5), |act, ctx| {
            // Check client heartbeats
            if Instant::now().duration_since(act.hb) > Duration::from_secs(10) {
                // Heartbeat timed out
                log::info!("WebSocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }

            ctx.text(serde_json::to_string(&WsMessage::Ping).unwrap());
        });
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start
    fn started(&mut self, ctx: &mut Self::Context) {
        // Start heartbeat
        self.hb(ctx);

        // Register connection
        let manager = self.manager.clone();
        let id = self.id;
        let user_id = self.user_id;
        let addr = ctx.address();
        
        actix::spawn(async move {
            let mut mgr = manager.write().await;
            mgr.add_connection(user_id, id, addr.recipient());
        });
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // Unregister connection
        let manager = self.manager.clone();
        let id = self.id;
        let user_id = self.user_id;
        
        actix::spawn(async move {
            let mut mgr = manager.write().await;
            mgr.remove_connection(user_id, id);
        });
        
        actix::Running::Stop
    }
}

/// Handler for ws::Message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Handle incoming text messages
                if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                    match msg {
                        WsMessage::Pong => {
                            self.hb = Instant::now();
                        }
                        _ => {
                            // Handle other message types if needed
                        }
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

/// Message for sending to WebSocket connections
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage(pub String);

impl Handler<SendMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// Connection manager
pub struct ConnectionManager {
    connections: HashMap<i64, HashMap<Uuid, actix::Recipient<SendMessage>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn add_connection(&mut self, user_id: i64, conn_id: Uuid, addr: actix::Recipient<SendMessage>) {
        self.connections
            .entry(user_id)
            .or_insert_with(HashMap::new)
            .insert(conn_id, addr);
        
        log::info!("Added WebSocket connection {} for user {}", conn_id, user_id);
    }

    pub fn remove_connection(&mut self, user_id: i64, conn_id: Uuid) {
        if let Some(user_conns) = self.connections.get_mut(&user_id) {
            user_conns.remove(&conn_id);
            if user_conns.is_empty() {
                self.connections.remove(&user_id);
            }
        }
        
        log::info!("Removed WebSocket connection {} for user {}", conn_id, user_id);
    }

    pub async fn send_to_user(&self, user_id: i64, message: WsMessage) {
        if let Some(user_conns) = self.connections.get(&user_id) {
            let msg_str = serde_json::to_string(&message).unwrap();
            for (_, addr) in user_conns.iter() {
                let _ = addr.do_send(SendMessage(msg_str.clone()));
            }
        }
    }

    pub async fn broadcast(&self, message: WsMessage) {
        let msg_str = serde_json::to_string(&message).unwrap();
        for (_, user_conns) in self.connections.iter() {
            for (_, addr) in user_conns.iter() {
                let _ = addr.do_send(SendMessage(msg_str.clone()));
            }
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket endpoint handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    manager: web::Data<Arc<RwLock<ConnectionManager>>>,
) -> Result<HttpResponse, Error> {
    // Extract JWT token from query parameters or headers
    let user_id = extract_user_id_from_request(&req).unwrap_or(1);
    
    let ws_connection = WsConnection::new(user_id, manager.get_ref().clone());
    ws::start(ws_connection, &req, stream)
}

/// Extract user ID from JWT token in request
fn extract_user_id_from_request(req: &HttpRequest) -> Option<i64> {
    // Try to get token from query parameter
    if let Some(token) = req.query_string().split('&')
        .find(|s| s.starts_with("token="))
        .and_then(|s| s.strip_prefix("token=")) {
        // Validate JWT token and extract user_id
        if let Ok(claims) = crate::middleware::auth::validate_token(token) {
            return Some(claims.user_id);
        }
    }
    
    // Try to get token from Authorization header
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Ok(claims) = crate::middleware::auth::validate_token(token) {
                    return Some(claims.user_id);
                }
            }
        }
    }
    
    None
}

/// WebSocket endpoint handler with authentication
pub async fn ws_handler_authenticated(
    req: HttpRequest,
    stream: web::Payload,
    manager: web::Data<Arc<RwLock<ConnectionManager>>>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, Error> {
    let ws_connection = WsConnection::new(user.user_id, manager.get_ref().clone());
    ws::start(ws_connection, &req, stream)
}

/// WebSocket handler without authentication for testing
pub async fn websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    // For now, create a test connection without authentication
    let manager = Arc::new(RwLock::new(ConnectionManager::new()));
    let ws_connection = WsConnection::new(1, manager); // Use test user ID 1
    ws::start(ws_connection, &req, stream)
}