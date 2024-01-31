use crate::Client;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

const CREATE_URL: &str = "https://oapi.dingtalk.com/chat/create";
const DISSOLVE_URL: &str =
    "https://api.dingtalk.com/v1.0/devicemng/customers/chatRooms/groups/dissolve";

impl Client {
    /// create group via [`CreateRequest`], return group (open_conversation_id, chatid)
    pub async fn create_group(&self, req: CreateRequest) -> Result<(String, String)> {
        let token = self.token().await?;
        let resp: CreateResposne = self
            .post(format!("{}?access_token={}", CREATE_URL, token), req)
            .await?;

        if resp.errcode != 0 {
            bail!("create group error: {} - {}", resp.errcode, resp.errmsg);
        }

        Ok((resp.open_conversation_id, resp.chatid))
    }

    /// dissolve group via `open_conversation_id`
    pub async fn dissolve_group(&self, open_conversation_id: impl Into<String>) -> Result<()> {
        let response: DissolveResponse = self
            .post(
                DISSOLVE_URL,
                json!({"openConversationId": open_conversation_id.into()}),
            )
            .await?;
        if !response.success {
            bail!("dissolve group error: {}", response.result);
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct DissolveResponse {
    success: bool,
    result: String,
}

/// group create request
///
/// refer to [official doc](https://open.dingtalk.com/document/orgapp/create-group-session#h2-3kz-gpn-suq) see more detail for each field
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub name: String,
    pub owner: String,
    pub useridlist: Vec<String>,

    /// Can new members view 100 historical messages
    /// - 1: can
    /// - 0: can not (default)
    pub show_history_type: u8,

    /// Can the group be searched
    /// - 0: can not (default)
    /// - 1: can
    pub searchable: u8,

    /// Do user need verification to join the group
    /// - 0: need not (default)
    /// - 1: need
    pub validation_type: u8,

    /// @all usage scope
    /// - 0: all people can use @all (default)
    /// - 1: only group owner can use @all
    pub mention_all_authority: u8,

    /// Group management type
    /// - 0: everyone can manage (default)
    /// - 1: only group owner can manage
    pub management_type: u8,

    /// Whether to enable chat banned in group
    /// - 0: no (default)
    /// - 1: yes
    pub chat_banned_type: u8,
}

/// group create response
///
/// refer to [official doc](https://open.dingtalk.com/document/orgapp/create-group-session#h2-xyy-jpo-c7q) for more details
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateResposne {
    #[serde(default)]
    open_conversation_id: String,
    #[serde(default)]
    chatid: String,
    #[serde(default)]
    #[allow(dead_code)]
    conversation_tag: u32,
    errmsg: String,
    errcode: u32,
}
