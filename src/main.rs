pub mod game;

use std::usize::MAX;

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

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting Dora Explorer bot...");

    let route = load_route_from_path("route.json").expect("Couldn't load route.");

    let bot = Bot::from_env();

    Dispatcher::builder(
        bot,
        Update::filter_message()
            .enter_dialogue::<Message, InMemStorage<GameState>, GameState>()
            .branch(dptree::case![GameState::Start].endpoint(start))
            .branch(
                dptree::case![GameState::ReceiveAnswer { step, hp, score }]
                    .endpoint(recieve_answer),
            ),
    )
    .dependencies(dptree::deps![InMemStorage::<GameState>::new(), route])
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
    route: Route,
) -> HandlerResult {
    let last = step == route.route.len() - 1;
    match msg.text() {
        Some(text) => {
            if text.to_lowercase() == route.route[step].answer.to_lowercase() {
                if !last {
                    dialogue
                        .update(GameState::ReceiveAnswer {
                            step: step + 1,
                            hp: MAX_HP,
                            score: score + hp,
                        })
                        .await?;
                    bot.send_message(msg.chat.id, "Correct!").await?;
                    bot.send_message(msg.chat.id, route.route[step + 1].clue.as_str())
                        .await?;
                } else {
                    dialogue.update(GameState::Completed).await?;
                    bot.send_message(
                        msg.chat.id,
                        format!("🔥 You finished. Your final score was {score}"),
                    )
                    .await?;
                }
            } else {
                if hp > 1 {
                    dialogue
                        .update(GameState::ReceiveAnswer {
                            step,
                            hp: hp - 1,
                            score,
                        })
                        .await?;

                    let new_hp = hp - 1;
                    bot.send_message(msg.chat.id, format!("Wrong answer! Lives left: {new_hp}"))
                        .await?;
                } else {
                    bot.send_message(msg.chat.id, "No lives left! Moving on...")
                        .await?;
                    if !last {
                        dialogue
                            .update(GameState::ReceiveAnswer {
                                step: step + 1,
                                hp: MAX_HP,
                                score,
                            })
                            .await?;
                        bot.send_message(msg.chat.id, route.route[step + 1].clue.as_str())
                            .await?;
                    } else {
                        dialogue.update(GameState::Completed).await?;
                        bot.send_message(
                            msg.chat.id,
                            format!("🔥 You finished. Your final score was {score}"),
                        )
                        .await?;
                    }
                }
            }
        }
        None => {
            bot.send_message(msg.chat.id, "Enter the answer...").await?;
        }
    };
    Ok(())
}

async fn start(bot: Bot, dialogue: GameDialogue, msg: Message, route: Route) -> HandlerResult {
    bot.send_message(msg.chat.id, "Let's start the Amazing Race! Type ")
        .await?;
    bot.send_message(msg.chat.id, route.route[0].clue.as_str())
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
    #[command(description = "Submit the answer to the clue")]
    Submit { prompt: String },
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::StartRace => bot.send_message(msg.chat.id, "").await?,
        Command::Submit { prompt } => {
            bot.send_message(msg.chat.id, format!("Your submission was {prompt}"))
                .await?
        }
    };

    Ok(())
}
