use anyhow::Result;
use dingtalk_stream_sdk_rust::{
    down::{MsgContent, RobotRecvMessage},
    up::{EventAckData, MessageTemplate, RobotSendMessage, UploadType},
    Client, TOPIC_ROBOT,
};
use indoc::formatdoc;
use log::{debug, info};
use rand::{distributions::Alphanumeric, Rng};
use tokio::fs::File;

const CLIENT_ID: &'static str = "ding2kdwyyilaknq8zj5";
const CLIENT_SECRET: &'static str =
    "vlDWox885jMZQLm5cQ6lBTtdjqQXEfK-PK5dIL29tPECYdAE0i1A_7wum76BLxzO";

#[tokio::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    info!("Demo started");
    let client = Client::new(CLIENT_ID, CLIENT_SECRET)?;
    client
        .register_callback_listener(TOPIC_ROBOT, |client, msg| {
            async move {
                let RobotRecvMessage {
                    content,
                    sender_staff_id,
                    conversation_id,
                    conversation_type,
                    sender_nick,
                    ..
                } = msg;
                info!("Message Received from {}: {:?}", sender_nick, content);

                match &content {
                    MsgContent::File { download_code, .. }
                    | MsgContent::Picture { download_code, .. }
                    | MsgContent::Audio { download_code, .. }
                    | MsgContent::Video { download_code, .. } => {
                        let filename = format!(
                            "media_file_{}",
                            rand::thread_rng()
                                .sample_iter(Alphanumeric)
                                .take(8)
                                .map(char::from)
                                .collect::<String>()
                        );
                        let file = File::create(&filename).await?;
                        client.download(download_code, file).await?;
                        debug!("downloaded file {}", filename);

                        if conversation_type == "1" {
                            if let MsgContent::File { file_name, .. } = &content {
                                let media_id = client.upload(filename, UploadType::File).await?;
                                RobotSendMessage::single(
                                    client.clone(),
                                    sender_staff_id.clone(),
                                    MessageTemplate::SampleFile {
                                        media_id,
                                        file_name: file_name.clone(),
                                        file_type: "file".to_owned(),
                                    },
                                )?
                                .send()
                                .await?;
                            }
                        }
                    }
                    _ => {}
                }

                let echo_message = MessageTemplate::SampleActionCard2 {
                    title: "title".to_owned(),
                    text: formatdoc! {r#"
                        # Title
                        ![](https://picsum.photos/800/600)
                        - Line 1
                        - Line 2

                        ```
                        {:?}
                        ```
                        "#,
                        content
                    },
                    action_title_1: "View Detail".to_owned(),
                    action_url_1: "http://www.dingtalk.com".to_owned(),
                    action_title_2: "Random".to_owned(),
                    action_url_2: "http://picsum.photos/300/800".to_owned(),
                };
                // conversation_type 1 is single chat, 2 is group chat
                if conversation_type == "1" {
                    RobotSendMessage::single(client, sender_staff_id, echo_message)?
                        .send()
                        .await?;
                } else if conversation_type == "2" {
                    RobotSendMessage::group(client, conversation_id, echo_message)?
                        .send()
                        .await?;
                }

                Ok::<_, anyhow::Error>(())
            }
        })
        .register_all_event_listener(|msg| {
            info!("event: {:?}", msg);
            EventAckData::default()
        })
        .connect()
        .await?;

    Ok(())
}
