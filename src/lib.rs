use anyhow::{bail, Result};
use async_broadcast::{Receiver, Sender};
use down::{ClientDownStream, RobotRecvMessage};
use futures::{stream::SplitStream, Future, StreamExt};
use log::{debug, error, info, trace, warn};
use native_tls::TlsConnector;
use reqwest::{header::ACCEPT, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use tokio::{
    net::TcpStream,
    sync::Notify,
    time::{sleep, Duration},
};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{Error, Message},
    Connector, MaybeTlsStream, WebSocketStream,
};
use up::{EventAckData, Sink};

pub mod down;
pub mod up;

#[derive(Debug)]
pub struct Client {
    reconnect_interval: u64,
    heartbeat_interval: u64,
    pub config: Arc<Mutex<ClientConfig>>,
    client: reqwest::Client,
    rx: Receiver<Arc<ClientDownStream>>,
    tx: Sender<Arc<ClientDownStream>>,
    on_event_callback: EventCallback,
    sink: tokio::sync::Mutex<Option<Sink>>,
    alive: AtomicBool,
}

struct EventCallback(RwLock<Box<dyn Fn(ClientDownStream) -> EventAckData + Send + Sync>>);

impl std::fmt::Debug for EventCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EventCallback").finish()
    }
}

impl Client {
    /// Create new client, need to specific the id and secret they provided when creating the robot
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Result<Arc<Self>> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        let (tx, rx) = async_broadcast::broadcast(32);
        Ok(Arc::new(Self {
            config: Arc::new(Mutex::new(ClientConfig {
                client_id,
                client_secret,
                ..Default::default()
            })),
            client: ClientBuilder::new()
                .no_proxy()
                .danger_accept_invalid_certs(true)
                .build()?,
            reconnect_interval: 1000,
            heartbeat_interval: 8000,
            tx,
            rx,
            sink: tokio::sync::Mutex::new(None),
            on_event_callback: EventCallback(RwLock::new(Box::new(|p| {
                info!("default event callback, event received: {:?}", p);
                EventAckData::default()
            }))),
            alive: AtomicBool::new(false),
        }))
    }

    /// Change the User-Agent
    pub fn ua(self: Arc<Self>, value: impl Into<String>) -> Arc<Self> {
        self.config.lock().unwrap().ua = value.into();
        self
    }

    /// Enable or disable client side keep alive heartbeat, default is false
    pub fn keep_alive(self: Arc<Self>, value: bool) -> Arc<Self> {
        self.config.lock().unwrap().other.keep_alive = value;
        self
    }

    /// Add listener to watch all event.   
    /// Calling this interface multiple times will replace the old listener with a new one.
    pub fn register_all_event_listener<P>(self: Arc<Self>, on_event_received: P) -> Arc<Self>
    where
        P: Fn(ClientDownStream) -> EventAckData + Send + Sync + 'static,
    {
        *self.on_event_callback.0.write().unwrap() = Box::new(on_event_received);
        self
    }

    /// Add listener to watch specifc event id
    pub fn register_callback_listener<P, F>(
        self: Arc<Self>,
        event_id: impl AsRef<str>,
        callback: P,
    ) -> Arc<Self>
    where
        P: Fn(Arc<Self>, RobotRecvMessage) -> F + Send + 'static,
        F: Future<Output = Result<()>> + Send,
    {
        let event_id = event_id.as_ref();
        {
            let mut config = self.config.lock().unwrap();
            if !config
                .subscriptions
                .iter()
                .any(|s| s.topic == event_id && s.r#type == "CALLBACK")
            {
                config.subscriptions.push(Subscription {
                    topic: event_id.to_owned(),
                    r#type: "CALLBACK".to_owned(),
                });
            }
        }

        tokio::spawn({
            let mut rx = self.rx.clone();
            let s = self.clone();
            async move {
                while let Ok(msg) = rx.recv().await {
                    match serde_json::from_str(&msg.data) {
                        Ok(msg) => {
                            if let Err(e) = callback(s.clone(), msg).await {
                                error!("robot callback error: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("can not parse data: {:?}", e);
                        }
                    }
                }
            }
        });

        self
    }

    async fn get_endpoint(&self) -> Result<String> {
        let url = {
            let config = self.config.lock().unwrap();
            debug!("get connect endpoint by config {:#?}", *config);
            format!(
                "{GET_TOKEN_URL}?appkey={}&appsecret={}",
                config.client_id, config.client_secret
            )
        };
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            bail!(
                "get token http error: {} - {}",
                response.status(),
                response.text().await?
            );
        }

        let token: TokenResponse = response.json().await?;
        if token.errcode != 0 {
            bail!("get token content error: {:?}", token);
        }

        debug!("get token: {:?}", token);
        let token = token.access_token;
        self.config.lock().unwrap().other.access_token = token.clone();

        let response = self
            .client
            .post(GATEWAY_URL)
            .json(&*self.config)
            .header(ACCEPT, "application/json")
            .header("access-token", token)
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(
                "get endpoint http error: {} - {}",
                response.status(),
                response.text().await?
            );
        }

        let endpoint: EndpointResponse = response.json().await?;
        debug!("get endpoint: {:?}", endpoint);
        let EndpointResponse { endpoint, ticket } = endpoint;

        Ok(format!("{endpoint}?ticket={ticket}"))
    }

    async fn serve(self: &Arc<Self>, url: String) -> Result<()> {
        let tls_connect = Connector::NativeTls({
            TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .build()?
        });

        let (stream, _) =
            match connect_async_tls_with_config(&url, None, false, Some(tls_connect)).await {
                Ok(x) => {
                    self.alive.store(true, Ordering::SeqCst);
                    x
                }
                Err(e) => {
                    if let Error::Http(ref h) = e {
                        bail!(
                            "connect websocket http error: {} - {}",
                            h.status(),
                            String::from_utf8_lossy(h.body().as_deref().unwrap_or_default())
                        );
                    } else {
                        bail!("connect websocket error: {:?}", e);
                    }
                }
            };

        let (sink, stream) = stream.split();
        *self.sink.lock().await = Some(sink);
        let aborting = Arc::new(Notify::new());
        if self.config.lock().unwrap().other.keep_alive {
            tokio::spawn({
                let s = self.clone();
                let aborting = aborting.clone();
                async move {
                    loop {
                        if !s.alive.load(Ordering::SeqCst) {
                            aborting.notify_one();
                            break;
                        }

                        trace!("websocket ping");
                        s.alive.store(false, Ordering::SeqCst);
                        let _ = s.ping().await;
                        sleep(Duration::from_millis(s.heartbeat_interval)).await;
                    }
                }
            });
        }

        tokio::select! {
            _ = aborting.notified() => { warn!("server not responsed, aborting"); }
            _ = self.process(stream) => { warn!("server error or closed"); }
        }

        self.alive.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn process(
        &self,
        mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Result<()> {
        while let Some(message) = stream.next().await {
            let message = match message {
                Ok(m) => m,
                Err(e) => {
                    error!("recv websocket message error: {:?}", e);
                    break;
                }
            };

            match message {
                Message::Text(t) => {
                    debug!("recv websocket text: {t}");
                    match serde_json::from_str::<ClientDownStream>(&t) {
                        Ok(p) => self.on_down_stream(p).await?,
                        Err(e) => {
                            warn!("parse websocket text error: {:?}", e)
                        }
                    }
                }
                Message::Pong(_) => {
                    trace!("websocket pong");
                    self.alive.store(true, Ordering::SeqCst)
                }
                Message::Close(c) => {
                    warn!(
                        "Websocket closed: {}",
                        if let Some(c) = c {
                            c.to_string()
                        } else {
                            "Unknown reason".to_owned()
                        }
                    );

                    break;
                }

                _ => {
                    warn!("Unhandled websocket message: {:?}", message)
                }
            }
        }

        Ok(())
    }

    /// connect to api gateway, and begin the websocket stream process
    pub async fn connect(self: Arc<Self>) -> Result<()> {
        loop {
            let c = self.clone();
            let url = c.get_endpoint().await?;
            c.serve(url).await?;

            if c.config.lock().unwrap().other.auto_reconnect {
                debug!(
                    "Reconnecting in {} seconds...",
                    self.reconnect_interval / 1000
                );

                sleep(Duration::from_millis(self.reconnect_interval)).await;
                debug!("initial reconnecting...");
            } else {
                break;
            }
        }

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct TokenResponse {
    errcode: u32,
    access_token: String,
    errmsg: String,
    expires_in: u32,
}

#[derive(Debug, Deserialize)]
struct EndpointResponse {
    endpoint: String,
    ticket: String,
}

const GATEWAY_URL: &'static str = "https://api.dingtalk.com/v1.0/gateway/connections/open";
const GET_TOKEN_URL: &'static str = "https://oapi.dingtalk.com/gettoken";
/** robot message callback */
pub const TOPIC_ROBOT: &'static str = "/v1.0/im/bot/messages/get";
/** card callback */
pub const TOPIC_CARD: &'static str = "/v1.0/card/instances/callback";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub ua: String,
    pub subscriptions: Vec<Subscription>,
    #[serde(skip_serializing)]
    pub other: OtherConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: Default::default(),
            client_secret: Default::default(),
            ua: Default::default(),
            subscriptions: vec![
                Subscription {
                    r#type: "EVENT".to_owned(),
                    topic: "*".to_owned(),
                },
                Subscription {
                    r#type: "SYSTEM".to_owned(),
                    topic: "*".to_owned(),
                },
            ],
            other: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct OtherConfig {
    pub keep_alive: bool,
    pub access_token: String,
    pub auto_reconnect: bool,
}

impl Default for OtherConfig {
    fn default() -> Self {
        Self {
            keep_alive: true,
            access_token: Default::default(),
            auto_reconnect: true,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Subscription {
    pub r#type: String,
    pub topic: String,
}
