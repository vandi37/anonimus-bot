mod constants;
mod error;

use crate::constants::{ADMIN_CHAT_ID, REDIS_URL, TELEGRAM_BOT_TOKEN};
use crate::error::Error;
use redis::{Client, Commands};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::types::{MessageId, ThreadId};
use tokio::sync::Mutex;

struct BotState {
    redis: Mutex<Client>,
    admin_chat_id: ChatId,
}

impl BotState {
    fn get_admin_chat_id(&self) -> &ChatId {
        &self.admin_chat_id
    }
}

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    Help,
    Start,
}

#[derive(Serialize, Deserialize)]
struct MessageLink {
    original_chat_id: ChatId,
    original_tread_id: Option<ThreadId>,
    original_message_id: MessageId,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting anonymous message bot...");

    dotenv::dotenv().ok();
    let bot_token = env::var(TELEGRAM_BOT_TOKEN).unwrap();
    let redis_url = env::var(REDIS_URL).unwrap();
    let admin_chat_id = env::var(ADMIN_CHAT_ID).unwrap().parse::<i64>().unwrap();
    let redis_client = Client::open(redis_url).expect("Failed to create Redis client");
    let _: () = redis_client
        .get_connection()
        .unwrap()
        .set("test", "test")
        .unwrap();

    let bot = Bot::new(bot_token);

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(
            dptree::entry()
                .filter(|msg: Message, state: Arc<BotState>| {
                    &msg.chat.id == state.get_admin_chat_id()
                })
                .filter(|msg: Message| msg.reply_to_message().is_some())
                .endpoint(admin_reply_handler),
        )
        .branch(
            dptree::entry()
                .filter(|msg: Message, state: Arc<BotState>| {
                    &msg.chat.id != state.get_admin_chat_id()
                })
                .endpoint(user_message_handler),
        );

    let state = Arc::new(BotState {
        redis: Mutex::new(redis_client),
        admin_chat_id: ChatId(admin_chat_id),
    });

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .error_handler(LoggingErrorHandler::new())
        .default_handler(|upd| async move {
            log::warn!("Unhandled update: {:?}", upd);
        })
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn command_handler(_: Arc<BotState>, bot: Bot, msg: Message) -> Result<(), Error> {
    bot.send_message(
        msg.chat.id,
        "🔰 Пиши сообщения боту и они будут присылаться его владельцу анонимно",
    )
    .await?;
    Ok(())
}

async fn user_message_handler(bot: Bot, state: Arc<BotState>, msg: Message) -> Result<(), Error> {
    let send = bot
        .copy_message(state.admin_chat_id, msg.chat.id, msg.id)
        .await?;
    let link = MessageLink {
        original_chat_id: msg.chat.id,
        original_message_id: msg.id,
        original_tread_id: msg.thread_id,
    };

    let link_json = serde_json::to_string(&link)?;
    let _: () = state
        .redis
        .lock()
        .await
        .get_connection()?
        .set(send.0, link_json)?;
    Ok(())
}

async fn admin_reply_handler(bot: Bot, state: Arc<BotState>, msg: Message) -> Result<(), Error> {
    let reply_to = match msg.reply_to_message() {
        Some(x) => x,
        None => return Ok(()),
    };

    let link_json: String = state
        .redis
        .lock()
        .await
        .get_connection()?
        .get(reply_to.id.0)?;

    if link_json.is_empty() {
        return Ok(());
    }

    let link: MessageLink = serde_json::from_str(&link_json)?;

    let mut msg = bot
        .copy_message(link.original_chat_id, msg.chat.id, msg.id)
        .reply_to(link.original_message_id);
    if let Some(tread_id) = link.original_tread_id {
        msg = msg.message_thread_id(tread_id);
    }
    msg.send().await?;

    Ok(())
}
