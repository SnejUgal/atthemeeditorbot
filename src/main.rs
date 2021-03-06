use attheme::{Attheme, ColorSignature};
use attheme_editor_api::{download, upload, Error, Theme, ThemeId};
use dotenv::dotenv;
use std::{borrow::Borrow, path::Path, sync::Arc};
use tbot::{
    contexts::{Command, Document, Text},
    prelude::*,
    types::{
        chat::Action::{Typing, UploadDocument},
        input_file,
        keyboard::inline::{Button, ButtonKind},
    },
    Bot,
};
use tokio::select;

mod localization;

#[tokio::main]
async fn main() {
    let _ = dotenv();
    let mut bot = Bot::from_env("BOT_TOKEN").event_loop();

    bot.start(handle_start);

    bot.help(|context| async move {
        let message = localization::help_message();
        let call_result = context.send_message(message).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    bot.document(handle_document);

    if let Ok(url) = std::env::var("WEBHOOK_URL") {
        bot.webhook(&url, 5004).http().start().await.unwrap();
    } else {
        bot.polling().start().await.unwrap();
    }
}

async fn handle_start(context: Arc<Command<Text>>) {
    if context.text.value.is_empty() {
        let message = localization::start_message();
        let call_result = context.send_message(message).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }

        return;
    }

    let download_theme = async {
        let download = download(&context.text.value).await;
        let Theme { name, content } = match download {
            Ok(theme) => theme,
            Err(Error::BadRequest(..)) => {
                let message = localization::theme_expired();
                let call_result = context.send_message(message).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
                return;
            }
            Err(err) => {
                dbg!(err);
                return;
            }
        };
        let theme = content.to_bytes(ColorSignature::Hex);

        if theme.is_empty() {
            let message = localization::empty_theme();
            let call_result = context.send_message(message).call().await;

            if let Err(err) = call_result {
                dbg!(err);
            }

            return;
        }

        let file_name = format!("{}.attheme", name);
        let caption = localization::theme_file_caption();
        let document =
            input_file::Document::bytes(&file_name, &theme).caption(caption);
        let call_result = context.send_document(document).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    };

    select! {
        _ = download_theme => (),
        Err(error) = context.send_chat_action_in_loop(UploadDocument) => {
            dbg!(error);
        },
    }
}

async fn handle_document(context: Arc<Document>) {
    let file_name = match &context.document.file_name {
        Some(file_name) if file_name.ends_with(".attheme") => file_name,
        _ => {
            let message = localization::wrong_file_type();
            let call_result =
                context.send_message_in_reply(message).call().await;

            if let Err(err) = call_result {
                dbg!(err);
            }

            return;
        }
    };

    let upload_theme = async {
        let call_result = context.bot.get_file(&context.document).call().await;
        let file = match call_result {
            Ok(file) => file,
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        let result = context.bot.download_file(&file).await;
        let theme = match result {
            Ok(bytes) => Attheme::from_bytes(&bytes),
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        let name = Path::new(&file_name).file_stem().unwrap();
        let name = name.to_string_lossy();
        let name = name.borrow();

        let theme_id = match upload(&name, &theme).await {
            Ok(ThemeId { theme_id }) => theme_id,
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        let editor_link =
            format!("https://attheme-editor.snejugal.ru/?themeId={}", theme_id);
        let button_text = localization::open_theme_button(name);
        let keyboard: &[&[_]] =
            &[&[Button::new(&button_text, ButtonKind::Url(&editor_link))]];

        let message = localization::theme_uploaded();
        let call_result = context
            .send_message_in_reply(message)
            .reply_markup(keyboard)
            .call()
            .await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    };

    select! {
        _ = upload_theme => (),
        Err(error) = context.send_chat_action_in_loop(Typing) => {
            dbg!(error);
        },
    }
}
