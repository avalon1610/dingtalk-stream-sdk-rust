use anyhow::Result;
use dingtalk_stream_sdk_rust::{
    down::RobotRecvMessage,
    up::{EventAckData, MessageTemplate, RobotSendMessage},
    Client, TOPIC_ROBOT,
};
use log::info;

// const CLIENT_ID: &'static str = "ding2kdwyyilaknq8zj5";
// const CLIENT_SECRET: &'static str =
//     "vlDWox885jMZQLm5cQ6lBTtdjqQXEfK-PK5dIL29tPECYdAE0i1A_7wum76BLxzO";

const CLIENT_ID: &'static str = "ding7jysiq3otlsn9ksw";
const CLIENT_SECRET: &'static str =
    "fr90usMkWe0CImJ-S8tv2HNqnSpKNDlEt8kAf2vuFMPdplXXhwm0W00CvqsYE9eu";

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

                // conversation type 1 is single chat, 2 is group chat
                if conversation_type == "1" {
                    RobotSendMessage::single(
                        client,
                        sender_staff_id,
                        MessageTemplate::SampleText {
                            content: format!("Echo Single: {:?}", content),
                        },
                    )?
                    .send()
                    .await?;
                } else if conversation_type == "2" {
                    RobotSendMessage::group(
                        client,
                        conversation_id,
                        MessageTemplate::SampleText {
                            content: format!("Echo Group: {:?}", content),
                        },
                    )?
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
