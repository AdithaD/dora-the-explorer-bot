pub mod game;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use std::env;

use game::{load_route_from_path, Route};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*, utils::command::BotCommands};

type GameDialogue = Dialogue<GameState, InMemStorage<GameState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
enum GameState {
    #[default]
    Start,
    ReceiveAnswer {
        step: usize,
        hp: usize,
        score: usize,
    },
    Completed,
}

const MAX_HP: usize = 3;

#[derive(Debug, Clone)]
struct Data {
    route: Route,
    admin_id: Option<ChatId>,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting Dora Explorer bot...");

    let route = load_route_from_path("route.json").expect("Couldn't load route.");

    let bot = Bot::from_env();

    let admin_id = match env::var("ADMIN_CHAT_ID") {
        Ok(text) => match text.parse::<i64>() {
            Ok(int) => Some(ChatId(int)),
            Err(_) => None,
        },
        Err(_) => None,
    };

    let data = Data {
        route,
        admin_id: admin_id,
    };

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer))
        .branch(
            dptree::filter(|is_game_active: Arc<AtomicBool>| {
                is_game_active.load(Ordering::Relaxed)
            })
            .enter_dialogue::<Message, InMemStorage<GameState>, GameState>()
            .branch(dptree::case![GameState::Start].endpoint(start))
            .branch(
                dptree::case![GameState::ReceiveAnswer { step, hp, score }]
                    .endpoint(recieve_answer),
            ),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            InMemStorage::<GameState>::new(),
            data,
            Arc::new(AtomicBool::new(false))
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    // Command::repl(bot, answer).await;
}

async fn recieve_answer(
    bot: Bot,
    dialogue: GameDialogue,
    msg: Message,
    (step, hp, score): (usize, usize, usize),
    data: Data,
) -> HandlerResult {
    let next_location = |msg_string: &'static str, score_mod: usize, success: bool| {
        move_to_next_location(
            msg_string,
            score_mod,
            &bot,
            &dialogue,
            &msg,
            (step, hp, score),
            &data,
            success,
        )
    };
    match msg.photo() {
        Some(_) => match data.admin_id {
            Some(admin_id) => {
                bot.forward_message(admin_id, msg.chat.id, msg.id).await?;
            }
            None => (),
        },
        None => match msg.text() {
            Some(text) => {
                if data.route.route[step]
                    .answer
                    .iter()
                    .any(|candidate| return candidate.to_lowercase() == text.to_lowercase())
                {
                    // Correct answer case
                    next_location("Correct!", hp, true).await?;
                } else {
                    // Incorrect answer case
                    let new_hp = hp - 1;

                    match data.admin_id {
                        Some(id) => {
                            bot.send_message(
                                id,
                                format!(
                                    "User {} submitted the wrong answer \"{}\" for {}. \nLives left: {}",
                                    msg.chat.username().unwrap_or("UNKNOWN USER"),
                                    text,
                                    data.route.route[step].title,
                                    new_hp
                                ),
                            )
                            .await?;
                        }
                        None => (),
                    };

                    if hp > 1 {
                        bot.send_message(
                            msg.chat.id,
                            format!("Wrong answer! Tries left for this clue: {new_hp}"),
                        )
                        .await?;

                        // Let the player have another try if they still have lives
                        dialogue
                            .update(GameState::ReceiveAnswer {
                                step,
                                hp: hp - 1,
                                score,
                            })
                            .await?;
                    } else {
                        next_location("No lives left! Moving on...", 0, false).await?;
                    }
                }
            }
            None => {
                bot.send_message(msg.chat.id, "Enter the answer...").await?;
            }
        },
    }

    Ok(())
}

async fn move_to_next_location(
    msg_string: &str,
    score_mod: usize,
    bot: &Bot,
    dialogue: &GameDialogue,
    msg: &Message,
    (step, hp, score): (usize, usize, usize),
    data: &Data,
    success: bool,
) -> HandlerResult {
    let last = step == data.route.route.len() - 1;

    bot.send_message(msg.chat.id, msg_string).await?;

    if !last {
        match data.admin_id {
            Some(id) => match success {
                false => {
                    bot.send_message(
                        id,
                        format!(
                            "User {} has no lives left for {}! \nMoving on to {}",
                            msg.chat.username().unwrap_or("UNKNOWN USER"),
                            data.route.route[step].title,
                            data.route.route[step + 1].title
                        ),
                    )
                    .await?;
                }
                true => {
                    bot.send_message(
                        id,
                        format!(
                            "User {} submitted the correct answer. Moving onto {}",
                            msg.chat.username().unwrap_or("UNKNOWN USER"),
                            data.route.route[step + 1].title
                        ),
                    )
                    .await?;
                }
            },
            None => (),
        };

        dialogue
            .update(GameState::ReceiveAnswer {
                step: step + 1,
                hp: MAX_HP,
                score: score + score_mod,
            })
            .await?;
        bot.send_message(msg.chat.id, data.route.route[step + 1].clue.as_str())
            .await?;
    } else {
        let final_score = score + hp;
        dialogue.update(GameState::Completed).await?;
        bot.send_message(
            msg.chat.id,
            format!("🔥 You finished 🔥\n Your final score was {final_score}"),
        )
        .await?;

        match data.admin_id {
            Some(id) => {
                bot.send_message(
                    id,
                    format!(
                        "User {} finished with {final_score}",
                        msg.chat.username().unwrap_or("UNKNOWN USER"),
                    ),
                )
                .await?;
            }
            None => (),
        };
    }
    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Help with using the bot.")]
    Help,
    #[command(description = "Start the Amazing Race")]
    StartRace,
}

async fn start(bot: Bot, dialogue: GameDialogue, msg: Message, data: Data) -> HandlerResult {
    bot.send_message(msg.chat.id, data.route.route[0].clue.as_str())
        .await?;
    dialogue
        .update(GameState::ReceiveAnswer {
            step: 0,
            hp: MAX_HP,
            score: 0,
        })
        .await?;
    Ok(())
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    is_game_active: Arc<AtomicBool>,
) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::StartRace => {
            is_game_active.fetch_or(true, Ordering::Relaxed);
            bot.send_message(
                msg.chat.id,
                "Let's start the Amazing Race! Type any text to start!",
            )
            .await?
        }
    };

    Ok(())
}
