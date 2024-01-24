use crate::{up::ClientUpStream, Client};
use anyhow::Result;
use log::{debug, error, warn};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

impl Client {
    pub(crate) async fn on_down_stream(&self, p: ClientDownStream) -> Result<()> {
        match p.r#type.as_str() {
            "SYSTEM" => self.on_system(p).await?,
            "EVENT" => self.on_event(p).await?,
            "CALLBACK" => {
                let msg = ClientUpStream::new(
                    serde_json::to_string(&json!({"response" : {}}))?,
                    p.headers.message_id.clone(),
                );
                self.send(msg).await?;
                self.tx.broadcast(Arc::new(p)).await?;
            }
            _ => error!("unknown message type: {}", p.r#type),
        }

        Ok(())
    }

    async fn on_event(&self, p: ClientDownStream) -> Result<()> {
        debug!("event received: {:?}", p);
        let message_id = p.headers.message_id.clone();
        let ack = self.on_event_callback.0.read().unwrap()(p);
        let msg = ClientUpStream::new(serde_json::to_string(&ack)?, message_id);
        self.send(msg).await?;

        Ok(())
    }

    async fn on_system(&self, p: ClientDownStream) -> Result<()> {
        match p.headers.topic.as_str() {
            "CONNECTED" => debug!("[SYSTEM]: connected"),
            "REGISTERED" => debug!("[SYSTEM]: registered"),
            "disconnect" => debug!("[SYSTEM]: disconnect"),
            "KEEPALIVE" => debug!("[SYSTEM]: keepalive"),
            "ping" => {
                debug!("[SYSTEM]: ping");
                let msg = ClientUpStream::new(p.data, p.headers.message_id);
                self.send(msg).await?;
            }
            _ => warn!("unknown system message: {}", p.headers.topic),
        }

        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDownStream {
    pub spec_version: String,
    pub r#type: String,
    pub headers: StreamDownHeaders,
    pub data: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDownHeaders {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub connection_id: String,
    pub content_type: String,
    pub message_id: String,
    pub time: String,
    pub topic: String,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub event_born_time: String,
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_corp_id: String,
    #[serde(default)]
    pub event_unified_app_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RobotRecvMessage {
    pub msg_id: String,
    pub msgtype: String,
    #[serde(alias = "text")]
    pub content: MsgContent,

    pub conversation_id: String,
    /// 1 - single chat
    /// 2 - group chat
    pub conversation_type: String,
    #[serde(default)]
    pub conversation_title: String,

    #[serde(default)]
    pub at_users: Vec<User>,
    #[serde(default)]
    pub is_in_at_list: bool,

    #[serde(default)]
    pub chatbot_corp_id: String,
    pub chatbot_user_id: String,

    pub sender_id: String,
    pub sender_nick: String,
    #[serde(default)]
    pub sender_corp_id: String,
    #[serde(default)]
    pub sender_staff_id: String,

    pub session_webhook_expired_time: u64,
    pub session_webhook: String,

    #[serde(default)]
    pub is_admin: bool,
    pub create_at: u64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub dingtalk_id: String,
    #[serde(default)]
    pub staff_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", untagged)]
pub enum MsgContent {
    #[serde(rename_all = "camelCase")]
    Text { content: String },
    #[serde(rename_all = "camelCase")]
    File {
        download_code: String,
        file_name: String,
    },
    #[serde(rename_all = "camelCase")]
    Picture {
        download_code: String,
        #[serde(default)]
        picture_download_code: String,
    },
    #[serde(rename_all = "camelCase")]
    RichText { rich_text: Vec<RichText> },
    #[serde(rename_all = "camelCase")]
    Audio {
        duration: u32,
        download_code: String,
        recognition: String,
    },
    #[serde(rename_all = "camelCase")]
    Video {
        duration: u32,
        download_code: String,
        video_type: String,
    },
    #[serde(rename_all = "camelCase")]
    UnknownMsgType { unknown_msg_type: String },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RichText {
    #[serde(rename_all = "camelCase")]
    Text { text: String },
    #[serde(rename_all = "camelCase")]
    Picture {
        download_code: String,
        r#type: String,
    },
}