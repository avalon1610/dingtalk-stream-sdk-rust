use crate::Client;
use anyhow::{bail, Result};
use futures::{stream::SplitSink, SinkExt};
use log::debug;
use serde::Serialize;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

pub(crate) type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
impl Client {
    pub(crate) async fn send<T: Serialize>(&self, msg: T) -> Result<()> {
        let msg = serde_json::to_string(&msg)?;
        self.send_message(Message::text(msg)).await
    }

    pub(crate) async fn ping(&self) -> Result<()> {
        self.send_message(Message::Ping(Vec::new())).await
    }

    pub(crate) async fn send_message(&self, msg: Message) -> Result<()> {
        let mut sink = self.sink.lock().await;
        let Some(sink) = sink.as_mut() else {
            bail!("stream not connected");
        };
        sink.send(msg).await?;

        Ok(())
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpStream {
    pub code: u32,
    pub headers: StreamUpHeader,
    pub message: String,
    pub data: String,
}

impl ClientUpStream {
    pub fn new(data: impl Into<String>, message_id: impl Into<String>) -> Self {
        let data = data.into();
        let message_id = message_id.into();

        Self {
            code: 200,
            headers: StreamUpHeader {
                message_id,
                content_type: "application/json".to_owned(),
            },
            message: "OK".to_owned(),
            data,
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamUpHeader {
    pub content_type: String, // always application/json
    pub message_id: String,   // same StreamDownHeaders::message_id
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotSendMessage {
    pub robot_code: String,
    #[serde(flatten)]
    pub target: SendMessageTarget,
    pub msg_key: String,
    pub msg_param: String,

    #[serde(skip_serializing)]
    client: Arc<Client>,
}

const BATCH_SEND_URL: &'static str = "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend";
const GROUP_SEND_URL: &'static str = "https://api.dingtalk.com/v1.0/robot/groupMessages/send";

impl RobotSendMessage {
    /// send message back to group chat
    pub fn group(
        client: Arc<Client>,
        conversation_id: impl Into<String>,
        message: MessageTemplate,
    ) -> Result<Self> {
        let client_id = client.config.lock().unwrap().client_id.clone();
        Ok(Self {
            robot_code: client_id,
            target: SendMessageTarget::Group {
                open_conversation_id: conversation_id.into(),
            },
            msg_key: message.to_string(),
            msg_param: message.try_into()?,
            client,
        })
    }

    pub async fn send(&self) -> Result<()> {
        let access_token = self
            .client
            .config
            .lock()
            .unwrap()
            .other
            .access_token
            .clone();
        debug!("send: {}", serde_json::to_string(self).unwrap());
        let response = self
            .client
            .client
            .post({
                match self.target {
                    SendMessageTarget::Batch { .. } => BATCH_SEND_URL,
                    SendMessageTarget::Group { .. } => GROUP_SEND_URL,
                }
            })
            .header("x-acs-dingtalk-access-token", access_token)
            .json(self)
            .send()
            .await?;

        if response.status().is_success() {
            debug!(
                "robot send ok: [{}] {:?}",
                response.status(),
                response.text().await?
            );
        } else {
            bail!(
                "robot send error: [{}] {:?}",
                response.status(),
                response.text().await?
            );
        }
        Ok(())
    }

    ///  batch send message to multiple users
    pub fn batch(
        client: Arc<Client>,
        user_ids: Vec<String>,
        message: MessageTemplate,
    ) -> Result<Self> {
        let client_id = client.config.lock().unwrap().client_id.clone();
        Ok(Self {
            robot_code: client_id,
            target: SendMessageTarget::Batch { user_ids: user_ids },
            msg_key: message.to_string(),
            msg_param: message.try_into()?,
            client,
        })
    }

    /// send message back to single user
    pub fn single(client: Arc<Client>, user_id: String, message: MessageTemplate) -> Result<Self> {
        Self::batch(client, vec![user_id], message)
    }
}

#[derive(Serialize)]
pub struct EventAckData {
    pub status: &'static str,
    #[serde(default)]
    pub message: String,
}

impl Default for EventAckData {
    fn default() -> Self {
        Self {
            status: EventAckData::SUCCESS,
            message: Default::default(),
        }
    }
}

impl EventAckData {
    pub const SUCCESS: &'static str = "SUCCESS";
    pub const LATER: &'static str = "LATER";
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum SendMessageTarget {
    #[serde(rename_all = "camelCase")]
    Group { open_conversation_id: String },
    #[serde(rename_all = "camelCase")]
    Batch { user_ids: Vec<String> },
}

#[derive(Serialize, strum::Display)]
#[serde(rename_all = "camelCase", untagged)]
#[strum(serialize_all = "camelCase")]
pub enum MessageTemplate {
    SampleText {
        content: String,
    },
    SampleMarkdown {
        title: String,
        text: String,
    },
    SampleImageMsg {
        #[serde(rename = "photoURL")]
        photo_url: String,
    },
    SampleLink {
        text: String,
        title: String,
        pic_url: String,
        message_url: String,
    },
    SampleActionCard {
        title: String,
        text: String,
        single_title: String,
        #[serde(rename = "singleURL")]
        single_url: String,
    },
    SampleActionCard2 {
        title: String,
        text: String,
        action_title_1: String,
        #[serde(rename = "actionURL1")]
        action_url_1: String,
        action_title_2: String,
        #[serde(rename = "actionURL2")]
        action_url_2: String,
    },
    SampleActionCard3 {
        title: String,
        text: String,
        action_title_1: String,
        #[serde(rename = "actionURL1")]
        action_url_1: String,
        action_title_2: String,
        #[serde(rename = "actionURL2")]
        action_url_2: String,
        action_title_3: String,
        #[serde(rename = "actionURL3")]
        action_url_3: String,
    },
    SampleActionCard4 {
        title: String,
        text: String,
        action_title_1: String,
        #[serde(rename = "actionURL1")]
        action_url_1: String,
        action_title_2: String,
        #[serde(rename = "actionURL2")]
        action_url_2: String,
        action_title_3: String,
        #[serde(rename = "actionURL3")]
        action_url_3: String,
        action_title_4: String,
        #[serde(rename = "actionURL4")]
        action_url_4: String,
    },
    SampleActionCard5 {
        title: String,
        text: String,
        action_title_1: String,
        #[serde(rename = "actionURL1")]
        action_url_1: String,
        action_title_2: String,
        #[serde(rename = "actionURL2")]
        action_url_2: String,
        action_title_3: String,
        #[serde(rename = "actionURL3")]
        action_url_3: String,
        action_title_4: String,
        #[serde(rename = "actionURL4")]
        action_url_4: String,
    },
    SampleActionCard6 {
        title: String,
        text: String,
        button_title_1: String,
        button_url_1: String,
        button_title_2: String,
        button_url_2: String,
    },
    SampleAudio {
        media_id: String,
        duration: String,
    },
    SampleFile {
        media_id: String,
        file_name: String,
        file_type: String,
    },
    SampleVideo {
        duration: String,
        video_media_id: String,
        video_type: String,
        pic_media_id: String,
    },
}

impl TryInto<String> for MessageTemplate {
    type Error = serde_json::Error;

    fn try_into(self) -> std::result::Result<String, Self::Error> {
        serde_json::to_string(&self)
    }
}
