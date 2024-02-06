# dingtalk-stream-sdk-rust

This an **UNOFFICIAL** Rust SDK focused on the development of DingTalk robots.

*USE IT ON YOUR OWN RISK*

Reference to DingTalk open platform docs [here](https://open.dingtalk.com/document/orgapp/robot-overview)

## Major Function

The functions included in SDK:
- Receive message from conversation between user and robot
    - [`Client::register_callback_listener`]
    - where [`RobotRecvMessage::conversation_type`] == 1
- Receive message from group conversation when robot has been @
    - [`Client::register_callback_listener`]
    - where [`RobotRecvMessage::conversation_type`] == 2
- Send various types of message to bulk users (or single user)
    - [`RobotSendMessage::single`](up::RobotSendMessage::single)
    - [`RobotSendMessage::batch`](up::RobotSendMessage::batch)
    - [`RobotSendMessage::send`](up::RobotSendMessage::send)
- Send message to specific group conversation
    - [`RobotSendMessage::group`](up::RobotSendMessage::group)
    - [`RobotSendMessage::send`](up::RobotSendMessage::send)
- Download media file user sent
    - [`Client::download`]
- Upload media file sent to users
    - [`Client::upload`]
- Create group chat
    - [`Client::create_group`]

See more details in examples

## Additional helper proc-macro

provide a proc-macro to make construct [`SampleActionCard2`](up::MessageTemplate::SampleActionCard2) ~ [`SampleActionCard5`](up::MessageTemplate::SampleActionCard5) more convernient.
see more in [action_card!](`msg_macro::action_card`)

